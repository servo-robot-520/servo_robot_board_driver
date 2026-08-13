//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:55
//! Frame format definition

use crate::error::FrameError;
use alloc::vec::Vec;

use crate::battery_state::BatteryState;
use crate::command::Command;
use crate::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::event::BoardEvent;
use crate::imu::ImuData;
use crate::log::LogMessage;
use crate::power::PowerData;
use crate::servo::ServoCmdWrapper;
use crate::system::SystemInfo;
/// 帧头
pub const FRAME_HEAD: u8 = 0xAA;

/// 帧头长度 (HEAD + TYPE + LEN)
const FRAME_HEADER_SIZE: usize = 4;
/// CRC 长度
const FRAME_CRC_SIZE: usize = 2;

/// 帧类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    // ═══ 上行 (STM32 → PC) ═══
    Imu = 0x01,
    Power = 0x02,
    // 0x03 保留（原 Thermal，已合并到 System）
    Config = 0x04,
    Battery = 0x05,
    System = 0x06,
    Event = 0x07,
    Log = 0x08,

    // ═══ 下行 (PC → STM32) ═══
    CfgWrite = 0x80,
    CfgQuery = 0x81,
    CfgQueryAll = 0x82,
    // Commands for forwarding servo operations, including read and write
    ServoForward = 0x83,
    // Firmware data chunk for OTA update
    FirmwareUpdate = 0x84,
    // One-shot command (Reset, Shutdown, Ota)
    Command = 0x85,

    // ═══ 应答 (STM32 → PC) ═══
    AckCfgWrite = 0xC0,
    AckCfgQuery = 0xC1,
    AckCfgQueryAll = 0xC2,
    // Respond to the servo's command
    AckServoCmd = 0xC3,
    // Ack firmware data chunk (status + received offset)
    AckFirmwareUpdate = 0xC4,
    // Ack command execution
    AckCommand = 0xC5,

    // ═══ 未知类型 ═══
    Unknown(u8),
}

impl FrameType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x01 => Self::Imu,
            0x02 => Self::Power,
            // 0x03 保留（原 Thermal，已合并到 System）
            0x04 => Self::Config,
            0x05 => Self::Battery,
            0x06 => Self::System,
            0x07 => Self::Event,
            0x08 => Self::Log,
            0x80 => Self::CfgWrite,
            0x81 => Self::CfgQuery,
            0x82 => Self::CfgQueryAll,
            0x83 => Self::ServoForward,
            0x84 => Self::FirmwareUpdate,
            0x85 => Self::Command,
            0xC0 => Self::AckCfgWrite,
            0xC1 => Self::AckCfgQuery,
            0xC2 => Self::AckCfgQueryAll,
            0xC3 => Self::AckServoCmd,
            0xC4 => Self::AckFirmwareUpdate,
            0xC5 => Self::AckCommand,
            _ => Self::Unknown(v),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Imu => 0x01,
            Self::Power => 0x02,
            Self::Config => 0x04,
            Self::Battery => 0x05,
            Self::System => 0x06,
            Self::Event => 0x07,
            Self::Log => 0x08,
            Self::CfgWrite => 0x80,
            Self::CfgQuery => 0x81,
            Self::CfgQueryAll => 0x82,
            Self::ServoForward => 0x83,
            Self::FirmwareUpdate => 0x84,
            Self::Command => 0x85,
            Self::AckCfgWrite => 0xC0,
            Self::AckCfgQuery => 0xC1,
            Self::AckCfgQueryAll => 0xC2,
            Self::AckServoCmd => 0xC3,
            Self::AckFirmwareUpdate => 0xC4,
            Self::AckCommand => 0xC5,
            Self::Unknown(v) => *v,
        }
    }

    pub fn is_uplink(&self) -> bool {
        matches!(
            self,
            Self::Imu
                | Self::Power
                | Self::Config
                | Self::Battery
                | Self::System
                | Self::Event
                | Self::Log
        )
    }

    pub fn is_downlink(&self) -> bool {
        matches!(
            self,
            Self::CfgWrite
                | Self::CfgQuery
                | Self::CfgQueryAll
                | Self::ServoForward
                | Self::FirmwareUpdate
                | Self::Command
        )
    }

    pub fn is_response(&self) -> bool {
        matches!(
            self,
            Self::AckCfgWrite
                | Self::AckCfgQuery
                | Self::AckCfgQueryAll
                | Self::AckServoCmd
                | Self::AckFirmwareUpdate
                | Self::AckCommand
        )
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Imu => "IMU",
            Self::Power => "Power",
            Self::Config => "Config",
            Self::Battery => "Battery",
            Self::System => "System",
            Self::Event => "Event",
            Self::Log => "Log",
            Self::CfgWrite => "CfgWrite",
            Self::CfgQuery => "CfgQuery",
            Self::CfgQueryAll => "CfgQueryAll",
            Self::ServoForward => "ServoForward",
            Self::FirmwareUpdate => "FirmwareUpdate",
            Self::Command => "Command",
            Self::AckCfgWrite => "AckCfgWrite",
            Self::AckCfgQuery => "AckCfgQuery",
            Self::AckCfgQueryAll => "AckCfgQueryAll",
            Self::AckServoCmd => "AckServoCmd",
            Self::AckFirmwareUpdate => "AckFirmwareUpdate",
            Self::AckCommand => "AckCommand",
            Self::Unknown(_) => "Unknown",
        }
    }
}

impl core::fmt::Display for FrameType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unknown(v) => write!(f, "Unknown({:#04x})", v),
            _ => write!(f, "{}", self.name()),
        }
    }
}

/// 低级帧结构
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub frame_type: FrameType,
    pub payload: Vec<u8>,
}

impl RawFrame {
    /// 从字节缓冲区解码一帧
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), FrameError> {
        let header_pos = buf
            .iter()
            .position(|&b| b == FRAME_HEAD)
            .ok_or(FrameError::NoHeader)?;

        if buf.len() - header_pos < FRAME_HEADER_SIZE {
            return Err(FrameError::Incomplete {
                needed: FRAME_HEADER_SIZE - (buf.len() - header_pos),
            });
        }

        let frame_type = FrameType::from_u8(buf[header_pos + 1]);
        let payload_len = u16::from_le_bytes([buf[header_pos + 2], buf[header_pos + 3]]) as usize;
        let total_len = FRAME_HEADER_SIZE + payload_len + FRAME_CRC_SIZE;

        if buf.len() - header_pos < total_len {
            return Err(FrameError::Incomplete {
                needed: total_len - (buf.len() - header_pos),
            });
        }

        let payload_start = header_pos + FRAME_HEADER_SIZE;
        let payload_end = payload_start + payload_len;
        let payload = buf[payload_start..payload_end].to_vec();

        let crc_start = payload_end;
        let received_crc = u16::from_le_bytes([buf[crc_start], buf[crc_start + 1]]);

        let crc_data = &buf[header_pos + 1..payload_end];
        let calculated_crc = crate::crc::crc16_ccitt_table(crc_data);

        if received_crc != calculated_crc {
            return Err(FrameError::CrcMismatch {
                expected: calculated_crc,
                got: received_crc,
            });
        }

        Ok((
            RawFrame {
                frame_type,
                payload,
            },
            header_pos + total_len,
        ))
    }

    /// 编码为字节
    pub fn encode(&self) -> Vec<u8> {
        let payload_len = self.payload.len();
        let total_len = FRAME_HEADER_SIZE + payload_len + FRAME_CRC_SIZE;
        let mut buf = Vec::with_capacity(total_len);

        buf.push(FRAME_HEAD);
        buf.push(self.frame_type.as_u8());

        let len_bytes = (payload_len as u16).to_le_bytes();
        buf.push(len_bytes[0]);
        buf.push(len_bytes[1]);

        buf.extend_from_slice(&self.payload);

        let crc_data = &buf[1..];
        let crc = crate::crc::crc16_ccitt_table(crc_data);
        let crc_bytes = crc.to_le_bytes();
        buf.push(crc_bytes[0]);
        buf.push(crc_bytes[1]);

        buf
    }
}

/// 可序列化为帧 payload 的类型
pub trait ToPayload {
    fn to_payload(&self) -> Vec<u8>;
}

/// 可从帧 payload 反序列化的类型
pub trait FromPayload: Sized {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError>;
}

/// 类型化帧枚举
#[derive(Debug, Clone)]
pub enum TypedFrame {
    // 上行帧
    Imu(ImuData),
    Power(PowerData),
    Config(BoardConfigSnapshot),
    Battery(BatteryState),
    System(SystemInfo),
    Event(BoardEvent),
    Log(LogMessage),

    // 应答帧
    AckCfgWrite { success: bool },
    AckCfgQuery(Config),
    AckCfgQueryAll(BoardConfigSnapshot),
    AckServoCmd(ServoCmdWrapper),
    AckFirmwareUpdate { success: bool, offset: u32 },
    AckCommand { success: bool },

    // 下行帧（用于发送）
    ConfigWrite(Config),
    ConfigQuery(ConfigType),
    ConfigQueryAll,
    ServoForward(ServoCmdWrapper),
    FirmwareUpdate(ServoCmdWrapper), // reuses wrapper for raw data chunks
    Command(Command),
}

impl TypedFrame {
    pub fn from_raw(frame: &RawFrame) -> Result<Self, FrameError> {
        match frame.frame_type {
            FrameType::Imu => Ok(TypedFrame::Imu(ImuData::from_bytes(&frame.payload)?)),
            FrameType::Power => Ok(TypedFrame::Power(PowerData::from_bytes(&frame.payload)?)),
            FrameType::Config => Ok(TypedFrame::Config(BoardConfigSnapshot::from_bytes(
                &frame.payload,
            )?)),
            FrameType::Battery => Ok(TypedFrame::Battery(BatteryState::from_bytes(
                &frame.payload,
            )?)),
            FrameType::System => Ok(TypedFrame::System(SystemInfo::from_bytes(&frame.payload)?)),
            FrameType::Event => Ok(TypedFrame::Event(BoardEvent::from_bytes(&frame.payload)?)),
            FrameType::Log => Ok(TypedFrame::Log(LogMessage::from_bytes(&frame.payload)?)),
            FrameType::AckCfgWrite => {
                if frame.payload.is_empty() {
                    return Err(FrameError::PayloadTooShort {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(TypedFrame::AckCfgWrite {
                    success: frame.payload[0] != 0,
                })
            }
            FrameType::AckCfgQuery => {
                Ok(TypedFrame::AckCfgQuery(Config::from_bytes(&frame.payload)?))
            }
            FrameType::AckCfgQueryAll => Ok(TypedFrame::AckCfgQueryAll(
                BoardConfigSnapshot::from_bytes(&frame.payload)?,
            )),
            FrameType::ServoForward => Ok(TypedFrame::ServoForward(ServoCmdWrapper::from_payload(
                &frame.payload,
            )?)),
            FrameType::FirmwareUpdate => Ok(TypedFrame::FirmwareUpdate(
                ServoCmdWrapper::from_payload(&frame.payload)?,
            )),
            FrameType::Command => Ok(TypedFrame::Command(Command::from_payload(&frame.payload)?)),
            FrameType::AckServoCmd => Ok(TypedFrame::AckServoCmd(ServoCmdWrapper::from_payload(
                &frame.payload,
            )?)),
            FrameType::AckFirmwareUpdate => {
                if frame.payload.len() < 5 {
                    return Err(FrameError::PayloadTooShort {
                        expected: 5,
                        got: frame.payload.len(),
                    });
                }
                let success = frame.payload[0] != 0;
                let offset = u32::from_le_bytes([
                    frame.payload[1],
                    frame.payload[2],
                    frame.payload[3],
                    frame.payload[4],
                ]);
                Ok(TypedFrame::AckFirmwareUpdate { success, offset })
            }
            FrameType::AckCommand => {
                if frame.payload.is_empty() {
                    return Err(FrameError::PayloadTooShort {
                        expected: 1,
                        got: 0,
                    });
                }
                Ok(TypedFrame::AckCommand {
                    success: frame.payload[0] != 0,
                })
            }
            // 下行帧类型不应在接收端解析
            FrameType::CfgWrite | FrameType::CfgQuery | FrameType::CfgQueryAll => {
                Err(FrameError::PayloadDecode("Downlink frame not expected"))
            }
            FrameType::Unknown(_v) => Err(FrameError::PayloadDecode("Unknown frame type")),
        }
    }

    pub fn frame_type(&self) -> FrameType {
        match self {
            TypedFrame::Imu(_) => FrameType::Imu,
            TypedFrame::Power(_) => FrameType::Power,
            TypedFrame::Config(_) => FrameType::Config,
            TypedFrame::Battery(_) => FrameType::Battery,
            TypedFrame::System(_) => FrameType::System,
            TypedFrame::Event(_) => FrameType::Event,
            TypedFrame::Log(_) => FrameType::Log,
            TypedFrame::AckCfgWrite { .. } => FrameType::AckCfgWrite,
            TypedFrame::AckCfgQuery(_) => FrameType::AckCfgQuery,
            TypedFrame::AckCfgQueryAll(_) => FrameType::AckCfgQueryAll,
            TypedFrame::AckServoCmd(_) => FrameType::AckServoCmd,
            TypedFrame::AckFirmwareUpdate { .. } => FrameType::AckFirmwareUpdate,
            TypedFrame::AckCommand { .. } => FrameType::AckCommand,
            TypedFrame::ConfigWrite(_) => FrameType::CfgWrite,
            TypedFrame::ConfigQuery(_) => FrameType::CfgQuery,
            TypedFrame::ConfigQueryAll => FrameType::CfgQueryAll,
            TypedFrame::ServoForward(_) => FrameType::ServoForward,
            TypedFrame::FirmwareUpdate(_) => FrameType::FirmwareUpdate,
            TypedFrame::Command(_) => FrameType::Command,
        }
    }
}

impl RawFrame {
    /// 解析为类型化帧
    pub fn parse_typed(&self) -> Result<TypedFrame, FrameError> {
        TypedFrame::from_raw(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_type_from_u8() {
        assert_eq!(FrameType::from_u8(0x01), FrameType::Imu);
        assert_eq!(FrameType::from_u8(0x80), FrameType::CfgWrite);
        assert_eq!(FrameType::from_u8(0xC0), FrameType::AckCfgWrite);
        assert_eq!(FrameType::from_u8(0xFF), FrameType::Unknown(0xFF));
    }

    /// 读取 #[repr(u8)] 枚举的判别值（测试专用）。
    /// FrameType 含 Unknown(u8) 数据变体，Rust 不允许用 `as u8` 强转，
    /// 故通过内存读取判别值，用于验证判别值与 as_u8() 一致。
    fn discriminant(v: FrameType) -> u8 {
        unsafe { *(&v as *const FrameType as *const u8) }
    }

    #[test]
    fn test_frame_type_wire_values() {
        // 枚举判别值必须与线上编解码值（as_u8）一致，
        // 防止新增协议类型时漏改枚举判别值导致漂移
        let variants = [
            FrameType::Imu,
            FrameType::Power,
            FrameType::Config,
            FrameType::Battery,
            FrameType::System,
            FrameType::Event,
            FrameType::Log,
            FrameType::CfgWrite,
            FrameType::CfgQuery,
            FrameType::CfgQueryAll,
            FrameType::ServoForward,
            FrameType::FirmwareUpdate,
            FrameType::Command,
            FrameType::AckCfgWrite,
            FrameType::AckCfgQuery,
            FrameType::AckCfgQueryAll,
            FrameType::AckServoCmd,
            FrameType::AckFirmwareUpdate,
            FrameType::AckCommand,
        ];
        for v in variants {
            assert_eq!(
                discriminant(v),
                v.as_u8(),
                "discriminant mismatch for {:?}",
                v
            );
        }
        // 关键线上值（0x03 保留，Config 从 0x04 起）
        assert_eq!(FrameType::Config.as_u8(), 0x04);
        assert_eq!(FrameType::Battery.as_u8(), 0x05);
        assert_eq!(FrameType::System.as_u8(), 0x06);
        assert_eq!(FrameType::Event.as_u8(), 0x07);
        assert_eq!(FrameType::Log.as_u8(), 0x08);
    }

    #[test]
    fn test_frame_type_roundtrip() {
        // from_u8(as_u8(v)) == v，编解码对称
        for v in [
            FrameType::Imu,
            FrameType::Power,
            FrameType::Config,
            FrameType::Battery,
            FrameType::System,
            FrameType::Event,
            FrameType::Log,
            FrameType::CfgWrite,
            FrameType::CfgQuery,
            FrameType::CfgQueryAll,
            FrameType::ServoForward,
            FrameType::FirmwareUpdate,
            FrameType::Command,
            FrameType::AckCfgWrite,
            FrameType::AckCfgQuery,
            FrameType::AckCfgQueryAll,
            FrameType::AckServoCmd,
            FrameType::AckFirmwareUpdate,
            FrameType::AckCommand,
        ] {
            assert_eq!(
                FrameType::from_u8(v.as_u8()),
                v,
                "roundtrip failed for {:?}",
                v
            );
        }
    }

    #[test]
    fn test_raw_frame_encode_decode() {
        let frame = RawFrame {
            frame_type: FrameType::Imu,
            payload: vec![0x01, 0x02, 0x03, 0x04],
        };
        let encoded = frame.encode();
        let (decoded, consumed) = RawFrame::decode(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.frame_type, frame.frame_type);
        assert_eq!(decoded.payload, frame.payload);
    }
}
