//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 20:15

//! 异步传输层 trait

use crate::error::DriverError;
use std::future::Future;
use std::pin::Pin;

/// 异步传输层抽象
///
/// 使用 tokio 异步运行时，支持非阻塞 I/O
#[cfg(feature = "async")]
pub trait AsyncTransport: Send + 'static {
    /// 异步读取一帧原始数据
    fn read_frame(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, DriverError>> + Send + '_>>;

    /// 异步写入一帧原始数据
    fn write_frame(
        &mut self,
        frame: &[u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>>;

    /// 异步关闭传输
    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>>;
}

/// 异步传输层工厂 trait
#[cfg(feature = "async")]
pub trait AsyncTransportFactory: Send + Sync + 'static {
    /// 创建新的异步传输层连接
    fn create(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn AsyncTransport>, DriverError>> + Send + '_>>;
}

/// 闭包工厂适配器
#[cfg(feature = "async")]
pub struct FnAsyncTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn AsyncTransport>, DriverError> + Send + Sync + 'static,
{
    factory: F,
}

#[cfg(feature = "async")]
impl<F> FnAsyncTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn AsyncTransport>, DriverError> + Send + Sync + 'static,
{
    pub fn new(factory: F) -> Self {
        FnAsyncTransportFactory { factory }
    }
}

#[cfg(feature = "async")]
impl<F> AsyncTransportFactory for FnAsyncTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn AsyncTransport>, DriverError> + Send + Sync + 'static,
{
    fn create(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn AsyncTransport>, DriverError>> + Send + '_>>
    {
        let result = (self.factory)();
        Box::pin(async move { result })
    }
}
