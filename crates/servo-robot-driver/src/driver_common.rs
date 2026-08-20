//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 21:43

//! Driver shares functions

use crate::dispatch::DriverEvent;
use crate::error::DriverError;
use crate::protocol::config::BoardConfigSnapshot;
use crate::protocol::frame::{FrameType, RawFrame, ToPayload, TypedFrame};
use crate::protocol::request::{Request, RequestType};
use crate::state::DriverState;
use std::sync::Arc;

/// 构建 Request 帧并编码（统一入口，替代原 6 个 encode_* 函数）
///
/// 超出协议 payload 上限(255B)的请求直接拒绝,避免生成 MCU 无法解析的帧
/// (LEN 截断/固件缓冲区溢出)。
pub(crate) fn encode_request(request: &Request) -> Result<Vec<u8>, DriverError> {
    let payload = request.to_payload();
    if payload.len() > crate::protocol::frame::MAX_PAYLOAD_SIZE {
        return Err(DriverError::PayloadTooLarge {
            max: crate::protocol::frame::MAX_PAYLOAD_SIZE,
            got: payload.len(),
        });
    }
    Ok(RawFrame {
        frame_type: FrameType::Request,
        payload,
    }
    .encode())
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
        TypedFrame::Request(_) => {
            // 下行帧不应在驱动接收端出现
            return None;
        }
        TypedFrame::Response(response) => {
            // 根据 request_type 更新内部状态
            match response.request_type {
                RequestType::ConfigQueryAll | RequestType::ConfigQuery => {
                    if response.success
                        && let Ok(snapshot) = BoardConfigSnapshot::from_bytes(&response.data)
                    {
                        state.update_config(snapshot);
                    }
                }
                RequestType::DeviceInfo => {
                    if response.success
                        && let Ok(info) =
                            crate::protocol::device_info::DeviceInfo::from_bytes(&response.data)
                    {
                        state.update_device_info(info);
                    }
                }
                _ => {}
            }
            DriverEvent::Response(response)
        }
    };

    Some(event)
}
