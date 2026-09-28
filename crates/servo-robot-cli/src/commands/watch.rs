//! `watch <kind>` — 连续输出 STM32 上报的数据（回调驱动，Ctrl-C 退出）

use crate::cli::{Cli, Kind};
use crate::output;
use crate::session;
use serde_json::Value;
use servo_robot_driver::DriverCallback;
use servo_robot_driver::DriverError;
use servo_robot_protocol::battery_state::BatteryState;
use servo_robot_protocol::diagnostic::Diagnostic;
use servo_robot_protocol::event::BoardEvent;
use servo_robot_protocol::imu::ImuData;
use servo_robot_protocol::log::LogMessage;
use servo_robot_protocol::power::PowerData;

/// 按 kind 过滤上行帧并逐行输出（文本单行 / JSON 为 NDJSON）
pub struct WatchCallback {
    kinds: Vec<Kind>,
    json: bool,
}

impl WatchCallback {
    fn want(&self, k: Kind) -> bool {
        self.kinds.contains(&k)
    }

    fn emit(&self, text: String, value: Value) {
        if self.json {
            println!(
                "{}",
                serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
            );
        } else {
            println!("{text}");
        }
    }
}

impl DriverCallback for WatchCallback {
    fn on_imu_data(&mut self, d: &ImuData) {
        if self.want(Kind::Imu) {
            self.emit(output::watch_imu(d), output::imu_json(d));
        }
    }

    fn on_power_data(&mut self, d: &PowerData) {
        if self.want(Kind::Power) {
            self.emit(output::watch_power(d), output::power_json(d));
        }
    }

    fn on_battery_state(&mut self, d: &BatteryState) {
        if self.want(Kind::Battery) {
            self.emit(output::watch_battery(d), output::battery_json(d));
        }
    }

    fn on_board_event(&mut self, e: &BoardEvent) {
        if self.want(Kind::Event) {
            self.emit(output::watch_event(e), output::event_json(e));
        }
    }

    fn on_diagnostic(&mut self, d: &Diagnostic) {
        if self.want(Kind::Diagnostic) {
            self.emit(output::watch_diagnostic(d), output::diagnostic_json(d));
        }
    }

    fn on_log(&mut self, ts: u64, m: &LogMessage) {
        if self.want(Kind::Log) {
            self.emit(output::watch_log(m), output::log_json(ts, m));
        }
    }

    fn on_error(&mut self, e: &DriverError) {
        eprintln!("driver error: {e}");
    }
}

/// 阻塞持续输出，直到 Ctrl-C（默认信号行为终止进程）
pub fn run(cli: &Cli, kinds: &[Kind]) -> i32 {
    let kinds = if kinds.is_empty() {
        Kind::ALL.to_vec()
    } else {
        kinds.to_vec()
    };
    let cb = WatchCallback {
        kinds,
        json: cli.json,
    };

    match session::start_with_watch(cli, cb) {
        Ok(driver) => {
            // 驱动保持存活；Ctrl-C(SIGINT) 默认终止进程，无需额外信号处理
            let _driver = driver;
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            e.report();
            e.exit_code()
        }
    }
}
