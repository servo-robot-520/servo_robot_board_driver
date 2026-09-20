//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:35

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// PowerData,The transmitted data is an int16.
/// For example, the servo voltage transmits 866, but in reality, it is 866/10 = 86.6
#[derive(Debug, Clone, Default)]
pub struct PowerData {
    // Servo power supply output voltage
    pub servo_voltage_mv: u16,
    // Servo power supply outputs current
    pub servo_current_ma: u16,
    // Charging input voltage
    pub charge_in_voltage_mv: u16,
    // Charging input current
    pub charge_in_current_ma: u16,
    // Battery voltage
    pub bat_voltage_mv: u16,
    // Battery current, reserved field currently empty
    pub bat_current_ma: i16,
    pub bat_out1_current_ma: u16,
    pub bat_out2_current_ma: u16,
    pub pwr_5v_current_ma: u16,
}

impl PowerData {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 12 {
            return Err(FrameError::PayloadTooShort {
                expected: 12,
                got: data.len(),
            });
        }
        let mut offset = 0;
        let servo_voltage_mv = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let servo_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let charge_in_voltage_mv = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let charge_in_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let bat_voltage_mv = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let bat_current_ma = i16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let bat_out1_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let bat_out2_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let pwr_5v_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
        Ok(PowerData {
            servo_voltage_mv,
            servo_current_ma,
            charge_in_voltage_mv,
            charge_in_current_ma,
            bat_voltage_mv,
            bat_current_ma,
            bat_out1_current_ma,
            bat_out2_current_ma,
            pwr_5v_current_ma,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(12);
        buf.extend_from_slice(&self.servo_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.servo_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_in_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.charge_in_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.bat_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out1_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out2_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_current_ma.to_le_bytes());
        buf
    }
}

impl ToPayload for PowerData {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for PowerData {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for PowerData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Convert u16 (*10) to f32 for display
        write!(
            f,
            "servo={:.1}V/{:.1}A pd_in={:.1}V/{:.1}A bat={:.1}V/{:.1}A out1={:.1}A out2={:.1}A 5v={:.1}A",
            self.servo_voltage_mv as f32 / 10.0,
            self.servo_current_ma as f32 / 10.0,
            self.charge_in_voltage_mv as f32 / 10.0,
            self.charge_in_current_ma as f32 / 10.0,
            self.bat_voltage_mv as f32 / 10.0,
            self.bat_current_ma as f32 / 10.0,
            self.bat_out1_current_ma as f32 / 10.0,
            self.bat_out2_current_ma as f32 / 10.0,
            self.pwr_5v_current_ma as f32 / 10.0,
        )
    }
}
