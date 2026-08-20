# servo_robot_board_driver

English | [简体中文](README.md)

**Rust host-side communication driver for ServoRobotBoard (Workspace)**

Bidirectional serial communication between host PC and ServoRobotBoard. This repository is a Cargo workspace containing a protocol crate and a driver crate, integrable with ROS2 nodes, TUIs, tests, and other host-side scenarios.

## Workspace Structure

| Crate | Description | Docs |
|-------|-------------|------|
| [servo-robot-protocol](crates/servo-robot-protocol/README_en.md) | Protocol layer: frame format, data types (IMU / Power / Battery / Diagnostic / DeviceInfo / Event / Log / Config / Servo), CRC. `no_std` + `alloc` compatible for both PC and embedded platforms | [中文](crates/servo-robot-protocol/README.md) · [English](crates/servo-robot-protocol/README_en.md) |
| [servo-robot-driver](crates/servo-robot-driver/README_en.md) | Driver layer: serial transport, `DriverCallback` callbacks, thread-safe state snapshots, auto-reconnect, mock transport, sync/async dual drivers (`Driver` / `AsyncDriver`) | [中文](crates/servo-robot-driver/README.md) · [English](crates/servo-robot-driver/README_en.md) |

```
┌──────────────────────────────────────────────┐
│              Host Application                │
│        (ROS2 Node / TUI / Test)              │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│            servo-robot-driver                 │
│    Driver / AsyncDriver / EventBus / State    │
│   Transport (Serial / Mock / Tokio / Custom)  │
└──────────────┬───────────────────────────────┘
               │
┌──────────────▼───────────────────────────────┐
│          servo-robot-protocol (no_std)        │
│      Frame / CRC / Data Type Definitions      │
└──────────────┬───────────────────────────────┘
               │ Serial (HEAD + TYPE + LEN + PAYLOAD + CRC)
               ▼
         ServoRobotBoard (STM32)
```

## Features

- **Frame protocol**: `HEAD + TYPE + LEN + PAYLOAD + CRC` with CRC-16/CCITT checksum
- **Request/Response model**: 9 frame types (7 uplink + Request + Response); all downlink operations use a unified Request frame, all acknowledgements use a unified Response frame
- **Complete data types**: IMU, Power, Battery, Diagnostic (runtime), DeviceInfo (static identity), Event, Log, Config, Servo
- **`no_std` compatible**: protocol layer works on embedded platforms (`embedded` feature)
- **Callback mechanism**: `DriverCallback` trait; board logs dispatched via `on_log`, default output through `log` crate
- **State snapshots**: thread-safe `DriverState::snapshot()` for TUI / ROS2 polling
- **Sync request-response**: `query_config_sync` / `write_config_sync` auto-wait for acknowledgement
- **Auto-reconnect**: configurable retry count with exponential backoff
- **Mock transport**: `MockTransport` simulates real data for development and testing
- **Sync/async dual drivers**: `Driver` / `AsyncDriver` (`async` feature)
- **C/C++ FFI**: `sr_driver_*` functions, `sr_version` version query, C/C++ header compatible

## Frame Protocol

`HEAD(0xAA) + TYPE(1B) + LEN(2B LE) + PAYLOAD(0~255B) + CRC16-CCITT(2B)`

9 frame types (7 uplink + Request + Response); all downlink operations use a unified Request frame, all responses use a unified Response frame.

See [servo-robot-protocol docs](crates/servo-robot-protocol/README_en.md#frame-types).

## Quick Start

Add the driver crate to `Cargo.toml`:

```toml
[dependencies]
servo-robot-driver = { path = "crates/servo-robot-driver" }
```

### Real Serial Port

```rust
use servo_robot_driver::{Driver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let mut driver = Driver::new(transport);
driver.start()?;
```

### Mock Transport (development / testing, requires `mock` feature)

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

### Sending Commands

```rust
use servo_robot_driver::protocol::request::RequestKind;

// fire-and-forget (no response wait)
driver.send_command(RequestKind::Reset)?;

// wait for response
let success = driver.send_command_sync(RequestKind::Reset)?;

// query device info
let response = driver.query_device_info()?;
```

### C/C++ FFI

```c
#include "servo_robot_driver.h"

// Get version
sr_version ver = sr_driver_version();
printf("Driver v%u.%u.%u\n", ver.major, ver.minor, ver.patch);

// Open serial port
char err[256];
sr_driver* d = sr_driver_open("/dev/ttyUSB0", 115200, err, sizeof(err));
sr_driver_start(d);
// ...
sr_driver_stop(d);
sr_driver_free(d);
```

### Async Driver (requires `async` feature)

`AsyncDriver` is a thin wrapper over the sync `Driver`: operations run via `spawn_blocking` in the tokio blocking pool; I/O and callbacks remain on the driver's dedicated thread.

```rust
use servo_robot_driver::{AsyncDriver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let driver = AsyncDriver::new(transport);
driver.start().await?;
```

## Feature Flags

| Crate | Feature | Description |
|-------|---------|-------------|
| servo-robot-driver | `mock` | Enable MockTransport simulated transport |
| servo-robot-driver | `async` | Enable AsyncDriver (thin sync Driver facade) |
| servo-robot-driver | `ffi` | Enable C FFI wrapper |
| servo-robot-protocol | `embedded` | Embedded mode (`no_std`) |

## Detailed Documentation

- [servo-robot-protocol docs](crates/servo-robot-protocol/README_en.md) — data type fields, bitflags, CRC, codec examples
- [servo-robot-driver docs](crates/servo-robot-driver/README_en.md) — architecture, data flow, threading model, reconnect, callback API, logging

## License

[GPL-3.0](LICENSE)
