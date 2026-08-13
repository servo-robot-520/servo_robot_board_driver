//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:30

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BatteryChargeStatus {
    #[default]
    Unknown = 0,
    Charging = 1,
    Discharging = 2,
    NotCharging = 3,
    Full = 4,
}
impl BatteryChargeStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::NotCharging,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BatteryHealth {
    #[default]
    Unknown = 0,
    Good = 1,
    Overheat = 2,
    Dead = 3,
    Overvoltage = 4,
}
impl BatteryHealth {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Good,
            2 => Self::Overheat,
            3 => Self::Dead,
            4 => Self::Overvoltage,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum BatteryTechnology {
    #[default]
    Unknown = 0,
    NiMh = 1,
    LiOn = 2,
    LiPo = 3,
    LiFe = 4,
    NiCd = 5,
    LiMn = 6,
}
impl BatteryTechnology {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::NiMh,
            2 => Self::LiOn,
            3 => Self::LiPo,
            4 => Self::LiFe,
            5 => Self::NiCd,
            6 => Self::LiMn,
            _ => Self::Unknown,
        }
    }
}

/// 电池状态
#[derive(Debug, Clone, Default)]
pub struct BatteryState {
    // Overall battery voltage (mV)
    pub voltage_mv: u16,
    // Battery current: + indicates charging, - indicates discharge (mA)
    pub current_ma: i16,
    // Battery capacity when fully charged (mAh)
    pub capacity_mah: u16,
    // Battery design capacity (mAh)
    pub design_capacity_mah: u16,
    // Relative state of charge (SOC, range 1~100)
    pub percentage: u8,
    // overall temperature，integer. reality is transmitted / 10
    pub temperature: i16,
    pub charge_status: BatteryChargeStatus,
    pub health: BatteryHealth,
    pub technology: BatteryTechnology,
    // Whether the battery is in place
    pub present: bool,
    pub serial_number: u16,
    // Voltage of each cell (mV)
    pub cell_voltages_mv: Vec<u16>,
    // Temperature of each cell, uint16. reality is transmitted / 10
    pub cell_temperatures: Vec<i16>,
}

impl BatteryState {
    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < 20 {
            return Err(FrameError::PayloadTooShort {
                expected: 20,
                got: data.len(),
            });
        }
        let mut o = 0;
        let voltage = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let current = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let capacity = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let design_capacity = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let percentage = data[o];
        o += 1;
        let temperature = i16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let charge_status = BatteryChargeStatus::from_u8(data[o]);
        o += 1;
        let health = BatteryHealth::from_u8(data[o]);
        o += 1;
        let technology = BatteryTechnology::from_u8(data[o]);
        o += 1;
        let present = data[o] != 0;
        o += 1;
        let serial_number = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;

        let remaining = data.len() - o;
        let cell_count = remaining / 4;
        let mut cell_voltages = Vec::with_capacity(cell_count);
        let mut cell_temperatures = Vec::with_capacity(cell_count);
        for _ in 0..cell_count {
            cell_voltages.push(u16::from_le_bytes([data[o], data[o + 1]]));
            o += 2;
            cell_temperatures.push(i16::from_le_bytes([data[o], data[o + 1]]));
            o += 2;
        }

        Ok(BatteryState {
            voltage_mv: voltage,
            current_ma: current,
            capacity_mah: capacity,
            design_capacity_mah: design_capacity,
            percentage,
            temperature,
            charge_status,
            health,
            technology,
            present,
            serial_number,
            cell_voltages_mv: cell_voltages,
            cell_temperatures,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let cell_count = self.cell_voltages_mv.len();
        let mut buf = Vec::with_capacity(20 + cell_count * 4);
        buf.extend_from_slice(&self.voltage_mv.to_le_bytes());
        buf.extend_from_slice(&self.current_ma.to_le_bytes());
        buf.extend_from_slice(&self.capacity_mah.to_le_bytes());
        buf.extend_from_slice(&self.design_capacity_mah.to_le_bytes());
        buf.push(self.percentage);
        buf.extend_from_slice(&self.temperature.to_le_bytes());
        buf.push(self.charge_status as u8);
        buf.push(self.health as u8);
        buf.push(self.technology as u8);
        buf.push(self.present as u8);
        buf.extend_from_slice(&self.serial_number.to_le_bytes());
        for (v, t) in self.cell_voltages_mv.iter().zip(self.cell_temperatures.iter()) {
            buf.extend_from_slice(&v.to_le_bytes());
            buf.extend_from_slice(&t.to_le_bytes());
        }
        buf
    }
}

impl ToPayload for BatteryState {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}
impl FromPayload for BatteryState {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for BatteryState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Convert integer types to f32 for display
        // percentage: u8 1~100, voltage: u16 mV, current: i16 mA, temperature: i16 *10
        write!(
            f,
            "{:.1}% {:.1}V {:.1}A {:.1}°C {}",
            self.percentage as f32,
            self.voltage_mv as f32 / 1000.0,
            self.current_ma as f32 / 1000.0,
            self.temperature as f32 / 10.0,
            match self.charge_status {
                BatteryChargeStatus::Charging => "CHG",
                BatteryChargeStatus::Discharging => "DIS",
                BatteryChargeStatus::Full => "FULL",
                BatteryChargeStatus::NotCharging => "NC",
                BatteryChargeStatus::Unknown => "?",
            }
        )
    }
}
