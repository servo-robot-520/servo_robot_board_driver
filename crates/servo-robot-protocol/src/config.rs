//! 电源管理主板配置

use crate::enum_with_from_u8;
use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use crate::log::LogLevel;
use alloc::vec::Vec;

enum_with_from_u8! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ConfigType {
        EnableBatOut1            = 0x10 => "Servo Power",
        EnablePwrBatOut2         = 0x11 => "Battery Extra Output",
        EnablePwr5V              = 0x12 => "5V Power",
        EnableCharge             = 0x13 => "Charge",
        EnableServoPwrMonitor    = 0x14 => "Servo Power Monitor",

        BatOut1CurrentLimitMa    = 0x20 => "Bat Out1 Current Limit",
        BatOut2CurrentLimitMa    = 0x21 => "Bat Out2 Current Limit",
        Pwr5VOutCurrentLimitMa   = 0x22 => "5V Out Current Limit",
        PwrServoCurrentLimitMa   = 0x23 => "Servo power out Current Limit",
        ChargeMinCurrentMa       = 0x24 => "Charge Min Current",
        ChargeMaxCurrentMa       = 0x25 => "Charge Max Current",

        PwrServoTempLimit        = 0x30 => "Servo Temp Limit",
        Pwr5vTempLimit           = 0x31 => "5V Temp Limit",
        ChargeTempDerating       = 0x32 => "Charge Temp Derating",
        ChargeTempLimit          = 0x33 => "Charge Temp Limit",

        // 舵机串口波特率，设为0禁用
        ServoBaudRate            = 0x40 => "Servo Baud Rate",
        ChargeStopSoc            = 0x41 => "Charge Stop Soc",
        ChargeStopVoltageMv      = 0x42 => "Charge Stop Voltage",
        // servo robot board发送的日志等级
        TxLogLevel               = 0x43 => "TxLog Level",
        // 未配置=NaN, BQ40Z50, BQ28Z610
        BMSIc                    = 0x44 => "BMS IC",
        // 未配置=NaN, MPU6500, MPU6050
        IMUIc                    = 0x45 => "IMU IC",
    }
}

impl ConfigType {
    pub fn unit(&self) -> &'static str {
        match self {
            Self::BatOut1CurrentLimitMa
            | Self::BatOut2CurrentLimitMa
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

    /// 是否开关类配置（1 字节 bool，取值 on/off）
    pub fn is_switch(&self) -> bool {
        matches!(
            self,
            Self::EnableBatOut1
                | Self::EnablePwrBatOut2
                | Self::EnablePwr5V
                | Self::EnableCharge
                | Self::EnableServoPwrMonitor
        )
    }

    /// 值 payload 大小(不含 type 字节)。当前所有 ConfigType 都有值 payload,
    /// 保留 `Option` 以便未来加入无值命令(Reset/Shutdown 类)。
    pub fn value_size(&self) -> Option<usize> {
        match self {
            // 开关: 1字节 (bool)
            Self::EnableBatOut1
            | Self::EnablePwrBatOut2
            | Self::EnablePwr5V
            | Self::EnableCharge
            // 当舵机电源监控启用时，
            // ADC 采集对应通道获取舵机电源电压和功率。
            | Self::EnableServoPwrMonitor => Some(1),
            // u8 值: 1字节
            Self::ChargeStopSoc | Self::TxLogLevel | Self::BMSIc | Self::IMUIc => Some(1),
            // u16 值: 2字节
            Self::BatOut1CurrentLimitMa
            | Self::BatOut2CurrentLimitMa
            | Self::Pwr5VOutCurrentLimitMa
            | Self::PwrServoCurrentLimitMa
            | Self::ChargeMinCurrentMa
            | Self::ChargeMaxCurrentMa
            | Self::PwrServoTempLimit
            | Self::Pwr5vTempLimit
            | Self::ChargeTempDerating
            | Self::ChargeTempLimit
            | Self::ChargeStopVoltageMv => Some(2),
            // u32 值: 4字节
            Self::ServoBaudRate => Some(4),
        }
    }
}

/// Configuration values
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Config {
    // 电池对外输出1开关
    EnableBatOut1(bool),
    // 电池对外输出2开关
    EnableBatOut2(bool),
    // 5V 电源开关
    EnablePwr5V(bool),
    // 充电开关
    EnableCharge(bool),
    EnableServoPwrMonitor(bool),

    // 电池输出1电流限制
    BatOut1CurrentLimitMa(u16),
    // 电池输出2电流限制
    BatOut2CurrentLimitMa(u16),
    // 5V output current limiting
    Pwr5VOutCurrentLimitMa(u16),
    // 舵机电源电流限制
    PwrServoCurrentLimitMa(u16),
    // 最小充电电流
    ChargeMinCurrentMa(u16),
    // 最大充电电流
    ChargeMaxCurrentMa(u16),

    // 舵机电源温度限制
    PwrServoTempLimit(u16),
    // 5V power temperature limit
    Pwr5vTempLimit(u16),
    // 充电电路降额温度
    ChargeTempDerating(u16),
    // 充电电路停止温度
    ChargeTempLimit(u16),

    // 舵机串口波特率
    ServoBaudRate(u32),
    // 充电容量上限百分比
    ChargeStopSoc(u8),
    // 充电截止电压
    ChargeStopVoltageMv(u16),
    // 发送日志等级
    TxLogLevel(LogLevel),
    // BMS IC type (0=NaN, 1=BQ40Z50, 2=BQ28Z10)
    BMSIc(u8),
    // IMU IC 类型 (0=未配置, 1=MPU6500, 2=MPU6050)
    IMUIc(u8),
}

impl Config {
    pub fn config_type(&self) -> ConfigType {
        match self {
            Self::EnableBatOut1(_) => ConfigType::EnableBatOut1,
            Self::EnableBatOut2(_) => ConfigType::EnablePwrBatOut2,
            Self::EnablePwr5V(_) => ConfigType::EnablePwr5V,
            Self::EnableCharge(_) => ConfigType::EnableCharge,
            Self::EnableServoPwrMonitor(_) => ConfigType::EnableServoPwrMonitor,

            Self::BatOut1CurrentLimitMa(_) => ConfigType::BatOut1CurrentLimitMa,
            Self::BatOut2CurrentLimitMa(_) => ConfigType::BatOut2CurrentLimitMa,
            Self::PwrServoCurrentLimitMa(_) => ConfigType::PwrServoCurrentLimitMa,
            Self::Pwr5VOutCurrentLimitMa(_) => ConfigType::Pwr5VOutCurrentLimitMa,
            Self::ChargeMinCurrentMa(_) => ConfigType::ChargeMinCurrentMa,
            Self::ChargeMaxCurrentMa(_) => ConfigType::ChargeMaxCurrentMa,

            Self::PwrServoTempLimit(_) => ConfigType::PwrServoTempLimit,
            Self::Pwr5vTempLimit(_) => ConfigType::Pwr5vTempLimit,
            Self::ChargeTempDerating(_) => ConfigType::ChargeTempDerating,
            Self::ChargeTempLimit(_) => ConfigType::ChargeTempLimit,

            Self::ServoBaudRate(_) => ConfigType::ServoBaudRate,
            Self::ChargeStopSoc(_) => ConfigType::ChargeStopSoc,
            Self::ChargeStopVoltageMv(_) => ConfigType::ChargeStopVoltageMv,
            Self::TxLogLevel(_) => ConfigType::TxLogLevel,
            Self::BMSIc(_) => ConfigType::BMSIc,
            Self::IMUIc(_) => ConfigType::IMUIc,
        }
    }

    pub fn value(&self) -> f32 {
        match self {
            Self::EnableBatOut1(on)
            | Self::EnablePwr5V(on)
            | Self::EnableCharge(on)
            | Self::EnableBatOut2(on)
            | Self::EnableServoPwrMonitor(on) => {
                if *on {
                    1.0
                } else {
                    0.0
                }
            }
            Self::PwrServoCurrentLimitMa(v)
            | Self::ChargeStopVoltageMv(v)
            | Self::ChargeMaxCurrentMa(v)
            | Self::PwrServoTempLimit(v)
            | Self::Pwr5vTempLimit(v)
            | Self::ChargeTempDerating(v)
            | Self::ChargeTempLimit(v) => *v as f32,
            Self::ServoBaudRate(v) => *v as f32,
            Self::BatOut2CurrentLimitMa(v)
            | Self::Pwr5VOutCurrentLimitMa(v)
            | Self::BatOut1CurrentLimitMa(v)
            | Self::ChargeMinCurrentMa(v) => *v as f32,
            Self::ChargeStopSoc(v) => *v as f32,
            Self::TxLogLevel(level) => *level as u8 as f32,
            Self::BMSIc(v) | Self::IMUIc(v) => *v as f32,
        }
    }

    pub fn from_type_value(typ: ConfigType, value: f32) -> Self {
        match typ {
            ConfigType::EnableBatOut1 => Self::EnableBatOut1(value != 0.0),
            ConfigType::EnablePwr5V => Self::EnablePwr5V(value != 0.0),
            ConfigType::EnableCharge => Self::EnableCharge(value != 0.0),
            ConfigType::EnablePwrBatOut2 => Self::EnableBatOut2(value != 0.0),
            ConfigType::EnableServoPwrMonitor => Self::EnableServoPwrMonitor(value != 0.0),

            ConfigType::BatOut1CurrentLimitMa => Self::BatOut1CurrentLimitMa(value as _),
            ConfigType::BatOut2CurrentLimitMa => Self::BatOut2CurrentLimitMa(value as _),
            ConfigType::Pwr5VOutCurrentLimitMa => Self::Pwr5VOutCurrentLimitMa(value as _),
            ConfigType::PwrServoCurrentLimitMa => Self::PwrServoCurrentLimitMa(value as _),
            ConfigType::ChargeMinCurrentMa => Self::ChargeMinCurrentMa(value as _),
            ConfigType::ChargeMaxCurrentMa => Self::ChargeMaxCurrentMa(value as _),

            ConfigType::PwrServoTempLimit => Self::PwrServoTempLimit(value as _),
            ConfigType::Pwr5vTempLimit => Self::Pwr5vTempLimit(value as _),
            ConfigType::ChargeTempDerating => Self::ChargeTempDerating(value as _),
            ConfigType::ChargeTempLimit => Self::ChargeTempLimit(value as _),

            ConfigType::ServoBaudRate => Self::ServoBaudRate(value as _),
            ConfigType::ChargeStopSoc => Self::ChargeStopSoc(value as _),
            ConfigType::ChargeStopVoltageMv => Self::ChargeStopVoltageMv(value as _),
            ConfigType::TxLogLevel => Self::TxLogLevel(LogLevel::from_u8(value as _)),
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

        // 所有 ConfigType 变体均有值载荷，无需特殊处理

        // 根据配置类型确定所需载荷大小
        let value_len = match config_type {
            // 开关: 1字节 (bool)
            ConfigType::EnableBatOut1
            | ConfigType::EnablePwr5V
            | ConfigType::EnableCharge
            | ConfigType::EnablePwrBatOut2
            | ConfigType::EnableServoPwrMonitor => 1,
            // u16 配置: 2字节
            ConfigType::BatOut1CurrentLimitMa
            | ConfigType::BatOut2CurrentLimitMa
            | ConfigType::Pwr5VOutCurrentLimitMa
            | ConfigType::PwrServoCurrentLimitMa
            | ConfigType::ChargeMinCurrentMa
            | ConfigType::PwrServoTempLimit
            | ConfigType::Pwr5vTempLimit
            | ConfigType::ChargeMaxCurrentMa
            | ConfigType::ChargeTempDerating
            | ConfigType::ChargeTempLimit
            | ConfigType::ChargeStopVoltageMv => 2,
            // ChargeStopSoc: 1字节 (u8)
            ConfigType::ChargeStopSoc => 1,
            // TxLogLevel: 1字节 (u8)
            ConfigType::TxLogLevel => 1,
            // u8 配置: 1字节
            ConfigType::BMSIc | ConfigType::IMUIc => 1,
            // u32 配置: 4字节
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
            | Self::EnableBatOut2(on)
            | Self::EnableServoPwrMonitor(on) => buf.push(*on as u8),
            Self::ChargeStopSoc(v) => buf.push(*v),
            Self::TxLogLevel(level) => buf.push(*level as u8),
            Self::PwrServoCurrentLimitMa(v)
            | Self::PwrServoTempLimit(v)
            | Self::Pwr5vTempLimit(v)
            | Self::ChargeMaxCurrentMa(v)
            | Self::ChargeTempDerating(v)
            | Self::ChargeTempLimit(v)
            | Self::ChargeStopVoltageMv(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::ServoBaudRate(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::BatOut2CurrentLimitMa(v)
            | Self::Pwr5VOutCurrentLimitMa(v)
            | Self::BatOut1CurrentLimitMa(v)
            | Self::ChargeMinCurrentMa(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Self::BMSIc(v) | Self::IMUIc(v) => buf.push(*v),
        }
        buf
    }
}

/// 获取配置值
pub fn get_config_value(c: &BoardConfigSnapshot, ct: ConfigType) -> Config {
    match ct {
        ConfigType::EnableBatOut1 => Config::EnableBatOut1(c.enable_bat_ou1),
        ConfigType::EnablePwrBatOut2 => Config::EnableBatOut2(c.enable_bat_out2),
        ConfigType::EnablePwr5V => Config::EnablePwr5V(c.enable_pwr_5v),
        ConfigType::EnableCharge => Config::EnableCharge(c.enable_charge),
        ConfigType::EnableServoPwrMonitor => {
            Config::EnableServoPwrMonitor(c.enable_servo_pwr_monitor)
        }

        ConfigType::BatOut1CurrentLimitMa => {
            Config::BatOut1CurrentLimitMa(c.bat_out1_current_limit_ma)
        }
        ConfigType::BatOut2CurrentLimitMa => {
            Config::BatOut2CurrentLimitMa(c.bat_out2_current_limit_ma)
        }
        ConfigType::Pwr5VOutCurrentLimitMa => {
            Config::Pwr5VOutCurrentLimitMa(c.pwr_5v_out_current_limit_ma)
        }
        ConfigType::PwrServoCurrentLimitMa => {
            Config::PwrServoCurrentLimitMa(c.servo_current_limit_ma)
        }
        ConfigType::ChargeMinCurrentMa => Config::ChargeMinCurrentMa(c.charge_min_current_ma),
        ConfigType::ChargeMaxCurrentMa => Config::ChargeMaxCurrentMa(c.charge_max_current_ma),

        ConfigType::PwrServoTempLimit => Config::PwrServoTempLimit(c.pwr_servo_temp_limit),
        ConfigType::Pwr5vTempLimit => Config::Pwr5vTempLimit(c.pwr_5v_temp_limit),
        ConfigType::ChargeTempDerating => Config::ChargeTempDerating(c.charge_temp_derating),
        ConfigType::ChargeTempLimit => Config::ChargeTempLimit(c.charge_temp_limit),

        ConfigType::ServoBaudRate => Config::ServoBaudRate(c.servo_baud_rate),
        ConfigType::ChargeStopSoc => Config::ChargeStopSoc(c.charge_stop_soc),
        ConfigType::ChargeStopVoltageMv => Config::ChargeStopVoltageMv(c.charge_stop_voltage_mv),
        ConfigType::TxLogLevel => Config::TxLogLevel(c.tx_log_level),
        ConfigType::BMSIc => Config::BMSIc(c.bms_ic),
        ConfigType::IMUIc => Config::IMUIc(c.imu_ic),
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
    pub enable_servo_pwr_monitor: bool,
    // === Current limits (0x20~0x25) ===
    /// Servo power supply current limit (mA)
    pub bat_out1_current_limit_ma: u16,
    /// Battery extra output current limit (mA)
    pub bat_out2_current_limit_ma: u16,
    /// 5V output current limit (mA)
    pub pwr_5v_out_current_limit_ma: u16,
    /// Servo power output current limit (mA)
    pub servo_current_limit_ma: u16,
    /// 最小充电电流 (mA)
    pub charge_min_current_ma: u16,
    /// 最大充电电流 (mA)
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
    pub charge_stop_soc: u8,
    /// Charging stop voltage (mV)
    pub charge_stop_voltage_mv: u16,
    /// Board log level
    pub tx_log_level: LogLevel,
    /// BMS IC type (0=NaN, 1=BQ40Z50, 2=BQ28Z10)
    pub bms_ic: u8,
    /// IMU IC 类型 (0=未配置, 1=MPU6500, 2=MPU6050)
    pub imu_ic: u8,
}

impl Default for BoardConfigSnapshot {
    fn default() -> Self {
        BoardConfigSnapshot {
            // 开关
            enable_bat_ou1: true,
            enable_bat_out2: true,
            enable_pwr_5v: true,
            enable_charge: true,
            enable_servo_pwr_monitor: true,
            // 电流限制
            bat_out1_current_limit_ma: 50,
            bat_out2_current_limit_ma: 0,
            pwr_5v_out_current_limit_ma: 0,
            servo_current_limit_ma: 0,
            charge_min_current_ma: 0,
            charge_max_current_ma: 90,
            // 温度限制
            pwr_servo_temp_limit: 800,
            pwr_5v_temp_limit: 700,
            charge_temp_derating: 600,
            charge_temp_limit: 700,
            // 杂项
            servo_baud_rate: 115200,
            charge_stop_soc: 100,
            charge_stop_voltage_mv: 168,
            tx_log_level: LogLevel::Info,
            bms_ic: 0,
            imu_ic: 0,
        }
    }
}

impl BoardConfigSnapshot {
    /// Payload size: 5 bool + 4 u8 + 11 u16 + 1 u32 = 35 bytes
    const PAYLOAD_SIZE: usize = 35;

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
        let enable_servo_pwr_monitor = data[o] != 0;
        o += 1;

        // === Current limits (0x20~0x25) — 6×u16 = 12 bytes ===
        let bat_out1_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let bat_out2_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let pwr_5v_out_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let servo_current_limit_ma = u16::from_le_bytes([data[o], data[o + 1]]);
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
            enable_servo_pwr_monitor,
            bat_out1_current_limit_ma,
            bat_out2_current_limit_ma,
            pwr_5v_out_current_limit_ma,
            servo_current_limit_ma,
            charge_min_current_ma,
            charge_max_current_ma,
            pwr_servo_temp_limit,
            pwr_5v_temp_limit,
            charge_temp_derating,
            charge_temp_limit,
            servo_baud_rate,
            charge_stop_soc: charge_stop_percentage,
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
        buf.push(self.enable_servo_pwr_monitor as u8);

        // === Current limits (0x20~0x25) ===
        buf.extend_from_slice(&self.bat_out1_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out2_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_out_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.servo_current_limit_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_min_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_max_current_ma.to_le_bytes());

        // === Temp limits (0x30~0x33) ===
        buf.extend_from_slice(&self.pwr_servo_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_temp_limit.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_derating.to_le_bytes());
        buf.extend_from_slice(&self.charge_temp_limit.to_le_bytes());

        // === Misc (0x40~0x45) ===
        buf.extend_from_slice(&self.servo_baud_rate.to_le_bytes());
        buf.push(self.charge_stop_soc);
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
            "sw=[{},{},{},{},{}, {}] \
             cur=[{},{},{},{},{},{:.1}A] \
             temp=[{:.1},{:.1},{:.1},{:.1}]°C \
             lvl={} bms={} imu={} \
             chg={:.1}mV/{}% baud={}",
            if self.enable_bat_ou1 { "O1" } else { "-" },
            if self.enable_bat_out2 { "O2" } else { "-" },
            if self.enable_pwr_5v { "5V" } else { "-" },
            if self.enable_charge { "CG" } else { "-" },
            if self.enable_servo_pwr_monitor {
                "SM"
            } else {
                "-"
            },
            self.charge_stop_soc,
            self.bat_out1_current_limit_ma,
            self.bat_out2_current_limit_ma,
            self.pwr_5v_out_current_limit_ma,
            self.servo_current_limit_ma,
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
            self.charge_stop_soc,
            self.servo_baud_rate,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_type_value_size() {
        // 测试 value_size() 返回正确大小
        assert_eq!(ConfigType::EnableBatOut1.value_size(), Some(1));
        assert_eq!(ConfigType::ChargeStopSoc.value_size(), Some(1));
        assert_eq!(ConfigType::BatOut1CurrentLimitMa.value_size(), Some(2));
        assert_eq!(ConfigType::ServoBaudRate.value_size(), Some(4));
    }

    #[test]
    fn test_config_encode_decode_roundtrip() {
        // 测试 Config 枚举编解码 — 覆盖所有变体
        let configs = vec![
            // 开关 (bool)
            Config::EnableBatOut1(true),
            Config::EnableBatOut2(true),
            Config::EnablePwr5V(true),
            Config::EnableCharge(false),
            // u8 值
            Config::ChargeStopSoc(80),
            Config::BMSIc(1),
            Config::IMUIc(2),
            // u16 值
            Config::PwrServoCurrentLimitMa(50),
            Config::BatOut2CurrentLimitMa(200),
            Config::Pwr5VOutCurrentLimitMa(500),
            Config::BatOut1CurrentLimitMa(100),
            Config::ChargeMinCurrentMa(10),
            Config::ChargeMaxCurrentMa(90),
            Config::PwrServoTempLimit(800),
            Config::Pwr5vTempLimit(700),
            Config::ChargeTempDerating(600),
            Config::ChargeTempLimit(700),
            Config::ChargeStopVoltageMv(168),
            // u32 值
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
        // 开关
        assert_eq!(config.enable_bat_ou1, decoded.enable_bat_ou1);
        assert_eq!(config.enable_bat_out2, decoded.enable_bat_out2);
        assert_eq!(config.enable_pwr_5v, decoded.enable_pwr_5v);
        assert_eq!(config.enable_charge, decoded.enable_charge);
        // 电流限制
        assert_eq!(
            config.bat_out1_current_limit_ma,
            decoded.bat_out1_current_limit_ma
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
            config.servo_current_limit_ma,
            decoded.servo_current_limit_ma
        );
        assert_eq!(config.charge_min_current_ma, decoded.charge_min_current_ma);
        assert_eq!(config.charge_max_current_ma, decoded.charge_max_current_ma);
        // 温度限制
        assert_eq!(config.pwr_servo_temp_limit, decoded.pwr_servo_temp_limit);
        assert_eq!(config.pwr_5v_temp_limit, decoded.pwr_5v_temp_limit);
        assert_eq!(config.charge_temp_derating, decoded.charge_temp_derating);
        assert_eq!(config.charge_temp_limit, decoded.charge_temp_limit);
        // 杂项
        assert_eq!(config.servo_baud_rate, decoded.servo_baud_rate);
        assert_eq!(
            config.charge_stop_soc,
            decoded.charge_stop_soc
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
            enable_servo_pwr_monitor: true,
            bat_out1_current_limit_ma: 100,
            bat_out2_current_limit_ma: 300,
            pwr_5v_out_current_limit_ma: 500,
            servo_current_limit_ma: 150,
            charge_min_current_ma: 10,
            charge_max_current_ma: 200,
            pwr_servo_temp_limit: 850,
            pwr_5v_temp_limit: 750,
            charge_temp_derating: 650,
            charge_temp_limit: 750,
            servo_baud_rate: 921600,
            charge_stop_soc: 80,
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
        assert_eq!(decoded.enable_servo_pwr_monitor, true);
        assert_eq!(decoded.bat_out1_current_limit_ma, 100);
        assert_eq!(decoded.bat_out2_current_limit_ma, 300);
        assert_eq!(decoded.pwr_5v_out_current_limit_ma, 500);
        assert_eq!(decoded.servo_current_limit_ma, 150);
        assert_eq!(decoded.charge_min_current_ma, 10);
        assert_eq!(decoded.charge_max_current_ma, 200);
        assert_eq!(decoded.pwr_servo_temp_limit, 850);
        assert_eq!(decoded.pwr_5v_temp_limit, 750);
        assert_eq!(decoded.charge_temp_derating, 650);
        assert_eq!(decoded.charge_temp_limit, 750);
        assert_eq!(decoded.charge_stop_voltage_mv, 168);
        assert_eq!(decoded.charge_stop_soc, 80);
        assert_eq!(decoded.servo_baud_rate, 921600);
        assert_eq!(decoded.bms_ic, 1);
        assert_eq!(decoded.imu_ic, 2);
    }

    /// ALL 覆盖全部变体，且 from_u8 / from_name / variant_name 两两自洽
    #[test]
    fn test_config_type_all_and_name_lookup() {
        assert_eq!(ConfigType::ALL.len(), 21);
        for ct in ConfigType::ALL {
            assert_eq!(ConfigType::from_u8(*ct as u8), Some(*ct));
            assert_eq!(ConfigType::from_name(ct.variant_name()), Some(*ct));
            // 显示名同样可解析（大小写不敏感）
            assert_eq!(ConfigType::from_name(ct.name()), Some(*ct));
            assert_eq!(ConfigType::from_name(&ct.name().to_uppercase()), Some(*ct));
        }
        assert_eq!(ConfigType::from_name("NotAConfigType"), None);
        assert_eq!(ConfigType::from_u8(0xFF), None);
    }

    /// 开关类恰好 5 项，与 value_size()==1 且非数值开关区分开
    #[test]
    fn test_config_type_is_switch() {
        let switches: Vec<ConfigType> = ConfigType::ALL
            .iter()
            .copied()
            .filter(|ct| ct.is_switch())
            .collect();
        assert_eq!(switches.len(), 5);
        assert!(ConfigType::EnableServoPwrMonitor.is_switch());
        // 1 字节数值配置不是开关
        assert!(!ConfigType::ChargeStopSoc.is_switch());
        assert!(!ConfigType::TxLogLevel.is_switch());
        assert!(!ConfigType::BMSIc.is_switch());
        assert!(!ConfigType::PwrServoCurrentLimitMa.is_switch());
    }
}
