// servo-robot-driver C++ 使用示例 — 类内回调模式(ROS2 节点风格)
//
// 回调桥:FFI 需要 C 链接的裸函数指针,类方法不能直接进 sr_callbacks。
// 方案:extern "C" thunk + userdata 指向 CallbackCtx(std::function 槽),
// 节点构造时用 lambda 把成员方法绑进槽位。ROS2 中把成员方法里的 printf
// 换成 topic 发布 / RCLCPP_INFO 即可。
//
// 编译:
//   cargo build --release --features ffi
//   g++ -std=c++17 -Wall -Wextra -I../include cpp_example.cpp -o cpp_example \
//       -L ../../../target/release -lservo_robot_driver \
//       -Wl,-rpath,$PWD/../../../target/release
//
// 运行(真实设备):
//   ./cpp_example /dev/ttyUSB0 115200
//
// 线程模型:回调在驱动分发线程执行;回调内禁止调用任何 sr_driver_*。

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <stdexcept>
#include <string>
#include <thread>

#include "servo_robot_driver.h"

namespace {

// ═══ RAII 句柄 ═══
class SrDriver {
public:
    SrDriver(const char* port, uint32_t baud) {
        char err[256];
        handle_ = sr_driver_open(port, baud, err, sizeof err);
        if (handle_ == nullptr) {
            throw std::runtime_error("sr_driver_open failed: " + std::string(err));
        }
    }

    ~SrDriver() { sr_driver_free(handle_); }

    SrDriver(const SrDriver&) = delete;
    SrDriver& operator=(const SrDriver&) = delete;

    sr_driver* get() const { return handle_; }

    static void throw_on_error(int rc, const char* what) {
        if (rc != SR_OK) {
            throw std::runtime_error(std::string(what) + " failed, error code " +
                                     std::to_string(rc));
        }
    }

private:
    sr_driver* handle_ = nullptr;
};

// ═══ 回调桥:std::function 槽位(构造时绑定,回调零分配) ═══
struct CallbackCtx {
    std::function<void(const sr_imu*)> on_imu;
    std::function<void(const sr_power*)> on_power;
    std::function<void(const sr_battery_state*)> on_battery;
    std::function<void(const sr_diagnostic*)> on_diagnostic;
    std::function<void(const sr_device_info*)> on_ack_device_info;
    std::function<void(uint8_t)> on_ack_cfg_write;
    std::function<void(const sr_config*)> on_ack_cfg_query;
    std::function<void(const sr_board_config*)> on_ack_cfg_query_all;
    std::function<void(const uint8_t*, size_t)> on_ack_servo_cmd;
    std::function<void(uint8_t)> on_ack_command;
    std::function<void(uint8_t, uint32_t)> on_ack_firmware_update;
    std::function<void(const sr_log_message*)> on_log;
    std::function<void(int)> on_error;
};

extern "C" {
void imu_thunk(void* u, const sr_imu* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_imu) c->on_imu(d);
}
void power_thunk(void* u, const sr_power* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_power) c->on_power(d);
}
void battery_thunk(void* u, const sr_battery_state* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_battery) c->on_battery(d);
}
void diagnostic_thunk(void* u, const sr_diagnostic* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_diagnostic) c->on_diagnostic(d);
}
void ack_device_info_thunk(void* u, const sr_device_info* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_device_info) c->on_ack_device_info(d);
}
void ack_cfg_write_thunk(void* u, uint8_t s) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_cfg_write) c->on_ack_cfg_write(s);
}
void ack_cfg_query_thunk(void* u, const sr_config* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_cfg_query) c->on_ack_cfg_query(d);
}
void ack_cfg_query_all_thunk(void* u, const sr_board_config* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_cfg_query_all) c->on_ack_cfg_query_all(d);
}
void ack_servo_cmd_thunk(void* u, const uint8_t* d, size_t l) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_servo_cmd) c->on_ack_servo_cmd(d, l);
}
void ack_command_thunk(void* u, uint8_t s) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_command) c->on_ack_command(s);
}
void ack_firmware_update_thunk(void* u, uint8_t s, uint32_t o) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_ack_firmware_update) c->on_ack_firmware_update(s, o);
}
void log_thunk(void* u, const sr_log_message* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_log) c->on_log(d);
}
void error_thunk(void* u, int code) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_error) c->on_error(code);
}
} // extern "C"

const char* err_name(int rc) {
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

// ═══ 驱动节点(ROS2 节点结构映射) ═══
//
// 成员顺序:ctx_ 先于 driver_ —— 析构时 driver_ 先销毁(join 分发线程,
// 期间 ctx_ 仍存活),回调不会用到已销毁的上下文。
class ServoRobotDriverNode {
public:
    ServoRobotDriverNode(const char* port, uint32_t baud) : driver_(port, baud) {
        // 绑定回调
        ctx_.on_imu = [this](const sr_imu* d) { onImu(d); };
        ctx_.on_power = [this](const sr_power* d) { onPower(d); };
        ctx_.on_battery = [this](const sr_battery_state* d) { onBattery(d); };
        ctx_.on_diagnostic = [this](const sr_diagnostic* d) { onDiagnostic(d); };
        ctx_.on_ack_device_info = [this](const sr_device_info* d) { onAckDeviceInfo(d); };
        ctx_.on_ack_cfg_write = [this](uint8_t s) { onAckCfgWrite(s); };
        ctx_.on_ack_cfg_query = [this](const sr_config* d) { onAckCfgQuery(d); };
        ctx_.on_ack_cfg_query_all = [this](const sr_board_config* d) { onAckCfgQueryAll(d); };
        ctx_.on_ack_servo_cmd = [this](const uint8_t* d, size_t l) { onAckServoCmd(d, l); };
        ctx_.on_ack_command = [this](uint8_t s) { onAckCommand(s); };
        ctx_.on_ack_firmware_update = [this](uint8_t s, uint32_t o) { onAckFirmwareUpdate(s, o); };
        ctx_.on_log = [this](const sr_log_message* d) { onLog(d); };
        ctx_.on_error = [this](int code) { onError(code); };

        sr_callbacks cbs{};
        cbs.userdata = &ctx_;
        cbs.on_imu_data = imu_thunk;
        cbs.on_power_data = power_thunk;
        cbs.on_battery_state = battery_thunk;
        cbs.on_diagnostic = diagnostic_thunk;
        cbs.on_ack_device_info = ack_device_info_thunk;
        cbs.on_ack_cfg_write = ack_cfg_write_thunk;
        cbs.on_ack_cfg_query = ack_cfg_query_thunk;
        cbs.on_ack_cfg_query_all = ack_cfg_query_all_thunk;
        cbs.on_ack_servo_cmd = ack_servo_cmd_thunk;
        cbs.on_ack_command = ack_command_thunk;
        cbs.on_ack_firmware_update = ack_firmware_update_thunk;
        cbs.on_log = log_thunk;
        cbs.on_error = error_thunk;
        SrDriver::throw_on_error(sr_driver_set_callbacks(driver_.get(), &cbs),
                                 "set_callbacks");
    }

    void run() {
        SrDriver::throw_on_error(sr_driver_start(driver_.get()), "start");

        // sr_driver_query_all_configs
        sr_board_config cfg{};
        int rc = sr_driver_query_all_configs(driver_.get(), &cfg);
        std::printf("query_all_configs: %s", err_name(rc));
        if (rc == SR_OK) std::printf(" baud=%u", cfg.servo_baud_rate);
        std::printf("\n");

        // sr_driver_query_config
        sr_config sc{};
        rc = sr_driver_query_config(driver_.get(), SR_CONFIG_SERVO_BAUD_RATE, &sc);
        std::printf("query_config(baud): %s", err_name(rc));
        if (rc == SR_OK) std::printf(" value=%.0f", sc.value);
        std::printf("\n");

        // sr_driver_query_device_info
        sr_device_info dev{};
        rc = sr_driver_query_device_info(driver_.get(), &dev);
        std::printf("query_device_info: %s", err_name(rc));
        if (rc == SR_OK) {
            std::printf(" id=0x%04x fw=%u.%u.%u ram=%uKB",
                        dev.device_id, dev.fw_major, dev.fw_minor, dev.fw_patch, dev.ram_kb);
        }
        std::printf("\n");

        // sr_driver_write_config_sync
        uint8_t ok = 0;
        sr_config wcfg{};
        wcfg.typ = SR_CONFIG_SERVO_BAUD_RATE;
        wcfg.value = 1000000.0f;
        rc = sr_driver_write_config_sync(driver_.get(), wcfg, &ok);
        std::printf("write_config_sync(baud=1000000): %s\n",
                    rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

        // sr_driver_write_config (async)
        wcfg.value = 115200.0f;
        rc = sr_driver_write_config(driver_.get(), wcfg);
        std::printf("write_config(baud=115200): %s\n", err_name(rc));

        // sr_driver_send_command_sync
        rc = sr_driver_send_command_sync(driver_.get(), 0x01 /* Reset */, &ok);
        std::printf("send_command_sync(Reset): %s\n",
                    rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

        // sr_driver_forward_servo
        const uint8_t servo_cmd[] = {0x01, 0x02};
        rc = sr_driver_forward_servo(driver_.get(), servo_cmd, sizeof servo_cmd);
        std::printf("forward_servo: %s\n", err_name(rc));

        // sr_driver_forward_servo_sync
        uint8_t servo_resp[64]{};
        size_t resp_len = 0;
        rc = sr_driver_forward_servo_sync(driver_.get(), servo_cmd, sizeof servo_cmd,
                                          servo_resp, sizeof servo_resp, &resp_len);
        std::printf("forward_servo_sync: %s resp_len=%zu\n", err_name(rc), resp_len);

        // sr_driver_firmware_update
        const uint8_t fw_data[] = {0xAA, 0x55, 0x01, 0x02};
        rc = sr_driver_firmware_update(driver_.get(), 0, fw_data, sizeof fw_data);
        std::printf("firmware_update: %s\n", err_name(rc));

        // sr_driver_last_error
        char last_err[256]{};
        rc = sr_driver_last_error(driver_.get(), last_err, sizeof last_err);
        std::printf("last_error: %s\n", err_name(rc));

        // 收 1 秒数据
        std::this_thread::sleep_for(std::chrono::milliseconds(1000));
        std::printf("\nreceived %lu imu, %lu power, %lu battery, %lu diag, %lu log\n",
                    imu_count_.load(), power_count_.load(), battery_count_.load(),
                    diagnostic_count_.load(), log_count_.load());

        // sr_driver_connect (重连示例，注释状态)
        // rc = sr_driver_connect(driver_.get(), "/dev/ttyUSB1", 115200);

        SrDriver::throw_on_error(sr_driver_stop(driver_.get()), "stop");
    }

private:
    void onImu(const sr_imu* data) {
        imu_count_.fetch_add(1, std::memory_order_relaxed);
        if (imu_count_.load(std::memory_order_relaxed) <= 3) {
            std::printf("[IMU #%lu] roll=%7.2f pitch=%7.2f yaw=%7.2f\n",
                        imu_count_.load(std::memory_order_relaxed),
                        data->roll, data->pitch, data->yaw);
        }
    }

    void onPower(const sr_power* data) {
        power_count_.fetch_add(1, std::memory_order_relaxed);
        if (power_count_.load(std::memory_order_relaxed) <= 3) {
            std::printf("[PWR] servo %5.1f V / %5.1f A, bat %5.1f V\n",
                        data->servo_voltage_mv / 10.0f,
                        data->servo_current_ma / 10.0f,
                        data->bat_voltage_mv / 10.0f);
        }
    }

    void onBattery(const sr_battery_state* state) {
        battery_count_.fetch_add(1, std::memory_order_relaxed);
        if (battery_count_.load(std::memory_order_relaxed) <= 3) {
            std::printf("[BAT] %u%%, temp %d.%d C, %u cells\n",
                        state->percentage, state->temperature / 10,
                        state->temperature % 10, (unsigned)state->cell_count);
        }
    }

    void onDiagnostic(const sr_diagnostic* diag) {
        diagnostic_count_.fetch_add(1, std::memory_order_relaxed);
        if (diagnostic_count_.load(std::memory_order_relaxed) <= 3) {
            std::printf("[DIAG] cpu=%u%% uptime=%us heap=%uKB mcu=%.1fC\n",
                        diag->cpu_usage_percent, diag->uptime_s,
                        diag->free_heap_kb, diag->temp_mcu / 10.0f);
        }
    }

    void onAckDeviceInfo(const sr_device_info* info) {
        std::printf("[DEVICE] id=0x%04x fw=%u.%u.%u ram=%uKB boot=%u app=%u ota=%u user=%uKB\n",
                    info->device_id, info->fw_major, info->fw_minor, info->fw_patch,
                    info->ram_kb, info->flash_boot_kb, info->flash_app_kb,
                    info->flash_ota_kb, info->flash_user_kb);
    }

    void onAckCfgWrite(uint8_t success) {
        std::printf("[ACK CFG WRITE] success=%u\n", success);
    }

    void onAckCfgQuery(const sr_config* cfg) {
        std::printf("[ACK CFG QUERY] typ=0x%02x value=%.1f\n", cfg->typ, cfg->value);
    }

    void onAckCfgQueryAll(const sr_board_config* cfg) {
        std::printf("[ACK CFG ALL] baud=%u servo_limit=%u\n",
                    cfg->servo_baud_rate, cfg->servo_current_limit_ma);
    }

    void onAckServoCmd(const uint8_t* data, size_t len) {
        std::printf("[ACK SERVO] %zu bytes:", len);
        for (size_t i = 0; i < len && i < 16; i++) std::printf(" %02x", data[i]);
        std::printf("\n");
    }

    void onAckCommand(uint8_t success) {
        std::printf("[ACK CMD] success=%u\n", success);
    }

    void onAckFirmwareUpdate(uint8_t success, uint32_t offset) {
        std::printf("[ACK FW] success=%u offset=%u\n", success, offset);
    }

    void onLog(const sr_log_message* msg) {
        log_count_.fetch_add(1, std::memory_order_relaxed);
        std::printf("[LOG] %s::%s: %s\n",
                    msg->file_name ? msg->file_name : "?",
                    msg->fun_name ? msg->fun_name : "?",
                    msg->msg ? msg->msg : "");
    }

    void onError(int code) {
        std::fprintf(stderr, "[ERROR] code=%d\n", code);
    }

    // 成员顺序:ctx_ 先于 driver_(见类注释)
    CallbackCtx ctx_;
    SrDriver driver_;

    std::atomic<uint64_t> imu_count_{0};
    std::atomic<uint64_t> power_count_{0};
    std::atomic<uint64_t> battery_count_{0};
    std::atomic<uint64_t> diagnostic_count_{0};
    std::atomic<uint64_t> log_count_{0};
};

} // namespace

int main(int argc, char** argv) {
    // sr_driver_version
    sr_version ver = sr_driver_version();
    std::printf("driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    const uint32_t baud = argc > 2 ? std::strtoul(argv[2], nullptr, 10) : 115200;

    try {
        std::printf("== servo-robot-driver C++ example ==\n");
        ServoRobotDriverNode node(port, baud);
        node.run();
        std::printf("== done ==\n");
        return 0;
    } catch (const std::exception& e) {
        std::fprintf(stderr, "FATAL: %s\n", e.what());
        return 1;
    }
}
