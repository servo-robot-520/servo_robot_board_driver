# servo_robot_board_driver

English | [简体中文](README.md)

**Rust host-side communication driver for ServoRobotBoard (Workspace)**

Bidirectional serial communication between a host computer and ServoRobotBoard. This repository is a Cargo workspace containing two crates — the protocol layer and the driver layer — designed for integration with ROS2 nodes, TUIs, tests, and other host-side applications.

## Workspace Structure

| Crate | Description | Docs |
|-------|-------------|------|
| [servo-robot-protocol](crates/servo-robot-protocol/README.md) | Protocol layer: frame format, data types (IMU / Power / Battery / System / Event / Log / Config / Servo, etc.), CRC checksum. `no_std` + `alloc` compatible, works on both PC and embedded platforms | [简体中文](crates/servo-robot-protocol/README.md) |
| [servo-robot-driver](crates/servo-robot-driver/README.md) | Driver layer: serial communication, `DriverCallback` callbacks, thread-safe state snapshots, auto-reconnect, mock transport, sync/async dual drivers (`Driver` / `AsyncDriver`) | [简体中文](crates/servo-robot-driver/README.md) · [English](crates/servo-robot-driver/README_en.md) |

```
┌──────────────────────────────────────────────┐
│                  User Code                     │
│        (ROS2 Node / TUI / Tests)               │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│            servo-robot-driver                 │
│    Driver / AsyncDriver / EventBus / State    │
│     Transport (Serial / Mock / Tokio / Custom)│
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│          servo-robot-protocol (no_std)        │
│      Frame / CRC / data type definitions      │
└──────────────┬───────────────────────────────┘
               │ Serial (HEAD + TYPE + LEN + PAYLOAD + CRC)
               ▼
         ServoRobotBoard (STM32)
```

## Features

- **Frame protocol**: `HEAD + TYPE + LEN + PAYLOAD + CRC` framing, CRC-16/CCITT checksum
- **Full data types**: IMU, Power, Battery, System (incl. temperature), Event, Log, Config, Servo, Command
- **`no_std` compatible**: protocol layer runs on embedded platforms (`embedded` feature)
- **Callback mechanism**: `DriverCallback` trait; board logs dispatched via `on_log`, default output through the `log` crate
- **State snapshots**: thread-safe `DriverState::snapshot()` for polling high-frequency data from TUIs / ROS2
- **Sync request-response**: `query_config_sync` / `write_config_sync` auto-wait for ACK
- **Auto-reconnect**: configurable retries with exponential backoff
- **Mock transport**: `MockTransport` simulates real data for development and testing
- **Sync/async dual drivers**: `Driver` / `AsyncDriver` (`async` feature)

## Frame Protocol

```
┌──────┬──────┬──────┬───────────────┬──────┐
│ HEAD │ TYPE │ LEN  │   PAYLOAD     │ CRC  │
│ 1B   │ 1B   │ 2B   │   0~255B      │ 2B   │
└──────┴──────┴──────┴───────────────┴──────┘

HEAD:    0xAA (fixed header)
TYPE:    Message type
LEN:     Payload length (little-endian uint16)
PAYLOAD: Data content
CRC:     CRC-16/CCITT checksum (from TYPE to end of PAYLOAD)
```

## Frame Types

> **Note**: `0x03` is reserved (old Thermal, merged into System).

| Type | Value | Direction | Description |
|------|-------|-----------|-------------|
| Imu | 0x01 | Uplink | IMU inertial measurement data |
| Power | 0x02 | Uplink | Power electrical data |
| Config | 0x04 | Uplink | Config snapshot |
| Battery | 0x05 | Uplink | Battery state |
| System | 0x06 | Uplink | System info + temperature data |
| Event | 0x07 | Uplink | Event |
| Log | 0x08 | Uplink | Log message |
| CfgWrite | 0x80 | Downlink | Write config |
| CfgQuery | 0x81 | Downlink | Query single config |
| CfgQueryAll | 0x82 | Downlink | Query all configs |
| ServoForward | 0x83 | Downlink | Forward servo command |
| FirmwareUpdate | 0x84 | Downlink | Firmware update |
| Command | 0x85 | Downlink | Generic command (Reset / Shutdown / OTA) |
| AckCfgWrite | 0xC0 | Response | Write ACK |
| AckCfgQuery | 0xC1 | Response | Single config response |
| AckCfgQueryAll | 0xC2 | Response | All configs response |
| AckServoCmd | 0xC3 | Response | Servo command response |
| AckFirmwareUpdate | 0xC4 | Response | Firmware update response |
| AckCommand | 0xC5 | Response | Command response |

## Quick Start

Add the driver crate to your `Cargo.toml`:

```toml
[dependencies]
servo-robot-driver = { path = "crates/servo-robot-driver" }
```

### Using a Real Serial Port

```rust
use servo_robot_driver::{Driver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let mut driver = Driver::new(transport);
driver.start()?;
```

### Using the Mock Transport (dev / testing, requires `mock` feature)

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

### Async Driver (requires `async` feature)

`AsyncDriver` is a thin facade over the sync `Driver`: operations run via `spawn_blocking` on the tokio blocking pool, while I/O and callbacks stay on the driver's dedicated threads.

```rust
use servo_robot_driver::{AsyncDriver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let driver = AsyncDriver::new(transport);
driver.start().await?;
```

## Feature Flags

| Crate | Feature | Description |
|-------|---------|-------------|
| servo-robot-driver | `mock` | Enable MockTransport |
| servo-robot-driver | `async` | Enable AsyncDriver (thin facade over sync Driver) |
| servo-robot-protocol | `embedded` | Embedded mode (`no_std`) |

## Detailed Docs

- [servo-robot-protocol docs](crates/servo-robot-protocol/README.md) — data type fields, bit flags, CRC, encode/decode examples
- [servo-robot-driver docs](crates/servo-robot-driver/README.md) — architecture, data flow, threading model, reconnection, callback API, logging

## License

[GPL-3.0](LICENSE)
