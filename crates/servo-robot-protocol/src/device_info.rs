//! Device identification and memory layout information
//!
//! Static hardware information: chip ID, firmware version, hardware version, Flash/RAM partition size.

use crate::error::FrameError;
use crate::frame::{FromPayload, ToPayload};
use alloc::vec::Vec;

/// 固件版本号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl Version {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Device identification and memory layout (static information)
///
/// Includes STM32 chip identifier, IMU ID, firmware version, and Flash/RAM partition size.
/// Unlike `Diagnostic`, the data in this structure does not change during runtime.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// STM32 device ID (DBGMCU.IDCODE)
    pub device_id: u16,
    /// STM32 unique ID
    pub uid: u32,
    /// IMU chip ID
    pub imu_id: u8,
    /// firmware version
    pub firmware_version: Version,
    /// hardware version
    pub hardware_version: Version,
    /// RAM size (KB)
    pub ram_kb: u16,
    /// Bootloader flash size (KB)
    pub flash_boot_kb: u16,
    /// Application flash size (KB)
    pub flash_app_kb: u16,
    /// OTA temp flash size (KB)
    pub flash_ota_kb: u16,
    /// User data flash size (KB)
    pub flash_user_kb: u16,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_id: 0,
            uid: 0,
            imu_id: 0,
            firmware_version: Version::new(0, 1, 0),
            hardware_version: Version::new(1, 0, 0),
            ram_kb: 0,
            flash_boot_kb: 0,
            flash_app_kb: 0,
            flash_ota_kb: 0,
            flash_user_kb: 0,
        }
    }
}

impl DeviceInfo {
    /// Payload size: 2+4+1+3+2+2+2+2+2 = 20 bytes
    pub const PAYLOAD_SIZE: usize = 20;

    pub fn from_bytes(data: &[u8]) -> Result<Self, FrameError> {
        if data.len() < Self::PAYLOAD_SIZE {
            return Err(FrameError::PayloadTooShort {
                expected: Self::PAYLOAD_SIZE,
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
        let firmware_version = Version::new(data[o], data[o + 1], data[o + 2]);
        o += 3;
        let hardware_version = Version::new(data[o], data[o + 1], data[o + 2]);
        o += 3;
        let ram_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let flash_boot_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let flash_app_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let flash_ota_kb = u16::from_le_bytes([data[o], data[o + 1]]);
        o += 2;
        let flash_user_kb = u16::from_le_bytes([data[o], data[o + 1]]);

        Ok(DeviceInfo {
            device_id,
            uid,
            imu_id,
            firmware_version,
            hardware_version,
            ram_kb,
            flash_boot_kb,
            flash_app_kb,
            flash_ota_kb,
            flash_user_kb,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::PAYLOAD_SIZE);
        buf.extend_from_slice(&self.device_id.to_le_bytes());
        buf.extend_from_slice(&self.uid.to_le_bytes());
        buf.push(self.imu_id);
        buf.push(self.firmware_version.major);
        buf.push(self.firmware_version.minor);
        buf.push(self.firmware_version.patch);
        buf.push(self.hardware_version.major);
        buf.push(self.hardware_version.minor);
        buf.push(self.hardware_version.patch);
        buf.extend_from_slice(&self.ram_kb.to_le_bytes());
        buf.extend_from_slice(&self.flash_boot_kb.to_le_bytes());
        buf.extend_from_slice(&self.flash_app_kb.to_le_bytes());
        buf.extend_from_slice(&self.flash_ota_kb.to_le_bytes());
        buf.extend_from_slice(&self.flash_user_kb.to_le_bytes());
        buf
    }
}

impl ToPayload for DeviceInfo {
    fn to_payload(&self) -> Vec<u8> {
        self.to_bytes()
    }
}

impl FromPayload for DeviceInfo {
    fn from_payload(p: &[u8]) -> Result<Self, FrameError> {
        Self::from_bytes(p)
    }
}

impl core::fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ID={:#06x} UID={:#010x} IMU={:#04x} FW={} HW={} RAM={}KB Flash:Boot={} App={} OTA={} User={}KB",
            self.device_id,
            self.uid,
            self.imu_id,
            self.firmware_version,
            self.hardware_version,
            self.ram_kb,
            self.flash_boot_kb,
            self.flash_app_kb,
            self.flash_ota_kb,
            self.flash_user_kb,
        )
    }
}
