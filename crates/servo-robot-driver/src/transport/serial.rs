//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! 基于 serialport crate 的传输层实现

use crate::error::DriverError;
use crate::transport::Transport;
use serialport::SerialPort;
use std::io::Read;
use std::io::Write;
use std::time::Duration;

/// 串口传输层实现
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    port_name: String,
}

impl SerialTransport {
    /// 创建新的串口传输层
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        SerialTransport {
            port,
            port_name: "<raw>".into(),
        }
    }

    /// 打开串口
    pub fn open(port_name: &str, baud_rate: u32) -> Result<Self, DriverError> {
        log::info!("Opening serial port: {} @ {} baud", port_name, baud_rate);
        let port = serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(100))
            .open()?;
        log::info!("Serial port opened: {}", port_name);
        Ok(SerialTransport {
            port,
            port_name: port_name.into(),
        })
    }
}

/// 从 Read trait 对象读取一帧数据
///
/// 统一的帧读取逻辑，供 SerialTransport 和 TokioSerialTransport 共用。
pub(crate) fn read_frame_from_reader(
    port: &mut dyn Read,
    port_name: &str,
) -> Result<Vec<u8>, DriverError> {
    // 读取帧头
    let mut header = [0u8; 1];
    loop {
        match port.read_exact(&mut header) {
            Ok(()) => {
                if header[0] == 0xAA {
                    break;
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    continue;
                }
                return Err(DriverError::Io(e.to_string()));
            }
        }
    }

    // 读取 TYPE
    let mut type_buf = [0u8; 1];
    port.read_exact(&mut type_buf)?;

    // 读取 LEN (2 bytes, little-endian)
    let mut len_buf = [0u8; 2];
    port.read_exact(&mut len_buf)?;
    let payload_len = u16::from_le_bytes(len_buf) as usize;

    // 读取 PAYLOAD
    let mut payload = vec![0u8; payload_len];
    port.read_exact(&mut payload)?;

    // 读取 CRC (2 bytes)
    let mut crc_buf = [0u8; 2];
    port.read_exact(&mut crc_buf)?;

    // 组装完整帧
    let mut frame = Vec::with_capacity(4 + payload_len + 2);
    frame.push(0xAA);
    frame.push(type_buf[0]);
    frame.extend_from_slice(&len_buf);
    frame.extend_from_slice(&payload);
    frame.extend_from_slice(&crc_buf);

    log::trace!(
        "[{}] RX: {:02X?} (type=0x{:02X}, len={})",
        port_name,
        frame,
        type_buf[0],
        payload_len
    );

    Ok(frame)
}

impl Transport for SerialTransport {
    fn read_frame(&mut self) -> Result<Vec<u8>, DriverError> {
        read_frame_from_reader(&mut *self.port, &self.port_name)
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), DriverError> {
        log::trace!(
            "[{}] TX: {:02X?} ({} bytes)",
            self.port_name,
            frame,
            frame.len()
        );
        self.port.write_all(frame)?;
        self.port.flush()?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), DriverError> {
        log::info!("Closing serial port: {}", self.port_name);
        // serialport crate 会在 Drop 时自动关闭
        Ok(())
    }
}
