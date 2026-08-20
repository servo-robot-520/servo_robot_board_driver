//! servo-robot-protocol - STM32 Definition of communication protocols
//!
//! Supports no_std + alloc, available for PC and embedded platforms.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod battery_state;
pub mod config;
pub mod crc;
pub mod device_info;
pub mod diagnostic;
pub mod error;
pub mod event;
pub mod frame;
pub mod imu;
pub mod log;
pub mod power;
pub mod request;
pub mod response;
pub mod servo;

/// 便捷重导出
pub use device_info::Version;
pub use request::{Request, RequestType};
pub use response::Response;

/// 通用枚举转换宏
#[macro_export]
macro_rules! enum_from_u8 {
    ($ty:ty, Unknown, $( $variant:ident = $val:literal ),*) => {
        impl $ty {
            pub fn from_u8(v: u8) -> Self {
                match v {
                    $( $val => Self::$variant, )*
                    _ => Self::Unknown,
                }
            }
        }
    };
}
