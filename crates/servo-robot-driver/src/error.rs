//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! Definition of error type

use thiserror::Error;

// 重导出协议层的 FrameError
pub use servo_robot_protocol::error::FrameError;

/// 驱动错误
#[derive(Error, Debug, Clone)]
pub enum DriverError {
    #[error("Serial port error: {0}")]
    Serial(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Frame parse error: {0}")]
    Frame(#[from] FrameError),

    #[error("Transport closed")]
    TransportClosed,

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("CRC mismatch: expected {expected:#06x}, got {got:#06x}")]
    CrcMismatch { expected: u16, got: u16 },

    #[error("Payload too short: expected {expected} bytes, got {got}")]
    PayloadTooShort { expected: usize, got: usize },

    #[error("Unknown frame type: {0:#04x}")]
    UnknownFrameType(u8),

    #[error("Driver not running")]
    NotRunning,

    #[error("Lock poisoned")]
    LockPoisoned,
}

impl From<serialport::Error> for DriverError {
    fn from(err: serialport::Error) -> Self {
        DriverError::Serial(err.to_string())
    }
}

impl From<std::io::Error> for DriverError {
    fn from(err: std::io::Error) -> Self {
        DriverError::Io(err.to_string())
    }
}
