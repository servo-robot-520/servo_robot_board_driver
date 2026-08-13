//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:41

//! 帧编解码器

use crate::error::FrameError;
use crate::protocol::frame::{FRAME_HEAD, RawFrame};

/// 帧编解码器
pub struct FrameCodec;

impl FrameCodec {
    /// 从字节缓冲区中解码一帧
    /// 返回 (frame, consumed_bytes)
    pub fn decode(buf: &[u8]) -> Result<(RawFrame, usize), FrameError> {
        RawFrame::decode(buf)
    }

    /// 将一帧编码为字节
    pub fn encode(frame: &RawFrame) -> Vec<u8> {
        frame.encode()
    }

    /// 在字节流中查找帧头位置
    pub fn find_header(buf: &[u8]) -> Option<usize> {
        buf.iter().position(|&b| b == FRAME_HEAD)
    }

    /// 检查缓冲区是否有完整的帧
    pub fn has_complete_frame(buf: &[u8]) -> bool {
        // 找到帧头
        let header_pos = match Self::find_header(buf) {
            Some(pos) => pos,
            None => return false,
        };

        // 检查是否有足够的数据读取帧头
        if buf.len() - header_pos < 4 {
            return false;
        }

        // 读取 payload 长度
        let payload_len = u16::from_le_bytes([buf[header_pos + 2], buf[header_pos + 3]]) as usize;

        // 检查是否有完整的帧
        let total_len = 4 + payload_len + 2;
        buf.len() - header_pos >= total_len
    }
}
