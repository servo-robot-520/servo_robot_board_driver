# servo-robot-protocol

English | [简体中文](README.md)

Definition of communication protocol between ServoRobotBoard and host computer, supports `no_std` + `alloc`, compatible with PC and embedded platforms.

## Features

- `#![no_std]` compatible, supports embedded environments
- Frame protocol parsing (HEAD + TYPE + LEN + PAYLOAD + CRC)
- Complete data type definitions (IMU, Power, Battery, Diagnostic (runtime diagnostics) + DeviceInfo (device identity), Event, Log, Config, Servo, Request/Response)
- CRC-16/CCITT checksum
- Platform switching via `embedded` feature

## Quick Start

### Reference Methods

```toml
# Option 1: Local path
[dependencies]
servo-robot-protocol = { path = "../servo-robot-protocol" }

# Option 2: GitHub reference
[dependencies]
servo-robot-protocol = { git = "https://github.com/greenhand520/servo_robot_board_driver", branch = "main" }

# Embedded mode (no_std)
[dependencies]
servo-robot-protocol = { git = "https://github.com/greenhand520/servo_robot_board_driver", branch = "main", default-features = false, features = ["embedded"] }
```

## Frame Format

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

> **Note**: `0x03` is reserved (old Thermal, merged into Diagnostic).

| Type | Value | Direction | Description |
|------|-------|-----------|-------------|
| Imu | 0x01 | Uplink | IMU inertial measurement data |
| Power | 0x02 | Uplink | Power electrical data |
| Config | 0x04 | Uplink | Config snapshot |
| Battery | 0x05 | Uplink | Battery state |
| Diagnostic | 0x06 | Uplink | Runtime diagnostics (CPU, memory, errors, temperatures) |
| Event | 0x07 | Uplink | Event |
| Log | 0x08 | Uplink | Log message |
| Request | 0x80 | Downlink | Unified request (see RequestKind) |
| Response | 0xC0 | Response | Unified response (see RequestKind) |

### RequestKind

All downlink operations are unified into a single `Request (0x80)` frame; the first byte of the payload (`RequestKind`) identifies the specific operation.

| RequestKind | Value | Description | Expects Response |
|-------------|-------|-------------|-----------------|
| Reset | 0x01 | Reboot MCU | No (fire-and-forget) |
| Shutdown | 0x02 | Shutdown (cut all power) | No (fire-and-forget) |
| Ota | 0x03 | Trigger OTA update | No (fire-and-forget) |
| ConfigWrite | 0x10 | Write single config item | Yes |
| ConfigQuery | 0x11 | Query single config item | Yes |
| ConfigQueryAll | 0x12 | Query all configs | Yes |
| DeviceInfo | 0x13 | Query device identity & memory layout (static) | Yes |
| ServoForward | 0x20 | Forward servo command | Yes |
| FirmwareUpdate | 0x21 | Firmware update data chunk | Yes |

### Request / Response Wire Format

```
Request:  FrameType(0x80) + payload[request_kind:1][data:N]
Response: FrameType(0xC0) + payload[request_kind:1][success:1][data:N]
```

- `request_kind`: echoes the `RequestKind` byte
- `success`: `0x01` = success, `0x00` = failure
- `data`: response-specific payload (e.g. DeviceInfo, BoardConfigSnapshot, Config value)

## Data Types

### Integer Type Convention

All protocol data uses integer types with scaling factors for efficient transmission:

| Data Type | Protocol Type | Scaling | Example |
|-----------|---------------|---------|---------|
| Voltage | u16 | mV | 8660 = 8.66V |
| Current | u16/i16 | mA | 1250 = 1.25A |
| Capacity | u16 | mAh | 4600 = 4600mAh |
| Temperature | i16 | ×10 | 571 = 57.1°C |
| Percentage | u8 | 1~100 | 75 = 75% |

### Uplink Data

| Type | Fields | Update Rate |
|------|--------|-------------|
| `ImuData` | accel[3], gyro[3], quaternion[4], timestamp_ms, roll, pitch, yaw | 100Hz |
| `PowerData` | servo_voltage_mv/current_ma, charge_in_voltage_mv/current_ma, bat_voltage_mv/current_ma | 20Hz |
| `BatteryState` | voltage_mv, current_ma, percentage, capacity_mah, cell_voltages_mv, ... | 10Hz |
| `DeviceInfo` | device_id, uid, imu_id, firmware_version, ram_kb, flash_boot_kb, flash_app_kb, flash_ota_kb, flash_user_kb | Query only |
| `Diagnostic` | uptime, cpu_usage, heap, stack, error_counts, frames_sent, pd_voltage/current, temperatures | 1Hz |
| `BoardEvent` | charge_phase, state_change_flags, protection_flags, error_flags | Triggered |
| `LogMessage` | level, file_name, fun_name, msg | Triggered |
| `BoardConfigSnapshot` | All config params + switch states | Event triggered |

### ImuData

IMU inertial measurement data. Payload length: 56 bytes (13 × f32 + u32).

| Field | Type | Description |
|-------|------|-------------|
| accel | [f32; 3] | Accelerometer |
| gyro | [f32; 3] | Gyroscope |
| quaternion | [f32; 4] | Quaternion (w, x, y, z) |
| timestamp_ms | u32 | Timestamp (ms) |
| roll / pitch / yaw | f32 | Attitude angles (deg) |

### PowerData

Power electrical measurements. Payload length: 12 bytes.

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| servo_voltage_mv | u16 | mV | Servo power supply voltage |
| servo_current_ma | u16 | mA | Servo power supply current |
| charge_in_voltage_mv | u16 | mV | USB-PD input voltage |
| charge_in_current_ma | u16 | mA | USB-PD input current |
| bat_voltage_mv | u16 | mV | Battery voltage |
| bat_current_ma | i16 | mA | Battery current (+ charging, - discharging) |

### BatteryState

Battery status information.

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| voltage_mv | u16 | mV | Total battery voltage |
| current_ma | i16 | mA | Battery current |
| percentage | u8 | 1~100 | Battery percentage |
| capacity_mah | u16 | mAh | Actual capacity |
| design_capacity_mah | u16 | mAh | Design capacity |
| temperature | i16 | ×10 | Battery temperature |
| charge_status | BatteryChargeStatus | - | Charge status |
| health | BatteryHealth | - | Battery health status |
| technology | BatteryTechnology | - | Battery technology type |
| present | bool | - | Whether battery is present |
| serial_number | u16 | - | Serial number |
| cell_voltages_mv | Vec\<u16\> | mV | Cell voltages |
| cell_temperatures | Vec\<i16\> | ×10 | Cell temperatures |

### DeviceInfo

Device identity and memory layout (static hardware information). Does not change at runtime. Obtained via `RequestKind::DeviceInfo`.

```rust
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}
// Display format: "major.minor.patch", e.g. "0.1.0"
```

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| device_id | u16 | - | STM32 device ID (DBGMCU.IDCODE) |
| uid | u32 | - | STM32 unique ID |
| imu_id | u8 | - | IMU chip ID |
| firmware_version | Version | - | Firmware version |
| ram_kb | u16 | KB | RAM size |
| flash_boot_kb | u16 | KB | Bootloader flash size |
| flash_app_kb | u16 | KB | Application flash size |
| flash_ota_kb | u16 | KB | OTA temp flash size |
| flash_user_kb | u16 | KB | User data flash size |

### Diagnostic

Runtime diagnostics (runtime state data that changes continuously during operation). Pushed at 1 Hz via `FrameType::Diagnostic (0x06)`.

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| uptime_s | u32 | s | Uptime (seconds) |
| cpu_usage_percent | u8 | % | CPU usage |
| free_heap_kb | u16 | KB | Free heap memory |
| stack_watermark_min_kb | u16 | KB | Stack watermark minimum |
| i2c_error_count | u16 | - | I2C error count |
| spi_error_count | u16 | - | SPI error count |
| uart_error_count | u16 | - | UART error count |
| usb_error_count | u16 | - | USB error count |
| frames_sent_total | u32 | - | Total frames sent |
| pd_request_voltage_mv | u16 | mV | PD protocol voltage |
| pd_request_current_ma | u16 | mA | PD protocol current |
| temp_servo_power | i16 | ×10 | Servo power temperature |
| temp_5v_power | i16 | ×10 | 5V power temperature |
| temp_mcu | i16 | ×10 | MCU temperature |
| temp_charge | i16 | ×10 | Charge circuit temperature |
| temp_battery | i16 | ×10 | Battery temperature |

### BoardEvent

Event data using bitfield flags. Payload length: 7 bytes:

```
[0]    charge_phase (u8)
[1..3] state_change_flags (u16 LE)
[3..5] protection_flags (u16 LE)
[5..7] error_flags (u16 LE)
```

| Field | Type | Description |
|-------|------|-------------|
| charge_phase | ChargePhase | Charging phase |
| state_change_flags | StateChangeFlags | State change flags |
| protection_flags | ProtectionFlags | Protection flags |
| error_flags | ErrorFlags | Error flags |

#### ChargePhase

| Value | Variant | Description |
|-------|---------|-------------|
| 0 | NotCharging | Not charging |
| 1 | PreCharge | Pre-charge |
| 2 | Cc | Constant-current charging |
| 3 | Cv | Constant-voltage charging |
| 4 | Full | Fully charged |
| 5 | PdSinkFault | PD sink fault |
| 6 | UnsupportedCharger | Unsupported charger |

#### StateChangeFlags

```rust
bitflags! {
    pub struct StateChangeFlags: u16 {
        const CHARGER_CONNECTED = 1 << 0;
        const FAN_ENABLED       = 1 << 1;
        const SERVO_POWER_ON    = 1 << 2;
        const POWER_5V_ON       = 1 << 3;
        const BAT_EXT_OUT_ON    = 1 << 4;
    }
}
```

#### ProtectionFlags

```rust
bitflags! {
    pub struct ProtectionFlags: u16 {
        const SERVO_OVERCURRENT = 1 << 0;
        const SERVO_THERMAL     = 1 << 1;
        const DCDC_5V_THERMAL   = 1 << 2;
        const CHARGE_DERATING   = 1 << 3;
        const CHARGE_THERMAL    = 1 << 4;
        const BATTERY_LOW       = 1 << 5;
    }
}
```

#### ErrorFlags

```rust
bitflags! {
    pub struct ErrorFlags: u16 {
        const UNKNOWN_ERROR = 1 << 0;
        const UART1_ERROR   = 1 << 1;
        const UART2_ERROR   = 1 << 2;
        const I2C1_ERROR    = 1 << 3;
        const I2C3_ERROR    = 1 << 4;
        const SPI1_ERROR    = 1 << 5;
        const USB_ERROR     = 1 << 6;
        const DMA_ERROR     = 1 << 7;
    }
}
```

#### Event Types & Categories

```rust
/// Event log entry
pub struct EventLog {
    pub ts: u64,           // Timestamp (ms)
    pub kind: EventKind,   // Event type
}

/// Event type enum (32 variants)
pub enum EventKind {
    // Charging events
    NotCharging, PreCharge, CcCharge, CvCharge, FullCharge,
    PdSinkFault, UnsupportedCharger,
    // Protection events
    ServoOvercurrent, PowerServoThermal, Power5vThermal,
    ChargeDerating, ChargeThermal, BatteryLow,
    // Error events
    UnknownError, Uart1Error, Uart2Error, I2c1Error, I2c3Error,
    Spi1Error, UsbError, DmaError,
    // State change events
    ChargerConnected, ChargerDisconnected, FanOn, FanOff,
    PowerServoOn, PowerServoOff, Power5vOn, Power5vOff,
    BatExtOutOn, BatExtOutOff,
}

/// Event category (4 types)
pub enum EventCategory {
    Charge,
    StateChange,
    Protection,
    Error,
}
```

`BoardEvent::diff_events(&prev)` compares against the previous state and extracts all new/changed events; `new_state_change_events` / `new_protection_events` / `new_error_events` extract per-category new events.

### LogMessage

Log message. Payload format:

```
[level:1][file_name\0][fun_name\0][msg...]
```

`file_name` and `fun_name` are null-terminated; `msg` takes all remaining bytes (UTF-8).

Log levels: `OFF=0`, `Debug=1`, `Info=2`, `Warn=3`, `Error=4`.

### Configuration Types

| Type | Value | Description |
|------|-------|-------------|
| SwitchServoPower | 0x10 | Servo power switch |
| Switch5VPower | 0x11 | 5V power switch |
| SwitchCharge | 0x12 | Charge switch |
| SwitchBatExtOut | 0x13 | Battery extra output switch |
| ChargeStopSoc | 0x20 | Charge stop SOC (%) |
| TxLogLevel | 0x21 | STM32 transmit log level |
| PowerServoCurrentLimitMa | 0x30 | Servo current limit (mA) |
| PowerServoTempLimit | 0x31 | Servo temperature limit (×10) |
| Power5vTempLimit | 0x32 | 5V temperature limit (×10) |
| ChargeMaxCurrentMa | 0x33 | Charge max current (mA) |
| ChargeTempDerating | 0x34 | Charge temp derating (×10) |
| ChargeTempLimit | 0x35 | Charge temp limit (×10) |
| ChargeStopVoltageMv | 0x36 | Charge stop voltage (mV) |
| ServoBaudRate | 0x37 | Servo communication baud rate (u32) |

### BoardConfigSnapshot

Board configuration snapshot for querying and displaying the current configuration state. Payload length: 24 bytes (4 bool + 2 u8 + 7 u16 + 1 u32).

| Field | Type | Unit | Default | Description |
|-------|------|------|---------|-------------|
| power_servo_on | bool | - | true | Servo power switch |
| power_5v_on | bool | - | true | 5V power switch |
| charge_on | bool | - | true | Charge switch |
| bat_ext_out_on | bool | - | true | Battery extra output switch |
| charge_stop_percentage | u8 | 1~100 | 100 | Charge stop percentage |
| tx_log_level | LogLevel | - | Info | STM32 transmit log level |
| servo_current_limit_ma | u16 | mA | 50 | Servo current limit |
| servo_temp_limit | u16 | ×10 | 800 | Servo temperature limit (80.0°C) |
| temp_5v_limit | u16 | ×10 | 700 | 5V temperature limit (70.0°C) |
| charge_max_current_ma | u16 | mA | 90 | Charge max current |
| charge_temp_derating | u16 | ×10 | 600 | Charge temp derating (60.0°C) |
| charge_temp_limit | u16 | ×10 | 700 | Charge stop temperature (70.0°C) |
| charge_stop_voltage_mv | u16 | mV | 168 | Charge stop voltage |
| servo_baud_rate | u32 | baud | 115200 | Servo communication baud rate |

### ServoCmdWrapper

Servo command wrapper for transparently forwarding raw bytes to the servo bus. It may contain multiple servo operation commands:

```rust
pub struct ServoCmdWrapper {
    data: Vec<u8>,  // Raw servo command bytes
}

impl ServoCmdWrapper {
    pub fn new(data: Vec<u8>) -> Self;
    pub fn data(&self) -> &[u8];
    pub fn into_data(self) -> Vec<u8>;
}
```

## Usage Examples

### Parse Uplink Frame

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType};
use servo_robot_protocol::imu::ImuData;

// Decode from bytes
let (frame, consumed) = RawFrame::decode(&raw_bytes)?;

// Parse into specific type
match frame.frame_type {
    FrameType::Imu => {
        let imu = ImuData::from_bytes(&frame.payload)?;
        println!("Roll: {:.1}°", imu.roll);
    }
    FrameType::Diagnostic => {
        let diag = Diagnostic::from_bytes(&frame.payload)?;
        println!("MCU Temp: {:.1}°C", diag.temp_mcu as f32 / 10.0);
    }
    _ => {}
}
```

### Send Request

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType};
use servo_robot_protocol::request::{Request, RequestKind};

// Query device info
let req = Request::new(RequestKind::DeviceInfo, vec![]);
let frame = RawFrame {
    frame_type: FrameType::Request,
    payload: req.to_payload(),
};
let bytes = frame.encode(); // Includes HEAD + TYPE + LEN + PAYLOAD + CRC
```

### Handle Response

```rust
use servo_robot_protocol::response::Response;
use servo_robot_protocol::request::RequestKind;
use servo_robot_protocol::device_info::DeviceInfo;

// Parse response
let resp = Response::from_payload(&frame.payload)?;
if resp.success {
    match resp.request_kind {
        RequestKind::DeviceInfo => {
            let info = DeviceInfo::from_bytes(&resp.data)?;
            println!("Firmware: {}", info.firmware_version);
        }
        RequestKind::ConfigQuery => { /* parse config value */ }
        _ => {}
    }
}
```

### Fire-and-Forget Commands

```rust
use servo_robot_protocol::request::{Request, RequestKind};

// Reset MCU — no response expected
let req = Request::new(RequestKind::Reset, vec![]);
// Shutdown — no response expected
let req = Request::new(RequestKind::Shutdown, vec![]);
// Trigger OTA — no response expected
let req = Request::new(RequestKind::Ota, vec![]);
```

### Typed Frame

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType, TypedFrame};

// RawFrame → TypedFrame, auto-dispatched parsing
let typed = frame.parse_typed()?;
match typed {
    TypedFrame::Imu(imu) => println!("IMU: {:?}", imu),
    TypedFrame::Battery(bat) => println!("Battery: {:.1}%", bat.percentage),
    TypedFrame::Diagnostic(diag) => println!("CPU: {}%", diag.cpu_usage_percent),
    TypedFrame::Response(resp) => println!("Response: kind={:?}, ok={}", resp.request_kind, resp.success),
    _ => {}
}
```

### Servo Command Forwarding

```rust
use servo_robot_protocol::servo::ServoCmdWrapper;
use servo_robot_protocol::request::{Request, RequestKind};
use servo_robot_protocol::frame::{RawFrame, FrameType};

// Create servo command from raw bytes
let cmd = ServoCmdWrapper::new(vec![0x01, 0x02, 0x03]);

// Encode as Request frame
let req = Request::new(RequestKind::ServoForward, cmd.to_payload());
let frame = RawFrame {
    frame_type: FrameType::Request,
    payload: req.to_payload(),
};
let bytes = frame.encode();
```

### Event Handling

```rust
use servo_robot_protocol::event::{BoardEvent, EventKind, EventCategory};

// Compare with previous state to get new events
let prev_event = BoardEvent::default();
let new_events = current_event.diff_events(&prev_event);

for kind in new_events {
    let category = kind.category();
    match category {
        EventCategory::Charge => println!("Charger event: {:?}", kind),
        EventCategory::Protection => println!("Protection event: {:?}", kind),
        EventCategory::Error => println!("Error event: {:?}", kind),
        _ => {}
    }
}
```

### CRC Calculation

```rust
use servo_robot_protocol::crc::crc16_ccitt_table;

let data = b"Hello";
let crc = crc16_ccitt_table(data);
```

## Module Structure

```
src/
├── lib.rs              # #![no_std] entry point
├── crc.rs              # CRC-16/CCITT
├── error.rs            # FrameError
├── frame.rs            # RawFrame, TypedFrame, FrameType, ToPayload/FromPayload
├── imu.rs              # ImuData
├── power.rs            # PowerData
├── battery_state.rs    # BatteryState
├── device_info.rs      # DeviceInfo, Version (static device identity)
├── diagnostic.rs       # Diagnostic (runtime diagnostics: CPU, memory, errors, temperatures)
├── event.rs            # BoardEvent, EventLog, EventKind, EventCategory
├── log.rs              # LogMessage, LogLevel
├── config.rs           # ConfigType, Config, BoardConfigSnapshot
├── request.rs          # RequestKind, Request
├── response.rs         # Response
└── servo.rs            # ServoCmdWrapper
```

## Dependencies

- `bitflags` - Bit flags (supports no_std)

## Feature Flags

| Feature | Description |
|---------|-------------|
| `std` (default) | Enable standard library support |
| `embedded` | Embedded mode (no_std) |
