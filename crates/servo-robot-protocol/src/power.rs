//! 电源相关数据

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// PowerData,The transmitted data is an int16.
/// For example, the servo voltage transmits 866, but in reality, it is 866/10 = 86.6
#[derive(Debug, Clone, Default)]
pub struct PowerData {
    // 舵机电源输出电压
    pub pwr_servo_voltage_mv: u16,
    // 舵机电源输出电流
    pub pwr_servo_current_ma: u16,
    // 充电输入电压
    pub charge_in_voltage_mv: u16,
    // 充电输入电流
    pub charge_in_current_ma: u16,
    // 电池电压
    pub bat_voltage_mv: u16,
    // 电池电流
    pub bat_current_ma: i16,
    pub bat_out1_current_ma: u16,
    pub bat_out2_current_ma: u16,
    pub pwr_5v_current_ma: u16,
    pub pwr_5v_voltage_mv: u16,
}

impl PowerData {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 20 {
            return Err(FrameError::PayloadTooShort {
                expected: 20,
                got: data.len(),
            });
        }
        let mut offset = 0;
        let pwr_servo_voltage_mv = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        let pwr_servo_current_ma = u16::from_le_bytes([data[offset], data[offset + 1]]);
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
        offset += 2;
        let pwr_5v_voltage_mv = u16::from_le_bytes([data[offset], data[offset + 1]]);
        Ok(PowerData {
            pwr_servo_voltage_mv,
            pwr_servo_current_ma,
            charge_in_voltage_mv,
            charge_in_current_ma,
            bat_voltage_mv,
            bat_current_ma,
            bat_out1_current_ma,
            bat_out2_current_ma,
            pwr_5v_current_ma,
            pwr_5v_voltage_mv,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);
        buf.extend_from_slice(&self.pwr_servo_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.pwr_servo_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.charge_in_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.charge_in_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.bat_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out1_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.bat_out2_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_current_ma.to_le_bytes());
        buf.extend_from_slice(&self.pwr_5v_voltage_mv.to_le_bytes());
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
        // u16 (×10) 转 f32 用于显示
        write!(
            f,
            "servo={:.1}V/{:.1}A pd_in={:.1}V/{:.1}A bat={:.1}V/{:.1}A out1={:.1}A out2={:.1}A 5v={:.1}V/{:.1}A",
            self.pwr_servo_voltage_mv as f32 / 10.0,
            self.pwr_servo_current_ma as f32 / 10.0,
            self.charge_in_voltage_mv as f32 / 10.0,
            self.charge_in_current_ma as f32 / 10.0,
            self.bat_voltage_mv as f32 / 10.0,
            self.bat_current_ma as f32 / 10.0,
            self.bat_out1_current_ma as f32 / 10.0,
            self.bat_out2_current_ma as f32 / 10.0,
            self.pwr_5v_voltage_mv as f32 / 10.0,
            self.pwr_5v_current_ma as f32 / 10.0,
        )
    }
}
