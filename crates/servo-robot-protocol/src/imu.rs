//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:31

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// IMU 数据
#[derive(Debug, Clone, Default)]
pub struct ImuData {
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
    pub quaternion: [f32; 4], // w, x, y, z
    pub timestamp_ms: u32,
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

impl ImuData {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 56 {
            return Err(FrameError::PayloadTooShort {
                expected: 56,
                got: data.len(),
            });
        }

        let mut offset = 0;
        let mut accel = [0.0f32; 3];
        for a in &mut accel {
            *a = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }
        let mut gyro = [0.0f32; 3];
        for g in &mut gyro {
            *g = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }
        let mut quaternion = [0.0f32; 4];
        for q in &mut quaternion {
            *q = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }
        let timestamp_ms = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let roll = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let pitch = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        let yaw = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);

        Ok(ImuData {
            accel,
            gyro,
            quaternion,
            timestamp_ms,
            roll,
            pitch,
            yaw,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(56);
        for a in &self.accel {
            buf.extend_from_slice(&a.to_le_bytes());
        }
        for g in &self.gyro {
            buf.extend_from_slice(&g.to_le_bytes());
        }
        for q in &self.quaternion {
            buf.extend_from_slice(&q.to_le_bytes());
        }
        buf.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&self.roll.to_le_bytes());
        buf.extend_from_slice(&self.pitch.to_le_bytes());
        buf.extend_from_slice(&self.yaw.to_le_bytes());
        buf
    }
}

impl ToPayload for ImuData {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

impl FromPayload for ImuData {
    fn from_payload(payload: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(payload)
    }
}

impl core::fmt::Display for ImuData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "r={:.1} p={:.1} y={:.1} a=[{:.2},{:.2},{:.2}] g=[{:.2},{:.2},{:.2}]",
            self.roll,
            self.pitch,
            self.yaw,
            self.accel[0],
            self.accel[1],
            self.accel[2],
            self.gyro[0],
            self.gyro[1],
            self.gyro[2]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imu_encode_decode() {
        let data = ImuData {
            accel: [1.0, 2.0, 3.0],
            gyro: [0.1, 0.2, 0.3],
            quaternion: [1.0, 0.0, 0.0, 0.0],
            timestamp_ms: 12345,
            roll: 45.0,
            pitch: 30.0,
            yaw: 90.0,
        };
        let bytes = data.to_bytes();
        assert_eq!(bytes.len(), 56);
        let decoded = ImuData::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.accel, data.accel);
        assert_eq!(decoded.timestamp_ms, 12345);
    }
}
