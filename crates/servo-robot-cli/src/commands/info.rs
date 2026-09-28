//! `info` — 查询设备信息（设备 ID / 版本 / 内存布局）

use crate::cli::Cli;
use crate::output;
use crate::session;
use crate::{CmdError, finish};

pub fn run(cli: &Cli) -> i32 {
    finish(execute(cli))
}

fn execute(cli: &Cli) -> Result<(), CmdError> {
    let mut driver = session::start(cli)?;
    let result = driver
        .query_device_info_sync()
        .map_err(|e| CmdError::runtime(format!("query device info failed: {e}")));
    let _ = driver.stop();
    let info = result?;

    if cli.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&output::info_json(&info)).unwrap_or_else(|_| "{}".into())
        );
    } else {
        println!("{}", output::info_text(&info));
    }
    Ok(())
}
