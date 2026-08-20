//! 应答帧定义
//!
//! 所有应答统一为 Response 帧，payload 首字节为 RequestType 表示应答哪个请求。

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use crate::request::RequestType;
use alloc::vec::Vec;

/// Response 帧 — 统一的应答帧结构
///
/// Wire format: FrameType(0xC0) + payload[request_type:1][success:1][data:N]
#[derive(Debug, Clone)]
pub struct Response {
    pub request_type: RequestType,
    pub success: bool,
    /// 应答附加数据（如 Config、BoardConfigSnapshot、DeviceInfo、ServoCmdWrapper 等）
    pub data: Vec<u8>,
}

impl Response {
    pub fn new(request_type: RequestType, success: bool, data: Vec<u8>) -> Self {
        Self {
            request_type,
            success,
            data,
        }
    }

    /// 简单应答（无附加数据）: Reset, Shutdown, Ota, ConfigWrite
    pub fn simple(request_type: RequestType, success: bool) -> Self {
        Self {
            request_type,
            success,
            data: Vec::new(),
        }
    }
}

impl ToPayload for Response {
    fn to_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + self.data.len());
        buf.push(self.request_type as u8);
        buf.push(self.success as u8);
        buf.extend_from_slice(&self.data);
        buf
    }
}

impl FromPayload for Response {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        if payload.len() < 2 {
            return Err(FrameError::PayloadTooShort {
                expected: 2,
                got: payload.len(),
            });
        }
        let request_type = RequestType::from_u8(payload[0])
            .ok_or(FrameError::PayloadDecode("Unknown request in response"))?;
        let success = payload[1] != 0;
        Ok(Self {
            request_type,
            success,
            data: payload[2..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_simple() {
        let resp = Response::simple(RequestType::Reset, true);
        let payload = resp.to_payload();
        assert_eq!(payload, vec![0x01, 0x01]);
        let decoded = Response::from_payload(&payload).unwrap();
        assert_eq!(decoded.request_type, RequestType::Reset);
        assert!(decoded.success);
        assert!(decoded.data.is_empty());
    }

    #[test]
    fn test_response_with_data() {
        let resp = Response::new(RequestType::DeviceInfo, true, vec![1, 2, 3]);
        let payload = resp.to_payload();
        assert_eq!(payload, vec![0x13, 0x01, 1, 2, 3]);
        let decoded = Response::from_payload(&payload).unwrap();
        assert_eq!(decoded.request_type, RequestType::DeviceInfo);
        assert!(decoded.success);
        assert_eq!(decoded.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_response_failure() {
        let resp = Response::simple(RequestType::ConfigWrite, false);
        let payload = resp.to_payload();
        assert_eq!(payload, vec![0x10, 0x00]);
        let decoded = Response::from_payload(&payload).unwrap();
        assert!(!decoded.success);
    }

    #[test]
    fn test_response_short_payload() {
        assert!(Response::from_payload(&[0x01]).is_err());
    }
}
