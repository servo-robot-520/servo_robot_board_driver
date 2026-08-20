/*
 * servo-robot-driver C 使用示例 — 演示全部 FFI 函数和回调
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

/* ═══ 具体应答回调（驱动内部自动分解 Response 后调用） ═══ */

static void on_ack_device_info(void* userdata, const sr_device_info* info) {
    (void)userdata;
    printf("[DEVICE] id=0x%04x uid=0x%08x imu=0x%02x fw=%u.%u.%u "
           "ram=%uKB boot=%u app=%u ota=%u user=%uKB\n",
           info->device_id, info->uid, info->imu_id,
           info->fw_major, info->fw_minor, info->fw_patch,
           info->ram_kb, info->flash_boot_kb, info->flash_app_kb,
           info->flash_ota_kb, info->flash_user_kb);
}

static void on_ack_cfg_write(void* userdata, uint8_t success) {
    (void)userdata;
    printf("[ACK CFG WRITE] success=%u\n", success);
}

static void on_ack_cfg_query(void* userdata, const sr_config* cfg) {
    (void)userdata;
    printf("[ACK CFG QUERY] typ=0x%02x value=%.1f\n", cfg->typ, cfg->value);
}

static void on_ack_cfg_query_all(void* userdata, const sr_board_config* cfg) {
    (void)userdata;
    printf("[ACK CFG ALL] baud=%u servo_limit=%u charge_stop=%u%%\n",
           cfg->servo_baud_rate, cfg->servo_current_limit_ma,
           cfg->charge_stop_percentage);
}

static void on_ack_servo_cmd(void* userdata, const uint8_t* data, size_t len) {
    (void)userdata;
    printf("[ACK SERVO] %zu bytes:", len);
    for (size_t i = 0; i < len && i < 16; i++) printf(" %02x", data[i]);
    printf("\n");
}

static void on_ack_command(void* userdata, uint8_t success) {
    (void)userdata;
    printf("[ACK CMD] success=%u\n", success);
}

static void on_ack_firmware_update(void* userdata, uint8_t success, uint32_t offset) {
    (void)userdata;
    printf("[ACK FW] success=%u offset=%u\n", success, offset);
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
    /* ── sr_driver_version: 获取驱动版本（始终成功，无需句柄） ── */
    sr_version ver = sr_driver_version();
    printf("driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    uint32_t baud = argc > 2 ? (uint32_t)strtoul(argv[2], NULL, 10) : 115200;

    /* ── sr_driver_open: 打开串口（不自动重连） ── */
    char err[256];
    sr_driver* d = sr_driver_open(port, baud, err, sizeof err);
    if (d == NULL) {
        fprintf(stderr, "FATAL: sr_driver_open failed: %s\n", err);
        return 1;
    }

    /* ── sr_driver_set_callbacks: 注册回调 ── */
    callback_ctx ctx = {0};
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof cbs);
    cbs.userdata = &ctx;
    cbs.on_imu_data = on_imu;
    cbs.on_power_data = on_power;
    cbs.on_battery_state = on_battery;
    cbs.on_diagnostic = on_diagnostic;
    cbs.on_ack_device_info = on_ack_device_info;
    cbs.on_ack_cfg_write = on_ack_cfg_write;
    cbs.on_ack_cfg_query = on_ack_cfg_query;
    cbs.on_ack_cfg_query_all = on_ack_cfg_query_all;
    cbs.on_ack_servo_cmd = on_ack_servo_cmd;
    cbs.on_ack_command = on_ack_command;
    cbs.on_ack_firmware_update = on_ack_firmware_update;
    cbs.on_log = on_log;
    cbs.on_error = on_error;
    if (sr_driver_set_callbacks(d, &cbs) != SR_OK) {
        fprintf(stderr, "FATAL: set_callbacks failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* ── sr_driver_start: 启动驱动 ── */
    if (sr_driver_start(d) != SR_OK) {
        fprintf(stderr, "FATAL: start failed\n");
        sr_driver_free(d);
        return 1;
    }

    /* ── sr_driver_query_all_configs: 查询全部配置（同步） ── */
    sr_board_config cfg;
    memset(&cfg, 0, sizeof cfg);
    int rc = sr_driver_query_all_configs(d, &cfg);
    printf("query_all_configs: %s", err_name(rc));
    if (rc == SR_OK) {
        printf(" baud=%u servo_limit=%u charge_stop=%u%%",
               cfg.servo_baud_rate, cfg.servo_current_limit_ma,
               cfg.charge_stop_percentage);
    }
    printf("\n");

    /* ── sr_driver_query_config: 查询单个配置（同步） ── */
    sr_config sc;
    rc = sr_driver_query_config(d, SR_CONFIG_SERVO_BAUD_RATE, &sc);
    printf("query_config(baud): %s", err_name(rc));
    if (rc == SR_OK) printf(" value=%.0f", sc.value);
    printf("\n");

    /* ── sr_driver_query_device_info: 查询设备信息（同步） ── */
    sr_device_info dev;
    rc = sr_driver_query_device_info(d, &dev);
    printf("query_device_info: %s", err_name(rc));
    if (rc == SR_OK) {
        printf(" id=0x%04x fw=%u.%u.%u ram=%uKB",
               dev.device_id, dev.fw_major, dev.fw_minor, dev.fw_patch, dev.ram_kb);
    }
    printf("\n");

    /* ── sr_driver_write_config_sync: 写配置（同步等待应答） ── */
    uint8_t ok = 0;
    sr_config wcfg = {.typ = SR_CONFIG_SERVO_BAUD_RATE, .value = 1000000.0f};
    rc = sr_driver_write_config_sync(d, wcfg, &ok);
    printf("write_config_sync(baud=1000000): %s\n",
           rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

    /* ── sr_driver_write_config: 写配置（不等待应答） ── */
    wcfg.value = 115200.0f;
    rc = sr_driver_write_config(d, wcfg);
    printf("write_config(baud=115200): %s\n", err_name(rc));

    /* ── sr_driver_send_command_sync: 发送系统命令（同步） ── */
    rc = sr_driver_send_command_sync(d, 0x01 /* Reset */, &ok);
    printf("send_command_sync(Reset): %s\n",
           rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

    /* ── sr_driver_send_command: 发送系统命令（不等待应答） ── */
    rc = sr_driver_send_command(d, 0x02 /* Shutdown */);
    printf("send_command(Shutdown): %s\n", err_name(rc));

    /* ── sr_driver_forward_servo: 转发舵机命令（不等待应答） ── */
    const uint8_t servo_cmd[] = {0x01, 0x02};
    rc = sr_driver_forward_servo(d, servo_cmd, sizeof servo_cmd);
    printf("forward_servo: %s\n", err_name(rc));

    /* ── sr_driver_forward_servo_sync: 转发舵机命令（同步等待响应） ── */
    uint8_t servo_resp[64];
    size_t servo_resp_len = 0;
    rc = sr_driver_forward_servo_sync(d, servo_cmd, sizeof servo_cmd,
                                      servo_resp, sizeof servo_resp, &servo_resp_len);
    printf("forward_servo_sync: %s resp_len=%zu\n", err_name(rc), servo_resp_len);

    /* ── sr_driver_firmware_update: 固件更新（不等待应答） ── */
    const uint8_t fw_data[] = {0xAA, 0x55, 0x01, 0x02};
    rc = sr_driver_firmware_update(d, 0, fw_data, sizeof fw_data);
    printf("firmware_update: %s\n", err_name(rc));

    /* ── sr_driver_firmware_update_sync: 固件更新（同步） ── */
    rc = sr_driver_firmware_update_sync(d, 0, fw_data, sizeof fw_data, &ok);
    printf("firmware_update_sync: %s\n",
           rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

    /* ── sr_driver_last_error: 获取最近错误描述 ── */
    char last_err[256];
    rc = sr_driver_last_error(d, last_err, sizeof last_err);
    printf("last_error: %s (rc=%d)\n", err_name(rc), rc);

    /* 收 1 秒数据 */
    sleep(1);
    printf("\nreceived %lu imu, %lu power, %lu battery, %lu diag, %lu log\n",
           ctx.imu_count, ctx.power_count, ctx.battery_count,
           ctx.diagnostic_count, ctx.log_count);

    /* ── sr_driver_connect: 重新连接（上层自行控制重连逻辑） ── */
    /*
    rc = sr_driver_connect(d, "/dev/ttyUSB1", 115200);
    if (rc != SR_OK) {
        printf("reconnect failed: %s\n", err_name(rc));
    }
    */

    /* ── sr_driver_stop + sr_driver_free ── */
    sr_driver_stop(d);
    sr_driver_free(d);
    printf("== done ==\n");
    return 0;
}
