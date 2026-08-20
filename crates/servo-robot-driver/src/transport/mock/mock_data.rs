//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 21:43

//! 模拟数据生成器

use rand::RngExt;
use std::f32::consts::PI;
use std::time::Instant;

/// IMU 模拟状态
pub struct ImuSimulator {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub gyro_bias: [f32; 3],
    pub start_time: Instant,
    pub last_update: Instant,
}

impl Default for ImuSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl ImuSimulator {
    pub fn new() -> Self {
        let now = Instant::now();
        let mut rng = rand::rng();
        ImuSimulator {
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
            gyro_bias: [
                rng.random_range(-0.5f32..0.5f32),
                rng.random_range(-0.5f32..0.5f32),
                rng.random_range(-0.1f32..0.1f32),
            ],
            start_time: now,
            last_update: now,
        }
    }

    pub fn generate(&mut self) -> crate::protocol::imu::ImuData {
        let mut rng = rand::rng();
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        // 更剧烈的姿态变化
        let drift_rate = 0.05f32;
        self.roll += drift_rate * dt + rng.random_range(-0.02f32..0.02f32);
        self.pitch += drift_rate * dt * 0.7 + rng.random_range(-0.02f32..0.02f32);
        self.yaw += drift_rate * dt * 0.5 + rng.random_range(-0.01f32..0.01f32);

        if self.roll > PI {
            self.roll -= 2.0 * PI;
        }
        if self.roll < -PI {
            self.roll += 2.0 * PI;
        }
        self.pitch = self.pitch.clamp(-PI / 2.0, PI / 2.0);
        if self.yaw > PI {
            self.yaw -= 2.0 * PI;
        }
        if self.yaw < -PI {
            self.yaw += 2.0 * PI;
        }

        let (sr, cr) = (self.roll / 2.0).sin_cos();
        let (sp, cp) = (self.pitch / 2.0).sin_cos();
        let (sy, cy) = (self.yaw / 2.0).sin_cos();
        let qw = cr * cp * cy + sr * sp * sy;
        let qx = sr * cp * cy - cr * sp * sy;
        let qy = cr * sp * cy + sr * cp * sy;
        let qz = cr * cp * sy - sr * sp * cy;

        let gravity = 9.81f32;
        let accel_x = -gravity * self.pitch.sin() + rng.random_range(-0.05f32..0.05f32);
        let accel_y = gravity * self.roll.sin() + rng.random_range(-0.05f32..0.05f32);
        let accel_z =
            gravity * self.pitch.cos() * self.roll.cos() + rng.random_range(-0.05f32..0.05f32);

        let gyro_x = self.gyro_bias[0] + rng.random_range(-0.1f32..0.1f32);
        let gyro_y = self.gyro_bias[1] + rng.random_range(-0.1f32..0.1f32);
        let gyro_z = self.gyro_bias[2] + rng.random_range(-0.05f32..0.05f32);

        let timestamp_ms = now.duration_since(self.start_time).as_millis() as u32;

        crate::protocol::imu::ImuData {
            accel: [accel_x, accel_y, accel_z],
            gyro: [gyro_x, gyro_y, gyro_z],
            quaternion: [qw, qx, qy, qz],
            timestamp_ms,
            roll: self.roll.to_degrees(),
            pitch: self.pitch.to_degrees(),
            yaw: self.yaw.to_degrees(),
        }
    }

    pub fn set_attitude(&mut self, roll_deg: f32, pitch_deg: f32, yaw_deg: f32) {
        self.roll = roll_deg.to_radians();
        self.pitch = pitch_deg.to_radians();
        self.yaw = yaw_deg.to_radians();
    }
}

/// 电源模拟状态
pub struct PowerSimulator {
    pub battery_voltage: f32,
    pub charging: bool,
}

impl Default for PowerSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerSimulator {
    pub fn new() -> Self {
        PowerSimulator {
            battery_voltage: 16.8f32,
            charging: false,
        }
    }

    pub fn generate(&mut self) -> crate::protocol::power::PowerData {
        let mut rng = rand::rng();

        // 充电缓慢上升(≤16.8V),放电缓慢下降(≥12.0V)
        let delta = if self.charging { 0.005f32 } else { -0.002f32 };
        self.battery_voltage = (self.battery_voltage + delta).clamp(12.0, 16.8);

        let (charge_voltage, charge_current): (f32, f32) = if self.charging {
            (
                20.0 + rng.random_range(-1.0f32..1.0f32),
                5.0 + rng.random_range(-4.5f32..5.0f32),
            )
        } else {
            (0.0, 0.0)
        };

        // Convert to u16: multiply by10 for protocol
        let servo_voltage = 8.6 + rng.random_range(-0.5f32..0.5f32);
        let servo_current = 12.5 + rng.random_range(-7.5f32..7.5f32);
        let bat_voltage = self.battery_voltage + rng.random_range(-0.3f32..0.3f32);
        let bat_current =
            if self.charging { 1.5 } else { -1.0 } + rng.random_range(-0.5f32..0.5f32);

        crate::protocol::power::PowerData {
            servo_voltage_mv: (servo_voltage * 10.0) as u16,
            servo_current_ma: (servo_current * 10.0) as u16,
            charge_in_voltage_mv: (charge_voltage * 10.0) as u16,
            charge_in_current_ma: (charge_current.max(0.0) * 10.0) as u16,
            bat_voltage_mv: (bat_voltage * 10.0) as u16,
            bat_current_ma: (bat_current * 10.0) as i16,
        }
    }
}

/// 温度模拟状态
pub struct ThermalSimulator {
    pub base_temp: f32,
    pub runtime_secs: f32,
}

/// 电池状态模拟
pub struct BatterySimulator {
    // 0~1
    pub percentage: f32,
    pub charging: bool,
    pub cell_count: usize,
}

impl Default for BatterySimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl BatterySimulator {
    pub fn new() -> Self {
        BatterySimulator {
            percentage: 0.8,
            charging: false,
            cell_count: 4,
        }
    }

    pub fn generate(&mut self) -> crate::protocol::battery_state::BatteryState {
        let mut rng = rand::rng();

        if self.charging {
            self.percentage = (self.percentage + 0.01f32).min(1.0);
        } else {
            self.percentage = (self.percentage - 0.005f32).max(0.0);
        }

        // 每节电芯电压随电量变化：满电4.2V，空电3.0V，限制在0~4.4V
        let cell_base = 3.0f32 + self.percentage * 1.2;
        let cell_voltages_f32: Vec<f32> = (0..self.cell_count)
            .map(|_| (cell_base + rng.random_range(-0.02f32..0.02f32)).clamp(0.0, 4.4))
            .collect();
        let voltage_f32: f32 = cell_voltages_f32.iter().sum();
        let cell_temps_f32: Vec<f32> = (0..self.cell_count)
            .map(|_| 28.0f32 + rng.random_range(-2.0f32..2.0f32))
            .collect();

        // 放电电流在0.5-20A之间波动
        let current_f32 = if self.charging {
            5.0f32 + rng.random_range(-4.5f32..5.0f32)
        } else {
            10.0f32 + rng.random_range(-9.5f32..10.0f32)
        };

        // 实际充满容量在4000~5200mAh之间波动
        let capacity_f32 = 4600.0f32 + rng.random_range(-600.0f32..600.0f32);

        // Convert to protocol types: voltage in mV, current in mA, capacity in mAh
        // percentage: 1~100, temperature: *10
        crate::protocol::battery_state::BatteryState {
            voltage_mv: (voltage_f32 * 1000.0) as u16,
            current_ma: (current_f32 * 1000.0) as i16,
            capacity_mah: capacity_f32 as u16,
            design_capacity_mah: 5000,
            percentage: (self.percentage * 100.0) as u8,
            temperature: ((28.0 + rng.random_range(-1.0f32..1.0)) * 10.0) as i16,
            charge_status: if self.charging {
                crate::protocol::battery_state::BatteryChargeStatus::Charging
            } else {
                crate::protocol::battery_state::BatteryChargeStatus::Discharging
            },
            health: crate::protocol::battery_state::BatteryHealth::Good,
            technology: crate::protocol::battery_state::BatteryTechnology::LiPo,
            present: true,
            serial_number: 12345,
            cell_voltages_mv: cell_voltages_f32
                .iter()
                .map(|v| (v * 1000.0) as u16)
                .collect(),
            cell_temperatures: cell_temps_f32.iter().map(|t| (t * 10.0) as i16).collect(),
        }
    }
}

/// 设备信息模拟（静态信息）
pub struct DeviceInfoSimulator {
    pub generated: bool,
}

impl Default for DeviceInfoSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceInfoSimulator {
    pub fn new() -> Self {
        DeviceInfoSimulator { generated: false }
    }

    pub fn generate(&mut self) -> crate::protocol::device_info::DeviceInfo {
        self.generated = true;
        crate::protocol::device_info::DeviceInfo {
            device_id: 0x4832, // STM32F4 device ID
            uid: 0x12345678,   // 模拟唯一ID
            imu_id: 0x70,      // IMU ID
            firmware_version: crate::protocol::device_info::Version::new(0, 1, 0),
            ram_kb: 128,
            flash_boot_kb: 16,
            flash_app_kb: 240,
            flash_ota_kb: 128,
            flash_user_kb: 128,
        }
    }
}

/// 诊断数据模拟（运行时状态）
pub struct DiagnosticSimulator {
    pub start_time: Instant,
    pub frame_count: u32,
    pub charging: bool,
    pub base_temp: f32,
    pub runtime_secs: f32,
}

impl Default for DiagnosticSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticSimulator {
    pub fn new() -> Self {
        DiagnosticSimulator {
            start_time: Instant::now(),
            frame_count: 0,
            charging: false,
            base_temp: 25.0f32,
            runtime_secs: 0.0,
        }
    }

    pub fn generate(&mut self, dt: f32) -> crate::protocol::diagnostic::Diagnostic {
        let mut rng = rand::rng();
        self.frame_count += 1;
        self.runtime_secs += dt;
        let uptime = self.start_time.elapsed().as_secs();

        // PD 握手信息（充电时固定 20V/5A）
        let (pd_voltage, pd_current) = if self.charging { (20000, 5000) } else { (0, 0) };

        // 温度模拟
        let warmup = (self.runtime_secs / 60.0).min(1.0) * 10.0;
        let temp_servo = self.base_temp + warmup + 15.0 + rng.random_range(-1.0f32..1.0);
        let temp_5v = self.base_temp + warmup + 10.0 + rng.random_range(-1.0f32..1.0);
        let temp_mcu = self.base_temp + warmup + 5.0 + rng.random_range(-0.5f32..0.5);
        let temp_charge = self.base_temp + warmup + 20.0 + rng.random_range(-1.0f32..1.0);
        let temp_bat = self.base_temp + warmup + 2.0 + rng.random_range(-0.5f32..0.5);

        crate::protocol::diagnostic::Diagnostic {
            uptime_s: uptime as u32,
            cpu_usage_percent: 35 + rng.random_range(0u8..15),
            free_heap_kb: 120 + rng.random_range(0u16..30),
            stack_watermark_min_kb: 8 + rng.random_range(0u16..4),
            i2c_error_count: 0,
            spi_error_count: 0,
            uart_error_count: 0,
            usb_error_count: 0,
            frames_sent_total: self.frame_count,
            pd_request_voltage_mv: pd_voltage,
            pd_request_current_ma: pd_current,
            temp_servo_power: (temp_servo * 10.0) as i16,
            temp_5v_power: (temp_5v * 10.0) as i16,
            temp_mcu: (temp_mcu * 10.0) as i16,
            temp_charge: (temp_charge * 10.0) as i16,
            temp_battery: (temp_bat * 10.0) as i16,
        }
    }
}

/// 事件模拟状态
pub struct EventSimulator {
    pub charging: bool,
    pub event_count: u32,
}

impl Default for EventSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl EventSimulator {
    pub fn new() -> Self {
        EventSimulator {
            charging: false,
            event_count: 0,
        }
    }

    pub fn generate(&mut self) -> crate::protocol::event::BoardEvent {
        self.event_count += 1;

        let charge_phase = if self.charging {
            // 模拟充电阶段变化
            match self.event_count % 100 {
                0..=30 => crate::protocol::event::ChargePhase::Cc,
                31..=60 => crate::protocol::event::ChargePhase::Cv,
                61..=90 => crate::protocol::event::ChargePhase::Full,
                _ => crate::protocol::event::ChargePhase::NotCharging,
            }
        } else {
            crate::protocol::event::ChargePhase::NotCharging
        };

        // 偶尔触发保护事件
        let protection_flags = if self.event_count.is_multiple_of(500) {
            crate::protocol::event::ProtectionFlags::SERVO_OVERCURRENT
        } else {
            crate::protocol::event::ProtectionFlags::empty()
        };

        // 偶尔触发错误事件
        let error_flags = if self.event_count.is_multiple_of(800) {
            crate::protocol::event::ErrorFlags::UART1_ERROR
        } else if self.event_count.is_multiple_of(1200) {
            crate::protocol::event::ErrorFlags::I2C1_ERROR
        } else {
            crate::protocol::event::ErrorFlags::empty()
        };

        use crate::protocol::event::StateChangeFlags;
        let mut state_change_flags = StateChangeFlags::empty();
        if self.charging {
            state_change_flags |= StateChangeFlags::CHARGER_CONNECTED;
        }
        if self.event_count % 100 < 50 {
            state_change_flags |= StateChangeFlags::FAN_ENABLED;
        }

        crate::protocol::event::BoardEvent {
            charge_phase,
            state_change_flags,
            protection_flags,
            error_flags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::frame::{FrameType, RawFrame};

    #[test]
    fn test_battery_voltage_range() {
        let mut sim = BatterySimulator::new();
        sim.percentage = 1.0; // 满电测试
        for _ in 0..100 {
            let state = sim.generate();
            // cell_voltages are now u16 (mV), check range 0~4400mV
            for (i, v) in state.cell_voltages_mv.iter().enumerate() {
                assert!(
                    *v <= 4400,
                    "Cell{} voltage {} mV out of range [0, 4400]",
                    i + 1,
                    v
                );
            }
        }
    }

    #[test]
    fn test_battery_encode_decode_roundtrip() {
        let mut sim = BatterySimulator::new();
        sim.percentage = 1.0;
        let state = sim.generate();

        let payload = state.to_bytes();
        let payload_len = payload.len();

        let frame = RawFrame {
            frame_type: FrameType::Battery,
            payload,
        };
        let encoded = frame.encode();
        let (decoded, _) = RawFrame::decode(&encoded).unwrap();

        assert_eq!(
            decoded.payload.len(),
            payload_len,
            "Payload length mismatch: {} vs {}",
            decoded.payload.len(),
            payload_len
        );

        let state2 =
            crate::protocol::battery_state::BatteryState::from_bytes(&decoded.payload).unwrap();

        // cell_voltages are u16 (mV), check exact match
        for (i, (v1, v2)) in state
            .cell_voltages_mv
            .iter()
            .zip(state2.cell_voltages_mv.iter())
            .enumerate()
        {
            assert_eq!(v1, v2, "Cell{} voltage mismatch: {} vs {}", i + 1, v1, v2);
        }
    }
}
