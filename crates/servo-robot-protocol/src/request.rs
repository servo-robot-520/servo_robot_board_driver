//! 请求帧定义
//!
//! 所有下行操作统一为 Request 帧，RequestKind 首字节区分具体操作类型。

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// 请求类型 — 下行 Request 帧 payload 首字节
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestKind {
    // ═══ 系统控制 (0x01~0x0F, fire-and-forget, 无应答) ═══
    /// 重启 MCU
    Reset = 0x01,
    /// 关机（切断全部电源）
    Shutdown = 0x02,
    /// 触发 OTA 更新（bootloader 拷贝 OTA Temp → App 后重启）
    Ota = 0x03,

    // ═══ 配置/查询 (0x10~0x1F, 需要应答) ═══
    /// 写入单个配置项
    ConfigWrite = 0x10,
    /// 查询单个配置项
    ConfigQuery = 0x11,
    /// 查询所有配置
    ConfigQueryAll = 0x12,
    /// 查询设备标识与内存布局（静态信息）
    DeviceInfo = 0x13,

    // ═══ 外设转发 (0x20~0x2F, 需要应答) ═══
    /// 转发舵机命令
    ServoForward = 0x20,
    /// 固件更新数据块
    FirmwareUpdate = 0x21,
}

impl RequestKind {
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

    /// PC 端是否应等待应答
    ///
    /// Reset/Shutdown/Ota 是 fire-and-forget：固件执行后立即重启或断电，
    /// 应答大概率丢失，PC 端不应阻塞等待。
    pub fn expects_response(&self) -> bool {
        !matches!(self, Self::Reset | Self::Shutdown | Self::Ota)
    }
}

impl core::fmt::Display for RequestKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Request 帧 — 统一的下行帧结构
///
/// Wire format: FrameType(0x80) + payload[request_kind:1][data:N]
#[derive(Debug, Clone)]
pub struct Request {
    pub kind: RequestKind,
    /// 去掉 kind 首字节后的原始 payload
    pub data: Vec<u8>,
}

impl Request {
    pub fn new(kind: RequestKind, data: Vec<u8>) -> Self {
        Self { kind, data }
    }

    /// 纯命令（无额外数据）: Reset, Shutdown, Ota, ConfigQueryAll, DeviceInfo
    pub fn simple(kind: RequestKind) -> Self {
        Self {
            kind,
            data: Vec::new(),
        }
    }
}

impl ToPayload for Request {
    fn to_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.data.len());
        buf.push(self.kind as u8);
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
        let kind =
            RequestKind::from_u8(payload[0]).ok_or(FrameError::PayloadDecode("Unknown request"))?;
        Ok(Self {
            kind,
            data: payload[1..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_simple_encode_decode() {
        let req = Request::simple(RequestKind::Reset);
        let payload = req.to_payload();
        assert_eq!(payload, vec![0x01]);
        let decoded = Request::from_payload(&payload).unwrap();
        assert_eq!(decoded.kind, RequestKind::Reset);
        assert!(decoded.data.is_empty());
    }

    #[test]
    fn test_request_with_data() {
        let req = Request::new(RequestKind::ConfigQuery, vec![0x10]);
        let payload = req.to_payload();
        assert_eq!(payload, vec![0x11, 0x10]);
        let decoded = Request::from_payload(&payload).unwrap();
        assert_eq!(decoded.kind, RequestKind::ConfigQuery);
        assert_eq!(decoded.data, vec![0x10]);
    }

    #[test]
    fn test_request_empty_payload() {
        assert!(Request::from_payload(&[]).is_err());
    }

    #[test]
    fn test_request_unknown_kind() {
        assert!(Request::from_payload(&[0xFF]).is_err());
    }

    #[test]
    fn test_expects_response() {
        assert!(!RequestKind::Reset.expects_response());
        assert!(!RequestKind::Shutdown.expects_response());
        assert!(!RequestKind::Ota.expects_response());
        assert!(RequestKind::ConfigWrite.expects_response());
        assert!(RequestKind::ConfigQuery.expects_response());
        assert!(RequestKind::ConfigQueryAll.expects_response());
        assert!(RequestKind::DeviceInfo.expects_response());
        assert!(RequestKind::ServoForward.expects_response());
        assert!(RequestKind::FirmwareUpdate.expects_response());
    }
}
