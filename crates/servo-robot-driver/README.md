# servo-robot-driver

[English](README_en.md) | 简体中文

上位机通过串口与 ServoRobotBoard 的通信驱动，用于两者之间的双向数据传输。

## 特性

- 帧协议解析（HEAD + TYPE + LEN + PAYLOAD + CRC）
- `DriverCallback` trait 回调机制
- 同步请求-响应 API（自动等待应答）
- 线程安全的状态快照 API
- 自动重连（可配置重试次数和退避策略）
- 模拟传输层（用于开发和测试）
- 同步/异步双驱动（`Driver` / `AsyncDriver`）
- 板级日志通过 `DriverCallback::on_log` 分发，默认通过 `log` 库输出
- C/C++ FFI（`sr_driver_*` API + `SrCallbacks` 回调表）

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         用户代码                                  │
│  (ROS2 Node / TUI / 测试 / C/C++ 通过 FFI)                      │
├─────────────────────────────────────────────────────────────────┤
│                   Driver / AsyncDriver                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │   EventBus   │  │ DriverState  │  │ 同步等待 (wait_for_*) │  │
│  │ (事件分发)    │  │ (状态快照)    │  │ (recv_timeout)       │  │
│  └──────┬───────┘  └──────────────┘  └──────────────────────┘  │
│         │                                                        │
│  ┌──────┴───────┐                                                │
│  │ driver_common │ (帧构建、帧解码)                                │
│  └──────┬───────┘                                                │
├─────────┼────────────────────────────────────────────────────────┤
│         ▼                                                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     Transport (trait)                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────────────────┐  │   │
│  │  │ Serial   │  │  Mock    │  │  Custom (扩展)          │  │   │
│  │  │ (串口)   │  │ (模拟)   │  │                        │  │   │
│  │  └──────────┘  └──────────┘  └────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│                       Protocol 层                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │  Frame   │  │  IMU     │  │  Power   │  │ Battery  │        │
│  │ (帧解析) │  │ (惯性)   │  │ (电源)   │  │ (电池)   │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Config   │  │Diagnostic│  │DeviceInfo│  │  Event   │        │
│  │ (配置)   │  │ (诊断)   │  │ (设备)   │  │ (事件)   │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                       │
│  │   Log    │  │ Request  │  │ Response │                       │
│  │ (日志)   │  │ (请求)   │  │ (应答)   │                       │
│  └──────────┘  └──────────┘  └──────────┘                       │
└──────────────────────────────────────────────────────────────────┘
```

### 数据流

```
STM32 ──串口──→ Transport.read_frame() ──→ decode_and_dispatch()
                                              │
                              ┌───────────────┼───────────────┐
                              ▼               ▼               ▼
                      state.update_*()   return Event    Response 事件
                              │               │               │
                              ▼               ▼               ▼
                      DriverState       bounded channel   Response 通道
                      (最新值快照)       (1024)           (同步等待)
                              │               │
                              ▼               ▼
                      state.snapshot()   分发线程 → dispatch(callbacks)
                      (TUI/ROS2 轮询)
```

- **高频周期数据**（IMU/Power/Battery 等）：通过 `state.snapshot()` 轮询获取最新值
- **低频触发数据**（Log/Event 等）：通过 `DriverCallback` 回调逐条处理

### 核心组件

#### 1. Transport Trait

```rust
pub trait Transport: Send + 'static {
    fn read_frame(&mut self) -> Result<Vec<u8>, DriverError>;
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), DriverError>;
    fn close(&mut self) -> Result<(), DriverError>;
}
```

- **SerialTransport**: 使用 `serialport` crate 的串口实现
- **MockTransport**: 模拟真实数据，用于开发和测试
- **AsyncDriver**: 同步 `Driver` 的薄异步门面（`spawn_blocking` 包装，feature gated）

#### 2. Driver

```rust
pub struct Driver {
    transport: Arc<Mutex<Option<Box<dyn Transport>>>>,
    bus: Arc<EventBus>,
    state: Arc<DriverState>,
    // ...
}
```

- **读取线程**: 独立线程持续读取帧、更新状态、发送事件到通道
- **分发线程**: 独立线程从通道消费事件，触发回调（不阻塞读取）
- **同步写入**: 通过 `Mutex` 保护的传输层写入
- **状态快照**: `DriverState` 提供线程安全的数据访问

#### 3. EventBus

```rust
pub struct EventBus {
    tx: Sender<DriverEvent>,           // bounded(1024)，主事件通道
    response_tx: Sender<DriverEvent>,  // bounded(64)，Response 专用通道
    callbacks: Arc<Mutex<Vec<Box<dyn DriverCallback>>>>,
}
```

- 双通道设计：主事件通道（bounded，自动背压）+ Response 专用通道（bounded，无等待者时丢弃）
- 回调在独立分发线程上触发，不阻塞读取线程

#### 4. Protocol 层

每个数据类型实现：
- `ToPayload`: 序列化为字节
- `FromPayload`: 从字节反序列化
- `from_bytes()` / `to_bytes()`: 底层字节操作

### 帧协议

帧格式、帧类型、数据类型的完整定义见 [servo-robot-protocol 文档](../servo-robot-protocol/README.md)。

驱动层负责帧的编解码、传输和状态管理，不重新定义协议类型。

### 线程模型

```
┌─────────────────────────────────────────────────┐
│                   用户线程                        │
│  driver.send_request() / driver.query_config()   │
│         │                                        │
│         ▼                                        │
│  ┌──────────────┐                                │
│  │ Mutex<Transport> │ ◄── 读写互斥               │
│  └──────┬───────┘                                │
│         │                                        │
├─────────┼────────────────────────────────────────┤
│         ▼                                        │
│  ┌──────────────┐                                │
│  │  读取线程     │  loop {                        │
│  │              │    transport.read_frame()       │
│  │              │    decode → state.update        │
│  │              │    → channel.send()             │
│  │              │  }                              │
│  └──────┬───────┘                                │
│         │ bounded channel (1024)                  │
│         ▼                                        │
│  ┌──────────────┐                                │
│  │  分发线程     │  loop {                        │
│  │              │    channel.recv()               │
│  │              │    → dispatch(callbacks)        │
│  │              │  }                              │
│  └──────────────┘                                │
│                                                  │
│  ┌──────────────┐                                │
│  │ Response 通道 │  同步等待 recv_timeout()        │
│  │ (bounded 64) │                                │
│  └──────────────┘                                │
└─────────────────────────────────────────────────┘
```

### 重连机制

```
连接断开
    │
    ▼
检查重连配置
    │
    ├── max_retries = 0 → 不重连
    │
    ▼
等待 retry_interval * backoff_multiplier^retry_count
    │
    ▼
调用 TransportFactory.create()
    │
    ├── 成功 → 重置计数器，继续
    │
    └── 失败 → retry_count++，重试
```

## 快速开始

### 添加依赖

```toml
[dependencies]
servo-robot-driver = { path = "../servo-robot-driver" }

# 启用模拟传输层（用于开发和测试）
servo-robot-driver = { path = "../servo-robot-driver", features = ["mock"] }

# 启用异步支持
servo-robot-driver = { path = "../servo-robot-driver", features = ["async"] }

# 启用 C/C++ FFI
servo-robot-driver = { path = "../servo-robot-driver", features = ["ffi"] }
```

### 使用 DriverCallback（推荐）

```rust
use servo_robot_driver::{Driver, MockTransport, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::battery_state::BatteryState;
use servo_robot_driver::protocol::device_info::DeviceInfo;
use servo_robot_driver::protocol::diagnostic::Diagnostic;
use servo_robot_driver::protocol::response::Response;
use servo_robot_driver::protocol::log::LogMessage;

struct MyCallback;

impl DriverCallback for MyCallback {
    fn on_imu_data(&mut self, data: &ImuData) {
        println!("IMU: roll={:.1}° pitch={:.1}° yaw={:.1}°",
            data.roll, data.pitch, data.yaw);
    }

    fn on_battery_state(&mut self, state: &BatteryState) {
        println!("Battery: {:.1}%", state.percentage);
    }

    fn on_diagnostic(&mut self, diag: &Diagnostic) {
        println!("CPU: {}%  uptime: {}s  MCU temp: {:.1}°C",
            diag.cpu_usage_percent, diag.uptime_s, diag.temp_mcu as f32 / 10.0);
    }

    fn on_device_info(&mut self, info: &DeviceInfo) {
        println!("Device: id={:#06x} FW={} RAM={}KB",
            info.device_id, info.firmware_version, info.ram_kb);
    }

    fn on_ack_cfg_query(&mut self, config: &Config) {
        println!("Config query response: {:?}", config);
    }

    // 覆盖默认日志处理
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        println!("[Board][{}] {}: {}", ts, log_msg.fun_name, log_msg.msg);
    }
}

let mock = MockTransport::new();
let mut driver = Driver::new(mock);
driver.register_callback(MyCallback);
driver.start()?;
```

### 使用状态快照（TUI 场景）

```rust
use servo_robot_driver::{Driver, MockTransport};

let mock = MockTransport::new();
let mut driver = Driver::new(mock);
driver.start()?;

let state = driver.state();
let snap = state.snapshot();
if let Some(imu) = &snap.imu {
    println!("Roll: {:.1}°", imu.roll);
}
if let Some(diag) = &snap.diagnostic {
    println!("CPU: {}%  uptime: {}s", diag.cpu_usage_percent, diag.uptime_s);
}
```

### 使用真实串口

```rust
use servo_robot_driver::{Driver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let mut driver = Driver::new(transport);
driver.start()?;
```

### 自动重连

```rust
use servo_robot_driver::{Driver, SerialTransport, FnTransportFactory};
// ...
let factory = FnTransportFactory::new(|| SerialTransport::open("/dev/ttyUSB0", 115200));
let mut driver = Driver::with_reconnect(factory, ReconnectConfig::default());
driver.start()?;
```

## C/C++ 集成（FFI）

启用 `ffi` feature 可把驱动编译为共享库（.so），供 C/C++ 直接调用。

### 编译 .so

```bash
# 仓库根目录;必须带 --features ffi,否则 .so 内没有可调用的 sr_* 符号
cargo build --release --features ffi
# 产物: target/release/libservo_robot_driver.so
nm -D --defined-only target/release/libservo_robot_driver.so | grep ' T sr_driver_'
```

头文件: `include/servo_robot_driver.h`（唯一接口契约,含结构体/枚举/函数声明/线程红线）。

### 一键打包（推荐）

```bash
crates/servo-robot-driver/ffi/package_ffi.sh [输出目录]
# 默认输出到仓库根 ffi-dist/:
#   ffi-dist/
#   ├── CMakeLists.txt            # 已配置好导入 .so + 示例构建
#   ├── README.md
#   ├── include/servo_robot_driver.h
#   ├── lib/libservo_robot_driver.so
#   └── examples/cpp_example.cpp, c_example.c
```

整个 `ffi-dist/` 复制进 C/C++ 项目即可:

```bash
cd ffi-dist
cmake -S . -B build && cmake --build build
./build/cpp_example /dev/ttyUSB0 115200
```

### 手动集成

```c
#include "servo_robot_driver.h"   // extern "C" 已包裹,C++ 直接 include

sr_version ver = sr_driver_version();
printf("driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

sr_driver* d = sr_driver_open("/dev/ttyUSB0", 115200, err, sizeof err);
sr_driver_start(d);

sr_board_config cfg;
sr_driver_query_all_configs(d, &cfg);        // 同步,阻塞 ≤1s
sr_driver_write_config_sync(d, (sr_config){.typ = SR_CONFIG_SERVO_BAUD_RATE, .value = 1000000}, &ok);

sr_driver_stop(d);
sr_driver_free(d);                            // 必须最后调用
```

```bash
gcc my_prog.c -I <include 目录> -L target/release -lservo_robot_driver \
    -Wl,-rpath,$PWD/target/release -o my_prog   # C++ 用 g++ 同理
```

运行时三选一: 编译时 `-Wl,-rpath` / `LD_LIBRARY_PATH` / 安装到 `/usr/local/lib` + `ldconfig`

### 示例

- **C**: [`ffi/c_example.c`](crates/servo-robot-driver/ffi/c_example.c) — 自由函数回调，完整驱动生命周期
- **C++**: [`ffi/cpp_example.cpp`](crates/servo-robot-driver/ffi/cpp_example.cpp) — 类内回调（thunk 桥），ROS2 节点风格
- **冒烟测试**: [`ffi/smoke_test.c`](crates/servo-robot-driver/ffi/smoke_test.c) — ABI 边界验证，非业务示例

### FFI 函数

| 函数 | 说明 |
|------|------|
| `sr_driver_version()` | 获取驱动版本号（`sr_version`）|
| `sr_driver_open()` | 打开串口，返回句柄（不自动重连）|
| `sr_driver_connect()` | 重新连接指定串口（上层实现重连）|
| `sr_driver_free()` | 释放句柄（自动 stop + join）|
| `sr_driver_start()` / `sr_driver_stop()` | 启动/停止驱动 |
| `sr_driver_write_config()` / `sr_driver_write_config_sync()` | 写入配置（异步/同步）|
| `sr_driver_query_config()` / `sr_driver_query_all_configs()` | 查询配置 |
| `sr_driver_forward_servo()` / `sr_driver_forward_servo_sync()` | 舵机命令透传 |
| `sr_driver_send_command()` / `sr_driver_send_command_sync()` | 板级命令（Reset/Shutdown/Ota）|
| `sr_driver_firmware_update()` / `sr_driver_firmware_update_sync()` | 固件更新 |
| `sr_driver_set_callbacks()` | 设置/替换回调表 |
| `sr_driver_last_error()` | 获取最近错误描述 |

### FFI 回调

`sr_callbacks` 中的可选回调（函数指针，NULL 表示不注册）：

| 回调 | 说明 |
|------|------|
| `on_imu_data` | IMU 数据（100Hz）|
| `on_power_data` | 电源数据（20Hz）|
| `on_battery_state` | 电池状态（10Hz）|
| `on_config_snapshot` | 配置快照 |
| `on_board_event` | 板级事件 |
| `on_diagnostic` | 运行时诊断（1Hz 推送）|
| `on_ack_device_info` | 设备信息应答（DeviceInfo 查询后触发）|
| `on_ack_cfg_write` | 配置写入确认 |
| `on_ack_cfg_query` | 单个配置查询响应 |
| `on_ack_cfg_query_all` | 所有配置查询响应 |
| `on_ack_servo_cmd` | 舵机命令响应 |
| `on_ack_command` | 系统命令确认（Reset/Shutdown/Ota）|
| `on_ack_firmware_update` | 固件更新确认 |
| `on_log` | 板级日志 |
| `on_error` | 错误通知 |

### 线程安全红线

- 回调在驱动**分发线程**上触发，回调内**禁止**调用任何 `sr_driver_*`（尤其 `sr_driver_free`，会自死锁）
- 回调参数指针仅在回调执行期间有效，不要跨调用保存
- 禁止在其他线程仍使用句柄时调用 `sr_driver_free`（use-after-free）
- 同步函数阻塞 ≤1s（驱动默认超时）
