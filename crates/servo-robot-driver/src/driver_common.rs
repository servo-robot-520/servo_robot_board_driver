//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 21:43

//! Driver shares functions

use crate::dispatch::DriverEvent;
use crate::protocol::command::Command;
use crate::protocol::config::{Config, ConfigType};
use crate::protocol::frame::{FrameType, RawFrame, ToPayload, TypedFrame};
use crate::protocol::servo::ServoCmdWrapper;
use crate::state::DriverState;
use std::sync::Arc;

/// 构建 CfgWrite 帧并编码
pub(crate) fn encode_cfg_write(config: &Config) -> Vec<u8> {
    RawFrame {
        frame_type: FrameType::CfgWrite,
        payload: config.to_bytes(),
    }
    .encode()
}

/// 构建 CfgQuery 帧并编码
pub(crate) fn encode_cfg_query(config_type: ConfigType) -> Vec<u8> {
    RawFrame {
        frame_type: FrameType::CfgQuery,
        payload: vec![config_type as u8],
    }
    .encode()
}

/// 构建 CfgQueryAll 帧并编码
pub(crate) fn encode_cfg_query_all() -> Vec<u8> {
    RawFrame {
        frame_type: FrameType::CfgQueryAll,
        payload: vec![],
    }
    .encode()
}

/// 构建 ServoForward 帧并编码
pub(crate) fn encode_servo_forward(cmd: &ServoCmdWrapper) -> Vec<u8> {
    RawFrame {
        frame_type: FrameType::ServoForward,
        payload: cmd.to_payload(),
    }
    .encode()
}

/// 构建 Command 帧并编码
pub(crate) fn encode_command(cmd: &Command) -> Vec<u8> {
    RawFrame {
        frame_type: FrameType::Command,
        payload: cmd.to_payload(),
    }
    .encode()
}

/// 构建 FirmwareUpdate 帧并编码
pub(crate) fn encode_firmware_update(offset: u32, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + data.len());
    payload.extend_from_slice(&offset.to_le_bytes());
    payload.extend_from_slice(data);
    RawFrame {
        frame_type: FrameType::FirmwareUpdate,
        payload,
    }
    .encode()
}

/// 解码原始帧数据并分发为 DriverEvent
///
/// 返回 `None` 表示应 continue（未知帧、解码失败），`Some(event)` 表示需要分发的事件。
pub(crate) fn decode_and_dispatch(
    frame_data: &[u8],
    state: &Arc<DriverState>,
) -> Option<DriverEvent> {
    let raw_frame = match RawFrame::decode(frame_data) {
        Ok((frame, _)) => frame,
        Err(e) => {
            log::warn!("Frame decode error: {}", e);
            state.increment_frames_dropped();
            return None;
        }
    };

    let typed_frame = match raw_frame.parse_typed() {
        Ok(frame) => frame,
        Err(e) => {
            log::warn!("Frame parse error: {}", e);
            state.increment_frames_dropped();
            return None;
        }
    };

    state.increment_frames_parsed();

    let event = match typed_frame {
        TypedFrame::Imu(data) => {
            state.update_imu(data.clone());
            DriverEvent::ImuData(data)
        }
        TypedFrame::Power(data) => {
            state.update_power(data.clone());
            DriverEvent::PowerData(data)
        }
        TypedFrame::Battery(bat) => {
            state.update_battery(bat.clone());
            DriverEvent::BatteryState(bat)
        }
        TypedFrame::Config(config) => {
            state.update_config(config.clone());
            DriverEvent::ConfigSnapshot(config)
        }
        TypedFrame::Event(event) => {
            state.update_event(event.clone());
            DriverEvent::BoardEvent(event)
        }
        TypedFrame::DeviceInfo(info) => {
            state.update_device_info(info.clone());
            DriverEvent::DeviceInfo(info)
        }
        TypedFrame::Diagnostic(diag) => {
            state.update_diagnostic(diag.clone());
            DriverEvent::Diagnostic(diag)
        }
        TypedFrame::Log(log_msg) => {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state.update_log(ts, log_msg.clone());
            DriverEvent::Log(ts, log_msg)
        }
        TypedFrame::AckCfgWrite { success } => DriverEvent::AckCfgWrite { success },
        TypedFrame::AckCfgQuery(config) => DriverEvent::AckCfgQuery(config),
        TypedFrame::AckCfgQueryAll(config_snapshot) => {
            state.update_config(config_snapshot.clone());
            DriverEvent::AckCfgQueryAll(config_snapshot)
        }
        TypedFrame::AckServoCmd(cmd) => DriverEvent::AckServoCmd(cmd),
        TypedFrame::AckCommand { success } => DriverEvent::AckCommand { success },
        TypedFrame::AckFirmwareUpdate { success, offset } => {
            DriverEvent::AckFirmwareUpdate { success, offset }
        }
        _ => return None,
    };

    Some(event)
}
