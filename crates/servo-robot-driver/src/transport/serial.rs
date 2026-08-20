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

/// 读错误映射:超时转结构化 `IoTimeout`(驱动读循环据此静默重试),
/// 其余保持 `Io`
fn map_read_err(e: std::io::Error) -> DriverError {
    if e.kind() == std::io::ErrorKind::TimedOut {
        DriverError::IoTimeout
    } else {
        DriverError::Io(e.to_string())
    }
}

/// 从 Read trait 对象读取一帧数据
///
/// 统一的帧读取逻辑，供 SerialTransport 使用。
pub(crate) fn read_frame_from_reader(
    port: &mut dyn Read,
    port_name: &str,
) -> Result<Vec<u8>, DriverError> {
    // 读取帧头
    //
    // 超时必须返回 `IoTimeout` 而不是继续循环:否则空闲串口(无任何字节)上
    // 本函数永不返回,读线程无法回到 `running` 检查,`stop()` join 永久挂死。
    let mut header = [0u8; 1];
    loop {
        match port.read_exact(&mut header) {
            Ok(()) => {
                if header[0] == 0xAA {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                return Err(DriverError::IoTimeout);
            }
            Err(e) => return Err(DriverError::Io(e.to_string())),
        }
    }

    // 读取 TYPE
    let mut type_buf = [0u8; 1];
    port.read_exact(&mut type_buf).map_err(map_read_err)?;

    // 读取 LEN (2 bytes, little-endian)
    let mut len_buf = [0u8; 2];
    port.read_exact(&mut len_buf).map_err(map_read_err)?;
    let payload_len = u16::from_le_bytes(len_buf) as usize;

    // 读取 PAYLOAD
    let mut payload = vec![0u8; payload_len];
    port.read_exact(&mut payload).map_err(map_read_err)?;

    // 读取 CRC (2 bytes)
    let mut crc_buf = [0u8; 2];
    port.read_exact(&mut crc_buf).map_err(map_read_err)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    /// 恒超时的空闲端口:帧头扫描循环必须快速返回 `IoTimeout`,
    /// 不能无限循环(曾导致读线程永不返回、`stop()` join 永久挂死)。
    #[test]
    fn test_scan_loop_returns_on_timeout() {
        struct IdleReader;
        impl std::io::Read for IdleReader {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "idle"))
            }
        }
        let mut port = IdleReader;
        let start = std::time::Instant::now();
        let result = read_frame_from_reader(&mut port, "test");
        assert!(matches!(result, Err(DriverError::IoTimeout)));
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "idle port must return promptly, not spin"
        );
    }
}
