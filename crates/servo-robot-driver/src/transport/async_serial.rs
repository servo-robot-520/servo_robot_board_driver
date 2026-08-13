//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 20:24

//! 基于 tokio 的异步串口传输层实现

use crate::error::DriverError;
use crate::transport::async_trait::AsyncTransport;
use crate::transport::serial::read_frame_from_reader;
use serialport::SerialPort;
use std::future::Future;
use std::io::Write;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 异步串口传输层
///
/// 使用 `tokio::task::spawn_blocking` 包装同步的 `serialport` crate
#[cfg(feature = "async")]
pub struct TokioSerialTransport {
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    port_name: String,
}

#[cfg(feature = "async")]
impl TokioSerialTransport {
    /// 创建新的异步串口传输层
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        TokioSerialTransport {
            port: Arc::new(Mutex::new(port)),
            port_name: "<raw>".into(),
        }
    }

    /// 打开串口
    pub fn open(port_name: &str, baud_rate: u32) -> Result<Self, DriverError> {
        log::info!(
            "Opening async serial port: {} @ {} baud",
            port_name,
            baud_rate
        );
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()?;
        log::info!("Async serial port opened: {}", port_name);
        Ok(TokioSerialTransport {
            port: Arc::new(Mutex::new(port)),
            port_name: port_name.into(),
        })
    }
}

#[cfg(feature = "async")]
impl AsyncTransport for TokioSerialTransport {
    fn read_frame(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, DriverError>> + Send + '_>> {
        let port = self.port.clone();
        let port_name = self.port_name.clone();

        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let mut port = port.lock().map_err(|_| DriverError::LockPoisoned)?;
                read_frame_from_reader(&mut *port, &port_name)
            })
            .await
            .map_err(|e| DriverError::Io(e.to_string()))?
        })
    }

    fn write_frame(
        &mut self,
        frame: &[u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>> {
        let port = self.port.clone();
        let frame = frame.to_vec();
        let port_name = self.port_name.clone();

        Box::pin(async move {
            log::debug!("[{}] TX: {:02X?} ({} bytes)", port_name, frame, frame.len());
            tokio::task::spawn_blocking(move || {
                let mut port = port.lock().map_err(|_| DriverError::LockPoisoned)?;
                port.write_all(&frame)?;
                port.flush()?;
                Ok(())
            })
            .await
            .map_err(|e| DriverError::Io(e.to_string()))?
        })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>> {
        log::info!("Closing async serial port: {}", self.port_name);
        Box::pin(async { Ok(()) })
    }
}
