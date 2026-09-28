//! 驱动会话：打开传输层（串口 / Mock）、注册回调、启停驱动

use crate::CmdError;
use crate::cli::Cli;
use crate::commands::watch::WatchCallback;
use servo_robot_driver::{Driver, MockTransport, SerialTransport};

/// 打开传输层并启动驱动（不注册回调）
pub fn start(cli: &Cli) -> Result<Driver, CmdError> {
    let mut driver = build(cli)?;
    driver
        .start()
        .map_err(|e| CmdError::runtime(format!("driver start failed: {e}")))?;
    Ok(driver)
}

/// 打开传输层、注册 watch 回调并启动驱动
///
/// 回调必须在 start 之前注册，避免漏掉启动瞬间的上行帧。
pub fn start_with_watch(cli: &Cli, cb: WatchCallback) -> Result<Driver, CmdError> {
    let mut driver = build(cli)?;
    driver.register_callback(cb);
    driver
        .start()
        .map_err(|e| CmdError::runtime(format!("driver start failed: {e}")))?;
    Ok(driver)
}

fn build(cli: &Cli) -> Result<Driver, CmdError> {
    if cli.mock {
        return Ok(Driver::new(MockTransport::new()));
    }
    let port = cli.port.as_deref().ok_or_else(|| {
        CmdError::runtime("missing --port <DEV> (env: SR_PORT); use --mock for hardware-free mode")
    })?;
    let transport = SerialTransport::open(port, cli.baud)
        .map_err(|e| CmdError::runtime(format!("open serial port {port} failed: {e}")))?;
    Ok(Driver::new(transport))
}
