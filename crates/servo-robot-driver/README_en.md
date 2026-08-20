# servo-robot-driver

English | [简体中文](README.md)

The communication driver between the host computer and ServoRobotBoard is used for bidirectional data transmission between the two through the serial port.

## Features

- Frame protocol parsing (HEAD + TYPE + LEN + PAYLOAD + CRC)
- `DriverCallback` trait callback mechanism
- Synchronous request-response API (auto-wait for response)
- Thread-safe state snapshot API
- Auto-reconnection (configurable retries and backoff)
- Mock transport layer (for development and testing)
- Sync/async dual drivers (`Driver` / `AsyncDriver`)
- Board log dispatch via `DriverCallback::on_log`, default output via `log` crate

## Architecture

### Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Code                                │
│  (ROS2 Node / TUI / Tests)                                       │
├─────────────────────────────────────────────────────────────────┤
│                   Driver / AsyncDriver                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │   EventBus   │  │ DriverState  │  │ Sync Wait (wait_for_*)│  │
│  │ (Dispatch)   │  │ (Snapshot)   │  │ (recv_timeout)       │   │
│  └──────┬───────┘  └──────────────┘  └──────────────────────┘   │
│         │                                                        │
│  ┌──────┴───────┐                                                │
│  │ driver_common │ (Frame encode / decode)                       │
│  └──────┬───────┘                                                │
├─────────┼────────────────────────────────────────────────────────┤
│         ▼                                                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                     Transport (trait)                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌────────────────────────┐  │   │
│  │  │ Serial   │  │  Mock    │  │  Custom (extension)    │  │   │
│  │  └──────────┘  └──────────┘  └────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────┤
│                       Protocol Layer                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │  Frame   │  │  IMU     │  │  Power   │  │ Battery  │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐        │
│  │ Config   │  │Diagnostic│  │DeviceInfo│  │  Event   │        │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘        │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                      │
│  │   Log    │  │ Request  │  │ Response │                      │
│  └──────────┘  └──────────┘  └──────────┘                      │
└──────────────────────────────────────────────────────────────────┘
```

### Data Flow

```
STM32 ──serial──→ Transport.read_frame() ──→ decode_and_dispatch()
                                                  │
                                  ┌───────────────┼───────────────┐
                                  ▼               ▼               ▼
                          state.update_*()   return Event    Response Event
                                  │               │               │
                                  ▼               ▼               ▼
                          DriverState       bounded channel   Response Channel
                          (Latest Snapshot)  (1024)           (Sync Wait)
                                  │               │
                                  ▼               ▼
                          state.snapshot()   Dispatch Thread → dispatch(callbacks)
                          (TUI/ROS2 Poll)
```

- **High-frequency periodic data** (IMU/Power/Battery, etc.): polled via `state.snapshot()`
- **Low-frequency triggered data** (Log/Event, etc.): processed via `DriverCallback` callbacks

### Core Components

#### 1. Transport Trait

```rust
pub trait Transport: Send + 'static {
    fn read_frame(&mut self) -> Result<Vec<u8>, DriverError>;
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), DriverError>;
    fn close(&mut self) -> Result<(), DriverError>;
}
```

- **SerialTransport**: Serial implementation using `serialport` crate
- **MockTransport**: Simulated data for development and testing
- **AsyncDriver**: Thin async facade over sync `Driver` (`spawn_blocking`, feature gated)

#### 2. Driver

```rust
pub struct Driver {
    transport: Arc<Mutex<Option<Box<dyn Transport>>>>,
    bus: Arc<EventBus>,
    state: Arc<DriverState>,
    // ...
}
```

- **Read Thread**: Dedicated thread continuously reads frames, updates state, sends events to channel
- **Dispatch Thread**: Dedicated thread consumes events from channel, triggers callbacks (non-blocking)
- **Synchronous Writes**: Transport writes protected by `Mutex`
- **State Snapshot**: `DriverState` provides thread-safe data access

#### 3. EventBus

```rust
pub struct EventBus {
    tx: Sender<DriverEvent>,       // bounded(1024), main event channel
    ack_tx: Sender<DriverEvent>,   // bounded(64), response channel (sync wait)
    callbacks: Arc<Mutex<Vec<Box<dyn DriverCallback>>>>,
}
```

- Dual-channel design: main event channel (bounded, auto backpressure) + response channel (bounded, for sync request-response)
- Callbacks fire on dedicated dispatch thread, non-blocking to read thread

#### 4. Protocol Layer

Each data type implements:
- `ToPayload`: Serialize to bytes
- `FromPayload`: Deserialize from bytes
- `from_bytes()` / `to_bytes()`: Low-level byte operations

### Frame Format

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

### Frame Types

> **Note**: `0x03` is reserved (old Thermal, merged into Diagnostic).

| Type | Value | Direction | Description |
|------|-------|-----------|-------------|
| Imu | 0x01 | Uplink | IMU inertial measurement data |
| Power | 0x02 | Uplink | Power electrical data |
| Config | 0x04 | Uplink | Config snapshot |
| Battery | 0x05 | Uplink | Battery state |
| Diagnostic | 0x06 | Uplink | Runtime diagnostic (CPU, memory, temps, errors) |
| Event | 0x07 | Uplink | Event |
| Log | 0x08 | Uplink | Log message |
| Request | 0x80 | Downlink | Unified request (RequestKind byte selects operation) |
| Response | 0xC0 | Response | Unified response ([request_kind:1][success:1][data:N]) |

### Request Types (RequestKind)

All downlink operations use a single `Request` frame (0x80). The first payload byte (`RequestKind`) selects the operation:

| RequestKind | Value | Description | Expects Response |
|-------------|-------|-------------|------------------|
| Reset | 0x01 | Reboot MCU | No (fire-and-forget) |
| Shutdown | 0x02 | Power off | No (fire-and-forget) |
| Ota | 0x03 | Trigger OTA update | No (fire-and-forget) |
| ConfigWrite | 0x10 | Write single config item | Yes |
| ConfigQuery | 0x11 | Query single config item | Yes |
| ConfigQueryAll | 0x12 | Query all configs | Yes |
| DeviceInfo | 0x13 | Query static device info | Yes |
| ServoForward | 0x20 | Forward servo command | Yes |
| FirmwareUpdate | 0x21 | Firmware data chunk for OTA | Yes |

### Threading Model

```
┌─────────────────────────────────────────────────┐
│                   User Thread                    │
│  driver.write_config() / driver.query_config()   │
│         │                                        │
│         ▼                                        │
│  ┌──────────────┐                                │
│  │ Mutex<Transport> │ ◄── Read/Write mutex       │
│  └──────┬───────┘                                │
│         │                                        │
├─────────┼────────────────────────────────────────┤
│         ▼                                        │
│  ┌──────────────┐                                │
│  │  Read Thread │  loop {                        │
│  │              │    transport.read_frame()       │
│  │              │    decode → state.update        │
│  │              │    → channel.send()             │
│  │              │  }                              │
│  └──────┬───────┘                                │
│         │ bounded channel (1024)                  │
│         ▼                                        │
│  ┌──────────────┐                                │
│  │ Dispatch Thr │  loop {                        │
│  │              │    channel.recv()               │
│  │              │    → dispatch(callbacks)        │
│  │              │  }                              │
│  └──────────────┘                                │
│                                                  │
│  ┌──────────────┐                                │
│  │  Response    │  Sync wait recv_timeout()      │
│  │  Channel     │                                │
│  │ (bounded 64) │                                │
│  └──────────────┘                                │
└─────────────────────────────────────────────────┘
```

### Reconnection

```
Connection Lost
    │
    ▼
Check Reconnect Config
    │
    ├── max_retries = 0 → No reconnect
    │
    ▼
Wait retry_interval * backoff_multiplier^retry_count
    │
    ▼
Call TransportFactory.create()
    │
    ├── Success → Reset counter, continue
    │
    └── Failure → retry_count++, retry
```

## Quick Start

### Add Dependency

```toml
[dependencies]
servo-robot-driver = { path = "../servo-robot-driver" }

# Enable mock transport (for dev/test)
servo-robot-driver = { path = "../servo-robot-driver", features = ["mock"] }

# Enable async support
servo-robot-driver = { path = "../servo-robot-driver", features = ["async"] }
```

### Using DriverCallback (Recommended)

```rust
use servo_robot_driver::{Driver, MockTransport, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::battery_state::BatteryState;
use servo_robot_driver::protocol::diagnostic::Diagnostic;
use servo_robot_driver::protocol::device_info::DeviceInfo;
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
        println!("CPU: {}%, heap: {}KB, temp_mcu: {:.1}°C",
            diag.cpu_usage_percent, diag.free_heap_kb, diag.temp_mcu as f32 / 10.0);
    }

    fn on_device_info(&mut self, info: &DeviceInfo) {
        println!("Device: id={:#06x}, fw={}", info.device_id, info.firmware_version);
    }

    fn on_response(&mut self, resp: &Response) {
        println!("Response: kind={:?}, success={}", resp.request_kind, resp.success);
    }

    // Override default log handling
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        println!("[Board][{}] {}: {}", ts, log_msg.fun_name, log_msg.msg);
    }
}

let mock = MockTransport::new();
let mut driver = Driver::new(mock);
driver.register_callback(MyCallback);
driver.start()?;
```

### Using State Snapshot (TUI Scenario)

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
    println!("CPU: {}%, temps: mcu={:.1}°C", diag.cpu_usage_percent, diag.temp_mcu as f32 / 10.0);
}
```

### Using Real Serial Port

```rust
use servo_robot_driver::{Driver, SerialTransport};

let transport = SerialTransport::open("/dev/ttyUSB0", 115200)?;
let mut driver = Driver::new(transport);
driver.start()?;
```

### Auto-Reconnect

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

// Set initial state
mock.set_battery_soc(80.0);
mock.set_initial_attitude(10.0, 5.0, 0.0);
mock.set_charging(true);

// Simulate disconnect (for reconnection testing)
mock.set_auto_disconnect(1000);

// Manual connection control
mock.disconnect();
mock.reconnect();

// Get written frames (for command verification)
let written = mock.written_frames();
```

### Simulated Data

| Data Type | Rate | Simulation |
|-----------|------|------------|
| IMU | 100Hz | Attitude drift, sensor noise, gravity |
| Power | 20Hz | Battery voltage variation, current fluctuation |
| Diagnostic | 1Hz | Uptime, CPU usage, temperatures, error counts |
| Battery | 10Hz | SOC drain, charge state |
| Event | 1Hz | Charge state, fan state, protection flags |

## Callback Mechanism

### DriverCallback Trait

```rust
use servo_robot_driver::{Driver, DriverCallback};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::battery_state::BatteryState;
use servo_robot_driver::protocol::response::Response;
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

    // Unified response callback (replaces old on_ack_cfg_write / on_ack_cfg_query / etc.)
    fn on_response(&mut self, resp: &Response) {
        println!("Response: kind={:?}, success={}, data_len={}",
            resp.request_kind, resp.success, resp.data.len());
    }

    // Board log callback (default: output via log crate)
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        println!("[Board][{}] {}: {}", ts, log_msg.fun_name, log_msg.msg);
    }
}

driver.register_callback(MyCallback { imu_count: 0 });
```

> **Note**: Callbacks fire on a dedicated dispatch thread, non-blocking to the read thread.

### Callback Methods

| Method | Trigger | Default Behavior |
|--------|---------|-----------------|
| `on_imu_data` | IMU frame (100Hz) | Empty |
| `on_power_data` | Power frame (20Hz) | Empty |
| `on_battery_state` | Battery frame (10Hz) | Empty |
| `on_config_snapshot` | Config snapshot | Empty |
| `on_board_event` | Event frame (1Hz) | Empty |
| `on_device_info` | DeviceInfo response | Empty |
| `on_diagnostic` | Diagnostic frame (1Hz) | Empty |
| `on_log(ts, log_msg)` | Board log frame | Output via `log` crate with `[ServoRobotBoard]` prefix |
| `on_response` | Unified response (all Request types) | Empty |
| `on_error` | Driver error | Empty |

## Log System

### Board Logs

Upstream `Log` frames from STM32 are dispatched via `DriverCallback::on_log`. Default implementation outputs via `log` crate with `[ServoRobotBoard]` prefix and timestamp:

```
[ServoRobotBoard] [14:30:05] config.c::handle_query: queried PowerServoCurrentLimit
[ServoRobotBoard] [14:30:06] imu.c::read_gyro: sensor timeout
[ServoRobotBoard] [14:30:07] main.c::app_init: boot complete
```

Log level mapping: `LogLevel::Error` → `log::error!`, `Warn` → `log::warn!`, `Info` → `log::info!`, `Debug` → `log::debug!`

### Custom Log Handling

Override `on_log` for custom log behavior (e.g., display in TUI panel, publish to ROS2 topic):

```rust
impl DriverCallback for MyCallback {
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        // Custom handling, no longer output to log
        self.log_buffer.push(format!("[{}] {}::{}: {}",
            ts, log_msg.file_name, log_msg.fun_name, log_msg.msg));
    }
}
```

### Using log Backend

Connect any `log` backend to see default board log output:

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

## Commands and Config

```rust
use servo_robot_driver::protocol::config::{Config, ConfigType};

// Async (no wait for response)
driver.query_config(ConfigType::PowerServoCurrentLimit)?;
driver.write_config(Config::PowerServoCurrentLimit(5.0))?;

// Sync (wait for response)
let config = driver.query_config_sync(ConfigType::PowerServoCurrentLimit)?;
let all_config = driver.query_all_configs_sync()?;
let success = driver.write_config_sync(Config::PowerServoCurrentLimit(5.0))?;
```

## C/C++ Integration (FFI)

Enable the `ffi` feature to compile the driver into a shared library (`.so`) callable from C/C++.

### Build the .so

```bash
# From the repo root; --features ffi is required, otherwise the .so exports no sr_* symbols
cargo build --release --features ffi
# Output: target/release/libservo_robot_driver.so
nm -D --defined-only target/release/libservo_robot_driver.so | grep ' T sr_driver_'
```

Header: `include/servo_robot_driver.h` (single interface contract: structs, enums, function declarations, threading red lines).

### Driver Version

```c
#include "servo_robot_driver.h"

sr_version ver = sr_driver_version();
printf("driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);
```

### One-shot Packaging (Recommended)

```bash
crates/servo-robot-driver/ffi/package_ffi.sh [output_dir]
# Default output at the repo root ffi-dist/:
#   ffi-dist/
#   ├── CMakeLists.txt            # pre-configured: imports .so + builds examples
#   ├── README.md
#   ├── include/servo_robot_driver.h
#   ├── lib/libservo_robot_driver.so
#   └── examples/cpp_example.cpp, c_example.c
```

Copy the whole `ffi-dist/` into a C/C++ project and build:

```bash
cd ffi-dist
cmake -S . -B build && cmake --build build
./build/cpp_example /dev/ttyUSB0 115200
```

### Manual Integration

```c
#include "servo_robot_driver.h"   // extern "C" guarded, safe to include from C++

sr_driver* d = sr_driver_open("/dev/ttyUSB0", 115200, err, sizeof err);
sr_driver_start(d);

sr_board_config cfg;
sr_driver_query_all_configs(d, &cfg);        // sync, blocks ≤1s
sr_driver_write_config_sync(d, (sr_config){.typ = SR_CONFIG_SERVO_BAUD_RATE, .value = 1000000}, &ok);

sr_driver_stop(d);
sr_driver_free(d);                            // must be called last
```

```bash
gcc my_prog.c -I <include dir> -L target/release -lservo_robot_driver \
    -Wl,-rpath,$PWD/target/release -o my_prog   # or g++ for C++
```

Runtime options (pick one): `-Wl,-rpath` at link time / `LD_LIBRARY_PATH` / install to `/usr/local/lib` + `ldconfig`.

### FFI Callback Example (C)

```c
#include "servo_robot_driver.h"

typedef struct {
    uint64_t response_count;
} my_ctx;

static void on_response(void* userdata, const sr_response* resp) {
    my_ctx* ctx = (my_ctx*)userdata;
    ctx->response_count++;
    printf("[RESP] kind=%u success=%u data_len=%zu\n",
           resp->request_kind, resp->success, resp->data_len);
}

int main(void) {
    char err[256];
    sr_driver* d = sr_driver_open("/dev/ttyUSB0", 115200, err, sizeof err);

    my_ctx ctx = {0};
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof cbs);
    cbs.userdata = &ctx;
    cbs.on_response = on_response;
    sr_driver_set_callbacks(d, &cbs);
    sr_driver_start(d);

    // ...

    sr_driver_stop(d);
    sr_driver_free(d);
}
```

### FFI Callback Example (C++, Class-based)

```cpp
#include "servo_robot_driver.h"
#include <functional>
#include <cstdio>

struct CallbackCtx {
    std::function<void(const sr_response*)> on_response;
    std::function<void(const sr_imu*)> on_imu;
};

extern "C" {
void response_thunk(void* u, const sr_response* r) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_response) c->on_response(r);
}
void imu_thunk(void* u, const sr_imu* d) {
    if (auto* c = static_cast<CallbackCtx*>(u); c && c->on_imu) c->on_imu(d);
}
} // extern "C"

// Usage in a class (ROS2 node style):
//   CallbackCtx ctx_;
//   ctx_.on_response = [this](const sr_response* r) { handle_response(r); };
//   ctx_.on_imu      = [this](const sr_imu* d)      { publish_imu(d); };
//   sr_callbacks cbs = {};
//   cbs.userdata = &ctx_;
//   cbs.on_response = response_thunk;
//   cbs.on_imu_data = imu_thunk;
//   sr_driver_set_callbacks(driver_, &cbs);
```

### Red Lines

- Callbacks run on the driver's **dispatch thread**; never call any `sr_driver_*` from inside a callback (especially `sr_driver_free` — self-deadlock)
- Callback argument pointers are valid only during the callback
- Never `sr_driver_free` while other threads are still using the handle (use-after-free)
- Sync functions block ≤1s (driver default timeout)

Full examples: `ffi/c_example.c` (C, free-function callbacks) and `ffi/cpp_example.cpp` (C++, class-based callbacks, ROS2-node style); `ffi/smoke_test.c` is an internal test, not an example.

## Feature Flags

| Feature | Dependency | Description |
|---------|-----------|-------------|
| `mock` | `rand` | Enable MockTransport |
| `async` | `tokio` | Enable AsyncDriver (thin facade over sync Driver) |
| `ffi` | — | Enable C FFI layer (.so exports) |
