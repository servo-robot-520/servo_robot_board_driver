# servo-robot-protocol

[English](README_en.md) | 简体中文

ServoRobotBoard 与上位机之间的通信协议定义，支持 `no_std` + `alloc`，可同时用于 PC 与嵌入式平台。

## 特性

- `#![no_std]` 兼容，支持嵌入式环境
- 帧协议解析（HEAD + TYPE + LEN + PAYLOAD + CRC）
- 完整的数据类型定义（IMU、Power、Battery、System（含温度）、Event、Log、Config、Servo、Command）
- CRC-16/CCITT 校验
- 通过 `embedded` feature 切换平台

## 快速开始

### 引用方式

```toml
# 方式一：本地路径
[dependencies]
servo-robot-protocol = { path = "../servo-robot-protocol" }

# 方式二：GitHub 引用
[dependencies]
servo-robot-protocol = { git = "https://github.com/greenhand520/servo_robot_board_driver", branch = "main" }

# 嵌入式模式（no_std）
[dependencies]
servo-robot-protocol = { git = "https://github.com/greenhand520/servo_robot_board_driver", branch = "main", default-features = false, features = ["embedded"] }
```

## 帧格式

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
| FirmwareUpdate | 0x84 | 下行 | 固件升级数据块 |
| Command | 0x85 | 下行 | 一次性命令（复位 / 关机 / OTA）|
| AckCfgWrite | 0xC0 | 应答 | 写入确认 |
| AckCfgQuery | 0xC1 | 应答 | 单个配置响应 |
| AckCfgQueryAll | 0xC2 | 应答 | 所有配置响应 |
| AckServoCmd | 0xC3 | 应答 | 舵机命令响应 |
| AckFirmwareUpdate | 0xC4 | 应答 | 固件块确认（成功 + 偏移）|
| AckCommand | 0xC5 | 应答 | 命令执行确认 |

## 数据类型

### 整数类型约定

所有协议数据均使用整数 + 缩放因子传输，提高传输效率：

| 数据类型 | 协议类型 | 缩放 | 示例 |
|-----------|---------------|---------|---------|
| 电压 | u16 | mV | 8660 = 8.66V |
| 电流 | u16/i16 | mA | 1250 = 1.25A |
| 容量 | u16 | mAh | 4600 = 4600mAh |
| 温度 | i16 | ×10 | 571 = 57.1°C |
| 百分比 | u8 | 1~100 | 75 = 75% |

### 上行数据

| 类型 | 字段 | 更新频率 |
|------|--------|-------------|
| `ImuData` | accel[3], gyro[3], quaternion[4], timestamp_ms, roll, pitch, yaw | 100Hz |
| `PowerData` | servo_voltage_mv/current_ma, charge_in_voltage_mv/current_ma, bat_voltage_mv/current_ma | 20Hz |
| `BatteryState` | voltage_mv, current_ma, percentage, capacity_mah, cell_voltages_mv, ... | 10Hz |
| `SystemInfo` | device_id, uid, uptime, cpu_usage, heap, stack, frames_sent, pd_voltage/current, version, temperatures | 1Hz |
| `BoardEvent` | charge_phase, state_change_flags, protection_flags, error_flags | 触发式 |
| `LogMessage` | level, file_name, fun_name, msg | 触发式 |
| `BoardConfigSnapshot` | 全部配置参数 + 开关状态 | 事件触发 |

### ImuData

IMU 惯性测量数据。payload 长度 56 字节（13 × f32 + u32）。

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| accel | [f32; 3] | 加速度计 |
| gyro | [f32; 3] | 陀螺仪 |
| quaternion | [f32; 4] | 四元数 (w, x, y, z) |
| timestamp_ms | u32 | 时间戳（毫秒）|
| roll / pitch / yaw | f32 | 姿态角（度）|

### PowerData

电源电气测量。payload 长度 12 字节。

| 字段 | 类型 | 单位 | 说明 |
|-------|------|------|-------------|
| servo_voltage_mv | u16 | mV | 舵机供电电压 |
| servo_current_ma | u16 | mA | 舵机供电电流 |
| charge_in_voltage_mv | u16 | mV | USB-PD 输入电压 |
| charge_in_current_ma | u16 | mA | USB-PD 输入电流 |
| bat_voltage_mv | u16 | mV | 电池电压 |
| bat_current_ma | i16 | mA | 电池电流（+ 充电，- 放电）|

### BatteryState

电池状态信息。

| 字段 | 类型 | 单位 | 说明 |
|-------|------|------|-------------|
| voltage_mv | u16 | mV | 电池总电压 |
| current_ma | i16 | mA | 电池电流 |
| percentage | u8 | 1~100 | 电池电量 |
| capacity_mah | u16 | mAh | 实际容量 |
| design_capacity_mah | u16 | mAh | 设计容量 |
| temperature | i16 | ×10 | 电池温度 |
| charge_status | BatteryChargeStatus | - | 充电状态 |
| health | BatteryHealth | - | 电池健康状态 |
| technology | BatteryTechnology | - | 电池技术类型 |
| present | bool | - | 电池是否在位 |
| serial_number | u16 | - | 序列号 |
| cell_voltages_mv | Vec\<u16\> | mV | 各电芯电压 |
| cell_temperatures | Vec\<i16\> | ×10 | 各电芯温度 |

### SystemInfo

系统信息（含温度数据，已合并原 ThermalData）。payload 长度 41 字节。

| 字段 | 类型 | 单位 | 说明 |
|-------|------|------|-------------|
| device_id | u16 | - | STM32 设备 ID |
| uid | u32 | - | STM32 唯一 ID |
| imu_id | u8 | - | IMU 芯片 ID |
| uptime_s | u32 | s | 运行时间（秒）|
| cpu_usage_percent | u8 | % | CPU 使用率 |
| free_heap_kb | u16 | KB | 空闲堆内存 |
| stack_watermark_min_kb | u16 | KB | 栈水位最低值 |
| i2c_error_count | u16 | - | I2C 错误计数 |
| spi_error_count | u16 | - | SPI 错误计数 |
| uart_error_count | u16 | - | UART 错误计数 |
| usb_error_count | u16 | - | USB 错误计数 |
| frames_sent_total | u32 | - | 已发送帧总数 |
| pd_request_voltage_mv | u16 | mV | PD 协议电压 |
| pd_request_current_ma | u16 | mA | PD 协议电流 |
| firmware_version | Version | - | 固件版本 |
| temp_servo_power | i16 | ×10 | 舵机供电温度 |
| temp_5v_power | i16 | ×10 | 5V 供电温度 |
| temp_mcu | i16 | ×10 | MCU 温度 |
| temp_charge | i16 | ×10 | 充电电路温度 |
| temp_battery | i16 | ×10 | 电池温度 |

#### Version 结构

```rust
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}
// 显示格式: "major.minor.patch"，例如 "0.1.0"
```

### BoardEvent

事件数据，使用位标志。payload 长度 7 字节：

```
[0]    charge_phase (u8)
[1..3] state_change_flags (u16 LE)
[3..5] protection_flags (u16 LE)
[5..7] error_flags (u16 LE)
```

| 字段 | 类型 | 说明 |
|-------|------|-------------|
| charge_phase | ChargePhase | 充电阶段 |
| state_change_flags | StateChangeFlags | 状态变化标志 |
| protection_flags | ProtectionFlags | 保护标志 |
| error_flags | ErrorFlags | 错误标志 |

#### ChargePhase

| 值 | 变体 | 说明 |
|-----|------|------|
| 0 | NotCharging | 未充电 |
| 1 | PreCharge | 预充电 |
| 2 | Cc | 恒流充电 |
| 3 | Cv | 恒压充电 |
| 4 | Full | 充满 |
| 5 | PdSinkFault | PD 受电端故障 |
| 6 | UnsupportedCharger | 不支持的充电器 |

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

#### 事件类型与分类

```rust
/// 历史事件记录（带时间戳）
pub struct EventLog {
    pub ts: u64,           // 时间戳（毫秒）
    pub kind: EventKind,   // 事件类型
}

/// 事件类型（32 个变体）
pub enum EventKind {
    // 充电事件
    NotCharging, PreCharge, CcCharge, CvCharge, FullCharge,
    PdSinkFault, UnsupportedCharger,
    // 保护事件
    ServoOvercurrent, PowerServoThermal, Power5vThermal,
    ChargeDerating, ChargeThermal, BatteryLow,
    // 错误事件
    UnknownError, Uart1Error, Uart2Error, I2c1Error, I2c3Error,
    Spi1Error, UsbError, DmaError,
    // 状态变化事件
    ChargerConnected, ChargerDisconnected, FanOn, FanOff,
    PowerServoOn, PowerServoOff, Power5vOn, Power5vOff,
    BatExtOutOn, BatExtOutOff,
}

/// 事件分类（4 类）
pub enum EventCategory {
    Charge,      // 充电
    StateChange, // 状态变化
    Protection,  // 保护
    Error,       // 错误
}
```

`BoardEvent::diff_events(&prev)` 可与前一状态对比，提取所有新增/变化事件；另有 `new_state_change_events` / `new_protection_events` / `new_error_events` 分别提取各类新增事件。

### LogMessage

日志消息。payload 格式：

```
[level:1][file_name\0][fun_name\0][msg...]
```

`file_name` 和 `fun_name` 以 null 结尾，`msg` 取剩余全部字节（UTF-8）。

日志级别：`OFF=0`、`Debug=1`、`Info=2`、`Warn=3`、`Error=4`。

### 配置类型

| 类型 | 值 | 说明 |
|------|-------|-------------|
| SwitchServoPower | 0x10 | 舵机电源开关 |
| Switch5VPower | 0x11 | 5V 电源开关 |
| SwitchCharge | 0x12 | 充电开关 |
| SwitchBatExtOut | 0x13 | 电池外部输出开关 |
| ChargeStopSoc | 0x20 | 充电停止 SOC (%) |
| TxLogLevel | 0x21 | STM32 发送日志等级 |
| PowerServoCurrentLimitMa | 0x30 | 舵机电流限制 (mA) |
| PowerServoTempLimit | 0x31 | 舵机温度限制 (×10) |
| Power5vTempLimit | 0x32 | 5V 温度限制 (×10) |
| ChargeMaxCurrentMa | 0x33 | 最大充电电流 (mA) |
| ChargeTempDerating | 0x34 | 充电降额温度 (×10) |
| ChargeTempLimit | 0x35 | 充电停止温度 (×10) |
| ChargeStopVoltageMv | 0x36 | 充电停止电压 (mV) |
| ServoBaudRate | 0x37 | 舵机通信波特率 (u32) |

> **注意**：复位 / 关机 / OTA 等一次性命令已移至 `command.rs` 的 `CommandType`，通过 `Command (0x85)` 帧发送，不再属于配置类型。

### BoardConfigSnapshot

板级配置快照，用于查询和显示当前配置状态。payload 长度 24 字节（4 bool + 2 u8 + 7 u16 + 1 u32）。

| 字段 | 类型 | 单位 | 默认值 | 说明 |
|-------|------|------|---------|-------------|
| power_servo_on | bool | - | true | 舵机电源开关 |
| power_5v_on | bool | - | true | 5V 电源开关 |
| charge_on | bool | - | true | 充电开关 |
| bat_ext_out_on | bool | - | true | 电池外部输出开关 |
| charge_stop_percentage | u8 | 1~100 | 100 | 充电停止百分比 |
| tx_log_level | LogLevel | - | Info | STM32 发送日志等级 |
| servo_current_limit_ma | u16 | mA | 50 | 舵机电流限制 |
| servo_temp_limit | u16 | ×10 | 800 | 舵机温度限制 (80.0°C) |
| temp_5v_limit | u16 | ×10 | 700 | 5V 温度限制 (70.0°C) |
| charge_max_current_ma | u16 | mA | 90 | 最大充电电流 |
| charge_temp_derating | u16 | ×10 | 600 | 充电降额温度 (60.0°C) |
| charge_temp_limit | u16 | ×10 | 700 | 充电停止温度 (70.0°C) |
| charge_stop_voltage_mv | u16 | mV | 168 | 充电停止电压 |
| servo_baud_rate | u32 | baud | 115200 | 舵机通信波特率 |

### 命令类型（Command）

通过 `Command (0x85)` 帧发送的一次性动作：

```rust
pub enum CommandType {
    Reset = 0x01,     // 复位 MCU
    Shutdown = 0x02,  // 关机（切断所有电源）
    Ota = 0x03,       // 触发 OTA 升级
}
```

### ServoCmdWrapper

舵机命令包装，用于向舵机总线透明转发原始字节，可包含多条舵机操作命令：

```rust
pub struct ServoCmdWrapper {
    data: Vec<u8>,  // 原始舵机命令字节
}

impl ServoCmdWrapper {
    pub fn new(data: Vec<u8>) -> Self;
    pub fn data(&self) -> &[u8];
    pub fn into_data(self) -> Vec<u8>;
}
```

## 使用示例

### 解析帧

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType};
use servo_robot_protocol::imu::ImuData;

// 从字节解码
let (frame, consumed) = RawFrame::decode(&raw_bytes)?;

// 解析为具体类型
match frame.frame_type {
    FrameType::Imu => {
        let imu = ImuData::from_bytes(&frame.payload)?;
        println!("Roll: {:.1}°", imu.roll);
    }
    FrameType::System => {
        let sys = SystemInfo::from_bytes(&frame.payload)?;
        println!("MCU Temp: {:.1}°C", sys.temp_mcu as f32 / 10.0);
    }
    _ => {}
}
```

### 编码帧

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType};
use servo_robot_protocol::config::Config;

let config = Config::PowerServoCurrentLimitMa(5000); // 5000mA = 5A
let frame = RawFrame {
    frame_type: FrameType::CfgWrite,
    payload: config.to_bytes(),
};
let bytes = frame.encode(); // 包含 HEAD + TYPE + LEN + PAYLOAD + CRC
```

### 类型化帧

```rust
use servo_robot_protocol::frame::{RawFrame, FrameType, TypedFrame};
use servo_robot_protocol::config::Config;

// RawFrame → TypedFrame 自动分发解析
let typed = frame.parse_typed()?;
match typed {
    TypedFrame::Imu(imu) => println!("IMU: {:?}", imu),
    TypedFrame::Battery(bat) => println!("Battery: {:.1}%", bat.percentage),
    _ => {}
}
```

### 舵机命令转发

```rust
use servo_robot_protocol::servo::ServoCmdWrapper;
use servo_robot_protocol::frame::{RawFrame, FrameType};

// 从原始字节创建舵机命令
let cmd = ServoCmdWrapper::new(vec![0x01, 0x02, 0x03]);

// 编码为帧
let frame = RawFrame {
    frame_type: FrameType::ServoForward,
    payload: cmd.to_payload(),
};
let bytes = frame.encode();
```

### 事件处理

```rust
use servo_robot_protocol::event::{BoardEvent, EventKind, EventCategory};

// 与前一状态对比，提取新增事件
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

### CRC 计算

```rust
use servo_robot_protocol::crc::crc16_ccitt_table;

let data = b"Hello";
let crc = crc16_ccitt_table(data);
```

## 模块结构

```
src/
├── lib.rs              # #![no_std] 入口
├── crc.rs              # CRC-16/CCITT
├── error.rs            # FrameError
├── frame.rs            # RawFrame, TypedFrame, FrameType, ToPayload/FromPayload
├── imu.rs              # ImuData
├── power.rs            # PowerData
├── battery_state.rs    # BatteryState
├── system.rs           # SystemInfo, Version（含温度数据）
├── event.rs            # BoardEvent, EventLog, EventKind, EventCategory
├── log.rs              # LogMessage, LogLevel
├── config.rs           # ConfigType, Config, BoardConfigSnapshot
├── command.rs          # CommandType, Command, AckCommand
└── servo.rs            # ServoCmdWrapper
```

## 依赖

- `bitflags` - 位标志（支持 no_std）

## Feature Flags

| Feature | 说明 |
|---------|-------------|
| `std`（默认） | 启用标准库支持 |
| `embedded` | 嵌入式模式（no_std）|
