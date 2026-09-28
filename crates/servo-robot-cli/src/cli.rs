//! clap 参数定义与配置类型/取值解析

use crate::output;
use clap::{Parser, Subcommand, ValueEnum};
use servo_robot_protocol::config::ConfigType;

#[derive(Debug, Parser)]
#[command(
    name = "servo-robot-cli\
    ",
    version,
    about = "ServoRobotBoard 命令行工具：板级配置读写与 STM32 上报数据获取"
)]
pub struct Cli {
    /// 串口设备路径（串口模式必填，如 /dev/ttyUSB0）
    #[arg(short, long, env = "SR_PORT", global = true)]
    pub port: Option<String>,

    /// 波特率
    #[arg(short, long, env = "SR_BAUD", default_value_t = 115200, global = true)]
    pub baud: u32,

    /// 等待上行帧的超时（毫秒）
    #[arg(short, long, default_value_t = 3000, global = true)]
    pub timeout: u64,

    /// 使用模拟传输层（MockTransport），无需硬件
    #[arg(long, global = true)]
    pub mock: bool,

    /// JSON 输出（watch 为 NDJSON，get all 为 JSON 数组）
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 板级配置读写
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },
    /// 查询设备信息（设备 ID / 版本 / 内存布局）
    Info,
    /// 获取 STM32 上报的数据（单次）
    Get {
        /// 数据类型：imu|power|battery|event|diagnostic|log|all
        kind: GetKind,
    },
    /// 连续输出 STM32 上报的数据（Ctrl-C 退出）
    Watch {
        /// 数据类型列表，省略 = 全部
        kinds: Vec<Kind>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// 查询全部配置项
    List,
    /// 查询单项配置
    Get {
        /// 配置类型：变体名/显示名（大小写不敏感）或 ID（0x22 / 34）
        r#type: String,
    },
    /// 写入单项配置
    Set {
        /// 配置类型：变体名/显示名（大小写不敏感）或 ID（0x22 / 34）
        r#type: String,
        /// 开关类：on|off|true|false|1|0；数值类：整数或小数
        value: String,
    },
    /// 列出全部配置类型（ID / 名称 / 单位 / 值字节数）
    Types,
}

/// 单次获取的目标（含 all 聚合）
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GetKind {
    Imu,
    Power,
    Battery,
    Event,
    Diagnostic,
    Log,
    /// 上述 5 类高频上报数据（不含 log；配置快照请用 config list）
    All,
}

/// 上行数据类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Kind {
    Imu,
    Power,
    Battery,
    Event,
    Diagnostic,
    Log,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Imu,
        Kind::Power,
        Kind::Battery,
        Kind::Event,
        Kind::Diagnostic,
        Kind::Log,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Imu => "imu",
            Kind::Power => "power",
            Kind::Battery => "battery",
            Kind::Event => "event",
            Kind::Diagnostic => "diagnostic",
            Kind::Log => "log",
        }
    }
}

impl GetKind {
    /// `all` 展开为高频 5 类（log 为稀疏触发帧，单独 `get log` 获取）
    pub fn to_kinds(self) -> Vec<Kind> {
        match self {
            GetKind::All => vec![
                Kind::Imu,
                Kind::Power,
                Kind::Battery,
                Kind::Event,
                Kind::Diagnostic,
            ],
            GetKind::Imu => vec![Kind::Imu],
            GetKind::Power => vec![Kind::Power],
            GetKind::Battery => vec![Kind::Battery],
            GetKind::Event => vec![Kind::Event],
            GetKind::Diagnostic => vec![Kind::Diagnostic],
            GetKind::Log => vec![Kind::Log],
        }
    }
}

/// 解析配置类型：变体名/显示名（大小写不敏感）或十进制/十六进制 ID
pub fn parse_config_type(s: &str) -> Result<ConfigType, String> {
    if let Some(ct) = ConfigType::from_name(s) {
        return Ok(ct);
    }
    let id: u32 = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|_| unknown_type_msg(s))?
    } else {
        s.parse::<u32>().map_err(|_| unknown_type_msg(s))?
    };
    let id = u8::try_from(id).map_err(|_| unknown_type_msg(s))?;
    ConfigType::from_u8(id).ok_or_else(|| unknown_type_msg(s))
}

fn unknown_type_msg(s: &str) -> String {
    format!(
        "unknown config type `{s}` (expected a variant name like EnableCharge, \
         a display name like \"Charge\", or an ID like 0x13 / 19)"
    )
}

/// 解析写入值（展示单位）：开关类仅接受布尔关键字，数值类按宽度/量程校验
///
/// 温度组（0x30~0x33）按 °C 输入（如 80.0），写入前再由
/// [`crate::output::display_to_wire`] 转换为 ×10 线上值。
pub fn parse_config_value(ct: ConfigType, s: &str) -> Result<f32, String> {
    if ct.is_switch() {
        return match s.to_ascii_lowercase().as_str() {
            "on" | "true" | "1" => Ok(1.0),
            "off" | "false" | "0" => Ok(0.0),
            _ => Err(format!(
                "`{}` is a switch: expected on|off|true|false|1|0, got `{s}`",
                ct.variant_name()
            )),
        };
    }

    let v: f64 = s.parse().map_err(|_| {
        format!(
            "invalid value `{s}` for `{}`: expected a number",
            ct.variant_name()
        )
    })?;
    if !v.is_finite() {
        return Err(format!("invalid value `{s}`: must be a finite number"));
    }
    let max = if output::is_temperature(ct) {
        // 温度组线上为 u16(×10)，展示单位为 °C
        f64::from(u16::MAX) / 10.0
    } else {
        match ct.value_size() {
            Some(1) => f64::from(u8::MAX),
            Some(2) => f64::from(u16::MAX),
            _ => f64::from(u32::MAX),
        }
    };
    if !(0.0..=max).contains(&v) {
        return Err(format!(
            "value {s} out of range for `{}`: expected 0..={max}",
            ct.variant_name()
        ));
    }
    Ok(v as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_type_by_variant_name_display_name_and_id() {
        assert_eq!(
            parse_config_type("EnableCharge"),
            Ok(ConfigType::EnableCharge)
        );
        assert_eq!(
            parse_config_type("enablecharge"),
            Ok(ConfigType::EnableCharge)
        );
        // 显示名（含空格）同样可解析
        assert_eq!(
            parse_config_type("Charge Max Current"),
            Ok(ConfigType::ChargeMaxCurrentMa)
        );
        assert_eq!(parse_config_type("0x13"), Ok(ConfigType::EnableCharge));
        assert_eq!(parse_config_type("19"), Ok(ConfigType::EnableCharge));
        assert!(parse_config_type("0x100").is_err());
        assert!(parse_config_type("NotAType").is_err());
    }

    #[test]
    fn parses_switch_values() {
        let ct = ConfigType::EnableCharge;
        assert_eq!(parse_config_value(ct, "on"), Ok(1.0));
        assert_eq!(parse_config_value(ct, "OFF"), Ok(0.0));
        assert_eq!(parse_config_value(ct, "true"), Ok(1.0));
        assert!(parse_config_value(ct, "50").is_err());
    }

    #[test]
    fn parses_numeric_values_with_range_check() {
        assert_eq!(
            parse_config_value(ConfigType::ChargeMaxCurrentMa, "80"),
            Ok(80.0)
        );
        assert_eq!(
            parse_config_value(ConfigType::ServoBaudRate, "921600"),
            Ok(921600.0)
        );
        // u16 上下界
        assert!(parse_config_value(ConfigType::BatOut1CurrentLimitMa, "65535").is_ok());
        assert!(parse_config_value(ConfigType::BatOut1CurrentLimitMa, "65536").is_err());
        assert!(parse_config_value(ConfigType::BatOut1CurrentLimitMa, "-1").is_err());
        // 数值类接受小数（按值宽度校验范围）
        assert!(parse_config_value(ConfigType::ChargeStopSoc, "1.5").is_ok());
        assert!(parse_config_value(ConfigType::TxLogLevel, "abc").is_err());
    }
}
