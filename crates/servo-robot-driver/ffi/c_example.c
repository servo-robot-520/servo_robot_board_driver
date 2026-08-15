/*
 * servo-robot-driver C 使用示例 — 完整驱动生命周期
 *
 * 编译:
 *   cargo build --release --features ffi
 *   gcc -Wall -Wextra -I../include c_example.c -o c_example
 *       -L ../../../target/release -lservo_robot_driver
 *       -Wl,-rpath,$PWD/../../../target/release
 *
 * 运行(真实设备):
 *   ./c_example /dev/ttyUSB0 115200
 *
 * 线程模型:回调在驱动分发线程执行;回调内禁止调用任何 sr_driver_*;
 * 回调参数指针仅在回调执行期间有效。
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "servo_robot_driver.h"

/* ═══ 回调上下文(userdata 透传) ═══ */
typedef struct {
    uint64_t imu_count;
    uint64_t power_count;
    uint64_t battery_count;
    uint64_t log_count;
} callback_ctx;

static void on_imu(void* userdata, const sr_imu* data) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->imu_count++;
    if (ctx->imu_count <= 3) {
        printf("[IMU #%lu] roll=%7.2f pitch=%7.2f yaw=%7.2f\n",
               ctx->imu_count, data->roll, data->pitch, data->yaw);
    }
}

static void on_power(void* userdata, const sr_power* data) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->power_count++;
    if (ctx->power_count <= 3) {
        /* 协议约定:电压/电流原始值 = 实际值 × 10(与 Rust Display 一致) */
        printf("[PWR] servo %5.1f V / %5.1f A, bat %5.1f V\n",
               data->servo_voltage_mv / 10.0f, data->servo_current_ma / 10.0f,
               data->bat_voltage_mv / 10.0f);
    }
}

static void on_battery(void* userdata, const sr_battery_state* state) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->battery_count++;
    if (ctx->battery_count <= 3) {
        printf("[BAT] %u%%, temp %d.%d C, %u cells\n",
               state->percentage, state->temperature / 10,
               state->temperature % 10, state->cell_count);
    }
}

static void on_log(void* userdata, const sr_log_message* msg) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->log_count++;
    printf("[BOARD LOG] %s::%s: %s\n",
           msg->file_name ? msg->file_name : "?",
           msg->fun_name ? msg->fun_name : "?", msg->msg ? msg->msg : "");
}

static void on_error(void* userdata, int error_code) {
    (void)userdata;
    fprintf(stderr, "[DRIVER ERROR] code=%d\n", error_code);
}

static const char* err_name(int rc) {
    switch (rc) {
        case SR_OK: return "OK";
        case SR_ERR_SERIAL: return "serial error";
        case SR_ERR_IO: return "io error";
        case SR_ERR_FRAME: return "frame parse error";
        case SR_ERR_TRANSPORT_CLOSED: return "transport closed";
        case SR_ERR_TIMEOUT: return "timeout";
        case SR_ERR_CRC: return "crc mismatch";
        case SR_ERR_NOT_RUNNING: return "not running";
        case SR_ERR_ALREADY_STARTED: return "already started";
        case SR_ERR_NULL: return "null pointer";
        case SR_ERR_INVALID_ARG: return "invalid argument";
        case SR_ERR_PANIC: return "panic in driver";
        default: return "unknown";
    }
}

int main(int argc, char** argv) {
    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    uint32_t baud = argc > 2 ? (uint32_t)strtoul(argv[2], NULL, 10) : 115200;

    char err[256];
    sr_driver* d = sr_driver_open(port, baud, err, sizeof err);
    if (d == NULL) {
        fprintf(stderr, "FATAL: sr_driver_open failed: %s\n", err);
        return 1;
    }

    /* 注册回调(全 NULL 的槽位被忽略) */
    callback_ctx ctx = {0, 0, 0, 0};
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof cbs);
    cbs.userdata = &ctx;
    cbs.on_imu_data = on_imu;
    cbs.on_power_data = on_power;
    cbs.on_battery_state = on_battery;
    cbs.on_log = on_log;
    cbs.on_error = on_error;
    if (sr_driver_set_callbacks(d, &cbs) != SR_OK) {
        fprintf(stderr, "FATAL: set_callbacks failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* 启动(读线程 + 分发线程) */
    if (sr_driver_start(d) != SR_OK) {
        fprintf(stderr, "FATAL: start failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* 查询全部配置(同步,阻塞 ≤1s) */
    sr_board_config cfg;
    memset(&cfg, 0, sizeof cfg);
    int rc = sr_driver_query_all_configs(d, &cfg);
    if (rc != SR_OK) {
        printf("query_all_configs: %s\n", err_name(rc));
    } else {
        printf("config: baud=%u, servo limit=%u mA, charge stop=%u%%, servo power %s\n",
               cfg.servo_baud_rate, cfg.servo_current_limit_ma,
               cfg.charge_stop_percentage, cfg.power_servo_on ? "ON" : "OFF");
    }

    /* 写配置并等待确认(同步) */
    uint8_t ok = 0;
    sr_config sc = {.typ = SR_CONFIG_SERVO_BAUD_RATE, .value = 1000000.0f};
    rc = sr_driver_write_config_sync(d, sc, &ok);
    printf("write_config(baud=1000000): %s\n",
           rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

    /* 舵机命令透传(不等待应答;字节内容取决于舵机协议) */
    const uint8_t servo_cmd[] = {0x01, 0x02}; /* 示例字节,按实际协议替换 */
    rc = sr_driver_forward_servo(d, servo_cmd, sizeof servo_cmd);
    if (rc != SR_OK) {
        printf("forward_servo: %s\n", err_name(rc));
    }

    /* 收 1 秒数据,观察回调 */
    sleep(1);
    printf("received %lu imu, %lu power, %lu battery, %lu log frames\n",
           ctx.imu_count, ctx.power_count, ctx.battery_count, ctx.log_count);

    /* 停止 + 释放(必须最后调用) */
    if (sr_driver_stop(d) != SR_OK) {
        fprintf(stderr, "FATAL: stop failed\n");
    }
    sr_driver_free(d);
    printf("== done ==\n");
    return 0;
}
