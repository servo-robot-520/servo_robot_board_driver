//! `get <kind>` — 单次获取 STM32 上报的数据（轮询状态快照至超时）

use crate::cli::{Cli, GetKind, Kind};
use crate::output;
use crate::session;
use crate::{CmdError, finish};
use serde_json::Value;
use servo_robot_driver::state::StateSnapshot;
use std::time::{Duration, Instant};

pub fn run(cli: &Cli, kind: GetKind) -> i32 {
    finish(execute(cli, kind))
}

fn execute(cli: &Cli, kind: GetKind) -> Result<(), CmdError> {
    let kinds = kind.to_kinds();
    let mut driver = session::start(cli)?;
    let state = driver.state();
    let deadline = Instant::now() + Duration::from_millis(cli.timeout);

    let (missing, snap) = loop {
        let s = state.snapshot();
        let m: Vec<Kind> = kinds.iter().copied().filter(|k| !present(&s, *k)).collect();
        if m.is_empty() {
            break (Vec::new(), s);
        }
        if Instant::now() >= deadline {
            break (m, s);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = driver.stop();

    if !missing.is_empty() {
        let names = missing
            .iter()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CmdError::runtime(format!(
            "timeout after {}ms: no frame received for: {names}",
            cli.timeout
        )));
    }

    print_outputs(&kinds, snap, cli.json)
}

fn present(snap: &StateSnapshot, k: Kind) -> bool {
    match k {
        Kind::Imu => snap.imu.is_some(),
        Kind::Power => snap.power.is_some(),
        Kind::Battery => snap.battery.is_some(),
        Kind::Event => snap.event.is_some(),
        Kind::Diagnostic => snap.diagnostic.is_some(),
        Kind::Log => snap.logs.back().is_some(),
    }
}

fn print_outputs(kinds: &[Kind], snap: StateSnapshot, json: bool) -> Result<(), CmdError> {
    let mut texts: Vec<(&str, String)> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    for k in kinds {
        let missing = || CmdError::runtime(format!("internal: {} frame missing", k.as_str()));
        let (text, value) = match k {
            Kind::Imu => {
                let d = snap.imu.as_ref().ok_or_else(missing)?;
                (output::imu_text(d), output::imu_json(d))
            }
            Kind::Power => {
                let d = snap.power.as_ref().ok_or_else(missing)?;
                (output::power_text(d), output::power_json(d))
            }
            Kind::Battery => {
                let d = snap.battery.as_ref().ok_or_else(missing)?;
                (output::battery_text(d), output::battery_json(d))
            }
            Kind::Event => {
                let d = snap.event.as_ref().ok_or_else(missing)?;
                (output::event_text(d), output::event_json(d))
            }
            Kind::Diagnostic => {
                let d = snap.diagnostic.as_ref().ok_or_else(missing)?;
                (output::diagnostic_text(d), output::diagnostic_json(d))
            }
            Kind::Log => {
                let e = snap.logs.back().ok_or_else(missing)?;
                (
                    output::log_text(e.ts, &e.msg),
                    output::log_json(e.ts, &e.msg),
                )
            }
        };
        texts.push((k.as_str(), text));
        values.push(value);
    }

    if json {
        let out = if values.len() == 1 {
            values.pop().unwrap_or(Value::Null)
        } else {
            Value::Array(values)
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into())
        );
    } else {
        let multi = texts.len() > 1;
        for (i, (name, text)) in texts.iter().enumerate() {
            if multi {
                if i > 0 {
                    println!();
                }
                println!("=== {name} ===");
            }
            println!("{text}");
        }
    }
    Ok(())
}
