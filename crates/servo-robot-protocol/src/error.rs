//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 12:01
//! Types of protocol errors

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// 未找到帧头
    NoHeader,
    /// 帧数据不完整
    Incomplete { needed: usize },
    /// CRC 校验失败
    CrcMismatch { expected: u16, got: u16 },
    /// Payload 解析错误
    PayloadDecode(&'static str),
    /// Payload 长度不足
    PayloadTooShort { expected: usize, got: usize },
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoHeader => write!(f, "No frame header found"),
            Self::Incomplete { needed } => {
                write!(f, "Incomplete frame, need {} more bytes", needed)
            }
            Self::CrcMismatch { expected, got } => {
                write!(
                    f,
                    "CRC mismatch: expected {:#06x}, got {:#06x}",
                    expected, got
                )
            }
            Self::PayloadDecode(msg) => write!(f, "Payload decode error: {}", msg),
            Self::PayloadTooShort { expected, got } => {
                write!(
                    f,
                    "Payload too short: expected {} bytes, got {}",
                    expected, got
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}
