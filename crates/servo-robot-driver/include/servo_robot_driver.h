/*
 * servo_robot_driver C API — 手写头文件,与 src/ffi.rs 严格一致
 *
 * 构建:  cd cargo build --release --features ffi
 * 链接:  -L target/release -lservo_robot_driver
 *
 * ═══ 线程与生命周期红线 ═══
 * 1. 所有回调在驱动内部的分发线程上触发,C 侧回调必须线程安全。
 * 2. 回调内禁止调用任何 sr_driver_* 函数(尤其 sr_driver_free:
 *    drop 会 join 分发线程,而该线程正卡在你的回调里 = 自死锁)。
 * 3. 回调参数指针仅在回调执行期间有效,不要跨调用保存。
 * 4. 同步函数阻塞 ≤1s(驱动默认超时),不要在实时路径调用。
 * 5. 同一句柄的所有调用线程安全(内部有锁),但回调线程与调用线程并存。
 * 6. GPL-3 许可:链接本库进入闭源程序有许可影响。
 */
#ifndef SERVO_ROBOT_DRIVER_H
#define SERVO_ROBOT_DRIVER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ═══ 不透明句柄 ═══ */
typedef struct sr_driver sr_driver;

/* ═══ 错误码(与 src/ffi.rs 常量一致) ═══ */
enum {
    SR_OK = 0,
    SR_ERR_SERIAL = -1,
    SR_ERR_IO = -2,
    SR_ERR_FRAME = -3,
    SR_ERR_TRANSPORT_CLOSED = -4,
    SR_ERR_TIMEOUT = -5,
    SR_ERR_CRC = -6,
    SR_ERR_PAYLOAD_TOO_SHORT = -7,
    SR_ERR_UNKNOWN_FRAME = -8,
    SR_ERR_NOT_RUNNING = -9,
    SR_ERR_LOCK_POISONED = -10,
    SR_ERR_NULL = -11,
    SR_ERR_INVALID_ARG = -12,
    SR_ERR_PANIC = -13,
};

/* ═══ 配置类型(ConfigType,0x10~0x37) ═══ */
enum {
    SR_CONFIG_SWITCH_SERVO_POWER = 0x10,
    SR_CONFIG_SWITCH_5V_POWER = 0x11,
    SR_CONFIG_SWITCH_CHARGE = 0x12,
    SR_CONFIG_SWITCH_BAT_EXT_OUT = 0x13,
    SR_CONFIG_CHARGE_STOP_SOC = 0x20,
    SR_CONFIG_TX_LOG_LEVEL = 0x21,
    SR_CONFIG_SERVO_CURRENT_LIMIT_MA = 0x30,
    SR_CONFIG_SERVO_TEMP_LIMIT = 0x31,
    SR_CONFIG_5V_TEMP_LIMIT = 0x32,
    SR_CONFIG_CHARGE_MAX_CURRENT_MA = 0x33,
    SR_CONFIG_CHARGE_TEMP_DERATING = 0x34,
    SR_CONFIG_CHARGE_TEMP_LIMIT = 0x35,
    SR_CONFIG_CHARGE_STOP_VOLTAGE_MV = 0x36,
    SR_CONFIG_SERVO_BAUD_RATE = 0x37,
};

/* ═══ 板级命令(CommandType) ═══ */
enum {
    SR_CMD_RESET = 0x01,    /* 重启 MCU */
    SR_CMD_SHUTDOWN = 0x02, /* 关机(切断全部电源) */
    SR_CMD_OTA = 0x03,      /* 触发 OTA 更新 */
};

/* ═══ 日志等级 ═══ */
enum {
    SR_LOG_OFF = 0,
    SR_LOG_DEBUG = 1,
    SR_LOG_INFO = 2,
    SR_LOG_WARN = 3,
    SR_LOG_ERROR = 4,
};

/* ═══ 电池状态枚举 ═══ */
enum {
    SR_BAT_CHARGE_UNKNOWN = 0,
    SR_BAT_CHARGE_CHARGING = 1,
    SR_BAT_CHARGE_DISCHARGING = 2,
    SR_BAT_CHARGE_NOT_CHARGING = 3,
    SR_BAT_CHARGE_FULL = 4,
};
enum {
    SR_BAT_HEALTH_UNKNOWN = 0,
    SR_BAT_HEALTH_GOOD = 1,
    SR_BAT_HEALTH_OVERHEAT = 2,
    SR_BAT_HEALTH_DEAD = 3,
    SR_BAT_HEALTH_OVERVOLTAGE = 4,
};
enum {
    SR_BAT_TECH_UNKNOWN = 0,
    SR_BAT_TECH_NIMH = 1,
    SR_BAT_TECH_LION = 2,
    SR_BAT_TECH_LIPO = 3,
    SR_BAT_TECH_LIFE = 4,
    SR_BAT_TECH_NICD = 5,
    SR_BAT_TECH_LIMN = 6,
};

/* ═══ 充电阶段 ═══ */
enum {
    SR_CHARGE_PHASE_NOT_CHARGING = 0,
    SR_CHARGE_PHASE_PRE_CHARGE = 1,
    SR_CHARGE_PHASE_CC = 2,
    SR_CHARGE_PHASE_CV = 3,
    SR_CHARGE_PHASE_FULL = 4,
    SR_CHARGE_PHASE_PD_SINK_FAULT = 5,
    SR_CHARGE_PHASE_UNSUPPORTED_CHARGER = 6,
};

/* ═══ 数据结构(字段顺序与 src/ffi.rs 严格一致) ═══ */

/* 配置值: type 为 SR_CONFIG_*, value 语义: bool→0/1, 数值→原值 */
typedef struct {
    uint8_t typ;
    float value;
} sr_config;

/* 板级配置全量快照 */
typedef struct {
    uint8_t power_servo_on;
    uint8_t power_5v_on;
    uint8_t charge_on;
    uint8_t bat_ext_out_on;
    uint8_t charge_stop_percentage;
    uint8_t tx_log_level;
    uint16_t servo_current_limit_ma;
    uint16_t servo_temp_limit;
    uint16_t temp_5v_limit;
    uint16_t charge_max_current_ma;
    uint16_t charge_temp_derating;
    uint16_t charge_temp_limit;
    uint16_t charge_stop_voltage_mv;
    uint32_t servo_baud_rate;
} sr_board_config;

typedef struct {
    float accel[3];
    float gyro[3];
    float quaternion[4]; /* w, x, y, z */
    uint32_t timestamp_ms;
    float roll;
    float pitch;
    float yaw;
} sr_imu;

typedef struct {
    uint16_t servo_voltage_mv;
    uint16_t servo_current_ma;
    uint16_t charge_in_voltage_mv;
    uint16_t charge_in_current_ma;
    uint16_t bat_voltage_mv;
    int16_t bat_current_ma;
} sr_power;

typedef struct {
    uint16_t voltage_mv;
    int16_t current_ma;
    uint16_t capacity_mah;
    uint16_t design_capacity_mah;
    uint8_t percentage;
    int16_t temperature;
    uint8_t charge_status; /* SR_BAT_CHARGE_* */
    uint8_t health;        /* SR_BAT_HEALTH_* */
    uint8_t technology;    /* SR_BAT_TECH_* */
    uint8_t present;
    uint16_t serial_number;
    /* 以下数组仅在回调执行期间有效 */
    const uint16_t* cell_voltages_mv;
    uint32_t cell_count;
    const int16_t* cell_temperatures;
    uint32_t cell_temp_count;
} sr_battery_state;

typedef struct {
    uint8_t charge_phase; /* SR_CHARGE_PHASE_* */
    uint16_t state_change_flags;
    uint16_t protection_flags;
    uint16_t error_flags;
} sr_board_event;

typedef struct {
    uint16_t device_id;
    uint32_t uid;
    uint8_t imu_id;
    uint32_t uptime_s;
    uint8_t cpu_usage_percent;
    uint16_t free_heap_kb;
    uint16_t stack_watermark_min_kb;
    uint16_t i2c_error_count;
    uint16_t spi_error_count;
    uint16_t uart_error_count;
    uint16_t usb_error_count;
    uint32_t frames_sent_total;
    uint16_t pd_request_voltage_mv;
    uint16_t pd_request_current_ma;
    uint8_t fw_major;
    uint8_t fw_minor;
    uint8_t fw_patch;
    int16_t temp_servo_power; /* 实际值 = 原始值 / 10 */
    int16_t temp_5v_power;
    int16_t temp_mcu;
    int16_t temp_charge;
    int16_t temp_battery;
} sr_system_info;

typedef struct {
    uint64_t ts_ms; /* Unix 时间戳(毫秒) */
    uint8_t level;  /* SR_LOG_* */
    /* NUL 结尾,仅在回调执行期间有效 */
    const char* file_name;
    const char* fun_name;
    const char* msg;
} sr_log_message;

/* ═══ 回调表(全 NULL 即可只注册部分;任意时刻可替换) ═══ */
typedef struct {
    void* userdata;
    void (*on_imu_data)(void* userdata, const sr_imu* data);
    void (*on_power_data)(void* userdata, const sr_power* data);
    void (*on_battery_state)(void* userdata, const sr_battery_state* state);
    void (*on_config_snapshot)(void* userdata, const sr_board_config* config);
    void (*on_board_event)(void* userdata, const sr_board_event* event);
    void (*on_system_info)(void* userdata, const sr_system_info* info);
    void (*on_log)(void* userdata, const sr_log_message* msg);
    void (*on_ack_cfg_write)(void* userdata, uint8_t success);
    void (*on_ack_cfg_query)(void* userdata, const sr_config* config);
    void (*on_ack_cfg_query_all)(void* userdata, const sr_board_config* config);
    void (*on_ack_servo_cmd)(void* userdata, const uint8_t* data, size_t len);
    void (*on_ack_command)(void* userdata, uint8_t success);
    void (*on_ack_firmware_update)(void* userdata, uint8_t success, uint32_t offset);
    void (*on_error)(void* userdata, int error_code);
} sr_callbacks;

/* ═══ 生命周期 ═══ */

/* 打开串口并创建驱动;失败返回 NULL,错误描述写入 err_buf */
sr_driver* sr_driver_open(const char* port, uint32_t baud_rate,
                          char* err_buf, size_t err_buf_len);

/* 打开串口并创建支持自动重连的驱动 */
sr_driver* sr_driver_open_reconnect(const char* port, uint32_t baud_rate,
                                    uint32_t max_retries, uint32_t retry_interval_ms,
                                    float backoff_multiplier, uint32_t max_retry_interval_ms,
                                    char* err_buf, size_t err_buf_len);

/* 释放句柄(内部 stop + join);NULL 安全 */
void sr_driver_free(sr_driver* d);

/* ═══ 控制 ═══ */
int sr_driver_start(sr_driver* d);
int sr_driver_stop(sr_driver* d);

/* ═══ 配置 ═══ */
int sr_driver_write_config(sr_driver* d, sr_config cfg);
int sr_driver_write_config_sync(sr_driver* d, sr_config cfg, uint8_t* out_success);
int sr_driver_query_config(sr_driver* d, uint8_t typ, sr_config* out);
int sr_driver_query_all_configs(sr_driver* d, sr_board_config* out);

/* ═══ 舵机(透传原始舵机命令字节) ═══ */
int sr_driver_forward_servo(sr_driver* d, const uint8_t* data, size_t len);
/* 应答写入调用方 buffer;cap 不足返回 SR_ERR_PAYLOAD_TOO_SHORT */
int sr_driver_forward_servo_sync(sr_driver* d, const uint8_t* data, size_t len,
                                 uint8_t* out, size_t cap, size_t* out_len);

/* ═══ 板级命令(SR_CMD_*) ═══ */
int sr_driver_send_command(sr_driver* d, uint8_t cmd);
int sr_driver_send_command_sync(sr_driver* d, uint8_t cmd, uint8_t* out_success);

/* ═══ 固件更新 ═══ */
int sr_driver_firmware_update(sr_driver* d, uint32_t offset, const uint8_t* data, size_t len);
int sr_driver_firmware_update_sync(sr_driver* d, uint32_t offset, const uint8_t* data,
                                   size_t len, uint8_t* out_success);

/* ═══ 回调与诊断 ═══ */
int sr_driver_set_callbacks(sr_driver* d, const sr_callbacks* cbs);
int sr_driver_last_error(sr_driver* d, char* buf, size_t len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SERVO_ROBOT_DRIVER_H */
