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

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         用户代码                                  │
│  (ROS2 Node / TUI / 测试)                                        │
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
│  │ Config  │  │  System  │  │  Event   │  │   Log    │        │
│  │ (配置)   │  │ (系统)   │  │ (事件)   │  │ (日志)   │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### 数据流

```
STM32 ──串口──→ Transport.read_frame() ──→ decode_and_dispatch()
                                              │
                              ┌───────────────┼───────────────┐
                              ▼               ▼               ▼
                      state.update_*()   return Event    ACK 事件
                              │               │               │
                              ▼               ▼               ▼
                      DriverState       bounded channel   ACK 通道
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
    tx: Sender<DriverEvent>,       // bounded(1024)，主事件通道
    ack_tx: Sender<DriverEvent>,   // unbounded，ACK 专用通道
    callbacks: Arc<Mutex<Vec<Box<dyn DriverCallback>>>>,
}
```

- 双通道设计：主事件通道（bounded，自动背压）+ ACK 专用通道（unbounded）
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

| 类型 | 值 | 方向 | 说明 |
|------|-----|------|------|
| Imu | 0x01 | 上行 | IMU 数据 |
| Power | 0x02 | 上行 | 电源数据 |
| Thermal | 0x03 | 上行 | 温度数据 |
| Config | 0x04 | 上行 | 配置快照 |
| Battery | 0x05 | 上行 | 电池状态 |
| System | 0x06 | 上行 | 系统信息 |
| Event | 0x07 | 上行 | 事件 |
| Log | 0x08 | 上行 | 日志消息 |
| CfgWrite | 0x80 | 下行 | 写入配置 |
| CfgQuery | 0x81 | 下行 | 查询配置 |
| CfgQueryAll | 0x82 | 下行 | 查询所有配置 |
| AckCfgWrite | 0xC0 | 应答 | 写入确认 |
| AckCfgQuery | 0xC1 | 应答 | 配置响应 |
| AckCfgQueryAll | 0xC2 | 应答 | 所有配置 |

### 线程模型

```
┌─────────────────────────────────────────────────┐
│                   用户线程                        │
│  driver.write_config() / driver.query_config()   │
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
│  │  ACK 通道     │  同步等待 recv_timeout()        │
│  │ (unbounded)  │                                │
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
```

### 使用 DriverCallback（推荐）

```rust
use servo_robot_driver::{Driver, MockTransport, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::battery_state::BatteryState;
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
use servo_robot_driver::reconnect::ReconnectConfig;
use std::time::Duration;

let factory = FnTransportFactory::new(|| {
    SerialTransport::open("/dev/ttyUSB0", 115200).map(|t| Box::new(t) as _)
});

let config = ReconnectConfig::new(5)
    .with_retry_interval(Duration::from_secs(1))
    .with_backoff_multiplier(2.0)
    .with_max_retry_interval(Duration::from_secs(30));

let mut driver = Driver::new_with_reconnect(factory, config);
driver.start()?;
```

## MockTransport API

```rust
use servo_robot_driver::MockTransport;

let mut mock = MockTransport::new();

// 设置初始状态
mock.set_battery_soc(80.0);
mock.set_initial_attitude(10.0, 5.0, 0.0);
mock.set_charging(true);

// 模拟断开连接（用于测试重连）
mock.set_auto_disconnect(1000);

// 手动控制连接
mock.disconnect();
mock.reconnect();

// 获取写入的帧（用于验证命令）
let written = mock.written_frames();
```

### 模拟数据特性

| 数据类型 | 更新频率 | 模拟特性 |
|---------|---------|---------|
| IMU | 100Hz | 姿态漂移、传感器噪声、重力分量 |
| Power | 20Hz | 电池电压变化、电流波动 |
| Thermal | 5Hz | 温度上升趋势、随机波动 |
| Battery | 10Hz | 电量消耗、充电状态 |
| System | 1Hz | 运行时间、CPU 使用率 |
| Event | 1Hz | 充电状态、风扇状态、保护标志 |

## 回调机制

### DriverCallback trait

```rust
use servo_robot_driver::{Driver, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::battery_state::BatteryState;
use servo_robot_driver::protocol::config::{BoardConfigSnapshot, Config};
use servo_robot_driver::protocol::log::LogMessage;

struct MyCallback {
    imu_count: u64,
}

impl DriverCallback for MyCallback {
    fn on_imu_data(&mut self, data: &ImuData) {
        self.imu_count += 1;
        println!("IMU #{}: roll={:.1}", self.imu_count, data.roll);
    }

    fn on_battery_state(&mut self, state: &BatteryState) {
        println!("Battery: {:.1}%", state.percentage);
    }

    fn on_ack_cfg_write(&mut self, success: bool) {
        println!("Config write ack: {}", success);
    }

    fn on_ack_cfg_query(&mut self, config: &Config) {
        println!("Config: {}", config);
    }

    // 板级日志回调（默认实现通过 log 库输出）
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        println!("[Board][{}] {}: {}", ts, log_msg.fun_name, log_msg.msg);
    }
}

driver.register_callback(MyCallback { imu_count: 0 });
```

> **注意**：回调在独立的分发线程上触发，不会阻塞读取线程。

### 回调方法列表

| 方法 | 触发时机 | 默认行为 |
|------|---------|---------|
| `on_imu_data` | IMU 数据帧 (100Hz) | 空 |
| `on_power_data` | 电源数据帧 (20Hz) | 空 |
| `on_thermal_data` | 温度数据帧 (5Hz) | 空 |
| `on_battery_state` | 电池状态帧 (10Hz) | 空 |
| `on_system_info` | 系统信息帧 (1Hz) | 空 |
| `on_config_snapshot` | 配置快照帧 | 空 |
| `on_board_event` | 事件帧 (1Hz) | 空 |
| `on_log(ts, log_msg)` | 板级日志帧 | 通过 `log` 库输出，带 `[ServoRobotBoard]` 前缀和时间戳 |
| `on_ack_cfg_write` | 配置写入确认 | 空 |
| `on_ack_cfg_query` | 单个配置查询响应 | 空 |
| `on_ack_cfg_query_all` | 所有配置查询响应 | 空 |
| `on_error` | 驱动错误 | 空 |

## 日志系统

### 板级日志

STM32 上行的 `Log` 帧通过 `DriverCallback::on_log` 分发。默认实现通过 `log` 库输出，带 `[ServoRobotBoard]` 前缀和时间戳：

```
[ServoRobotBoard] [14:30:05] config.c::handle_query: queried PowerServoCurrentLimit
[ServoRobotBoard] [14:30:06] imu.c::read_gyro: sensor timeout
[ServoRobotBoard] [14:30:07] main.c::app_init: boot complete
```

日志级别映射：`LogLevel::Error` → `log::error!`，`Warn` → `log::warn!`，`Info` → `log::info!`，`Debug` → `log::debug!`

### 自定义日志处理

覆盖 `on_log` 方法可自定义日志行为（如显示在 TUI 面板、发布到 ROS2 话题）：

```rust
impl DriverCallback for MyCallback {
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        // 自定义处理，不再输出到 log
        self.log_buffer.push(format!("[{}] {}::{}: {}",
            ts, log_msg.file_name, log_msg.fun_name, log_msg.msg));
    }
}
```

### 使用 log 后端

接入任意 `log` 后端即可看到默认的板级日志输出：

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

## 命令和配置

```rust
use servo_robot_driver::protocol::config::{Config, ConfigType};

// 异步（不等待应答）
driver.query_config(ConfigType::PowerServoCurrentLimit)?;
driver.write_config(Config::PowerServoCurrentLimit(5.0))?;

// 同步（等待应答）
let config = driver.query_config_sync(ConfigType::PowerServoCurrentLimit)?;
let all_config = driver.query_all_configs_sync()?;
let success = driver.write_config_sync(Config::PowerServoCurrentLimit(5.0))?;
```

## Feature Flags

| Feature | 依赖 | 说明 |
|---------|------|------|
| `mock` | `rand` | 启用 MockTransport 模拟传输层 |
| `async` | `tokio` | 启用 AsyncDriver（同步 Driver 的薄门面） |

