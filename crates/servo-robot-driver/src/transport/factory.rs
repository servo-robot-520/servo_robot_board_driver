//! 传输层工厂 trait

use crate::error::DriverError;
use crate::transport::Transport;

/// 传输层工厂 trait
///
/// 用于创建新的传输层连接，支持自动重连
pub trait TransportFactory: Send + Sync + 'static {
    /// 创建新的传输层连接
    fn create(&self) -> Result<Box<dyn Transport>, DriverError>;
}

/// 闭包工厂适配器
pub struct FnTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn Transport>, DriverError> + Send + Sync + 'static,
{
    factory: F,
}

impl<F> FnTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn Transport>, DriverError> + Send + Sync + 'static,
{
    pub fn new(factory: F) -> Self {
        FnTransportFactory { factory }
    }
}

impl<F> TransportFactory for FnTransportFactory<F>
where
    F: Fn() -> Result<Box<dyn Transport>, DriverError> + Send + Sync + 'static,
{
    fn create(&self) -> Result<Box<dyn Transport>, DriverError> {
        (self.factory)()
    }
}
