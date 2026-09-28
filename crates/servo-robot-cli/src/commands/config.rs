//! `config list|get|set|types` — 板级配置查询与写入

use crate::cli::{self, Cli, ConfigCommand};
use crate::output;
use crate::session;
use crate::{CmdError, finish};
use serde_json::{Value, json};
use servo_robot_driver::Driver;
use servo_robot_protocol::config::{Config, ConfigType, get_config_value};

pub fn run(cli: &Cli, cmd: &ConfigCommand) -> i32 {
    finish(execute(cli, cmd))
}

/// 命令参数在打开串口之前完成解析：用法错误(exit 2)先于硬件错误(exit 1)
enum Parsed {
    List,
    Get(ConfigType),
    Set(ConfigType, f32),
}

fn execute(cli: &Cli, cmd: &ConfigCommand) -> Result<(), CmdError> {
    if let ConfigCommand::Types = cmd {
        return print_types(cli.json);
    }

    let parsed = match cmd {
        ConfigCommand::List => Parsed::List,
        ConfigCommand::Get { r#type } => {
            Parsed::Get(cli::parse_config_type(r#type).map_err(CmdError::usage)?)
        }
        ConfigCommand::Set { r#type, value } => {
            let ct = cli::parse_config_type(r#type).map_err(CmdError::usage)?;
            let display = cli::parse_config_value(ct, value).map_err(CmdError::usage)?;
            Parsed::Set(ct, display)
        }
        ConfigCommand::Types => unreachable!(),
    };

    let mut driver = session::start(cli)?;
    let result = match parsed {
        Parsed::List => list(&driver, cli.json),
        Parsed::Get(ct) => get(&driver, ct, cli.json),
        Parsed::Set(ct, display) => set(&driver, ct, display, cli.json),
    };
    let _ = driver.stop();
    result
}

/// 查询全量配置快照并按 ID 分组打印
fn list(driver: &Driver, json: bool) -> Result<(), CmdError> {
    let snap = driver
        .query_all_configs_sync()
        .map_err(|e| CmdError::runtime(format!("query all configs failed: {e}")))?;

    if json {
        let items: Vec<Value> = ConfigType::ALL
            .iter()
            .map(|ct| {
                let (_, wire) = output::config_parts(&get_config_value(&snap, *ct));
                output::config_item_json(*ct, wire)
            })
            .collect();
        print_json(&Value::Array(items));
        return Ok(());
    }

    let mut last_group = u8::MAX;
    for ct in ConfigType::ALL {
        let group = (*ct as u8) >> 4;
        if group != last_group {
            if last_group != u8::MAX {
                println!();
            }
            println!("{}", group_header(group));
            last_group = group;
        }
        let (_, wire) = output::config_parts(&get_config_value(&snap, *ct));
        println!(
            "  {:.<26} {}",
            ct.name(),
            output::config_list_value_text(*ct, wire)
        );
    }
    Ok(())
}

fn group_header(group: u8) -> &'static str {
    match group {
        0x1 => "power switches:",
        0x2 => "current limits (mA):",
        0x3 => "temperature limits (°C):",
        _ => "misc:",
    }
}

/// 查询单项配置（请求-应答）
fn get(driver: &Driver, ct: ConfigType, json: bool) -> Result<(), CmdError> {
    let cfg = driver
        .query_config_sync(ct)
        .map_err(|e| CmdError::runtime(format!("query {} failed: {e}", ct.variant_name())))?;
    let (got_ct, wire) = output::config_parts(&cfg);
    if json {
        print_json(&output::config_item_json(got_ct, wire));
    } else {
        println!(
            "{} = {}",
            got_ct.variant_name(),
            output::config_value_text(got_ct, wire)
        );
    }
    Ok(())
}

/// 写入单项配置（展示单位 → ×10 线上值）并打印板端 ACK 结果
fn set(driver: &Driver, ct: ConfigType, display: f32, json: bool) -> Result<(), CmdError> {
    let wire = output::display_to_wire(ct, display);
    let ack = driver
        .write_config_sync(Config::from_type_value(ct, wire))
        .map_err(|e| CmdError::runtime(format!("write {} failed: {e}", ct.variant_name())))?;

    if json {
        print_json(&json!({
            "type": ct.variant_name(),
            "id": format!("0x{:02X}", ct as u8),
            "value": output::round1(display) as f64,
            "ack": ack,
        }));
    } else {
        println!(
            "{} = {} → ack: {}",
            ct.variant_name(),
            output::config_value_text(ct, wire),
            if ack { "success" } else { "failed" }
        );
    }

    if !ack {
        return Err(CmdError::runtime(format!(
            "board rejected config write: {}",
            ct.variant_name()
        )));
    }
    Ok(())
}

/// 列出全部配置类型（不依赖硬件）
fn print_types(json: bool) -> Result<(), CmdError> {
    if json {
        let items: Vec<Value> = ConfigType::ALL
            .iter()
            .map(|ct| {
                json!({
                    "type": ct.variant_name(),
                    "id": format!("0x{:02X}", *ct as u8),
                    "display": ct.name(),
                    "unit": unit_label(*ct),
                    "size": ct.value_size().unwrap_or(0),
                })
            })
            .collect();
        print_json(&Value::Array(items));
        return Ok(());
    }

    for ct in ConfigType::ALL {
        println!(
            "0x{:02X}  {:<26} {:<6} {}",
            *ct as u8,
            ct.variant_name(),
            unit_label(*ct),
            ct.value_size().unwrap_or(0)
        );
    }
    Ok(())
}

fn unit_label(ct: ConfigType) -> &'static str {
    if ct.is_switch() { "switch" } else { ct.unit() }
}

fn print_json(v: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
    );
}
