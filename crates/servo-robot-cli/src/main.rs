//! servo-robot-cli 入口：参数解析 → 命令分发 → 统一退出码
//!
//! 退出码约定：
//! - 0 成功
//! - 1 运行失败（打开串口失败 / 等待超时无数据 / 板端 ACK 失败 / 驱动错误）
//! - 2 用法错误（参数、配置类型名、取值解析失败；clap 解析错误同样为 2）

mod cli;
mod commands;
mod output;
mod session;

use clap::Parser;
use cli::{Cli, Command};

/// 命令执行错误：`Usage` → exit 2，`Runtime` → exit 1
pub enum CmdError {
    Usage(String),
    Runtime(String),
}

impl CmdError {
    pub fn usage(msg: impl Into<String>) -> Self {
        CmdError::Usage(msg.into())
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        CmdError::Runtime(msg.into())
    }

    pub fn message(&self) -> &str {
        match self {
            CmdError::Usage(m) | CmdError::Runtime(m) => m,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            CmdError::Usage(_) => 2,
            CmdError::Runtime(_) => 1,
        }
    }

    pub fn report(&self) {
        eprintln!("error: {}", self.message());
    }
}

impl From<String> for CmdError {
    fn from(msg: String) -> Self {
        CmdError::Runtime(msg)
    }
}

/// 统一收尾：打印错误并返回退出码
pub fn finish<T>(res: Result<T, CmdError>) -> i32 {
    match res {
        Ok(_) => 0,
        Err(e) => {
            e.report();
            e.exit_code()
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match &cli.command {
        Command::Config { cmd } => commands::config::run(&cli, cmd),
        Command::Info => commands::info::run(&cli),
        Command::Get { kind } => commands::get::run(&cli, *kind),
        Command::Watch { kinds } => commands::watch::run(&cli, kinds),
    };
    std::process::exit(code);
}
