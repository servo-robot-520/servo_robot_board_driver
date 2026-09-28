//! `--mock` 端到端冒烟测试：驱动真实二进制，断言输出与退出码
//!
//! 退出码约定见 `src/main.rs`：0 成功 / 1 运行失败 / 2 用法错误。

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_servo-robot-cli"))
}

fn run(args: &[&str]) -> (String, String, i32) {
    let out = bin()
        .args(args)
        .output()
        .expect("failed to spawn servo-robot-cli");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn config_types_lists_all_variants() {
    let (stdout, _, code) = run(&["--mock", "config", "types"]);
    assert_eq!(code, 0, "stderr: {stderr}", stderr = run(&[]).2);
    assert!(stdout.contains("EnableCharge"), "stdout:\n{stdout}");
    assert!(stdout.contains("0x14  EnableServoPwrMonitor"));
    // 21 个配置类型
    assert_eq!(stdout.lines().count(), 21);
}

#[test]
fn config_list_shows_grouped_snapshot() {
    let (stdout, stderr, code) = run(&["--mock", "config", "list"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(stdout.contains("power switches:"), "stdout:\n{stdout}");
    assert!(stdout.contains("current limits (mA):"));
    assert!(stdout.contains("temperature limits (°C):"));
    assert!(stdout.contains("misc:"));
    // 快照默认值：开关全开、温度为 °C 展示（线上 ×10）
    assert!(stdout.contains("Charge.................... on"));
    assert!(stdout.contains("Servo Temp Limit.......... 80.0"));
    assert!(stdout.contains("Charge Stop Soc........... 100 %"));
}

#[test]
fn config_get_by_variant_name_display_name_and_id() {
    let (stdout, stderr, code) = run(&["--mock", "config", "get", "enablecharge"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert_eq!(stdout.trim(), "EnableCharge = on");

    let (stdout, _, code) = run(&["--mock", "config", "get", "0x21"]);
    assert_eq!(code, 0);
    assert!(
        stdout.starts_with("BatOut2CurrentLimitMa = "),
        "stdout:\n{stdout}"
    );

    let (stdout, _, code) = run(&["--mock", "config", "get", "Charge Max Current"]);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "ChargeMaxCurrentMa = 90 mA");
}

#[test]
fn config_set_acks_and_scales_temperature() {
    let (stdout, stderr, code) = run(&["--mock", "config", "set", "ChargeMaxCurrentMa", "80"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(
        stdout.contains("ChargeMaxCurrentMa = 80 mA → ack: success"),
        "stdout:\n{stdout}"
    );

    // 温度按 °C 输入，线上 ×10；回显为展示单位
    let (stdout, stderr, code) = run(&["--mock", "config", "set", "PwrServoTempLimit", "75.5"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(
        stdout.contains("PwrServoTempLimit = 75.5 °C → ack: success"),
        "stdout:\n{stdout}"
    );
}

#[test]
fn usage_errors_exit_2() {
    // 未知配置类型
    let (stdout, stderr, code) = run(&["--mock", "config", "get", "NoSuchType"]);
    assert_eq!(code, 2, "stdout:\n{stdout} stderr:\n{stderr}");
    assert!(stderr.contains("unknown config type"), "stderr:\n{stderr}");

    // 超出 u16 量程
    let (_, stderr, code) = run(&["--mock", "config", "set", "BatOut1CurrentLimitMa", "70000"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("out of range"), "stderr:\n{stderr}");

    // 开关收到非布尔关键字
    let (_, stderr, code) = run(&["--mock", "config", "set", "EnableCharge", "banana"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("is a switch"), "stderr:\n{stderr}");
}

#[test]
fn missing_port_exits_1_before_any_usage_error() {
    let mut cmd = bin();
    let out = cmd
        .args(["config", "list"])
        .env_remove("SR_PORT")
        .output()
        .expect("failed to spawn servo-robot-cli");
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("missing --port"), "stderr:\n{stderr}");
}

#[test]
fn info_prints_device_identity() {
    let (stdout, stderr, code) = run(&["--mock", "info"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    assert!(stdout.contains("device_id : 0x4832"), "stdout:\n{stdout}");
    assert!(stdout.contains("uid       : 0x12345678"));
    assert!(stdout.contains("firmware  : 0.1.0"));
    assert!(stdout.contains("flash     : boot=16KB"));
}

#[test]
fn get_all_returns_five_pushed_kinds() {
    let (stdout, stderr, code) = run(&["--mock", "--timeout", "5000", "get", "all"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    for kind in ["imu", "power", "battery", "event", "diagnostic"] {
        assert!(
            stdout.contains(&format!("=== {kind} ===")),
            "missing {kind} block in stdout:\n{stdout}"
        );
    }
    assert!(stdout.contains("attitude :"), "stdout:\n{stdout}");
}

#[test]
fn get_json_is_valid_single_object() {
    let (stdout, stderr, code) = run(&["--mock", "--json", "get", "imu"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    assert_eq!(v["kind"], "imu");
    assert!(v["roll"].is_number());
}

#[test]
fn get_log_times_out_with_exit_1() {
    // mock 首条周期日志在 20s 后，100ms 内必然超时
    let (stdout, stderr, code) = run(&["--mock", "--timeout", "100", "get", "log"]);
    assert_eq!(code, 1, "stdout:\n{stdout}");
    assert!(stderr.contains("timeout after 100ms"), "stderr:\n{stderr}");
    assert!(stderr.contains("log"), "stderr:\n{stderr}");
}

#[test]
fn config_types_json_has_21_items() {
    let (stdout, stderr, code) = run(&["--mock", "--json", "config", "types"]);
    assert_eq!(code, 0, "stderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    let items = v.as_array().expect("JSON array");
    assert_eq!(items.len(), 21);
    assert_eq!(items[0]["type"], "EnableBatOut1");
    assert_eq!(items[0]["unit"], "switch");
    assert_eq!(items[0]["size"], 1);
    let temp = items
        .iter()
        .find(|i| i["type"] == "PwrServoTempLimit")
        .expect("temp entry");
    assert_eq!(temp["unit"], "°C");
}

#[test]
fn watch_streams_ndjson_lines() {
    let mut child = bin()
        .args(["--mock", "--json", "watch", "imu"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn watch");

    let mut stdout = child.stdout.take().expect("stdout piped");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(&mut stdout).read_line(&mut line);
        let _ = tx.send(line);
    });

    // mock IMU 为 100Hz，5s 内必有输出；读取线程有超时兜底，避免挂死
    let got = rx.recv_timeout(Duration::from_secs(5));
    let _ = child.kill();
    let _ = child.wait();

    let line = got.expect("watch produced no line within 5s");
    let v: serde_json::Value = serde_json::from_str(&line).expect("NDJSON line");
    assert_eq!(v["kind"], "imu");
    assert!(v["roll"].is_number());
}
