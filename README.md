# servo_robot_board_driver

[English](README_en.md) | 简体中文

**ServoRobotBoard 的 Rust 上位机通信驱动（Workspace）**

通过串口实现上位机与 ServoRobotBoard 之间的双向通信。本仓库是一个 Cargo workspace，包含协议层与驱动层两个 crate，可与 ROS2 Node / TUI / 测试等多种上位机场景集成。

## 工作区结构

| Crate | 说明 | 文档 |
|-------|------|------|
| [servo-robot-protocol](crates/servo-robot-protocol/README.md) | 协议层：帧格式、数据类型（IMU / Power / Battery / System / Event / Log / Config / Servo 等）、CRC 校验。`no_std` + `alloc` 兼容，同时支持 PC 与嵌入式平台 | [中文](crates/servo-robot-protocol/README.md) · [English](crates/servo-robot-protocol/README_en.md) |
| [servo-robot-driver](crates/servo-robot-driver/README.md) | 驱动层：串口通信、`DriverCallback` 回调、线程安全的状态快照、自动重连、模拟传输层、同步/异步双驱动（`Driver` / `AsyncDriver`） | [中文](crates/servo-robot-driver/README.md) · [English](crates/servo-robot-driver/README_en.md) |

```
┌──────────────────────────────────────────────┐
│                 上位机用户代码                   │
│        (ROS2 Node / TUI / 测试)               │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│            servo-robot-driver                 │
│    Driver / AsyncDriver / EventBus / State    │
│     Transport (Serial / Mock / Tokio / 自定义) │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│          servo-robot-protocol (no_std)        │
│      Frame / CRC / 各类数据类型定义            │
└──────────────┬───────────────────────────────┘
               │ 串口 (HEAD + TYPE + LEN + PAYLOAD + CRC)
               ▼
         ServoRobotBoard (STM32)
```

## 特性

- **帧协议**：`HEAD + TYPE + LEN + PAYLOAD + CRC` 帧格式，CRC-16/CCITT 校验
- **完整数据类型**：IMU、Power、Battery、System（含温度）、Event、Log、Config、Servo、Command
- **`no_std` 兼容**：协议层可在嵌入式平台上使用（`embedded` feature）
- **回调机制**：`DriverCallback` trait，板级日志经 `on_log` 分发，默认通过 `log` crate 输出
- **状态快照**：线程安全的 `DriverState::snapshot()`，适合 TUI / ROS2 轮询高频数据
- **同步请求-响应**：`query_config_sync` / `write_config_sync` 自动等待应答
- **自动重连**：可配置重试次数与指数退避策略
- **模拟传输层**：`MockTransport` 模拟真实数据，便于开发与测试
- **同步/异步双驱动**：`Driver` / `AsyncDriver`（`async` feature）

## 帧协议

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

## 帧类型

> **注意**：`0x03` 为保留值（原 Thermal，已合并进 System）。

| 类型 | 值 | 方向 | 说明 |
|------|-----|------|------|
| Imu | 0x01 | 上行 | IMU 惯性测量数据 |
| Power | 0x02 | 上行 | 电源电气数据 |
| Config | 0x04 | 上行 | 配置快照 |
| Battery | 0x05 | 上行 | 电池状态 |
| System | 0x06 | 上行 | 系统信息 + 温度数据 |
| Event | 0x07 | 上行 | 事件 |
| Log | 0x08 | 上行 | 日志消息 |
| CfgWrite | 0x80 | 下行 | 写入配置 |
| CfgQuery | 0x81 | 下行 | 查询单个配置 |
| CfgQueryAll | 0x82 | 下行 | 查询所有配置 |
| ServoForward | 0x83 | 下行 | 转发舵机命令 |
| FirmwareUpdate | 0x84 | 下行 | 固件升级 |
| Command | 0x85 | 下行 | 通用命令（复位 / 关机 / OTA）|
| AckCfgWrite | 0xC0 | 应答 | 写入确认 |
| AckCfgQuery | 0xC1 | 应答 | 单个配置响应 |
| AckCfgQueryAll | 0xC2 | 应答 | 所有配置响应 |
| AckServoCmd | 0xC3 | 应答 | 舵机命令响应 |
| AckFirmwareUpdate | 0xC4 | 应答 | 固件升级响应 |
| AckCommand | 0xC5 | 应答 | 命令响应 |

## 快速开始

在 `Cargo.toml` 中引用驱动 crate：

```toml
[dependencies]
servo-robot-driver = { path = "crates/servo-robot-driver" }
```

### 使用真实串口

```rust
use servo_robot_driver::{Driver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let mut driver = Driver::new(transport);
driver.start()?;
```

### 使用模拟传输层（开发 / 测试，需 `mock` feature）

```rust
use servo_robot_driver::{Driver, MockTransport, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;

struct MyCallback;

impl DriverCallback for MyCallback {
    fn on_imu_data(&mut self, data: &ImuData) {
        println!("IMU: roll={:.1}° pitch={:.1}° yaw={:.1}°",
            data.roll, data.pitch, data.yaw);
    }
}

let mock = MockTransport::new();
let mut driver = Driver::new(mock);
driver.register_callback(MyCallback);
driver.start()?;
```

### 异步驱动（需 `async` feature）

`AsyncDriver` 是同步 `Driver` 的薄封装：操作经 `spawn_blocking` 在 tokio 阻塞池执行，I/O 与回调仍在驱动专用线程上。

```rust
use servo_robot_driver::{AsyncDriver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let driver = AsyncDriver::new(transport);
driver.start().await?;
```

## Feature Flags

| Crate | Feature | 说明 |
|-------|---------|------|
| servo-robot-driver | `mock` | 启用 MockTransport 模拟传输层 |
| servo-robot-driver | `async` | 启用 AsyncDriver（同步 Driver 的薄门面） |
| servo-robot-protocol | `embedded` | 嵌入式模式（`no_std`）|

## 详细文档

- [servo-robot-protocol 协议层文档](crates/servo-robot-protocol/README.md) — 数据类型字段、位标志、CRC、编解码示例
- [servo-robot-driver 驱动层文档](crates/servo-robot-driver/README.md) — 架构设计、数据流、线程模型、重连机制、回调 API、日志系统

## 许可证

[GPL-3.0](LICENSE)
