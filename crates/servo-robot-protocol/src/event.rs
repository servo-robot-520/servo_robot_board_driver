//! 事件类型

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct StateChangeFlags: u16 {
        const CHARGER_CONNECTED      = 1 << 0;
        const FAN_ENABLED            = 1 << 1;
        const BAT_OUT1_ENABLED       = 1 << 2;
        const BAT_OUT2_ENABLED       = 1 << 3;
        const PWR_5V_ENABLED         = 1 << 4;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProtectionFlags: u16 {
        const BAT_OVERCURRENT       = 1 << 0;
        const PWR_SERVO_OVERCURRENT = 1 << 1;
        const PWR_5V_OVERCURRENT    = 1 << 2;
        const BAT_OUT1_OVERCURRENT  = 1 << 3;
        const BAT_OUT2_OVERCURRENT  = 1 << 4;

        const BAT_THERMAL           = 1 << 5;
        const PWR_SERVO_THERMAL     = 1 << 6;
        const PWR_5V_THERMAL        = 1 << 7;
        const CHARGE_DERATING       = 1 << 8;
        const CHARGE_THERMAL        = 1 << 9;

        const BATTERY_LOW           = 1 << 10;
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
    /// 线值 = 枚举判别值(0~6),与 `to_bytes`(`charge_phase as u8`)和头文件
    /// `sr_charge_phase` 严格一致。曾有一版 +1 错位映射,会导致
    /// `BoardEvent::to_bytes → from_bytes` 往返失败(见测试)。
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::NotCharging,
            1 => Self::PreCharge,
            2 => Self::Cc,
            3 => Self::Cv,
            4 => Self::Full,
            5 => Self::PdSinkFault,
            6 => Self::UnsupportedCharger,
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
const STATE_CHANGE_MAPPINGS: &[(u16, EventType, EventType)] = &[
    (
        StateChangeFlags::CHARGER_CONNECTED.bits(),
        EventType::ChargerConnected,
        EventType::ChargerDisconnected,
    ),
    (
        StateChangeFlags::FAN_ENABLED.bits(),
        EventType::FanOn,
        EventType::FanOff,
    ),
    (
        StateChangeFlags::BAT_OUT1_ENABLED.bits(),
        EventType::BatOut1On,
        EventType::BatOut1Off,
    ),
    (
        StateChangeFlags::BAT_OUT2_ENABLED.bits(),
        EventType::BatOut2On,
        EventType::BatOut2Off,
    ),
    (
        StateChangeFlags::PWR_5V_ENABLED.bits(),
        EventType::Pwr5vOn,
        EventType::Pwr5vOff,
    ),
];

/// 保护事件映射 (bit, 事件)
const PROTECTION_MAPPINGS: &[(u16, EventType)] = &[
    (
        ProtectionFlags::PWR_SERVO_OVERCURRENT.bits(),
        EventType::PwrServerOvercurrent,
    ),
    (
        ProtectionFlags::PWR_SERVO_THERMAL.bits(),
        EventType::PwrServoThermal,
    ),
    (
        ProtectionFlags::PWR_5V_THERMAL.bits(),
        EventType::Pwr5vThermal,
    ),
    (
        ProtectionFlags::CHARGE_DERATING.bits(),
        EventType::ChargeDerating,
    ),
    (
        ProtectionFlags::CHARGE_THERMAL.bits(),
        EventType::ChargeThermal,
    ),
    (ProtectionFlags::BATTERY_LOW.bits(), EventType::BatteryLow),
];

/// 错误事件映射 (bit, 事件)
const ERROR_MAPPINGS: &[(u16, EventType)] = &[
    (ErrorFlags::UNKNOWN_ERROR.bits(), EventType::UnknownError),
    (ErrorFlags::UART1_ERROR.bits(), EventType::Uart1Error),
    (ErrorFlags::UART2_ERROR.bits(), EventType::Uart2Error),
    (ErrorFlags::I2C1_ERROR.bits(), EventType::I2c1Error),
    (ErrorFlags::I2C3_ERROR.bits(), EventType::I2c3Error),
    (ErrorFlags::SPI1_ERROR.bits(), EventType::Spi1Error),
    (ErrorFlags::USB_ERROR.bits(), EventType::UsbError),
    (ErrorFlags::DMA_ERROR.bits(), EventType::DmaError),
];

// ═══ 事件类型 ═══

/// 历史事件记录（带时间戳）
#[derive(Debug, Clone)]
pub struct EventLog {
    pub ts: u64,
    pub kind: EventType,
}

/// 事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    // 充电事件
    NotCharging = 0,
    PreCharge,
    CcCharge,
    CvCharge,
    FullCharge,
    PdSinkFault,
    UnsupportedCharger,
    // 保护事件
    BatOvercurrent,
    PwrServerOvercurrent,
    Pwr5VOvercurrent,
    BatOut1Overcurrent,
    BatOut2Overcurrent,
    BatThermal,
    PwrServoThermal,
    Pwr5vThermal,
    ChargeDerating,
    ChargeThermal,
    BatteryLow,
    // 错误事件
    UnknownError,
    Uart1Error,
    Uart2Error,
    I2c1Error,
    I2c3Error,
    Spi1Error,
    UsbError,
    DmaError,
    // 状态变化事件
    ChargerConnected,
    ChargerDisconnected,
    FanOn,
    FanOff,
    BatOut1On,
    BatOut1Off,
    Pwr5vOn,
    Pwr5vOff,
    BatOut2On,
    BatOut2Off,
}

impl From<ChargePhase> for EventType {
    fn from(phase: ChargePhase) -> Self {
        match phase {
            ChargePhase::NotCharging => EventType::NotCharging,
            ChargePhase::PreCharge => EventType::PreCharge,
            ChargePhase::Cc => EventType::CcCharge,
            ChargePhase::Cv => EventType::CvCharge,
            ChargePhase::Full => EventType::FullCharge,
            ChargePhase::PdSinkFault => EventType::PdSinkFault,
            ChargePhase::UnsupportedCharger => EventType::UnsupportedCharger,
        }
    }
}

impl core::fmt::Display for EventType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotCharging => write!(f, "NOT_CHARGING"),
            Self::PreCharge => write!(f, "PRE_CHARGE"),
            Self::CcCharge => write!(f, "CC_CHARGE"),
            Self::CvCharge => write!(f, "CV_CHARGE"),
            Self::FullCharge => write!(f, "FULL_CHARGE"),
            Self::PdSinkFault => write!(f, "PD_SINK_FAULT"),
            Self::UnsupportedCharger => write!(f, "UNSUPPORTED_CHARGER"),
            Self::PwrServerOvercurrent => write!(f, "PWR_SERVO_OVERCURRENT"),
            Self::PwrServoThermal => write!(f, "PWR_SERVO_THERMAL"),
            Self::Pwr5vThermal => write!(f, "POWER_5V_THERMAL"),
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
            Self::BatOvercurrent => write!(f, "BAT_OVERCURRENT"),
            Self::Pwr5VOvercurrent => write!(f, "PWR_5V_OVERCURRENT"),
            Self::BatOut1Overcurrent => write!(f, "BAT_OUT1_OVERCURRENT"),
            Self::BatOut2Overcurrent => write!(f, "BAT_OUT2_OVERCURRENT"),
            Self::BatThermal => write!(f, "BAT_THERMAL"),
            Self::BatOut1On => write!(f, "BAT_OUT1_ON"),
            Self::BatOut1Off => write!(f, "BAT_OUT1_OFF"),
            Self::BatOut2On => write!(f, "BAT_OUT2_ON"),
            Self::BatOut2Off => write!(f, "BAT_OUT2_OFF"),
            Self::Pwr5vOn => write!(f, "PWR_5V_ON"),
            Self::Pwr5vOff => write!(f, "PWR_5V_OFF"),
        }
    }
}

impl EventType {
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
            Self::PwrServerOvercurrent
            | Self::PwrServoThermal
            | Self::Pwr5vThermal
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
            Self::BatOvercurrent
            | Self::Pwr5VOvercurrent
            | Self::BatOut1Overcurrent
            | Self::BatOut2Overcurrent
            | Self::BatThermal => EventCategory::Protection,
            Self::ChargerConnected
            | Self::ChargerDisconnected
            | Self::FanOn
            | Self::FanOff
            | Self::BatOut1On
            | Self::BatOut1Off
            | Self::BatOut2On
            | Self::BatOut2Off
            | Self::Pwr5vOn
            | Self::Pwr5vOff => EventCategory::StateChange,
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
    pub fn diff_events(&self, prev: &BoardEvent) -> Vec<EventType> {
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
    pub fn new_state_change_events(&self, prev: &StateChangeFlags) -> Vec<EventType> {
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
    pub fn new_protection_events(&self, prev: &ProtectionFlags) -> Vec<EventType> {
        let new = self.protection_flags.bits() & !prev.bits();
        PROTECTION_MAPPINGS
            .iter()
            .filter(|(bit, _)| new & bit != 0)
            .map(|(_, kind)| kind.clone())
            .collect()
    }

    /// 仅提取新增错误事件
    pub fn new_error_events(&self, prev: &ErrorFlags) -> Vec<EventType> {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// ChargePhase 线值必须 = 枚举判别值:全相位 to_bytes → from_bytes 往返一致
    #[test]
    fn test_charge_phase_roundtrip() {
        let phases = [
            ChargePhase::NotCharging,
            ChargePhase::PreCharge,
            ChargePhase::Cc,
            ChargePhase::Cv,
            ChargePhase::Full,
            ChargePhase::PdSinkFault,
            ChargePhase::UnsupportedCharger,
        ];
        for p in phases {
            let e = BoardEvent {
                charge_phase: p,
                state_change_flags: StateChangeFlags::empty(),
                protection_flags: ProtectionFlags::empty(),
                error_flags: ErrorFlags::empty(),
            };
            let decoded = BoardEvent::from_bytes(&e.to_bytes()).unwrap();
            assert_eq!(decoded.charge_phase, p, "roundtrip failed for {:?}", p);
        }
    }

    /// from_u8 与判别值/头文件 sr_charge_phase(0~6) 一致
    #[test]
    fn test_charge_phase_wire_values() {
        assert_eq!(ChargePhase::NotCharging as u8, 0);
        assert_eq!(ChargePhase::from_u8(0), ChargePhase::NotCharging);
        assert_eq!(ChargePhase::from_u8(2), ChargePhase::Cc);
        assert_eq!(ChargePhase::from_u8(4), ChargePhase::Full);
        assert_eq!(ChargePhase::from_u8(6), ChargePhase::UnsupportedCharger);
        assert_eq!(ChargePhase::from_u8(0xFF), ChargePhase::NotCharging); // 未知 → NotCharging
    }

    /// BoardEvent 7 字节帧格式 + 位标志往返
    #[test]
    fn test_board_event_encode_decode() {
        let e = BoardEvent {
            charge_phase: ChargePhase::Cv,
            state_change_flags: StateChangeFlags::CHARGER_CONNECTED | StateChangeFlags::FAN_ENABLED,
            protection_flags: ProtectionFlags::BATTERY_LOW,
            error_flags: ErrorFlags::UART1_ERROR,
        };
        let bytes = e.to_bytes();
        assert_eq!(bytes.len(), 7);
        assert_eq!(bytes[0], ChargePhase::Cv as u8);
        let decoded = BoardEvent::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.charge_phase, ChargePhase::Cv);
        assert_eq!(decoded.state_change_flags, e.state_change_flags);
        assert_eq!(decoded.protection_flags, e.protection_flags);
        assert_eq!(decoded.error_flags, e.error_flags);
    }

    /// 每个状态变化标志位恰好映射一条,on/off 成对且属于同一路开关
    #[test]
    fn test_state_change_mappings_pairing() {
        let expect = [
            (
                StateChangeFlags::CHARGER_CONNECTED,
                EventType::ChargerConnected,
                EventType::ChargerDisconnected,
            ),
            (
                StateChangeFlags::FAN_ENABLED,
                EventType::FanOn,
                EventType::FanOff,
            ),
            (
                StateChangeFlags::BAT_OUT1_ENABLED,
                EventType::BatOut1On,
                EventType::BatOut1Off,
            ),
            (
                StateChangeFlags::BAT_OUT2_ENABLED,
                EventType::BatOut2On,
                EventType::BatOut2Off,
            ),
            (
                StateChangeFlags::PWR_5V_ENABLED,
                EventType::Pwr5vOn,
                EventType::Pwr5vOff,
            ),
        ];
        assert_eq!(STATE_CHANGE_MAPPINGS.len(), expect.len());
        for (flag, on, off) in expect {
            let entry = STATE_CHANGE_MAPPINGS
                .iter()
                .find(|(bit, ..)| *bit == flag.bits())
                .unwrap_or_else(|| panic!("flag {:#x} has no mapping", flag.bits()));
            assert_eq!(
                (&entry.1, &entry.2),
                (&on, &off),
                "flag {:#x} mis-paired",
                flag.bits()
            );
        }
    }
}
