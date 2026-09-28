//! 文本 / JSON 输出渲染（字段级渲染，不依赖协议层 Display）

use serde_json::{Value, json};
use servo_robot_protocol::battery_state::BatteryState;
use servo_robot_protocol::config::{Config, ConfigType};
use servo_robot_protocol::device_info::DeviceInfo;
use servo_robot_protocol::diagnostic::Diagnostic;
use servo_robot_protocol::event::{BoardEvent, ErrorFlags, ProtectionFlags, StateChangeFlags};
use servo_robot_protocol::imu::ImuData;
use servo_robot_protocol::log::LogMessage;
use servo_robot_protocol::power::PowerData;

// ═══ 配置 ═══

/// 单项取值渲染（文本）：开关 → on/off，TxLogLevel → 级别名，其余数值 + 单位
pub fn config_value_text(ct: ConfigType, wire_value: f32) -> String {
    if ct.is_switch() {
        return if wire_value != 0.0 {
            "on".into()
        } else {
            "off".into()
        };
    }
    if ct == ConfigType::TxLogLevel {
        return servo_robot_protocol::log::LogLevel::from_u8(wire_value as u8).to_string();
    }
    let value = wire_to_display(ct, wire_value);
    let unit = ct.unit();
    if unit.is_empty() {
        format_number(value)
    } else if is_temperature(ct) {
        format!("{value:.1} {unit}")
    } else {
        format!("{} {}", format_number(value), unit)
    }
}

/// `config list` 行内取值：0x2x/0x3x 组由组标题携带单位，不重复输出
pub fn config_list_value_text(ct: ConfigType, wire_value: f32) -> String {
    let group = (ct as u8) >> 4;
    if group == 0x2 {
        return format_number(wire_value);
    }
    if group == 0x3 {
        return format!("{:.1}", wire_to_display(ct, wire_value));
    }
    config_value_text(ct, wire_value)
}

/// 配置项 JSON：`{"type","id","display","value","unit"}`，
/// value 为展示单位（开关 0|1、温度为 °C），恒为数字
pub fn config_item_json(ct: ConfigType, wire_value: f32) -> Value {
    json!({
        "type": ct.variant_name(),
        "id": format!("0x{:02X}", ct as u8),
        "display": ct.name(),
        "value": round1(wire_to_display(ct, wire_value)) as f64,
        "unit": if ct.is_switch() { "switch" } else { ct.unit() },
    })
}

/// 温度组（0x30~0x33）线上值为 ×10，其余组线上值即展示值
pub fn is_temperature(ct: ConfigType) -> bool {
    (ct as u8) >> 4 == 0x3
}

/// 线上值 → 展示值（温度 ÷10）
pub fn wire_to_display(ct: ConfigType, wire: f32) -> f32 {
    if is_temperature(ct) {
        wire / 10.0
    } else {
        wire
    }
}

/// 展示值 → 线上值（温度 ×10）
pub fn display_to_wire(ct: ConfigType, display: f32) -> f32 {
    if is_temperature(ct) {
        (display * 10.0).round()
    } else {
        display
    }
}

/// 从 Config 枚举取 (ConfigType, value)
pub fn config_parts(cfg: &Config) -> (ConfigType, f32) {
    (cfg.config_type(), cfg.value())
}

fn format_number(v: f32) -> String {
    format!("{v}")
}

/// 保留 1 位小数，消除 f32 → JSON 的展示噪声（仅用于派生值）
pub fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

// ═══ 设备信息 ═══

pub fn info_text(info: &DeviceInfo) -> String {
    format!(
        "device_id : 0x{:04X}\n\
         uid       : 0x{:08X}\n\
         imu_id    : 0x{:02X}\n\
         firmware  : {}\n\
         hardware  : {}\n\
         ram       : {} KB\n\
         flash     : boot={}KB app={}KB ota={}KB user={}KB",
        info.device_id,
        info.uid,
        info.imu_id,
        info.firmware_version,
        info.hardware_version,
        info.ram_kb,
        info.flash_boot_kb,
        info.flash_app_kb,
        info.flash_ota_kb,
        info.flash_user_kb,
    )
}

pub fn info_json(info: &DeviceInfo) -> Value {
    json!({
        "kind": "info",
        "device_id": info.device_id,
        "uid": info.uid,
        "imu_id": info.imu_id,
        "firmware": info.firmware_version.to_string(),
        "hardware": info.hardware_version.to_string(),
        "ram_kb": info.ram_kb,
        "flash": {
            "boot_kb": info.flash_boot_kb,
            "app_kb": info.flash_app_kb,
            "ota_kb": info.flash_ota_kb,
            "user_kb": info.flash_user_kb,
        },
    })
}

// ═══ IMU ═══

pub fn imu_text(d: &ImuData) -> String {
    format!(
        "attitude : roll={:+.2}° pitch={:+.2}° yaw={:+.2}°\n\
         gyro     : {:+.3} {:+.3} {:+.3} °/s\n\
         accel    : {:+.3} {:+.3} {:+.3} m/s²\n\
         ts       : {} ms",
        d.roll,
        d.pitch,
        d.yaw,
        d.gyro[0],
        d.gyro[1],
        d.gyro[2],
        d.accel[0],
        d.accel[1],
        d.accel[2],
        d.timestamp_ms,
    )
}

pub fn watch_imu(d: &ImuData) -> String {
    format!(
        "imu roll={:+.2} pitch={:+.2} yaw={:+.2} ts={}ms",
        d.roll, d.pitch, d.yaw, d.timestamp_ms
    )
}

pub fn imu_json(d: &ImuData) -> Value {
    json!({
        "kind": "imu",
        "accel": d.accel,
        "gyro": d.gyro,
        "quaternion": d.quaternion,
        "timestamp_ms": d.timestamp_ms,
        "roll": d.roll,
        "pitch": d.pitch,
        "yaw": d.yaw,
    })
}

// ═══ 电源 ═══

pub fn power_text(d: &PowerData) -> String {
    format!(
        "servo    : {:.1} V / {:.1} A\n\
         pd_in    : {:.1} V / {:.1} A\n\
         bat      : {:.1} V / {:.1} A\n\
         out1     : {:.1} A   out2 : {:.1} A\n\
         5v       : {:.1} V / {:.1} A",
        d.pwr_servo_voltage_mv as f32 / 10.0,
        d.pwr_servo_current_ma as f32 / 10.0,
        d.charge_in_voltage_mv as f32 / 10.0,
        d.charge_in_current_ma as f32 / 10.0,
        d.bat_voltage_mv as f32 / 10.0,
        d.bat_current_ma as f32 / 10.0,
        d.bat_out1_current_ma as f32 / 10.0,
        d.bat_out2_current_ma as f32 / 10.0,
        d.pwr_5v_voltage_mv as f32 / 10.0,
        d.pwr_5v_current_ma as f32 / 10.0,
    )
}

pub fn watch_power(d: &PowerData) -> String {
    format!(
        "power servo={:.1}V/{:.1}A pd_in={:.1}V/{:.1}A bat={:.1}V/{:.1}A \
         out1={:.1}A out2={:.1}A 5v={:.1}V/{:.1}A",
        d.pwr_servo_voltage_mv as f32 / 10.0,
        d.pwr_servo_current_ma as f32 / 10.0,
        d.charge_in_voltage_mv as f32 / 10.0,
        d.charge_in_current_ma as f32 / 10.0,
        d.bat_voltage_mv as f32 / 10.0,
        d.bat_current_ma as f32 / 10.0,
        d.bat_out1_current_ma as f32 / 10.0,
        d.bat_out2_current_ma as f32 / 10.0,
        d.pwr_5v_voltage_mv as f32 / 10.0,
        d.pwr_5v_current_ma as f32 / 10.0,
    )
}

pub fn power_json(d: &PowerData) -> Value {
    json!({
        "kind": "power",
        "pwr_servo_voltage_mv": d.pwr_servo_voltage_mv,
        "pwr_servo_current_ma": d.pwr_servo_current_ma,
        "charge_in_voltage_mv": d.charge_in_voltage_mv,
        "charge_in_current_ma": d.charge_in_current_ma,
        "bat_voltage_mv": d.bat_voltage_mv,
        "bat_current_ma": d.bat_current_ma,
        "bat_out1_current_ma": d.bat_out1_current_ma,
        "bat_out2_current_ma": d.bat_out2_current_ma,
        "pwr_5v_voltage_mv": d.pwr_5v_voltage_mv,
        "pwr_5v_current_ma": d.pwr_5v_current_ma,
    })
}

// ═══ 电池 ═══

pub fn battery_text(d: &BatteryState) -> String {
    format!(
        "soc      : {} %   voltage : {:.2} V   current : {:.2} A\n\
         temp     : {:.1} °C\n\
         status   : {:?}   health : {:?}   present : {}\n\
         capacity : {} / {} mAh   serial : 0x{:04X}\n\
         cells    : {} × {:.2} V [{}]",
        d.percentage,
        d.voltage_mv as f32 / 1000.0,
        d.current_ma as f32 / 1000.0,
        d.temperature as f32 / 10.0,
        d.charge_status,
        d.health,
        if d.present { "yes" } else { "no" },
        d.capacity_mah,
        d.design_capacity_mah,
        d.serial_number,
        d.cell_voltages_mv.len(),
        d.cell_voltages_mv.first().copied().unwrap_or(0) as f32 / 1000.0,
        d.cell_voltages_mv
            .iter()
            .map(|v| format!("{:.2}", *v as f32 / 1000.0))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn watch_battery(d: &BatteryState) -> String {
    format!(
        "battery soc={}% v={:.2}V i={:.2}A t={:.1}°C status={:?}",
        d.percentage,
        d.voltage_mv as f32 / 1000.0,
        d.current_ma as f32 / 1000.0,
        d.temperature as f32 / 10.0,
        d.charge_status,
    )
}

pub fn battery_json(d: &BatteryState) -> Value {
    json!({
        "kind": "battery",
        "voltage_mv": d.voltage_mv,
        "current_ma": d.current_ma,
        "capacity_mah": d.capacity_mah,
        "design_capacity_mah": d.design_capacity_mah,
        "percentage": d.percentage,
        "temperature_c": round1(d.temperature as f32 / 10.0),
        "charge_status": format!("{:?}", d.charge_status),
        "health": format!("{:?}", d.health),
        "technology": format!("{:?}", d.technology),
        "present": d.present,
        "serial_number": d.serial_number,
        "cell_voltages_mv": d.cell_voltages_mv,
        "cell_temperatures": d.cell_temperatures,
    })
}

// ═══ 事件 ═══

pub fn event_text(d: &BoardEvent) -> String {
    format!(
        "charge   : {:?}\n\
         state    : {}\n\
         protect  : {}\n\
         error    : {}",
        d.charge_phase,
        join_names(state_names(&d.state_change_flags)),
        join_names(protection_names(&d.protection_flags)),
        join_names(error_names(&d.error_flags)),
    )
}

pub fn watch_event(d: &BoardEvent) -> String {
    format!(
        "event charge={:?} state=[{}] prot=[{}] err=[{}]",
        d.charge_phase,
        join_names(state_names(&d.state_change_flags)),
        join_names(protection_names(&d.protection_flags)),
        join_names(error_names(&d.error_flags)),
    )
}

pub fn event_json(d: &BoardEvent) -> Value {
    json!({
        "kind": "event",
        "charge_phase": format!("{:?}", d.charge_phase),
        "state_change_flags": state_names(&d.state_change_flags),
        "protection_flags": protection_names(&d.protection_flags),
        "error_flags": error_names(&d.error_flags),
    })
}

fn join_names(names: Vec<&'static str>) -> String {
    if names.is_empty() {
        "(none)".into()
    } else {
        names.join(", ")
    }
}

fn state_names(f: &StateChangeFlags) -> Vec<&'static str> {
    f.iter_names().map(|(name, _)| name).collect()
}

fn protection_names(f: &ProtectionFlags) -> Vec<&'static str> {
    f.iter_names().map(|(name, _)| name).collect()
}

fn error_names(f: &ErrorFlags) -> Vec<&'static str> {
    f.iter_names().map(|(name, _)| name).collect()
}

// ═══ 诊断 ═══

pub fn diagnostic_text(d: &Diagnostic) -> String {
    format!(
        "uptime   : {} s   cpu : {} %   heap : {} KB   stack_min : {} KB\n\
         errors   : i2c={} spi={} uart={} usb={}\n\
         frames   : sent={}\n\
         pd       : {:.1} V / {:.1} A\n\
         temp     : servo_power={:.1} 5v={:.1} mcu={:.1} charge={:.1} battery={:.1} °C",
        d.uptime_s,
        d.cpu_usage_percent,
        d.free_heap_kb,
        d.stack_watermark_min_kb,
        d.i2c_error_count,
        d.spi_error_count,
        d.uart_error_count,
        d.usb_error_count,
        d.frames_sent_total,
        d.pd_request_voltage_mv as f32 / 1000.0,
        d.pd_request_current_ma as f32 / 1000.0,
        d.temp_servo_power as f32 / 10.0,
        d.temp_5v_power as f32 / 10.0,
        d.temp_mcu as f32 / 10.0,
        d.temp_charge as f32 / 10.0,
        d.temp_battery as f32 / 10.0,
    )
}

pub fn watch_diagnostic(d: &Diagnostic) -> String {
    format!(
        "diagnostic uptime={}s cpu={}% heap={}KB \
         temp=[{:.1} {:.1} {:.1} {:.1} {:.1}]°C",
        d.uptime_s,
        d.cpu_usage_percent,
        d.free_heap_kb,
        d.temp_servo_power as f32 / 10.0,
        d.temp_5v_power as f32 / 10.0,
        d.temp_mcu as f32 / 10.0,
        d.temp_charge as f32 / 10.0,
        d.temp_battery as f32 / 10.0,
    )
}

pub fn diagnostic_json(d: &Diagnostic) -> Value {
    json!({
        "kind": "diagnostic",
        "uptime_s": d.uptime_s,
        "cpu_usage_percent": d.cpu_usage_percent,
        "free_heap_kb": d.free_heap_kb,
        "stack_watermark_min_kb": d.stack_watermark_min_kb,
        "i2c_error_count": d.i2c_error_count,
        "spi_error_count": d.spi_error_count,
        "uart_error_count": d.uart_error_count,
        "usb_error_count": d.usb_error_count,
        "frames_sent_total": d.frames_sent_total,
        "pd_request_voltage_mv": d.pd_request_voltage_mv,
        "pd_request_current_ma": d.pd_request_current_ma,
        "temperatures_c": {
            "servo_power": round1(d.temp_servo_power as f32 / 10.0),
            "pwr_5v": round1(d.temp_5v_power as f32 / 10.0),
            "mcu": round1(d.temp_mcu as f32 / 10.0),
            "charge": round1(d.temp_charge as f32 / 10.0),
            "battery": round1(d.temp_battery as f32 / 10.0),
        },
    })
}

// ═══ 板级日志 ═══

pub fn log_text(ts: u64, m: &LogMessage) -> String {
    format!(
        "[{ts}] {} {}::{}: {}",
        m.level, m.file_name, m.fun_name, m.msg
    )
}

pub fn watch_log(m: &LogMessage) -> String {
    format!("log {} {}::{}: {}", m.level, m.file_name, m.fun_name, m.msg)
}

pub fn log_json(ts: u64, m: &LogMessage) -> Value {
    json!({
        "kind": "log",
        "ts": ts,
        "level": m.level.to_string(),
        "file": m.file_name,
        "function": m.fun_name,
        "message": m.msg,
    })
}
