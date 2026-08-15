// servo-robot-driver C++ 使用示例 — 类内回调模式(ROS2 节点风格)
//
// 回调桥:FFI 需要 C 链接的裸函数指针,类方法不能直接进 sr_callbacks。
// 方案:extern "C" thunk + userdata 指向 CallbackCtx(std::function 槽),
// 节点构造时用 lambda 把成员方法绑进槽位。ROS2 中把成员方法里的 printf
// 换成 topic 发布 / RCLCPP_INFO 即可。
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
        // ROS2 中:这里保持 thunk 桥,成员方法内做 publish/log
        ctx_.on_imu = [this](const sr_imu* d) { onImu(d); };
        ctx_.on_power = [this](const sr_power* d) { onPower(d); };
        ctx_.on_battery = [this](const sr_battery_state* d) { onBattery(d); };
        ctx_.on_log = [this](const sr_log_message* d) { onLog(d); };
        ctx_.on_error = [this](int code) { onError(code); };

        sr_callbacks cbs{};
        cbs.userdata = &ctx_;
        cbs.on_imu_data = imu_thunk;
        cbs.on_power_data = power_thunk;
        cbs.on_battery_state = battery_thunk;
        cbs.on_log = log_thunk;
        cbs.on_error = error_thunk;
        SrDriver::throw_on_error(sr_driver_set_callbacks(driver_.get(), &cbs),
                                 "set_callbacks");
    }

    // 完整生命周期(ROS2 中:start 在节点构造/on_configure,stop 在 on_shutdown)
    void run() {
        SrDriver::throw_on_error(sr_driver_start(driver_.get()), "start");

        // 查询全部配置(同步,阻塞 ≤1s)
        sr_board_config cfg{};
        int rc = sr_driver_query_all_configs(driver_.get(), &cfg);
        if (rc != SR_OK) {
            std::printf("query_all_configs: %s\n", err_name(rc));
        } else {
            std::printf("config: baud=%u, servo limit=%u mA, charge stop=%u%%, "
                        "servo power %s\n",
                        cfg.servo_baud_rate, cfg.servo_current_limit_ma,
                        cfg.charge_stop_percentage,
                        cfg.power_servo_on ? "ON" : "OFF");
        }

        // 写配置并等待确认(同步)
        uint8_t ok = 0;
        sr_config sc{};
        sc.typ = SR_CONFIG_SERVO_BAUD_RATE;
        sc.value = 1000000.0f;
        rc = sr_driver_write_config_sync(driver_.get(), sc, &ok);
        std::printf("write_config(baud=1000000): %s\n",
                    rc != SR_OK ? err_name(rc) : (ok ? "ACK" : "NACK"));

        // 舵机命令透传(不等待应答;字节内容取决于舵机协议)
        const uint8_t servo_cmd[] = {0x01, 0x02}; // 示例字节,按实际协议替换
        rc = sr_driver_forward_servo(driver_.get(), servo_cmd, sizeof servo_cmd);
        if (rc != SR_OK) {
            std::printf("forward_servo: %s\n", err_name(rc));
        }

        // 收 1 秒数据,观察成员回调(ROS2 中由 spin 驱动,无需显式 sleep)
        std::this_thread::sleep_for(std::chrono::milliseconds(1000));
        std::printf("received %lu imu, %lu power, %lu battery, %lu log frames\n",
                    imu_count_.load(), power_count_.load(),
                    battery_count_.load(), log_count_.load());

        SrDriver::throw_on_error(sr_driver_stop(driver_.get()), "stop");
    }

private:
    // ═══ 类内成员回调(ROS2 中替换 printf 为 topic 发布 / RCLCPP_INFO) ═══
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
            // 协议约定:电压/电流原始值 = 实际值 × 10(与 Rust Display 一致)
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
                        state->temperature % 10, state->cell_count);
        }
    }

    void onLog(const sr_log_message* msg) {
        log_count_.fetch_add(1, std::memory_order_relaxed);
        std::printf("[BOARD LOG] %s::%s: %s\n",
                    msg->file_name ? msg->file_name : "?",
                    msg->fun_name ? msg->fun_name : "?", msg->msg ? msg->msg : "");
    }

    void onError(int code) {
        std::fprintf(stderr, "[DRIVER ERROR] code=%d\n", code);
    }

    // 成员顺序:ctx_ 先于 driver_(见类注释)
    CallbackCtx ctx_;
    SrDriver driver_;

    // 跨线程计数:分发线程写,主线程读 → 必须原子
    std::atomic<uint64_t> imu_count_{0};
    std::atomic<uint64_t> power_count_{0};
    std::atomic<uint64_t> battery_count_{0};
    std::atomic<uint64_t> log_count_{0};
};

} // namespace

int main(int argc, char** argv) {
    const char* port = argc > 1 ? argv[1] : "/dev/ttyUSB0";
    const uint32_t baud = argc > 2 ? std::strtoul(argv[2], nullptr, 10) : 115200;

    try {
        std::printf("== servo-robot-driver C++ example (class-based callbacks) ==\n");
        ServoRobotDriverNode node(port, baud);
        node.run();
        std::printf("== done ==\n");
        return 0;
    } catch (const std::exception& e) {
        std::fprintf(stderr, "FATAL: %s\n", e.what());
        return 1;
    }
}
