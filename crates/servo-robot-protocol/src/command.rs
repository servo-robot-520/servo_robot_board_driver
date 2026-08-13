//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/8
//!
//! Board command definitions (one-shot actions, not persistent config)

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec;
use alloc::vec::Vec;

/// Command type enum — one-shot actions sent via `Command (0x85)` frame
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandType {
    /// Reboot the MCU
    Reset = 0x01,
    /// Shutdown (cut all power)
    Shutdown = 0x02,
    /// Trigger OTA update: bootloader copies OTA Temp → App, then reboots
    Ota = 0x03,
}

impl CommandType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Reset),
            0x02 => Some(Self::Shutdown),
            0x03 => Some(Self::Ota),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Reset => "Reset",
            Self::Shutdown => "Shutdown",
            Self::Ota => "OTA",
        }
    }
}

impl core::fmt::Display for CommandType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Command frame payload — just a command type byte
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub cmd: CommandType,
}

impl Command {
    pub fn new(cmd: CommandType) -> Self {
        Self { cmd }
    }
}

impl ToPayload for Command {
    fn to_payload(&self) -> Vec<u8> {
        vec![self.cmd as u8]
    }
}

impl FromPayload for Command {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        if payload.is_empty() {
            return Err(FrameError::PayloadTooShort {
                expected: 1,
                got: 0,
            });
        }
        let cmd = CommandType::from_u8(payload[0])
            .ok_or(FrameError::PayloadDecode("Unknown command type"))?;
        Ok(Self { cmd })
    }
}

/// AckCommand frame payload
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckCommand {
    pub success: bool,
}

impl AckCommand {
    pub fn new(success: bool) -> Self {
        Self { success }
    }
}

impl ToPayload for AckCommand {
    fn to_payload(&self) -> Vec<u8> {
        vec![self.success as u8]
    }
}

impl FromPayload for AckCommand {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        if payload.is_empty() {
            return Err(FrameError::PayloadTooShort {
                expected: 1,
                got: 0,
            });
        }
        Ok(Self {
            success: payload[0] != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_encode_decode() {
        let cmds = vec![
            Command::new(CommandType::Reset),
            Command::new(CommandType::Shutdown),
            Command::new(CommandType::Ota),
        ];
        for cmd in cmds {
            let bytes = cmd.to_payload();
            let decoded = Command::from_payload(&bytes).unwrap();
            assert_eq!(cmd, decoded);
        }
    }

    #[test]
    fn test_ack_command_encode_decode() {
        let ack = AckCommand::new(true);
        let bytes = ack.to_payload();
        let decoded = AckCommand::from_payload(&bytes).unwrap();
        assert_eq!(ack, decoded);

        let ack = AckCommand::new(false);
        let bytes = ack.to_payload();
        let decoded = AckCommand::from_payload(&bytes).unwrap();
        assert_eq!(ack, decoded);
    }
}
