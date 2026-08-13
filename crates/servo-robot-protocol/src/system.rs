//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:37

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl Version {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self { major, minor, patch }
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 系统信息（包含温度数据）
#[derive(Debug, Clone)]
pub struct SystemInfo {
    // STM32 device ID
    pub device_id: u16,
    // STM32 global ID
    pub uid: u32,
    // IMU's ID
    pub imu_id: u8,
    // Running time
    pub uptime_s: u32,
    // CPU Usage
    pub cpu_usage_percent: u8,
    pub free_heap_kb: u16,
    // The minimum available stack space since system startup
    pub stack_watermark_min_kb: u16,
    pub i2c_error_count: u16,
    pub spi_error_count: u16,
    pub uart_error_count: u16,
    pub usb_error_count: u16,
    // A total of frames sent
    pub frames_sent_total: u32,
    // PD handshake protocol voltage(mV)
    pub pd_request_voltage_mv: u16,
    // PD handshake protocol current(mA)
    pub pd_request_current_ma: u16,
    pub firmware_version: Version,

    // ═══ 温度数据 (i16, 实际值 = 原始值 / 10) ═══
    // Servo power supply temperature
    pub temp_servo_power: i16,
    // 5V power supply temperature
    pub temp_5v_power: i16,
    // MCU Temperature
    pub temp_mcu: i16,
    // Charging circuit temperature
    pub temp_charge: i16,
    // Battery overall temperature
    pub temp_battery: i16,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            device_id: 0,
            uid: 0,
            imu_id: 0,
            uptime_s: 0,
            cpu_usage_percent: 0,
            free_heap_kb: 0,
            stack_watermark_min_kb: 0,
            i2c_error_count: 0,
            spi_error_count: 0,
            uart_error_count: 0,
            usb_error_count: 0,
            frames_sent_total: 0,
            pd_request_voltage_mv: 0,
            pd_request_current_ma: 0,
            firmware_version: Version::new(0, 1, 0),
            temp_servo_power: 0,
            temp_5v_power: 0,
            temp_mcu: 0,
            temp_charge: 0,
            temp_battery: 0,
        }
    }
}

impl SystemInfo {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        // 31 bytes base + 10 bytes thermal = 41 bytes
        if data.len() < 41 {
            return Err(FrameError::PayloadTooShort {
                expected: 41,
                got: data.len(),
            });
        }
        let mut o = 0;
        let device_id = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let uid = u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        o += 4;
        let imu_id = data[o];
        o += 1;
        let uptime_s = u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        o += 4;
        let cpu_usage_percent = data[o];
        o += 1;
        let free_heap_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let stack_watermark_min_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let i2c_error_count = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let spi_error_count = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let uart_error_count = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let usb_error_count = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let frames_sent_total =
            u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
        o += 4;
        let pd_request_voltage_mv = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let pd_request_current_ma = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let version = Version::new(data[o], data[o + 1], data[o + 2]);
        o += 3;

        // Temperature data，the transmitted data is an integer.
        // For example, the servo power temperature transmits 571, but in reality, it is 571/10 = 57.1
        let temp_servo_power = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_5v_power = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_mcu = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_charge = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_battery = i16::from_le_bytes([data[o], data[o + 1]]);

        Ok(SystemInfo {
            device_id,
            uid,
            imu_id,
            uptime_s,
            cpu_usage_percent,
            free_heap_kb,
            stack_watermark_min_kb,
            i2c_error_count,
            spi_error_count,
            uart_error_count,
            usb_error_count,
            frames_sent_total,
            pd_request_voltage_mv,
            pd_request_current_ma,
            firmware_version: version,
            temp_servo_power,
            temp_5v_power,
            temp_mcu,
            temp_charge,
            temp_battery,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(41);
        buf.extend_from_slice(&self.device_id.to_le_bytes());
        buf.extend_from_slice(&self.uid.to_le_bytes());
        buf.push(self.imu_id);
        buf.extend_from_slice(&self.uptime_s.to_le_bytes());
        buf.push(self.cpu_usage_percent);
        buf.extend_from_slice(&self.free_heap_kb.to_le_bytes());
        buf.extend_from_slice(&self.stack_watermark_min_kb.to_le_bytes());
        buf.extend_from_slice(&self.i2c_error_count.to_le_bytes());
        buf.extend_from_slice(&self.spi_error_count.to_le_bytes());
        buf.extend_from_slice(&self.uart_error_count.to_le_bytes());
        buf.extend_from_slice(&self.usb_error_count.to_le_bytes());
        buf.extend_from_slice(&self.frames_sent_total.to_le_bytes());
        buf.extend_from_slice(&self.pd_request_voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.pd_request_current_ma.to_le_bytes());
        buf.push(self.firmware_version.major);
        buf.push(self.firmware_version.minor);
        buf.push(self.firmware_version.patch);

        // Thermal data
        buf.extend_from_slice(&self.temp_servo_power.to_le_bytes());
        buf.extend_from_slice(&self.temp_5v_power.to_le_bytes());
        buf.extend_from_slice(&self.temp_mcu.to_le_bytes());
        buf.extend_from_slice(&self.temp_charge.to_le_bytes());
        buf.extend_from_slice(&self.temp_battery.to_le_bytes());
        buf
    }
}

impl ToPayload for SystemInfo {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for SystemInfo {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for SystemInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "V{} up={}s CPU={}% heap={}KB frames={} T:mcu={:.1} sv={:.1} 5v={:.1} chg={:.1} bat={:.1}°C",
            self.firmware_version,
            self.uptime_s,
            self.cpu_usage_percent,
            self.free_heap_kb,
            self.frames_sent_total,
            self.temp_mcu as f32 / 10.0,
            self.temp_servo_power as f32 / 10.0,
            self.temp_5v_power as f32 / 10.0,
            self.temp_charge as f32 / 10.0,
            self.temp_battery as f32 / 10.0,
        )
    }
}
