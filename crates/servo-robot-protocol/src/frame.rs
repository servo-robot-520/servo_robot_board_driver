//! Frame format definition

use crate::error::FrameError;
use alloc::vec::Vec;

use crate::battery_state::BatteryState;
use crate::config::BoardConfigSnapshot;
use crate::diagnostic::Diagnostic;
use crate::event::BoardEvent;
use crate::imu::ImuData;
use crate::log::LogMessage;
use crate::power::PowerData;
use crate::request::Request;
use crate::response::Response;

/// Frame header
pub const FRAME_HEAD: u8 = 0xAA;

/// Frame header length (HEAD + TYPE + LEN)
const FRAME_HEADER_SIZE: usize = 4;
/// CRC length
const FRAME_CRC_SIZE: usize = 2;
/// Payload protocol limit (the protocol document specifies 0~255B; although LEN is u16,
/// frames exceeding the limit will be rejected,
/// and the allocation of single frames on the decode side will also be limited).
pub const MAX_PAYLOAD_SIZE: usize = 255;

/// 帧类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    // ═══ Uplink (STM32 → PC, firmware actively pushed) ═══
    Imu = 0x01,
    Power = 0x02,
    // 0x03 Retain (formerly Thermal, now merged into Diagnostic).
    Config = 0x04,
    Battery = 0x05,
    Diagnostic = 0x06,
    Event = 0x07,
    Log = 0x08,

    // ═══ Downlink (PC → STM32) ═══
    Request = 0x80,

    // ═══ Response (STM32 → PC) ═══
    Response = 0xC0,

    // ═══ Unknown type ═══
    Unknown(u8),
}

impl FrameType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0x01 => Self::Imu,
            0x02 => Self::Power,
            0x04 => Self::Config,
            0x05 => Self::Battery,
            0x06 => Self::Diagnostic,
            0x07 => Self::Event,
            0x08 => Self::Log,
            0x80 => Self::Request,
            0xC0 => Self::Response,
            _ => Self::Unknown(v),
        }
    }

    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Imu => 0x01,
            Self::Power => 0x02,
            Self::Config => 0x04,
            Self::Battery => 0x05,
            Self::Diagnostic => 0x06,
            Self::Event => 0x07,
            Self::Log => 0x08,
            Self::Request => 0x80,
            Self::Response => 0xC0,
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
                | Self::Diagnostic
                | Self::Event
                | Self::Log
        )
    }

    pub fn is_downlink(&self) -> bool {
        matches!(self, Self::Request)
    }

    pub fn is_response(&self) -> bool {
        matches!(self, Self::Response)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Imu => "IMU",
            Self::Power => "Power",
            Self::Config => "Config",
            Self::Battery => "Battery",
            Self::Diagnostic => "Diagnostic",
            Self::Event => "Event",
            Self::Log => "Log",
            Self::Request => "Request",
            Self::Response => "Response",
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
    /// Decode a frame from the byte buffer
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

        // Rejecting overloaded frames (protocol limit 255B): Corrupted/malicious streams cannot trigger large allocations.
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooLarge {
                max: MAX_PAYLOAD_SIZE,
                got: payload_len,
            });
        }

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

    /// Encoded as bytes
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

/// Types that can be serialized into frame payloads
pub trait ToPayload {
    fn to_payload(&self) -> Vec<u8>;
}

/// Types that can be deserialized from frame payload
pub trait FromPayload: Sized {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError>;
}

/// Typed Frame Enumeration
#[derive(Debug, Clone)]
pub enum TypedFrame {
    Imu(ImuData),
    Power(PowerData),
    Config(BoardConfigSnapshot),
    Battery(BatteryState),
    Diagnostic(Diagnostic),
    Event(BoardEvent),
    Log(LogMessage),

    Request(Request),

    Response(Response),
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
            FrameType::Diagnostic => Ok(TypedFrame::Diagnostic(Diagnostic::from_bytes(
                &frame.payload,
            )?)),
            FrameType::Event => Ok(TypedFrame::Event(BoardEvent::from_bytes(&frame.payload)?)),
            FrameType::Log => Ok(TypedFrame::Log(LogMessage::from_bytes(&frame.payload)?)),
            FrameType::Request => Ok(TypedFrame::Request(Request::from_payload(&frame.payload)?)),
            FrameType::Response => Ok(TypedFrame::Response(Response::from_payload(
                &frame.payload,
            )?)),
            FrameType::Unknown(_) => Err(FrameError::PayloadDecode("Unknown frame type")),
        }
    }

    pub fn frame_type(&self) -> FrameType {
        match self {
            TypedFrame::Imu(_) => FrameType::Imu,
            TypedFrame::Power(_) => FrameType::Power,
            TypedFrame::Config(_) => FrameType::Config,
            TypedFrame::Battery(_) => FrameType::Battery,
            TypedFrame::Diagnostic(_) => FrameType::Diagnostic,
            TypedFrame::Event(_) => FrameType::Event,
            TypedFrame::Log(_) => FrameType::Log,
            TypedFrame::Request(_) => FrameType::Request,
            TypedFrame::Response(_) => FrameType::Response,
        }
    }
}

impl RawFrame {
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
        assert_eq!(FrameType::from_u8(0x80), FrameType::Request);
        assert_eq!(FrameType::from_u8(0xC0), FrameType::Response);
        assert_eq!(FrameType::from_u8(0xFF), FrameType::Unknown(0xFF));
    }

    /// Read the discrimination value of the #[repr(u8)] enumeration (for testing purposes only).
    fn discriminant(v: FrameType) -> u8 {
        unsafe { *(&v as *const FrameType as *const u8) }
    }

    #[test]
    fn test_frame_type_wire_values() {
        let variants = [
            FrameType::Imu,
            FrameType::Power,
            FrameType::Config,
            FrameType::Battery,
            FrameType::Diagnostic,
            FrameType::Event,
            FrameType::Log,
            FrameType::Request,
            FrameType::Response,
        ];
        for v in variants {
            assert_eq!(
                discriminant(v),
                v.as_u8(),
                "discriminant mismatch for {:?}",
                v
            );
        }

        assert_eq!(FrameType::Config.as_u8(), 0x04);
        assert_eq!(FrameType::Battery.as_u8(), 0x05);
        assert_eq!(FrameType::Diagnostic.as_u8(), 0x06);
        assert_eq!(FrameType::Event.as_u8(), 0x07);
        assert_eq!(FrameType::Log.as_u8(), 0x08);
        assert_eq!(FrameType::Request.as_u8(), 0x80);
        assert_eq!(FrameType::Response.as_u8(), 0xC0);
    }

    #[test]
    fn test_frame_type_roundtrip() {
        for v in [
            FrameType::Imu,
            FrameType::Power,
            FrameType::Config,
            FrameType::Battery,
            FrameType::Diagnostic,
            FrameType::Event,
            FrameType::Log,
            FrameType::Request,
            FrameType::Response,
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

    /// Frames exceeding the LEN protocol limit (255B) must be rejected to prevent large allocations from being triggered.
    #[test]
    fn test_decode_rejects_oversized_payload() {
        let mut frame = vec![FRAME_HEAD, FrameType::Imu.as_u8(), 0x2C, 0x01]; // LEN=300 LE
        frame.extend_from_slice(&[0u8; 300]);
        frame.extend_from_slice(&[0, 0]);
        assert!(matches!(
            RawFrame::decode(&frame),
            Err(FrameError::PayloadTooLarge { max: 255, got: 300 })
        ));
    }
}
