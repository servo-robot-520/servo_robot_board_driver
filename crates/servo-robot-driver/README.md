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

### 帧格式

```
┌──────┬──────┬──────┬───────────────┬──────┐
│ HEAD │ TYPE │ LEN  │   PAYLOAD     │ CRC  │
│ 1B   │ 1B   │ 2B   │   0~255B      │ 2B   │
└──────┴──────┴──────┴───────────────┴──────┘

HEAD:    0xAA (固定帧头)
TYPE:    消息类型
LEN:     payload 长度 (小端 uint16)
PAYLOAD: 数据内容
CRC:     CRC-16/CCITT 校验 (从 TYPE 到 PAYLOAD 末尾)
```

### 帧类型

上行数据（STM32 → PC）：

| 类型 | 值 | 说明 |
|------|-----|------|
| Imu | 0x01 | IMU 惯性测量数据 |
| Power | 0x02 | 电源电气数据 |
| Config | 0x04 | 配置快照 |
| Battery | 0x05 | 电池状态 |
| Diagnostic | 0x06 | 运行时诊断（CPU/内存/温度等）|
| Event | 0x07 | 板级事件 |
| Log | 0x08 | 日志消息 |

下行请求（PC → STM32）：

| 类型 | 值 | 说明 |
|------|-----|------|
| Request | 0x80 | 统一请求帧（`RequestKind` 首字节区分）|

应答（STM32 → PC）：

| 类型 | 值 | 说明 |
|------|-----|------|
| Response | 0xC0 | 统一应答帧（`request_kind` + `success` + `data`）|

#### RequestKind

| 变体 | 值 | 说明 | 需要应答 |
|------|-----|------|---------|
| Reset | 0x01 | 重启 MCU | 否 |
| Shutdown | 0x02 | 关机 | 否 |
| Ota | 0x03 | 触发 OTA 更新 | 否 |
| ConfigWrite | 0x10 | 写入单个配置 | 是 |
| ConfigQuery | 0x11 | 查询单个配置 | 是 |
| ConfigQueryAll | 0x12 | 查询所有配置 | 是 |
| DeviceInfo | 0x13 | 查询设备标识与内存布局 | 是 |
| ServoForward | 0x20 | 转发舵机命令 | 是 |
| FirmwareUpdate | 0x21 | 固件更新数据块 | 是 |

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

    fn on_device_info(&mut self, info: &DeviceInfo) {
        println!("Device: id={:#06x} firmware={}", info.device_id, info.firmware_version);
    }

    fn on_diagnostic(&mut self, diag: &Diagnostic) {
        println!("CPU: {}%  uptime: {}s  MCU temp: {:.1}°C",
            diag.cpu_usage_percent, diag.uptime_s, diag.temp_mcu as f32 / 10.0);
    }

    fn on_response(&mut self, resp: &Response) {
        println!("Response: {:?} success={} data={:?}",
            resp.request_kind, resp.success, resp.data);
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

### C/C++ FFI 快速开始

启用 `ffi` feature 后，驱动以 C ABI 暴露 `sr_driver_*` API，头文件位于 `include/servo_robot_driver.h`。

```c
#include "servo_robot_driver.h"
#include <stdio.h>

// 应答回调
static void on_response(void *ud, const SrResponse *resp) {
    printf("Response: kind=%d success=%d\n", resp->request_kind, resp->success);
}

// IMU 回调
static void on_imu(void *ud, const SrImu *imu) {
    printf("IMU: roll=%.1f pitch=%.1f yaw=%.1f\n", imu->roll, imu->pitch, imu->yaw);
}

// 诊断回调（1Hz 推送）
static void on_diagnostic(void *ud, const SrDiagnostic *diag) {
    printf("CPU: %u%% uptime: %us MCU temp: %.1f°C\n",
           diag->cpu_usage_percent, diag->uptime_s, (float)diag->temp_mcu / 10.0f);
}

int main(void) {
    // 查询驱动版本
    SrVersion ver = sr_driver_version();
    printf("Driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

    // 打开串口（支持自动重连）
    char err[256];
    sr_driver *drv = sr_driver_open_reconnect(
        "/dev/ttyUSB0", 115200,
        5,      // max_retries
        1000,   // retry_interval_ms
        2.0f,   // backoff_multiplier
        10000,  // max_retry_interval_ms
        err, sizeof(err)
    );
    if (!drv) {
        fprintf(stderr, "Open failed: %s\n", err);
        return 1;
    }

    // 注册回调
    sr_callbacks cbs = {
        .on_imu           = on_imu,
        .on_diagnostic    = on_diagnostic,
        .on_response      = on_response,
    };
    sr_driver_set_callbacks(drv, &cbs);

    // 启动驱动
    sr_driver_start(drv);

    // ... 业务逻辑 ...

    // 释放资源
    sr_driver_free(drv);
    return 0;
}
```

#### FFI 函数一览

| 函数 | 说明 |
|------|------|
| `sr_driver_version()` | 获取驱动版本号（`SrVersion`）|
| `sr_driver_open()` | 打开串口，返回句柄 |
| `sr_driver_open_reconnect()` | 打开串口并启用自动重连 |
| `sr_driver_free()` | 释放句柄（自动 stop + join）|
| `sr_driver_start()` | 启动驱动（读取线程 + 分发线程）|
| `sr_driver_stop()` | 停止驱动 |
| `sr_driver_write_config()` | 写入单个配置 |
| `sr_driver_write_config_sync()` | 写入配置（同步等待应答）|
| `sr_driver_query_config()` | 查询单个配置 |
| `sr_driver_query_all_configs()` | 查询所有配置 |
| `sr_driver_forward_servo()` | 转发舵机命令 |
| `sr_driver_forward_servo_sync()` | 转发舵机命令（同步等待应答）|
| `sr_driver_send_command()` | 发送板级命令（Reset/Shutdown/Ota）|
| `sr_driver_send_command_sync()` | 发送命令（同步等待确认）|
| `sr_driver_firmware_update()` | 固件更新数据块 |
| `sr_driver_firmware_update_sync()` | 固件更新（同步等待确认）|
| `sr_driver_set_callbacks()` | 设置/替换回调表 |
| `sr_driver_last_error()` | 获取最近一次错误描述 |

#### FFI 回调表

`sr_callbacks` 结构体包含以下可选回调（函数指针，NULL 表示不注册）：

| 回调 | 说明 |
|------|------|
| `on_imu` | IMU 数据（100Hz）|
| `on_power` | 电源数据（20Hz）|
| `on_battery` | 电池状态（10Hz）|
| `on_config` | 配置快照 |
| `on_board_event` | 板级事件 |
| `on_device_info` | 设备标识与内存布局（静态信息，查询后推送）|
| `on_diagnostic` | 运行时诊断（1Hz 推送）|
| `on_log` | 板级日志 |
| `on_response` | 统一应答（替代原多个 `on_ack_*` 回调）|
| `on_error` | 错误通知 |
