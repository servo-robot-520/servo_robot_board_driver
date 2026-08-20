//! 运行时诊断数据
//!
//! CPU 占用、内存、错误计数、温度等运行时状态信息。

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// 运行时诊断数据（原 SystemInfo 去掉设备标识字段）
///
/// 包含运行时状态：CPU 占用、堆栈、错误计数、PD 参数、温度等。
/// 与 `DeviceInfo` 不同，本结构体的数据在运行期间持续变化。
#[derive(Debug, Clone, Default)]
pub struct Diagnostic {
    /// 运行时间 (s)
    pub uptime_s: u32,
    /// CPU 占用率 (%)
    pub cpu_usage_percent: u8,
    /// 空闲堆 (KB)
    pub free_heap_kb: u16,
    /// 启动以来最小剩余栈空间 (KB)
    pub stack_watermark_min_kb: u16,
    pub i2c_error_count: u16,
    pub spi_error_count: u16,
    pub uart_error_count: u16,
    pub usb_error_count: u16,
    /// 累计发送帧数
    pub frames_sent_total: u32,
    /// PD 握手请求电压 (mV)
    pub pd_request_voltage_mv: u16,
    /// PD 握手请求电流 (mA)
    pub pd_request_current_ma: u16,

    // ═══ 温度数据 (i16, 实际值 = 原始值 / 10) ═══
    /// 舵机电源温度
    pub temp_servo_power: i16,
    /// 5V 电源温度
    pub temp_5v_power: i16,
    /// MCU 温度
    pub temp_mcu: i16,
    /// 充电电路温度
    pub temp_charge: i16,
    /// 电池温度
    pub temp_battery: i16,
}

impl Diagnostic {
    /// Payload size: 4+1+2+2+2+2+2+2+4+2+2+2+2+2+2+2 = 31 bytes
    pub const PAYLOAD_SIZE: usize = 31;

    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < Self::PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooShort {
                expected: Self::PAYLOAD_SIZE,
                got: data.len(),
            });
        }
        let mut o = 0;
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

        // Temperature data (i16, 实际值 = 原始值 / 10)
        let temp_servo_power = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_5v_power = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_mcu = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_charge = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let temp_battery = i16::from_le_bytes([data[o], data[o + 1]]);

        Ok(Diagnostic {
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
            temp_servo_power,
            temp_5v_power,
            temp_mcu,
            temp_charge,
            temp_battery,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::PAYLOAD_SIZE);
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

        // Temperature data
        buf.extend_from_slice(&self.temp_servo_power.to_le_bytes());
        buf.extend_from_slice(&self.temp_5v_power.to_le_bytes());
        buf.extend_from_slice(&self.temp_mcu.to_le_bytes());
        buf.extend_from_slice(&self.temp_charge.to_le_bytes());
        buf.extend_from_slice(&self.temp_battery.to_le_bytes());
        buf
    }
}

impl ToPayload for Diagnostic {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

impl FromPayload for Diagnostic {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "up={}s CPU={}% heap={}KB frames={} T:mcu={:.1} sv={:.1} 5v={:.1} chg={:.1} bat={:.1}°C",
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
