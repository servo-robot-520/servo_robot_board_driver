/*
 * servo-robot-driver C 使用示例 — 完整驱动生命周期
 *
 * 编译:
 *   cargo build --release --features ffi
 *   gcc -Wall -Wextra -I../include c_example.c -o c_example \
 *       -L ../../../target/release -lservo_robot_driver \
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
    uint64_t diagnostic_count;
    uint64_t response_count;
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
               state->temperature % 10, (unsigned)state->cell_count);
    }
}

static void on_diagnostic(void* userdata, const sr_diagnostic* diag) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->diagnostic_count++;
    if (ctx->diagnostic_count <= 3) {
        printf("[DIAG] cpu=%u%% uptime=%us heap=%uKB mcu=%.1fC\n",
               diag->cpu_usage_percent, diag->uptime_s,
               diag->free_heap_kb, diag->temp_mcu / 10.0f);
    }
}

static void on_response(void* userdata, const sr_response* resp) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->response_count++;
    printf("[RESP] kind=%u success=%u data_len=%zu\n",
           resp->request_kind, resp->success, resp->data_len);
}

static void on_log(void* userdata, const sr_log_message* msg) {
    callback_ctx* ctx = (callback_ctx*)userdata;
    ctx->log_count++;
    printf("[LOG] %s::%s: %s\n",
           msg->file_name ? msg->file_name : "?",
           msg->fun_name ? msg->fun_name : "?",
           msg->msg ? msg->msg : "");
}

static void on_error(void* userdata, int error_code) {
    (void)userdata;
    fprintf(stderr, "[ERROR] code=%d\n", error_code);
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
    /* 查询驱动版本 */
    sr_version ver = sr_driver_version();
    printf("driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    uint32_t baud = argc > 2 ? (uint32_t)strtoul(argv[2], NULL, 10) : 115200;

    /* 打开串口（不自动重连） */
    char err[256];
    sr_driver* d = sr_driver_open(port, baud, err, sizeof err);
    if (d == NULL) {
        fprintf(stderr, "FATAL: sr_driver_open failed: %s\n", err);
        return 1;
    }

    /* 注册回调 */
    callback_ctx ctx = {0};
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof cbs);
    cbs.userdata = &ctx;
    cbs.on_imu_data = on_imu;
    cbs.on_power_data = on_power;
    cbs.on_battery_state = on_battery;
    cbs.on_diagnostic = on_diagnostic;
    cbs.on_response = on_response;
    cbs.on_log = on_log;
    cbs.on_error = on_error;
    if (sr_driver_set_callbacks(d, &cbs) != SR_OK) {
        fprintf(stderr, "FATAL: set_callbacks failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* 启动 */
    if (sr_driver_start(d) != SR_OK) {
        fprintf(stderr, "FATAL: start failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* 查询全部配置 */
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

    /* 写配置 */
    uint8_t ok = 0;
    sr_config sc = {.typ = SR_CONFIG_SERVO_BAUD_RATE, .value = 1000000.0f};
    rc = sr_driver_write_config_sync(d, sc, &ok);
    printf("write_config(baud=1000000): %s\n",
           rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

    /* 收 1 秒数据 */
    sleep(1);
    printf("received %lu imu, %lu power, %lu battery, %lu diag, %lu resp, %lu log\n",
           ctx.imu_count, ctx.power_count, ctx.battery_count,
           ctx.diagnostic_count, ctx.response_count, ctx.log_count);

    /* 重连示例（上层自行控制） */
    /*
    rc = sr_driver_connect(d, "/dev/ttyUSB1", 115200);
    if (rc != SR_OK) {
        printf("reconnect failed: %s\n", err_name(rc));
    }
    */

    /* 停止 + 释放 */
    sr_driver_stop(d);
    sr_driver_free(d);
    printf("== done ==\n");
    return 0;
}
