// servo-robot-driver C++ 使用示例 — 演示完整驱动生命周期
//
// 编译:
//   cargo build --release --features ffi
//   g++ -std=c++17 -Wall -Wextra -I../include cpp_example.cpp -o cpp_example
//       -L ../../../target/release -lservo_robot_driver
//       -Wl,-rpath,$PWD/../../../target/release
//
// 运行(真实设备):
//   ./cpp_example /dev/ttyUSB0 115200
//
// 运行(无设备,验证错误路径):
//   ./cpp_example /dev/nonexistent 115200
//
// 线程红线:回调在驱动分发线程触发,回调内禁止调用任何 sr_driver_*;
// 回调参数指针仅在回调执行期间有效。

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <stdexcept>
#include <string>
#include <thread>

#include "servo_robot_driver.h"

namespace {

// ═══ RAII 句柄:析构自动释放,C++ 惯用法封装 ═══
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

    // 包装错误码 → 异常(带上错误码;详细描述可查 sr_driver_last_error)
    static void throw_on_error(int rc, const char* what) {
        if (rc != SR_OK) {
            throw std::runtime_error(std::string(what) + " failed, error code " +
                                     std::to_string(rc));
        }
    }

private:
    sr_driver* handle_ = nullptr;
};

// ═══ 回调上下文(userdata 透传) ═══
struct CallbackCtx {
    uint64_t imu_count = 0;
    uint64_t power_count = 0;
    uint64_t battery_count = 0;
    uint64_t log_count = 0;
};

// 回调均为 C 链接(函数指针表),经 userdata 还原上下文
extern "C" {

void on_imu(void* userdata, const sr_imu* data) {
    auto* ctx = static_cast<CallbackCtx*>(userdata);
    ctx->imu_count++;
    if (ctx->imu_count <= 3) {
        std::printf("[IMU #%lu] roll=%7.2f pitch=%7.2f yaw=%7.2f\n",
                    ctx->imu_count, data->roll, data->pitch, data->yaw);
    }
}

void on_power(void* userdata, const sr_power* data) {
    auto* ctx = static_cast<CallbackCtx*>(userdata);
    ctx->power_count++;
    if (ctx->power_count <= 3) {
        // 协议约定:电压/电流原始值 = 实际值 × 10(与 Rust Display 一致)
        std::printf("[PWR] servo %5.1f V / %5.1f A, bat %5.1f V\n",
                    data->servo_voltage_mv / 10.0f, data->servo_current_ma / 10.0f,
                    data->bat_voltage_mv / 10.0f);
    }
}

void on_battery(void* userdata, const sr_battery_state* state) {
    auto* ctx = static_cast<CallbackCtx*>(userdata);
    ctx->battery_count++;
    if (ctx->battery_count <= 3) {
        std::printf("[BAT] %u%%, temp %d.%d C, %u cells\n",
                    state->percentage, state->temperature / 10,
                    state->temperature % 10, state->cell_count);
    }
}

void on_log(void* userdata, const sr_log_message* msg) {
    auto* ctx = static_cast<CallbackCtx*>(userdata);
    ctx->log_count++;
    std::printf("[BOARD LOG] %s::%s: %s\n",
                msg->file_name ? msg->file_name : "?",
                msg->fun_name ? msg->fun_name : "?", msg->msg ? msg->msg : "");
}

void on_error(void* userdata, int error_code) {
    (void)userdata;
    std::fprintf(stderr, "[DRIVER ERROR] code=%d\n", error_code);
}

} // extern "C"

// 错误码 → 描述(示例用;生产代码可直接查 SR_ERR_* 常量)
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

} // namespace

int main(int argc, char** argv) {
    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    const uint32_t baud = argc > 2 ? std::strtoul(argv[2], nullptr, 10) : 115200;

    try {
        std::printf("== servo-robot-driver C++ example ==\n");

        // 1. 打开串口(RAII 持有)
        SrDriver driver(port, baud);
        std::printf("opened %s @ %u baud\n", port, baud);

        // 2. 注册回调(全 NULL 的槽位被忽略;可任意时刻替换)
        CallbackCtx ctx;
        sr_callbacks cbs{};
        cbs.userdata = &ctx;
        cbs.on_imu_data = on_imu;
        cbs.on_power_data = on_power;
        cbs.on_battery_state = on_battery;
        cbs.on_log = on_log;
        cbs.on_error = on_error;
        SrDriver::throw_on_error(sr_driver_set_callbacks(driver.get(), &cbs),
                                 "set_callbacks");

        // 3. 启动(读线程 + 分发线程)
        SrDriver::throw_on_error(sr_driver_start(driver.get()), "start");

        // 4. 查询全部配置(同步,阻塞 ≤1s)
        sr_board_config cfg{};
        int rc = sr_driver_query_all_configs(driver.get(), &cfg);
        if (rc != SR_OK) {
            std::printf("query_all_configs: %s\n", err_name(rc));
        } else {
            std::printf("config: baud=%u, servo limit=%u mA, charge stop=%u%%, "
                        "servo power %s\n",
                        cfg.servo_baud_rate, cfg.servo_current_limit_ma,
                        cfg.charge_stop_percentage,
                        cfg.power_servo_on ? "ON" : "OFF");
        }

        // 5. 写配置并等待确认(同步)
        uint8_t success = 0;
        sr_config sc{};
        sc.typ = SR_CONFIG_SERVO_BAUD_RATE;
        sc.value = 1000000.0f; // 1,000,000 baud
        rc = sr_driver_write_config_sync(driver.get(), sc, &success);
        if (rc != SR_OK) {
            std::printf("write_config(baud=1000000): %s\n", err_name(rc));
        } else {
            std::printf("write_config(baud=1000000): %s\n",
                        success ? "ACK" : "NACK");
        }

        // 6. 舵机命令透传(不等待应答;字节内容取决于舵机协议)
        const uint8_t servo_cmd[] = {0x01, 0x02}; // 示例字节,按实际协议替换
        rc = sr_driver_forward_servo(driver.get(), servo_cmd, sizeof servo_cmd);
        if (rc != SR_OK) {
            std::printf("forward_servo: %s\n", err_name(rc));
        }

        // 7. 板级命令(同步;注意 Reset 会重启板子,默认不发送)
        // uint8_t cmd_ok = 0;
        // rc = sr_driver_send_command_sync(driver.get(), SR_CMD_RESET, &cmd_ok);
        // std::printf("send_command(Reset): %s\n", cmd_ok ? "ACK" : "NACK");

        // 8. 收 1 秒数据,观察回调
        std::this_thread::sleep_for(std::chrono::milliseconds(1000));
        std::printf("received %lu imu, %lu power, %lu battery, %lu log frames\n",
                    ctx.imu_count, ctx.power_count, ctx.battery_count, ctx.log_count);

        // 9. 停止(join 读/分发线程);析构时自动 free
        SrDriver::throw_on_error(sr_driver_stop(driver.get()), "stop");

        std::printf("== done ==\n");
        return 0;

    } catch (const std::exception& e) {
        std::fprintf(stderr, "FATAL: %s\n", e.what());
        return 1;
    }
}
