//! Request Frame Definition
//!
//! All downlink operations are uniformly represented by a Request frame,
//! with the first byte of RequestType distinguishing the specific operation type.

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// Request type — Downlink Request frame payload first byte
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestType {
    // ═══ System control (0x01~0x0F, fire-and-forget, no response) ═══
    /// restart MCU
    Reset = 0x01,
    /// Power off (disconnect all power)
    Shutdown = 0x02,
    /// Trigger OTA update (bootloader copies OTA Temp → App and then restarts)
    Ota = 0x03,

    // ═══ Configuration/Query (0x10~0x1F, response required) ═══
    /// Write a single configuration item
    ConfigWrite = 0x10,
    /// Query a single configuration item
    ConfigQuery = 0x11,
    /// Query all configurations
    ConfigQueryAll = 0x12,
    /// Query device identifier and memory layout (static information)
    DeviceInfo = 0x13,

    // ═══ Peripheral forwarding (0x20~0x2F, response required) ═══
    /// Forward servo commands
    ServoForward = 0x20,
    /// Firmware update data block
    FirmwareUpdate = 0x21,
}

impl RequestType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Reset),
            0x02 => Some(Self::Shutdown),
            0x03 => Some(Self::Ota),
            0x10 => Some(Self::ConfigWrite),
            0x11 => Some(Self::ConfigQuery),
            0x12 => Some(Self::ConfigQueryAll),
            0x13 => Some(Self::DeviceInfo),
            0x20 => Some(Self::ServoForward),
            0x21 => Some(Self::FirmwareUpdate),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Reset => "Reset",
            Self::Shutdown => "Shutdown",
            Self::Ota => "OTA",
            Self::ConfigWrite => "ConfigWrite",
            Self::ConfigQuery => "ConfigQuery",
            Self::ConfigQueryAll => "ConfigQueryAll",
            Self::DeviceInfo => "DeviceInfo",
            Self::ServoForward => "ServoForward",
            Self::FirmwareUpdate => "FirmwareUpdate",
        }
    }

    /// Should the PC client wait for a response?
    ///
    /// Reset/Shutdown/Ota is fire-and-forget: the firmware will immediately reboot or power off after execution.
    /// the responses are likely to be lost; the PC should not block and wait.
    pub fn expects_response(&self) -> bool {
        !matches!(self, Self::Reset | Self::Shutdown | Self::Ota)
    }
}

impl core::fmt::Display for RequestType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Request frame — a unified downlink frame structure
///
/// Wire format: FrameType(0x80) + payload[request_type:1][data:N]
#[derive(Debug, Clone)]
pub struct Request {
    pub request_type: RequestType,
    /// The original payload after removing the first byte of kind
    pub data: Vec<u8>,
}

impl Request {
    pub fn new(request_type: RequestType, data: Vec<u8>) -> Self {
        Self { request_type, data }
    }

    /// Pure commands (no additional data): Reset, Shutdown, Ota, ConfigQueryAll, DeviceInfo
    pub fn simple(request_type: RequestType) -> Self {
        Self {
            request_type,
            data: Vec::new(),
        }
    }
}

impl ToPayload for Request {
    fn to_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.data.len());
        buf.push(self.request_type as u8);
        buf.extend_from_slice(&self.data);
        buf
    }
}

impl FromPayload for Request {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        if payload.is_empty() {
            return Err(FrameError::PayloadTooShort {
                expected: 1,
                got: 0,
            });
        }
        let request_type =
            RequestType::from_u8(payload[0]).ok_or(FrameError::PayloadDecode("Unknown request"))?;
        Ok(Self {
            request_type,
            data: payload[1..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_simple_encode_decode() {
        let req = Request::simple(RequestType::Reset);
        let payload = req.to_payload();
        assert_eq!(payload, vec![0x01]);
        let decoded = Request::from_payload(&payload).unwrap();
        assert_eq!(decoded.request_type, RequestType::Reset);
        assert!(decoded.data.is_empty());
    }

    #[test]
    fn test_request_with_data() {
        let req = Request::new(RequestType::ConfigQuery, vec![0x10]);
        let payload = req.to_payload();
        assert_eq!(payload, vec![0x11, 0x10]);
        let decoded = Request::from_payload(&payload).unwrap();
        assert_eq!(decoded.request_type, RequestType::ConfigQuery);
        assert_eq!(decoded.data, vec![0x10]);
    }

    #[test]
    fn test_request_empty_payload() {
        assert!(Request::from_payload(&[]).is_err());
    }

    #[test]
    fn test_request_unknown_type() {
        assert!(Request::from_payload(&[0xFF]).is_err());
    }

    #[test]
    fn test_expects_response() {
        assert!(!RequestType::Reset.expects_response());
        assert!(!RequestType::Shutdown.expects_response());
        assert!(!RequestType::Ota.expects_response());
        assert!(RequestType::ConfigWrite.expects_response());
        assert!(RequestType::ConfigQuery.expects_response());
        assert!(RequestType::ConfigQueryAll.expects_response());
        assert!(RequestType::DeviceInfo.expects_response());
        assert!(RequestType::ServoForward.expects_response());
        assert!(RequestType::FirmwareUpdate.expects_response());
    }
}
