//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:35

//! 传输层抽象

pub mod factory;
pub mod frame_codec;
#[cfg(feature = "mock")]
pub mod mock;
pub mod serial;

#[cfg(feature = "async")]
pub mod async_serial;
#[cfg(feature = "async")]
pub mod async_trait;

use crate::error::DriverError;

/// 传输层抽象，支持未来替换为 tokio 异步实现
pub trait Transport: Send + 'static {
    /// 读取一帧原始数据（阻塞）
    fn read_frame(&mut self) -> Result<Vec<u8>, DriverError>;

    /// 写入一帧原始数据
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), DriverError>;

    /// 关闭传输
    fn close(&mut self) -> Result<(), DriverError>;
}

// 重导出工厂相关类型
pub use factory::{FnTransportFactory, TransportFactory};

// 重导出模拟传输层
#[cfg(feature = "mock")]
pub use mock::MockTransport;

// 重导出异步传输层
#[cfg(feature = "async")]
pub use async_serial::TokioSerialTransport;
#[cfg(feature = "async")]
pub use async_trait::{AsyncTransport, AsyncTransportFactory, FnAsyncTransportFactory};
#[cfg(all(feature = "mock", feature = "async"))]
pub use mock::AsyncMockTransport;
