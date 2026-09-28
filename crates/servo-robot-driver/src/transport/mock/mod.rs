//! 模拟传输层实现

pub(crate) mod mock_core;
pub mod mock_data;

use crate::error::DriverError;
use crate::transport::Transport;
use mock_core::MockCore;
use std::time::Duration;

/// 模拟传输层（同步）
pub struct MockTransport {
    core: MockCore,
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTransport {
    pub fn new() -> Self {
        log::info!("MockTransport created (mock mode)");
        MockTransport {
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

impl Transport for MockTransport {
    fn read_frame(&mut self) -> Result<Vec<u8>, DriverError> {
        if self.core.check_disconnect() {
            return Err(DriverError::TransportClosed);
        }

        if let Some(frame) = self.core.try_read_frame() {
            log::trace!("[mock] RX: {:02X?} ({} bytes)", frame, frame.len());
            return Ok(frame);
        }

        // 生成新数据帧后再取
        self.core.generate_frames();
        match self.core.try_read_frame() {
            Some(frame) => {
                log::trace!("[mock] RX: {:02X?} ({} bytes)", frame, frame.len());
                std::thread::sleep(Duration::from_millis(1));
                Ok(frame)
            }
            None => {
                std::thread::sleep(Duration::from_millis(5));
                Err(DriverError::IoTimeout)
            }
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), DriverError> {
        if !self.core.connected {
            return Err(DriverError::TransportClosed);
        }
        log::trace!("[mock] TX: {:02X?} ({} bytes)", frame, frame.len());
        self.core.prepare_write(frame);
        Ok(())
    }

    fn close(&mut self) -> Result<(), DriverError> {
        log::info!("MockTransport closed");
        self.core.connected = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::config::{Config, ConfigType};
    use crate::protocol::frame::{FrameType, FromPayload, RawFrame, ToPayload};
    use crate::protocol::request::{Request, RequestType};
    use crate::protocol::response::Response;

    #[test]
    fn test_mock_transport_imu() {
        let mut mock = MockTransport::new();
        mock.set_initial_attitude(10.0, 5.0, 0.0);

        std::thread::sleep(Duration::from_millis(15));

        for _ in 0..20 {
            match mock.read_frame() {
                Ok(frame) => {
                    let (raw, _) = RawFrame::decode(&frame).unwrap();
                    if raw.frame_type == FrameType::Imu {
                        let imu = crate::protocol::imu::ImuData::from_bytes(&raw.payload).unwrap();
                        assert!(imu.accel[2] > 8.0 && imu.accel[2] < 12.0);
                        assert!(imu.quaternion[0].abs() <= 1.0);
                        return;
                    }
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        panic!("No IMU frame received");
    }

    #[test]
    fn test_mock_transport_config_query() {
        let mut mock = MockTransport::new();

        let query = Request::new(
            RequestType::ConfigQuery,
            vec![ConfigType::BatOut1CurrentLimitMa as u8],
        );
        let frame = RawFrame {
            frame_type: FrameType::Request,
            payload: query.to_payload(),
        };
        mock.write_frame(&frame.encode()).unwrap();

        let response = mock.read_frame().unwrap();
        let (raw, _) = RawFrame::decode(&response).unwrap();
        assert_eq!(raw.frame_type, FrameType::Response);

        let resp = Response::from_payload(&raw.payload).unwrap();
        let config = Config::from_bytes(&resp.data).unwrap();
        assert_eq!(config.value(), 50.0);
    }

    #[test]
    fn test_mock_transport_disconnect() {
        let mut mock = MockTransport::new();
        mock.disconnect();

        let result = mock.read_frame();
        assert!(matches!(result, Err(DriverError::TransportClosed)));
    }
}
