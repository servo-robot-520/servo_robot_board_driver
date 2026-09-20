//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:30
//! Board Config

use crate::enum_with_from_u8;
use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use crate::log::LogLevel;
use alloc::vec::Vec;

enum_with_from_u8! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConfigType {
        EnableBatOut1            = 0x10 => "Servo Power",
        EnableBatOut2            = 0x11 => "Battery Extra Output",
        EnablePwr5V              = 0x12 => "5V Power",
        EnableCharge             = 0x13 => "Charge",
        EnableExtServoPower      = 0x14 => "Ext Servo Power",

        PwrBatOut1CurrentLimitMa = 0x20 => "Bat Out1 Current Limit",
        PwrBatOut2CurrentLimitMa = 0x21 => "Bat Out2 Current Limit",
        Pwr5VOutCurrentLimitMa   = 0x22 => "5V Out Current Limit",
        PwrServoCurrentLimitMa   = 0x23 => "Servo power out Current Limit",
        ChargeMinCurrentMa       = 0x24 => "Charge Min Current",
        ChargeMaxCurrentMa       = 0x25 => "Charge Max Current",

        PwrServoTempLimit        = 0x30 => "Servo Temp Limit",
        Pwr5vTempLimit           = 0x31 => "5V Temp Limit",
        ChargeTempDerating       = 0x32 => "Charge Temp Derating",
        ChargeTempLimit          = 0x33 => "Charge Temp Limit",

        // Set baud rate for serial port communication with servos, disable when set to 0
        ServoBaudRate            = 0x40 => "Servo Baud Rate",
        ChargeStopSoc            = 0x41 => "Charge Stop Soc",
        ChargeStopVoltageMv      = 0x42 => "Charge Stop Voltage",
        // servo robot board发送的日志等级
        TxLogLevel               = 0x43 => "TxLog Level",
        // Nan, BQ40Z50, BQ28Z610
        BMSIc                    = 0x44 => "BMS IC",
        // Nan, MPU6500, MPU6050
        IMUIc                    = 0x45 => "IMU IC",
    }
}

impl ConfigType {
    pub fn unit(&self) -> &'static str {
        match self {
            Self::PwrBatOut1CurrentLimitMa
            | Self::PwrBatOut2CurrentLimitMa
            | Self::Pwr5VOutCurrentLimitMa
            | Self::PwrServoCurrentLimitMa
            | Self::ChargeMinCurrentMa
            | Self::ChargeMaxCurrentMa => "mA",
            Self::PwrServoTempLimit
            | Self::Pwr5vTempLimit
            | Self::ChargeTempDerating
            | Self::ChargeTempLimit => "°C",
            Self::ChargeStopVoltageMv => "mV",
            Self::ChargeStopSoc => "%",
            _ => "",
        }
    }

    /// 值 payload 大小(不含 type 字节)。当前所有 ConfigType 都有值 payload,
    /// 保留 `Option` 以便未来加入无值命令(Reset/Shutdown 类)。
    pub fn value_size(&self) -> Option<usize> {
        match self {
            // Switches: 1 byte (bool)
            Self::EnableBatOut1
            | Self::EnableBatOut2
            | Self::EnablePwr5V
            | Self::EnableCharge
            // When the external servo power supply is enabled,
            // the ADC collects data from the corresponding channel to obtain the servo power supply voltage and power.
            | Self::EnableExtServoPower => Some(1),
            // u8 values: 1 byte
            Self::ChargeStopSoc | Self::TxLogLevel | Self::BMSIc | Self::IMUIc => Some(1),
            // u16 values: 2 bytes
            Self::PwrBatOut1CurrentLimitMa
            | Self::PwrBatOut2CurrentLimitMa
            | Self::Pwr5VOutCurrentLimitMa
            | Self::PwrServoCurrentLimitMa
            | Self::ChargeMinCurrentMa
            | Self::ChargeMaxCurrentMa
            | Self::PwrServoTempLimit
            | Self::Pwr5vTempLimit
            | Self::ChargeTempDerating
            | Self::ChargeTempLimit
            | Self::ChargeStopVoltageMv => Some(2),
            // u32 values: 4 bytes
            Self::ServoBaudRate => Some(4),
        }
    }
}

/// Configuration values
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Config {
    // Switch the servo power supply
    EnableBatOut1(bool),
    // Switch 5v power supply
    EnablePwr5V(bool),
    // Switch on and off to charge the battery
    EnableCharge(bool),
    // Switching on battery extra output
    EnableBatOut2(bool),
    // Charging capacity limit, such as charging only up to 80%
    ChargeStopSoc(u8),
    // The log level of to send
    TxLogLevel(LogLevel),
    // Servo power supply current limiting
    PowerServoCurrentLimitMa(u16),
    // Servo power supply temperature restriction
    PowerServoTempLimit(u16),
    // 5V power temperature limit
    Power5vTempLimit(u16),
    // Maximum charging current
    ChargeMaxCurrentMa(u16),
    // The temperature of the charging circuit when charging starts to drop current
    ChargeTempDerating(u16),
    // The temperature of the charging circuit when charging is stopped
    ChargeTempLimit(u16),
    // Charging stop-voltage range
    ChargeStopVoltageMv(u16),
    // Set baud rate for serial port communication with servos
    ServoBaudRate(u32),
    // Battery extra output current limiting
    PowerBatOut2CurrentLimitMa(u16),
    // 5V output current limiting
    Power5VOutCurrentLimitMa(u16),
    // Servo power output current limiting
    PowerServoOutCurrentLimitMa(u16),
    // Minimum charging current
    ChargeMinCurrentMa(u16),
    // BMS IC type (0=NaN, 1=BQ40Z50, 2=BQ28Z10)
    BMSIc(u8),
    // IMU IC type (0=NaN, 1=MPU6500, 2=MPU6050)
    IMUIc(u8),
}

impl Config {
    pub fn config_type(&self) -> ConfigType {
        match self {
            Self::EnableBatOut1(_) => ConfigType::EnableBatOut1,
            Self::EnablePwr5V(_) => ConfigType::EnablePwr5V,
            Self::EnableCharge(_) => ConfigType::EnableCharge,
            Self::EnableBatOut2(_) => ConfigType::EnableBatOut2,
            Self::ChargeStopSoc(_) => ConfigType::ChargeStopSoc,
            Self::TxLogLevel(_) => ConfigType::TxLogLevel,
            Self::PowerServoCurrentLimitMa(_) => ConfigType::PwrBatOut1CurrentLimitMa,
            Self::PowerServoTempLimit(_) => ConfigType::PwrServoTempLimit,
            Self::Power5vTempLimit(_) => ConfigType::Pwr5vTempLimit,
            Self::ChargeMaxCurrentMa(_) => ConfigType::ChargeMaxCurrentMa,
            Self::ChargeTempDerating(_) => ConfigType::ChargeTempDerating,
            Self::ChargeTempLimit(_) => ConfigType::ChargeTempLimit,
            Self::ChargeStopVoltageMv(_) => ConfigType::ChargeStopVoltageMv,
            Self::ServoBaudRate(_) => ConfigType::ServoBaudRate,
            Self::PowerBatOut2CurrentLimitMa(_) => ConfigType::PwrBatOut2CurrentLimitMa,
            Self::Power5VOutCurrentLimitMa(_) => ConfigType::Pwr5VOutCurrentLimitMa,
            Self::PowerServoOutCurrentLimitMa(_) => ConfigType::PwrServoCurrentLimitMa,
            Self::ChargeMinCurrentMa(_) => ConfigType::ChargeMinCurrentMa,
            Self::BMSIc(_) => ConfigType::BMSIc,
            Self::IMUIc(_) => ConfigType::IMUIc,
        }
    }

    pub fn value(&self) -> f32 {
        match self {
            Self::EnableBatOut1(on)
            | Self::EnablePwr5V(on)
            | Self::EnableCharge(on)
            | Self::EnableBatOut2(on) => {
                if *on {
                    1.0
                } else {
                    0.0
                }
            }
            Self::ChargeStopSoc(v) => *v as f32,
            Self::TxLogLevel(level) => *level as u8 as f32,
            Self::PowerServoCurrentLimitMa(v)
            | Self::ChargeStopVoltageMv(v)
            | Self::ChargeMaxCurrentMa(v)
            | Self::PowerServoTempLimit(v)
            | Self::Power5vTempLimit(v)
            | Self::ChargeTempDerating(v)
            | Self::ChargeTempLimit(v) => *v as f32,
            Self::ServoBaudRate(v) => *v as f32,
            Self::PowerBatOut2CurrentLimitMa(v)
            | Self::Power5VOutCurrentLimitMa(v)
            | Self::PowerServoOutCurrentLimitMa(v)
            | Self::ChargeMinCurrentMa(v) => *v as f32,
            Self::BMSIc(v) | Self::IMUIc(v) => *v as f32,
        }
    }

    pub fn from_type_value(typ: ConfigType, value: f32) -> Self {
        match typ {
            ConfigType::EnableBatOut1 => Self::EnableBatOut1(value != 0.0),
            ConfigType::EnablePwr5V => Self::EnablePwr5V(value != 0.0),
            ConfigType::EnableCharge => Self::EnableCharge(value != 0.0),
            ConfigType::EnableBatOut2 => Self::EnableBatOut2(value != 0.0),
            ConfigType::EnableExtServoPower => Self::EnablePwr5V(value != 0.0),
            ConfigType::ChargeStopSoc => Self::ChargeStopSoc(value as _),
            ConfigType::TxLogLevel => Self::TxLogLevel(LogLevel::from_u8(value as _)),
            ConfigType::PwrBatOut1CurrentLimitMa => Self::PowerServoCurrentLimitMa(value as _),
            ConfigType::PwrServoTempLimit => Self::PowerServoTempLimit(value as _),
            ConfigType::Pwr5vTempLimit => Self::Power5vTempLimit(value as _),
            ConfigType::ChargeMaxCurrentMa => Self::ChargeMaxCurrentMa(value as _),
            ConfigType::ChargeTempDerating => Self::ChargeTempDerating(value as _),
            ConfigType::ChargeTempLimit => Self::ChargeTempLimit(value as _),
            ConfigType::ChargeStopVoltageMv => Self::ChargeStopVoltageMv(value as _),
            ConfigType::ServoBaudRate => Self::ServoBaudRate(value as _),
            ConfigType::PwrBatOut2CurrentLimitMa => Self::PowerBatOut2CurrentLimitMa(value as _),
            ConfigType::Pwr5VOutCurrentLimitMa => Self::Power5VOutCurrentLimitMa(value as _),
            ConfigType::PwrServoCurrentLimitMa => Self::PowerServoOutCurrentLimitMa(value as _),
            ConfigType::ChargeMinCurrentMa => Self::ChargeMinCurrentMa(value as _),
            ConfigType::BMSIc => Self::BMSIc(value as _),
            ConfigType::IMUIc => Self::IMUIc(value as _),
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.is_empty() {
            return Err(FrameError::PayloadTooShort {
                expected: 1,
                got: 0,
            });
        }
        let config_type =
            ConfigType::from_u8(data[0]).ok_or(FrameError::PayloadDecode("Unknown config type"))?;

        // No special handling needed — all remaining ConfigType variants have value payloads

        // Determine required payload size based on config type
        let value_len = match config_type {
            // Switches: 1 byte (bool)
            ConfigType::EnableBatOut1
            | ConfigType::EnablePwr5V
            | ConfigType::EnableCharge
            | ConfigType::EnableBatOut2
            | ConfigType::EnableExtServoPower => 1,
            // ChargeStopSoc: 1 byte (u8)
            ConfigType::ChargeStopSoc => 1,
            // TxLogLevel: 1 byte (u8)
            ConfigType::TxLogLevel => 1,
            // u8 configs: 1 byte
            ConfigType::BMSIc | ConfigType::IMUIc => 1,
            // u16 configs: 2 bytes
            ConfigType::PwrBatOut1CurrentLimitMa
            | ConfigType::PwrBatOut2CurrentLimitMa
            | ConfigType::Pwr5VOutCurrentLimitMa
            | ConfigType::PwrServoCurrentLimitMa
            | ConfigType::ChargeMinCurrentMa
            | ConfigType::PwrServoTempLimit
            | ConfigType::Pwr5vTempLimit
            | ConfigType::ChargeMaxCurrentMa
            | ConfigType::ChargeTempDerating
            | ConfigType::ChargeTempLimit
            | ConfigType::ChargeStopVoltageMv => 2,
            // u32 config: 4 bytes
            ConfigType::ServoBaudRate => 4,
        };

        let total = 1 + value_len; // type byte + value bytes
        if data.len() < total {
            return Err(FrameError::PayloadTooShort {
                expected: total,
                got: data.len(),
            });
        }

        let value = match value_len {
            1 => data[1] as u32,
            2 => u16::from_le_bytes([data[1], data[2]]) as u32,
            4 => u32::from_le_bytes([data[1], data[2], data[3], data[4]]),
            _ => unreachable!(),
        };

        Ok(Config::from_type_value(config_type, value as _))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.push(self.config_type() as u8);
        match self {
            Self::EnableBatOut1(on)
            | Self::EnablePwr5V(on)
            | Self::EnableCharge(on)
            | Self::EnableBatOut2(on) => buf.push(*on as u8),
            Self::ChargeStopSoc(v) => buf.push(*v),
            Self::TxLogLevel(level) => buf.push(*level as u8),
            Self::PowerServoCurrentLimitMa(v)
            | Self::PowerServoTempLimit(v)
            | Self::Power5vTempLimit(v)
            | Self::ChargeMaxCurrentMa(v)
            | Self::ChargeTempDerating(v)
            | Self::ChargeTempLimit(v)
            | Self::ChargeStopVoltageMv(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::ServoBaudRate(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::PowerBatOut2CurrentLimitMa(v)
            | Self::Power5VOutCurrentLimitMa(v)
            | Self::PowerServoOutCurrentLimitMa(v)
            | Self::ChargeMinCurrentMa(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::BMSIc(v) | Self::IMUIc(v) => buf.push(*v),
        }
        buf
    }
}

impl ToPayload for Config {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for Config {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

/// Snapshot of board-level configuration
///
/// Field order matches ConfigType enum values:
/// - Switches (0x10~0x13): enable_bat_ou1, enable_bat_out2, enable_pwr_5v, enable_charge
/// - Current limits (0x20~0x25): bat_out1/2, 5v, servo current limits, charge min/max current
/// - Temp limits (0x30~0x33): servo/5v temp limits, charge temp derating/limit
/// - Misc (0x40~0x45): baud rate, charge stop soc/voltage, tx log level, BMS/IMU IC
#[derive(Debug, Clone)]
pub struct BoardConfigSnapshot {
    // === Switches (0x10~0x13) ===
    pub enable_bat_ou1: bool,
    pub enable_bat_out2: bool,
    pub enable_pwr_5v: bool,
    pub enable_charge: bool,
    pub enable_ext_servo_power: bool,
    // === Current limits (0x20~0x25) ===
    /// Servo power supply current limit (mA)
    pub servo_current_limit_ma: u16,
    /// Battery extra output current limit (mA)
    pub bat_out2_current_limit_ma: u16,
    /// 5V output current limit (mA)
    pub pwr_5v_out_current_limit_ma: u16,
    /// Servo power output current limit (mA)
    pub servo_out_current_limit_ma: u16,
    /// Minimum charging current (mA)
    pub charge_min_current_ma: u16,
    /// Maximum charging current (mA)
    pub charge_max_current_ma: u16,
    // === Temp limits (0x30~0x33) ===
    /// Servo power supply temperature limit (×10)
    pub pwr_servo_temp_limit: u16,
    /// 5V power temperature limit (×10)
    pub pwr_5v_temp_limit: u16,
    /// Charging temperature derating threshold (×10)
    pub charge_temp_derating: u16,
    /// Charging temperature limit (×10)
    pub charge_temp_limit: u16,
    // === Misc (0x40~0x45) ===
    /// STM32 servo communication baud rate
    pub servo_baud_rate: u32,
    /// Charging capacity limit (1~100)
    pub charge_stop_percentage: u8,
    /// Charging stop voltage (mV)
    pub charge_stop_voltage_mv: u16,
    /// Board log level
    pub tx_log_level: LogLevel,
    /// BMS IC type (0=NaN, 1=BQ40Z50, 2=BQ28Z10)
    pub bms_ic: u8,
    /// IMU IC type (0=NaN, 1=MPU6500, 2=MPU6050)
    pub imu_ic: u8,
}

impl Default for BoardConfigSnapshot {
    fn default() -> Self {
        BoardConfigSnapshot {
            // Switches
            enable_bat_ou1: true,
            enable_bat_out2: true,
            enable_pwr_5v: true,
            enable_charge: true,
            enable_ext_servo_power: true,
            // Current limits
            servo_current_limit_ma: 50,
            bat_out2_current_limit_ma: 0,
            pwr_5v_out_current_limit_ma: 0,
            servo_out_current_limit_ma: 0,
            charge_min_current_ma: 0,
            charge_max_current_ma: 90,
            // Temp limits
            pwr_servo_temp_limit: 800,
            pwr_5v_temp_limit: 700,
            charge_temp_derating: 600,
            charge_temp_limit: 700,
            // Misc
            servo_baud_rate: 115200,
            charge_stop_percentage: 100,
            charge_stop_voltage_mv: 168,
            tx_log_level: LogLevel::Info,
            bms_ic: 0,
            imu_ic: 0,
        }
    }
}

impl BoardConfigSnapshot {
    /// Payload size: 4 bool + 4 u8 + 11 u16 + 1 u32 = 34 bytes
    const PAYLOAD_SIZE: usize = 34;

    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < Self::PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooShort {
                expected: Self::PAYLOAD_SIZE,
                got: data.len(),
            });
        }
        let mut o = 0;

        // === Switches (0x10~0x13) — 4 bytes ===
        let enable_bat_ou1 = data[o] != 0;
        o += 1;
        let enable_bat_out2 = data[o] != 0;
        o += 1;
        let enable_pwr_5v = data[o] != 0;
        o += 1;
        let enable_charge = data[o] != 0;
        o += 1;
        let enable_ext_servo_power = data[o] != 0;
        o += 1;

        // === Current limits (0x20~0x25) — 6×u16 = 12 bytes ===
        let servo_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let bat_out2_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let pwr_5v_out_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let servo_out_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_min_current_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_max_current_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;

        // === Temp limits (0x30~0x33) — 4×u16 = 8 bytes ===
        let pwr_servo_temp_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let pwr_5v_temp_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_temp_derating = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_temp_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;

        // === Misc (0x40~0x45) — u32 + u8 + u16 + u8 + u8 = 11 bytes ===
        let servo_baud_rate = u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        o += 4;
        let charge_stop_percentage = data[o];
        o += 1;
        let charge_stop_voltage_mv = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let tx_log_level = LogLevel::from_u8(data[o]);
        o += 1;
        let bms_ic = data[o];
        o += 1;
        let imu_ic = data[o];
        let _ = o; // suppress unused warning

        Ok(BoardConfigSnapshot {
            enable_bat_ou1,
            enable_bat_out2,
            enable_pwr_5v,
            enable_charge,
            enable_ext_servo_power,
            servo_current_limit_ma,
            bat_out2_current_limit_ma,
            pwr_5v_out_current_limit_ma,
            servo_out_current_limit_ma,
            charge_min_current_ma,
            charge_max_current_ma,
            pwr_servo_temp_limit,
            pwr_5v_temp_limit,
            charge_temp_derating,
            charge_temp_limit,
            servo_baud_rate,
            charge_stop_percentage,
            charge_stop_voltage_mv,
            tx_log_level,
            bms_ic,
            imu_ic,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::PAYLOAD_SIZE);

        // === Switches (0x10~0x13) ===
        buf.push(self.enable_bat_ou1 as u8);
        buf.push(self.enable_bat_out2 as u8);
        buf.push(self.enable_pwr_5v as u8);
        buf.push(self.enable_charge as u8);
        buf.push(self.enable_ext_servo_power as u8);

        // === Current limits (0x20~0x25) ===
        buf.extend_from_slice(&self.servo_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out2_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_out_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.servo_out_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_min_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_max_current_ma.to_le_bytes());

        // === Temp limits (0x30~0x33) ===
        buf.extend_from_slice(&self.pwr_servo_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_derating.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_limit.to_le_bytes());

        // === Misc (0x40~0x45) ===
        buf.extend_from_slice(&self.servo_baud_rate.to_le_bytes());
        buf.push(self.charge_stop_percentage);
        buf.extend_from_slice(&self.charge_stop_voltage_mv.to_le_bytes());
        buf.push(self.tx_log_level as u8);
        buf.push(self.bms_ic);
        buf.push(self.imu_ic);

        buf
    }
}

impl ToPayload for BoardConfigSnapshot {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for BoardConfigSnapshot {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for BoardConfigSnapshot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "sw=[{},{},{},{},{}] \
             cur=[{},{},{},{},{},{:.1}A] \
             temp=[{:.1},{:.1},{:.1},{:.1}]°C \
             lvl={} bms={} imu={} \
             chg={:.1}mV/{}% baud={}",
            if self.enable_bat_ou1 { "S" } else { "-" },
            if self.enable_bat_out2 { "B" } else { "-" },
            if self.enable_pwr_5v { "5" } else { "-" },
            if self.enable_charge { "C" } else { "-" },
            self.charge_stop_percentage,
            self.servo_current_limit_ma,
            self.bat_out2_current_limit_ma,
            self.pwr_5v_out_current_limit_ma,
            self.servo_out_current_limit_ma,
            self.charge_min_current_ma,
            self.charge_max_current_ma as f32,
            self.pwr_servo_temp_limit as f32 / 10.0,
            self.pwr_5v_temp_limit as f32 / 10.0,
            self.charge_temp_derating as f32 / 10.0,
            self.charge_temp_limit as f32 / 10.0,
            self.tx_log_level as u8,
            self.bms_ic,
            self.imu_ic,
            self.charge_stop_voltage_mv,
            self.charge_stop_percentage,
            self.servo_baud_rate,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_type_value_size() {
        // Test that value_size() returns correct sizes
        assert_eq!(ConfigType::EnableBatOut1.value_size(), Some(1));
        assert_eq!(ConfigType::ChargeStopSoc.value_size(), Some(1));
        assert_eq!(ConfigType::PwrBatOut1CurrentLimitMa.value_size(), Some(2));
        assert_eq!(ConfigType::ServoBaudRate.value_size(), Some(4));
    }

    #[test]
    fn test_config_encode_decode_roundtrip() {
        // Test Config enum encoding/decoding — covers all variants
        let configs = vec![
            // Switches (bool)
            Config::EnableBatOut1(true),
            Config::EnableBatOut1(false),
            Config::EnablePwr5V(true),
            Config::EnableCharge(false),
            Config::EnableBatOut2(true),
            // u8 values
            Config::ChargeStopSoc(80),
            Config::BMSIc(1),
            Config::IMUIc(2),
            // u16 values
            Config::PowerServoCurrentLimitMa(50),
            Config::PowerBatOut2CurrentLimitMa(200),
            Config::Power5VOutCurrentLimitMa(500),
            Config::PowerServoOutCurrentLimitMa(100),
            Config::ChargeMinCurrentMa(10),
            Config::ChargeMaxCurrentMa(90),
            Config::PowerServoTempLimit(800),
            Config::Power5vTempLimit(700),
            Config::ChargeTempDerating(600),
            Config::ChargeTempLimit(700),
            Config::ChargeStopVoltageMv(168),
            // u32 value
            Config::ServoBaudRate(115200),
        ];

        for config in configs {
            let bytes = config.to_bytes();
            let decoded = Config::from_bytes(&bytes).unwrap();
            assert_eq!(config, decoded, "Failed for {:?}", config);
        }
    }

    #[test]
    fn test_board_config_snapshot_encode_decode_roundtrip() {
        let config = BoardConfigSnapshot::default();
        let bytes = config.to_bytes();
        assert_eq!(bytes.len(), BoardConfigSnapshot::PAYLOAD_SIZE);

        let decoded = BoardConfigSnapshot::from_bytes(&bytes).unwrap();
        // Switches
        assert_eq!(config.enable_bat_ou1, decoded.enable_bat_ou1);
        assert_eq!(config.enable_bat_out2, decoded.enable_bat_out2);
        assert_eq!(config.enable_pwr_5v, decoded.enable_pwr_5v);
        assert_eq!(config.enable_charge, decoded.enable_charge);
        // Current limits
        assert_eq!(
            config.servo_current_limit_ma,
            decoded.servo_current_limit_ma
        );
        assert_eq!(
            config.bat_out2_current_limit_ma,
            decoded.bat_out2_current_limit_ma
        );
        assert_eq!(
            config.pwr_5v_out_current_limit_ma,
            decoded.pwr_5v_out_current_limit_ma
        );
        assert_eq!(
            config.servo_out_current_limit_ma,
            decoded.servo_out_current_limit_ma
        );
        assert_eq!(config.charge_min_current_ma, decoded.charge_min_current_ma);
        assert_eq!(config.charge_max_current_ma, decoded.charge_max_current_ma);
        // Temp limits
        assert_eq!(config.pwr_servo_temp_limit, decoded.pwr_servo_temp_limit);
        assert_eq!(config.pwr_5v_temp_limit, decoded.pwr_5v_temp_limit);
        assert_eq!(config.charge_temp_derating, decoded.charge_temp_derating);
        assert_eq!(config.charge_temp_limit, decoded.charge_temp_limit);
        // Misc
        assert_eq!(config.servo_baud_rate, decoded.servo_baud_rate);
        assert_eq!(
            config.charge_stop_percentage,
            decoded.charge_stop_percentage
        );
        assert_eq!(
            config.charge_stop_voltage_mv,
            decoded.charge_stop_voltage_mv
        );
        assert_eq!(config.bms_ic, decoded.bms_ic);
        assert_eq!(config.imu_ic, decoded.imu_ic);
    }

    #[test]
    fn test_board_config_snapshot_custom_values() {
        let config = BoardConfigSnapshot {
            enable_bat_ou1: true,
            enable_bat_out2: false,
            enable_pwr_5v: false,
            enable_charge: true,
            enable_ext_servo_power: true,
            servo_current_limit_ma: 100,
            bat_out2_current_limit_ma: 300,
            pwr_5v_out_current_limit_ma: 500,
            servo_out_current_limit_ma: 150,
            charge_min_current_ma: 10,
            charge_max_current_ma: 200,
            pwr_servo_temp_limit: 850,
            pwr_5v_temp_limit: 750,
            charge_temp_derating: 650,
            charge_temp_limit: 750,
            servo_baud_rate: 921600,
            charge_stop_percentage: 80,
            charge_stop_voltage_mv: 168,
            tx_log_level: LogLevel::Debug,
            bms_ic: 1,
            imu_ic: 2,
        };

        let bytes = config.to_bytes();
        assert_eq!(bytes.len(), BoardConfigSnapshot::PAYLOAD_SIZE);
        let decoded = BoardConfigSnapshot::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.enable_bat_ou1, true);
        assert_eq!(decoded.enable_bat_out2, false);
        assert_eq!(decoded.enable_pwr_5v, false);
        assert_eq!(decoded.enable_charge, true);
        assert_eq!(decoded.enable_ext_servo_power, true);
        assert_eq!(decoded.servo_current_limit_ma, 100);
        assert_eq!(decoded.bat_out2_current_limit_ma, 300);
        assert_eq!(decoded.pwr_5v_out_current_limit_ma, 500);
        assert_eq!(decoded.servo_out_current_limit_ma, 150);
        assert_eq!(decoded.charge_min_current_ma, 10);
        assert_eq!(decoded.charge_max_current_ma, 200);
        assert_eq!(decoded.pwr_servo_temp_limit, 850);
        assert_eq!(decoded.pwr_5v_temp_limit, 750);
        assert_eq!(decoded.charge_temp_derating, 650);
        assert_eq!(decoded.charge_temp_limit, 750);
        assert_eq!(decoded.charge_stop_voltage_mv, 168);
        assert_eq!(decoded.charge_stop_percentage, 80);
        assert_eq!(decoded.servo_baud_rate, 921600);
        assert_eq!(decoded.bms_ic, 1);
        assert_eq!(decoded.imu_ic, 2);
    }
}
