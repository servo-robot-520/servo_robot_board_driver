//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:30
//! Board Config

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use crate::log::LogLevel;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConfigType {
    SwitchServoPower = 0x10,
    Switch5VPower = 0x11,
    SwitchCharge = 0x12,
    SwitchBatExtOut = 0x13,
    ChargeStopSoc = 0x20,
    // servo robot board发送的日志等级
    TxLogLevel = 0x21,
    PowerServoCurrentLimitMa = 0x30,
    PowerServoTempLimit = 0x31,
    Power5vTempLimit = 0x32,
    ChargeMaxCurrentMa = 0x33,
    ChargeTempDerating = 0x34,
    ChargeTempLimit = 0x35,
    ChargeStopVoltageMv = 0x36,
    // Set baud rate for serial port communication with servos
    ServoBaudRate = 0x37,
}

impl ConfigType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x10 => Some(Self::SwitchServoPower),
            0x11 => Some(Self::Switch5VPower),
            0x12 => Some(Self::SwitchCharge),
            0x13 => Some(Self::SwitchBatExtOut),
            0x20 => Some(Self::ChargeStopSoc),
            0x21 => Some(Self::TxLogLevel),
            0x30 => Some(Self::PowerServoCurrentLimitMa),
            0x31 => Some(Self::PowerServoTempLimit),
            0x32 => Some(Self::Power5vTempLimit),
            0x33 => Some(Self::ChargeMaxCurrentMa),
            0x34 => Some(Self::ChargeTempDerating),
            0x35 => Some(Self::ChargeTempLimit),
            0x36 => Some(Self::ChargeStopVoltageMv),
            0x37 => Some(Self::ServoBaudRate),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::SwitchServoPower => "Servo Power",
            Self::Switch5VPower => "5V Power",
            Self::SwitchCharge => "Charge",
            Self::SwitchBatExtOut => "Battery Extra Output",
            Self::ChargeStopSoc => "Charge Stop Soc",
            Self::TxLogLevel => "TxLog Level",
            Self::PowerServoCurrentLimitMa => "Servo Current Limit(ma)",
            Self::PowerServoTempLimit => "Servo Temp Limit",
            Self::Power5vTempLimit => "5V Temp Limit",
            Self::ChargeMaxCurrentMa => "Charge Max Current(ma)",
            Self::ChargeTempDerating => "Charge Temp Derating",
            Self::ChargeTempLimit => "Charge Temp Limit",
            Self::ChargeStopVoltageMv => "Charge Stop Voltage(mv)",
            Self::ServoBaudRate => "Servo Baud Rate",
        }
    }

    pub fn unit(&self) -> &'static str {
        match self {
            Self::PowerServoCurrentLimitMa | Self::ChargeMaxCurrentMa => "mA",
            Self::PowerServoTempLimit
            | Self::Power5vTempLimit
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
            Self::SwitchServoPower
            | Self::Switch5VPower
            | Self::SwitchCharge
            | Self::SwitchBatExtOut => Some(1),
            // u8 values: 1 byte
            Self::ChargeStopSoc | Self::TxLogLevel => Some(1),
            // u16 values: 2 bytes
            Self::PowerServoCurrentLimitMa
            | Self::PowerServoTempLimit
            | Self::Power5vTempLimit
            | Self::ChargeMaxCurrentMa
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
    SwitchPowerServo(bool),
    // Switch 5v power supply
    SwitchPower5V(bool),
    // Switch on and off to charge the battery
    SwitchCharge(bool),
    // Switching on battery extra output
    SwitchBatExtOut(bool),
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
}

impl Config {
    pub fn config_type(&self) -> ConfigType {
        match self {
            Self::SwitchPowerServo(_) => ConfigType::SwitchServoPower,
            Self::SwitchPower5V(_) => ConfigType::Switch5VPower,
            Self::SwitchCharge(_) => ConfigType::SwitchCharge,
            Self::SwitchBatExtOut(_) => ConfigType::SwitchBatExtOut,
            Self::ChargeStopSoc(_) => ConfigType::ChargeStopSoc,
            Self::TxLogLevel(_) => ConfigType::TxLogLevel,
            Self::PowerServoCurrentLimitMa(_) => ConfigType::PowerServoCurrentLimitMa,
            Self::PowerServoTempLimit(_) => ConfigType::PowerServoTempLimit,
            Self::Power5vTempLimit(_) => ConfigType::Power5vTempLimit,
            Self::ChargeMaxCurrentMa(_) => ConfigType::ChargeMaxCurrentMa,
            Self::ChargeTempDerating(_) => ConfigType::ChargeTempDerating,
            Self::ChargeTempLimit(_) => ConfigType::ChargeTempLimit,
            Self::ChargeStopVoltageMv(_) => ConfigType::ChargeStopVoltageMv,
            Self::ServoBaudRate(_) => ConfigType::ServoBaudRate,
        }
    }

    pub fn value(&self) -> f32 {
        match self {
            Self::SwitchPowerServo(on)
            | Self::SwitchPower5V(on)
            | Self::SwitchCharge(on)
            | Self::SwitchBatExtOut(on) => {
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
        }
    }

    pub fn from_type_value(typ: ConfigType, value: f32) -> Self {
        match typ {
            ConfigType::SwitchServoPower => Self::SwitchPowerServo(value != 0.0),
            ConfigType::Switch5VPower => Self::SwitchPower5V(value != 0.0),
            ConfigType::SwitchCharge => Self::SwitchCharge(value != 0.0),
            ConfigType::SwitchBatExtOut => Self::SwitchBatExtOut(value != 0.0),
            ConfigType::ChargeStopSoc => Self::ChargeStopSoc(value as _),
            ConfigType::TxLogLevel => Self::TxLogLevel(LogLevel::from_u8(value as _)),
            ConfigType::PowerServoCurrentLimitMa => Self::PowerServoCurrentLimitMa(value as _),
            ConfigType::PowerServoTempLimit => Self::PowerServoTempLimit(value as _),
            ConfigType::Power5vTempLimit => Self::Power5vTempLimit(value as _),
            ConfigType::ChargeMaxCurrentMa => Self::ChargeMaxCurrentMa(value as _),
            ConfigType::ChargeTempDerating => Self::ChargeTempDerating(value as _),
            ConfigType::ChargeTempLimit => Self::ChargeTempLimit(value as _),
            ConfigType::ChargeStopVoltageMv => Self::ChargeStopVoltageMv(value as _),
            ConfigType::ServoBaudRate => Self::ServoBaudRate(value as _),
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
            ConfigType::SwitchServoPower
            | ConfigType::Switch5VPower
            | ConfigType::SwitchCharge
            | ConfigType::SwitchBatExtOut => 1,
            // ChargeStopSoc: 1 byte (u8)
            ConfigType::ChargeStopSoc => 1,
            // TxLogLevel: 1 byte (u8)
            ConfigType::TxLogLevel => 1,
            // u16 configs: 2 bytes
            ConfigType::PowerServoCurrentLimitMa
            | ConfigType::PowerServoTempLimit
            | ConfigType::Power5vTempLimit
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
            Self::SwitchPowerServo(on)
            | Self::SwitchPower5V(on)
            | Self::SwitchCharge(on)
            | Self::SwitchBatExtOut(on) => buf.push(*on as u8),
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
/// - Switches (0x10~0x13): power_servo_on, power_5v_on, charge_on, bat_ext_out_on
/// - Charge settings (0x20~0x21): charge_stop_percentage, tx_log_level
/// - Limits (0x30~0x37): servo_current_limit_ma, servo_temp_limit, temp_5v_limit,
///   charge_max_current_ma, charge_temp_derating, charge_temp_limit, charge_stop_voltage_mv,
///   set_servo_baud_rate
#[derive(Debug, Clone)]
pub struct BoardConfigSnapshot {
    // === Switches (0x10~0x13) ===
    pub power_servo_on: bool,
    pub power_5v_on: bool,
    pub charge_on: bool,
    pub bat_ext_out_on: bool,
    // === Charge settings (0x20~0x21) ===
    /// Charging capacity limit (1~100)
    pub charge_stop_percentage: u8,
    pub tx_log_level: LogLevel,
    // === Limits (0x30~0x37) ===
    /// Servo power supply current limit (mA)
    pub servo_current_limit_ma: u16,
    /// Servo power supply temperature limit (×10)
    pub servo_temp_limit: u16,
    /// 5V power temperature limit (×10)
    pub temp_5v_limit: u16,
    /// Maximum charging current (mA)
    pub charge_max_current_ma: u16,
    /// Charging temperature derating threshold (×10)
    pub charge_temp_derating: u16,
    /// Charging temperature limit (×10)
    pub charge_temp_limit: u16,
    /// Charging stop voltage (mV)
    pub charge_stop_voltage_mv: u16,
    /// STM32 servo communication baud rate
    pub servo_baud_rate: u32,
}

impl Default for BoardConfigSnapshot {
    fn default() -> Self {
        BoardConfigSnapshot {
            // Switches
            power_servo_on: true,
            power_5v_on: true,
            charge_on: true,
            bat_ext_out_on: true,
            // Charge settings
            charge_stop_percentage: 100,
            tx_log_level: LogLevel::Info,
            // Limits
            servo_current_limit_ma: 50,
            servo_temp_limit: 800,
            temp_5v_limit: 700,
            charge_max_current_ma: 90,
            charge_temp_derating: 600,
            charge_temp_limit: 700,
            charge_stop_voltage_mv: 168,
            servo_baud_rate: 115200,
        }
    }
}

impl BoardConfigSnapshot {
    /// Payload size: 4 bool + 2 u8 + 7 u16 + 1 u32 = 24 bytes
    const PAYLOAD_SIZE: usize = 24;

    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < Self::PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooShort {
                expected: Self::PAYLOAD_SIZE,
                got: data.len(),
            });
        }
        let mut o = 0;

        // === Switches (0x10~0x13) ===
        let power_servo_on = data[o] != 0;
        o += 1;
        let power_5v_on = data[o] != 0;
        o += 1;
        let charge_on = data[o] != 0;
        o += 1;
        let bat_ext_out_on = data[o] != 0;
        o += 1;

        // === Charge settings (0x20~0x21) ===
        let charge_stop_percentage = data[o];
        o += 1;
        let tx_log_level = LogLevel::from_u8(data[o]);
        o += 1;

        // === Limits (0x30~0x37) ===
        let servo_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let servo_temp_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_5v_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_max_current_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_temp_derating = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_temp_limit = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_stop_voltage_mv = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let set_servo_baud_rate =
            u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);

        Ok(BoardConfigSnapshot {
            power_servo_on,
            power_5v_on,
            charge_on,
            bat_ext_out_on,
            charge_stop_percentage,
            tx_log_level,
            servo_current_limit_ma,
            servo_temp_limit,
            temp_5v_limit,
            charge_max_current_ma,
            charge_temp_derating,
            charge_temp_limit,
            charge_stop_voltage_mv,
            servo_baud_rate: set_servo_baud_rate,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::PAYLOAD_SIZE);

        // === Switches (0x10~0x13) ===
        buf.push(self.power_servo_on as u8);
        buf.push(self.power_5v_on as u8);
        buf.push(self.charge_on as u8);
        buf.push(self.bat_ext_out_on as u8);

        // === Charge settings (0x20~0x21) ===
        buf.push(self.charge_stop_percentage);
        buf.push(self.tx_log_level as u8);

        // === Limits (0x30~0x37) ===
        buf.extend_from_slice(&self.servo_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.servo_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.temp_5v_limit.to_le_bytes());
        buf.extend_from_slice(&self.charge_max_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_derating.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.charge_stop_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.servo_baud_rate.to_le_bytes());

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
        // Convert u16 (*10) to f32 for display
        write!(
            f,
            "sw=[{},{},{},{},{}] lvl={} servo={:.1}mA/{:.1}°C 5v={:.1}°C chg={:.1}A/{:.1}-{:.1}°C/{:.1}mV/{}% baud={}",
            if self.power_servo_on { "S" } else { "-" },
            if self.power_5v_on { "5" } else { "-" },
            if self.charge_on { "C" } else { "-" },
            if self.bat_ext_out_on { "B" } else { "-" },
            if self.charge_stop_percentage < 100 {
                "L"
            } else {
                "-"
            },
            self.tx_log_level as u8,
            self.servo_current_limit_ma,
            self.servo_temp_limit as f32 / 10.0,
            self.temp_5v_limit as f32 / 10.0,
            self.charge_max_current_ma,
            self.charge_temp_derating as f32 / 10.0,
            self.charge_temp_limit as f32 / 10.0,
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
        assert_eq!(ConfigType::SwitchServoPower.value_size(), Some(1));
        assert_eq!(ConfigType::ChargeStopSoc.value_size(), Some(1));
        assert_eq!(ConfigType::PowerServoCurrentLimitMa.value_size(), Some(2));
        assert_eq!(ConfigType::ServoBaudRate.value_size(), Some(4));
    }

    #[test]
    fn test_config_encode_decode_roundtrip() {
        // Test Config enum encoding/decoding
        let configs = vec![
            Config::SwitchPowerServo(true),
            Config::SwitchPowerServo(false),
            Config::ChargeStopSoc(80),
            Config::PowerServoCurrentLimitMa(50),
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
        assert_eq!(config.power_servo_on, decoded.power_servo_on);
        assert_eq!(config.power_5v_on, decoded.power_5v_on);
        assert_eq!(config.charge_on, decoded.charge_on);
        assert_eq!(config.bat_ext_out_on, decoded.bat_ext_out_on);
        assert_eq!(
            config.charge_stop_percentage,
            decoded.charge_stop_percentage
        );
        assert_eq!(
            config.servo_current_limit_ma,
            decoded.servo_current_limit_ma
        );
        assert_eq!(config.servo_temp_limit, decoded.servo_temp_limit);
        assert_eq!(config.temp_5v_limit, decoded.temp_5v_limit);
        assert_eq!(config.charge_max_current_ma, decoded.charge_max_current_ma);
        assert_eq!(config.charge_temp_derating, decoded.charge_temp_derating);
        assert_eq!(config.charge_temp_limit, decoded.charge_temp_limit);
        assert_eq!(
            config.charge_stop_voltage_mv,
            decoded.charge_stop_voltage_mv
        );
        assert_eq!(config.servo_baud_rate, decoded.servo_baud_rate);
    }

    #[test]
    fn test_board_config_snapshot_custom_values() {
        let config = BoardConfigSnapshot {
            power_servo_on: true,
            power_5v_on: false,
            charge_on: true,
            bat_ext_out_on: false,
            charge_stop_percentage: 80,
            tx_log_level: LogLevel::Debug,
            servo_current_limit_ma: 100,
            servo_temp_limit: 850,
            temp_5v_limit: 750,
            charge_max_current_ma: 200,
            charge_temp_derating: 650,
            charge_temp_limit: 750,
            charge_stop_voltage_mv: 168,
            servo_baud_rate: 921600,
        };

        let bytes = config.to_bytes();
        let decoded = BoardConfigSnapshot::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.power_servo_on, true);
        assert_eq!(decoded.power_5v_on, false);
        assert_eq!(decoded.charge_on, true);
        assert_eq!(decoded.bat_ext_out_on, false);
        assert_eq!(decoded.charge_stop_percentage, 80);
        assert_eq!(decoded.servo_current_limit_ma, 100);
        assert_eq!(decoded.servo_temp_limit, 850);
        assert_eq!(decoded.temp_5v_limit, 750);
        assert_eq!(decoded.charge_max_current_ma, 200);
        assert_eq!(decoded.charge_temp_derating, 650);
        assert_eq!(decoded.charge_temp_limit, 750);
        assert_eq!(decoded.charge_stop_voltage_mv, 168);
        assert_eq!(decoded.servo_baud_rate, 921600);
    }
}
