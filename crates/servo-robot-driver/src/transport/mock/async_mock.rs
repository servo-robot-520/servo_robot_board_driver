//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 21:43

//! 异步模拟传输层实现

use super::mock_core::MockCore;
use crate::error::DriverError;
use crate::transport::async_trait::AsyncTransport;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// 异步模拟传输层
#[cfg(all(feature = "mock", feature = "async"))]
pub struct AsyncMockTransport {
    core: MockCore,
}

#[cfg(all(feature = "mock", feature = "async"))]
impl AsyncMockTransport {
    pub fn new() -> Self {
        log::info!("AsyncMockTransport created (mock mode)");
        AsyncMockTransport {
            core: MockCore::new(),
        }
    }

    pub fn set_battery_soc(&mut self, percentage: f32) {
        self.core.set_battery_soc(percentage);
    }
    pub fn set_initial_attitude(&mut self, roll_deg: f32, pitch_deg: f32, yaw_deg: f32) {
        self.core.set_initial_attitude(roll_deg, pitch_deg, yaw_deg);
    }
    pub fn set_charging(&mut self, charging: bool) {
        self.core.set_charging(charging);
    }
    pub fn set_charging_probability(&mut self, p: f64) {
        self.core.set_charging_probability(p);
    }
    pub fn set_auto_disconnect(&mut self, after_frames: u64) {
        self.core.set_auto_disconnect(after_frames);
    }
    pub fn disconnect(&mut self) {
        self.core.disconnect();
    }
    pub fn reconnect(&mut self) {
        self.core.reconnect();
    }
    pub fn written_frames(&self) -> &[Vec<u8>] {
        self.core.written_frames()
    }
}

#[cfg(all(feature = "mock", feature = "async"))]
impl AsyncTransport for AsyncMockTransport {
    fn read_frame(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, DriverError>> + Send + '_>> {
        Box::pin(async move {
            if self.core.check_disconnect() {
                return Err(DriverError::TransportClosed);
            }

            if let Some(frame) = self.core.try_read_frame() {
                log::debug!("[async_mock] RX: {:02X?} ({} bytes)", frame, frame.len());
                return Ok(frame);
            }

            self.core.generate_frames();
            match self.core.try_read_frame() {
                Some(frame) => {
                    log::debug!("[async_mock] RX: {:02X?} ({} bytes)", frame, frame.len());
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    Ok(frame)
                }
                None => {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    Err(DriverError::Io("timed out".to_string()))
                }
            }
        })
    }

    fn write_frame(
        &mut self,
        frame: &[u8],
    ) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>> {
        let frame = frame.to_vec();
        Box::pin(async move {
            if !self.core.connected {
                return Err(DriverError::TransportClosed);
            }
            log::debug!("[async_mock] TX: {:02X?} ({} bytes)", frame, frame.len());
            self.core.prepare_write(&frame);
            Ok(())
        })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = Result<(), DriverError>> + Send + '_>> {
        log::info!("AsyncMockTransport closed");
        Box::pin(async {
            self.core.connected = false;
            Ok(())
        })
    }
}
