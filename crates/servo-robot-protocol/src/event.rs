//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:40
//! Event types

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StateChangeFlags: u16 {
        const CHARGER_CONNECTED = 1 << 0;
        const FAN_ENABLED       = 1 << 1;
        const SERVO_POWER_ON    = 1 << 2;
        const POWER_5V_ON       = 1 << 3;
        const BAT_EXT_OUT_ON    = 1 << 4;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProtectionFlags: u16 {
        const SERVO_OVERCURRENT = 1 << 0;
        const SERVO_THERMAL     = 1 << 1;
        const DCDC_5V_THERMAL   = 1 << 2;
        const CHARGE_DERATING   = 1 << 3;
        const CHARGE_THERMAL    = 1 << 4;
        const BATTERY_LOW       = 1 << 5;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChargePhase {
    NotCharging = 0,
    PreCharge = 1,
    Cc = 2,
    Cv = 3,
    Full = 4,
    PdSinkFault = 5,
    UnsupportedCharger = 6,
}

impl ChargePhase {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::NotCharging,
            2 => Self::PreCharge,
            3 => Self::Cc,
            4 => Self::Cv,
            5 => Self::Full,
            6 => Self::PdSinkFault,
            7 => Self::UnsupportedCharger,
            _ => Self::NotCharging,
        }
    }
}

// ═══ 事件分类 ═══

/// 事件分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCategory {
    Charge,
    StateChange,
    Protection,
    Error,
}

/// 状态变化事件映射 (bit, on事件, off事件)
const STATE_CHANGE_MAPPINGS: &[(u16, EventKind, EventKind)] = &[
    (
        StateChangeFlags::CHARGER_CONNECTED.bits(),
        EventKind::ChargerConnected,
        EventKind::ChargerDisconnected,
    ),
    (
        StateChangeFlags::FAN_ENABLED.bits(),
        EventKind::FanOn,
        EventKind::FanOff,
    ),
    (
        StateChangeFlags::SERVO_POWER_ON.bits(),
        EventKind::PowerServoOn,
        EventKind::PowerServoOff,
    ),
    (
        StateChangeFlags::POWER_5V_ON.bits(),
        EventKind::Power5vOn,
        EventKind::Power5vOff,
    ),
    (
        StateChangeFlags::BAT_EXT_OUT_ON.bits(),
        EventKind::BatExtOutOn,
        EventKind::BatExtOutOff,
    ),
];

/// 保护事件映射 (bit, 事件)
const PROTECTION_MAPPINGS: &[(u16, EventKind)] = &[
    (
        ProtectionFlags::SERVO_OVERCURRENT.bits(),
        EventKind::ServoOvercurrent,
    ),
    (
        ProtectionFlags::SERVO_THERMAL.bits(),
        EventKind::PowerServoThermal,
    ),
    (
        ProtectionFlags::DCDC_5V_THERMAL.bits(),
        EventKind::Power5vThermal,
    ),
    (
        ProtectionFlags::CHARGE_DERATING.bits(),
        EventKind::ChargeDerating,
    ),
    (
        ProtectionFlags::CHARGE_THERMAL.bits(),
        EventKind::ChargeThermal,
    ),
    (ProtectionFlags::BATTERY_LOW.bits(), EventKind::BatteryLow),
];

/// 错误事件映射 (bit, 事件)
const ERROR_MAPPINGS: &[(u16, EventKind)] = &[
    (ErrorFlags::UNKNOWN_ERROR.bits(), EventKind::UnknownError),
    (ErrorFlags::UART1_ERROR.bits(), EventKind::Uart1Error),
    (ErrorFlags::UART2_ERROR.bits(), EventKind::Uart2Error),
    (ErrorFlags::I2C1_ERROR.bits(), EventKind::I2c1Error),
    (ErrorFlags::I2C3_ERROR.bits(), EventKind::I2c3Error),
    (ErrorFlags::SPI1_ERROR.bits(), EventKind::Spi1Error),
    (ErrorFlags::USB_ERROR.bits(), EventKind::UsbError),
    (ErrorFlags::DMA_ERROR.bits(), EventKind::DmaError),
];

// ═══ 事件类型 ═══

/// 历史事件记录（带时间戳）
#[derive(Debug, Clone)]
pub struct EventLog {
    pub ts: u64,
    pub kind: EventKind,
}

/// 事件类型
#[derive(Debug, Clone)]
pub enum EventKind {
    // Charging event
    NotCharging = 0,
    PreCharge,
    CcCharge,
    CvCharge,
    FullCharge,
    PdSinkFault,
    UnsupportedCharger,
    // Protect the event
    ServoOvercurrent,
    PowerServoThermal,
    Power5vThermal,
    ChargeDerating,
    ChargeThermal,
    BatteryLow,
    // Error event
    UnknownError,
    Uart1Error,
    Uart2Error,
    I2c1Error,
    I2c3Error,
    Spi1Error,
    UsbError,
    DmaError,
    // Status changes event
    ChargerConnected,
    ChargerDisconnected,
    FanOn,
    FanOff,
    PowerServoOn,
    PowerServoOff,
    Power5vOn,
    Power5vOff,
    BatExtOutOn,
    BatExtOutOff,
}

impl From<ChargePhase> for EventKind {
    fn from(phase: ChargePhase) -> Self {
        match phase {
            ChargePhase::NotCharging => EventKind::NotCharging,
            ChargePhase::PreCharge => EventKind::PreCharge,
            ChargePhase::Cc => EventKind::CcCharge,
            ChargePhase::Cv => EventKind::CvCharge,
            ChargePhase::Full => EventKind::FullCharge,
            ChargePhase::PdSinkFault => EventKind::PdSinkFault,
            ChargePhase::UnsupportedCharger => EventKind::UnsupportedCharger,
        }
    }
}

impl core::fmt::Display for EventKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotCharging => write!(f, "NOT_CHARGING"),
            Self::PreCharge => write!(f, "PRE_CHARGE"),
            Self::CcCharge => write!(f, "CC_CHARGE"),
            Self::CvCharge => write!(f, "CV_CHARGE"),
            Self::FullCharge => write!(f, "FULL_CHARGE"),
            Self::PdSinkFault => write!(f, "PD_SINK_FAULT"),
            Self::UnsupportedCharger => write!(f, "UNSUPPORTED_CHARGER"),
            Self::ServoOvercurrent => write!(f, "SERVO_OVERCURRENT"),
            Self::PowerServoThermal => write!(f, "POWER_SERVO_THERMAL"),
            Self::Power5vThermal => write!(f, "POWER_5V_THERMAL"),
            Self::ChargeDerating => write!(f, "CHARGE_DERATING"),
            Self::ChargeThermal => write!(f, "CHARGE_THERMAL"),
            Self::BatteryLow => write!(f, "BATTERY_LOW"),
            Self::UnknownError => write!(f, "UNKNOWN_ERROR"),
            Self::Uart1Error => write!(f, "UART1_ERROR"),
            Self::Uart2Error => write!(f, "UART2_ERROR"),
            Self::I2c1Error => write!(f, "I2C1_ERROR"),
            Self::I2c3Error => write!(f, "I2C3_ERROR"),
            Self::Spi1Error => write!(f, "SPI1_ERROR"),
            Self::UsbError => write!(f, "USB_ERROR"),
            Self::DmaError => write!(f, "DMA_ERROR"),
            Self::ChargerConnected => write!(f, "CHARGER_CONNECTED"),
            Self::ChargerDisconnected => write!(f, "CHARGER_DISCONNECTED"),
            Self::FanOn => write!(f, "FAN_ON"),
            Self::FanOff => write!(f, "FAN_OFF"),
            Self::PowerServoOn => write!(f, "POWER_SERVO_ON"),
            Self::PowerServoOff => write!(f, "POWER_SERVO_OFF"),
            Self::Power5vOn => write!(f, "POWER_5V_ON"),
            Self::Power5vOff => write!(f, "POWER_5V_OFF"),
            Self::BatExtOutOn => write!(f, "BAT_EXT_OUT_ON"),
            Self::BatExtOutOff => write!(f, "BAT_EXT_OUT_OFF"),
        }
    }
}

impl EventKind {
    /// 事件分类（用于 UI 显示 emoji 和颜色）
    pub fn category(&self) -> EventCategory {
        match self {
            Self::NotCharging
            | Self::PreCharge
            | Self::CcCharge
            | Self::CvCharge
            | Self::FullCharge
            | Self::PdSinkFault
            | Self::UnsupportedCharger => EventCategory::Charge,
            Self::ServoOvercurrent
            | Self::PowerServoThermal
            | Self::Power5vThermal
            | Self::ChargeThermal
            | Self::ChargeDerating
            | Self::BatteryLow => EventCategory::Protection,
            Self::UnknownError
            | Self::Uart1Error
            | Self::Uart2Error
            | Self::I2c1Error
            | Self::I2c3Error
            | Self::Spi1Error
            | Self::UsbError
            | Self::DmaError => EventCategory::Error,
            Self::ChargerConnected
            | Self::ChargerDisconnected
            | Self::FanOn
            | Self::FanOff
            | Self::PowerServoOn
            | Self::PowerServoOff
            | Self::Power5vOn
            | Self::Power5vOff
            | Self::BatExtOutOn
            | Self::BatExtOutOff => EventCategory::StateChange,
        }
    }
}

// ═══ BoardEvent ═══

/// 事件
///
/// 帧格式（7 字节）：
///   [0]    charge_phase (u8)
///   [1..3] state_change_flags (u16 LE)
///   [3..5] protection_flags (u16 LE)
///   [5..7] error_flags (u16 LE)
#[derive(Debug, Clone)]
pub struct BoardEvent {
    pub charge_phase: ChargePhase,
    pub state_change_flags: StateChangeFlags,
    pub protection_flags: ProtectionFlags,
    pub error_flags: ErrorFlags,
}

impl Default for BoardEvent {
    fn default() -> Self {
        BoardEvent {
            charge_phase: ChargePhase::NotCharging,
            state_change_flags: StateChangeFlags::empty(),
            protection_flags: ProtectionFlags::empty(),
            error_flags: ErrorFlags::empty(),
        }
    }
}

impl BoardEvent {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 7 {
            return Err(FrameError::PayloadTooShort {
                expected: 7,
                got: data.len(),
            });
        }
        let charge_phase = ChargePhase::from_u8(data[0]);
        let state_change_flags =
            StateChangeFlags::from_bits(u16::from_le_bytes([data[1], data[2]]))
                .unwrap_or(StateChangeFlags::empty());
        let protection_flags = ProtectionFlags::from_bits(u16::from_le_bytes([data[3], data[4]]))
            .unwrap_or(ProtectionFlags::empty());
        let error_flags = ErrorFlags::from_bits(u16::from_le_bytes([data[5], data[6]]))
            .unwrap_or(ErrorFlags::empty());
        Ok(BoardEvent {
            charge_phase,
            state_change_flags,
            protection_flags,
            error_flags,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(7);
        buf.push(self.charge_phase as u8);
        buf.extend_from_slice(&self.state_change_flags.bits().to_le_bytes());
        buf.extend_from_slice(&self.protection_flags.bits().to_le_bytes());
        buf.extend_from_slice(&self.error_flags.bits().to_le_bytes());
        buf
    }

    /// 与前一状态对比，提取所有新增/变化事件
    pub fn diff_events(&self, prev: &BoardEvent) -> Vec<EventKind> {
        let mut events = Vec::new();

        if self.charge_phase != prev.charge_phase {
            events.push(self.charge_phase.into())
        }

        // 状态变化（含 Charger）：检测翻转
        for &(bit, ref kind_on, ref kind_off) in STATE_CHANGE_MAPPINGS {
            let changed =
                (self.state_change_flags.bits() ^ prev.state_change_flags.bits()) & bit != 0;
            if changed {
                let is_on = self.state_change_flags.bits() & bit != 0;
                events.push(if is_on {
                    kind_on.clone()
                } else {
                    kind_off.clone()
                });
            }
        }

        // 保护事件：检测新增
        let new_prot = self.protection_flags.bits() & !prev.protection_flags.bits();
        for &(bit, ref kind) in PROTECTION_MAPPINGS {
            if new_prot & bit != 0 {
                events.push(kind.clone());
            }
        }

        // 错误事件：检测新增
        let new_err = self.error_flags.bits() & !prev.error_flags.bits();
        for &(bit, ref kind) in ERROR_MAPPINGS {
            if new_err & bit != 0 {
                events.push(kind.clone());
            }
        }

        events
    }

    /// 仅提取新增状态变化事件（含 Charger）
    pub fn new_state_change_events(&self, prev: &StateChangeFlags) -> Vec<EventKind> {
        let changed = self.state_change_flags.bits() ^ prev.bits();
        STATE_CHANGE_MAPPINGS
            .iter()
            .filter(|(bit, _, _)| changed & bit != 0)
            .map(|(bit, kind_on, kind_off)| {
                if self.state_change_flags.bits() & bit != 0 {
                    kind_on.clone()
                } else {
                    kind_off.clone()
                }
            })
            .collect()
    }

    /// 仅提取新增保护事件
    pub fn new_protection_events(&self, prev: &ProtectionFlags) -> Vec<EventKind> {
        let new = self.protection_flags.bits() & !prev.bits();
        PROTECTION_MAPPINGS
            .iter()
            .filter(|(bit, _)| new & bit != 0)
            .map(|(_, kind)| kind.clone())
            .collect()
    }

    /// 仅提取新增错误事件
    pub fn new_error_events(&self, prev: &ErrorFlags) -> Vec<EventKind> {
        let new = self.error_flags.bits() & !prev.bits();
        ERROR_MAPPINGS
            .iter()
            .filter(|(bit, _)| new & bit != 0)
            .map(|(_, kind)| kind.clone())
            .collect()
    }
}

impl ToPayload for BoardEvent {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for BoardEvent {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for BoardEvent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "phase={} state=0x{:04X} prot=0x{:04X} err=0x{:04X}",
            self.charge_phase as u8,
            self.state_change_flags.bits(),
            self.protection_flags.bits(),
            self.error_flags.bits()
        )
    }
}
