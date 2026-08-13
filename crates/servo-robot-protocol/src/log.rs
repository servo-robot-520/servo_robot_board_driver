//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/5 09:40
//! Log message

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum LogLevel {
    #[default]
    OFF = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl LogLevel {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => LogLevel::OFF,
            1 => LogLevel::Debug,
            2 => LogLevel::Info,
            3 => LogLevel::Warn,
            _ => Self::OFF,
        }
    }
}

impl core::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OFF => write!(f, "OFF"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// 日志消息
///
/// Payload 格式: `[level:1][file_name\0][fun_name\0][msg...]`
/// file_name 和 fun_name 以 null 结尾，msg 取剩余全部字节（UTF-8）
#[derive(Debug, Clone, Default)]
pub struct LogMessage {
    pub level: LogLevel,
    pub file_name: String,
    pub fun_name: String,
    pub msg: String,
}

impl LogMessage {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.is_empty() {
            return Err(FrameError::PayloadTooShort {
                expected: 1,
                got: 0,
            });
        }

        let level = LogLevel::from_u8(data[0]);
        let rest = &data[1..];

        // 查找第一个 null 分隔 file_name
        let file_end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(FrameError::PayloadDecode(
                "missing null terminator for file_name",
            ))?;
        let file_name = core::str::from_utf8(&rest[..file_end])
            .map_err(|_| FrameError::PayloadDecode("invalid utf8 in file_name"))?
            .to_string();

        let rest = &rest[file_end + 1..];

        // 查找第二个 null 分隔 fun_name
        let fun_end = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(FrameError::PayloadDecode(
                "missing null terminator for fun_name",
            ))?;
        let fun_name = core::str::from_utf8(&rest[..fun_end])
            .map_err(|_| FrameError::PayloadDecode("invalid utf8 in fun_name"))?
            .to_string();

        // 剩余为 msg
        let msg_bytes = &rest[fun_end + 1..];
        let msg = core::str::from_utf8(msg_bytes)
            .map_err(|_| FrameError::PayloadDecode("invalid utf8 in msg"))?
            .to_string();

        Ok(LogMessage {
            level,
            file_name,
            fun_name,
            msg,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let file_bytes = self.file_name.as_bytes();
        let fun_bytes = self.fun_name.as_bytes();
        let msg_bytes = self.msg.as_bytes();

        // 1 (level) + file + 1 (null) + fun + 1 (null) + msg
        let cap = 1 + file_bytes.len() + 1 + fun_bytes.len() + 1 + msg_bytes.len();
        let mut buf = Vec::with_capacity(cap);

        buf.push(self.level as u8);
        buf.extend_from_slice(file_bytes);
        buf.push(0);
        buf.extend_from_slice(fun_bytes);
        buf.push(0);
        buf.extend_from_slice(msg_bytes);

        buf
    }
}

impl ToPayload for LogMessage {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

impl FromPayload for LogMessage {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for LogMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "[{}] {}::{}: {}",
            self.level, self.file_name, self.fun_name, self.msg
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_encode_decode_roundtrip() {
        let log = LogMessage {
            level: LogLevel::Warn,
            file_name: "main.c".into(),
            fun_name: "app_init".into(),
            msg: "sensor timeout".into(),
        };
        let bytes = log.to_bytes();
        let decoded = LogMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.level, LogLevel::Warn);
        assert_eq!(decoded.file_name, "main.c");
        assert_eq!(decoded.fun_name, "app_init");
        assert_eq!(decoded.msg, "sensor timeout");
    }

    #[test]
    fn test_log_display() {
        let log = LogMessage {
            level: LogLevel::Error,
            file_name: "imu.c".into(),
            fun_name: "read_gyro".into(),
            msg: "bus busy".into(),
        };
        assert_eq!(log.to_string(), "[ERROR] imu.c::read_gyro: bus busy");
    }

    #[test]
    fn test_log_empty_payload() {
        assert!(LogMessage::from_bytes(&[]).is_err());
    }

    #[test]
    fn test_log_minimal() {
        // level(Info=2) + "\0" + "\0" + "" (empty msg)
        let bytes = vec![0x02, 0, 0];
        let log = LogMessage::from_bytes(&bytes).unwrap();
        assert_eq!(log.level, LogLevel::Info);
        assert_eq!(log.file_name, "");
        assert_eq!(log.fun_name, "");
        assert_eq!(log.msg, "");
    }
}
