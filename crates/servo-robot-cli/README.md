# servo-robot-cli

English | [简体中文](README.md)

ServoRobotBoard 的命令行工具：通过串口完成板级**配置读写**，并**获取 STM32 上报的数据**。

## 特性

- 配置：全量查询、单项查询/写入、配置类型列表；类型支持变体名 / 显示名 / ID 三种写法（大小写不敏感）。
- 数据：`get` 单次获取、`watch` 连续流式输出，覆盖 7 类上行帧（IMU / Power / Battery / Event / Diagnostic / Log + 设备信息查询）。
- 单位换算：文本输出为工程单位（V / A / °C / %），温度按 °C 输入输出（线上 ×10 由工具换算）。
- `--mock`：使用驱动内置 `MockTransport`，无需硬件即可演示与测试。
- `--json`：结构化输出；`watch --json` 为 NDJSON（每帧一行）。

## 构建与运行

```bash
cargo build -p servo-robot-cli --release
# 实连串口
target/release/servo-robot-cli -p /dev/ttyUSB0 config list
# 无硬件
cargo run -q -p servo-robot-cli -- --mock get all
```

## 全局选项

| 选项 | 环境变量 | 默认 | 说明 |
|------|----------|------|------|
| `-p, --port <DEV>` | `SR_PORT` | 无（串口模式必填） | 串口设备路径，如 `/dev/ttyUSB0` |
| `-b, --baud <N>` | `SR_BAUD` | `115200` | 波特率 |
| `-t, --timeout <MS>` | — | `3000` | `get` 等待上行帧的超时 |
| `--mock` | — | 关 | 使用 `MockTransport`，忽略 `--port/--baud` |
| `--json` | — | 关 | JSON 输出（`watch` 为 NDJSON，`get all` 为数组） |
| `-h, --help` / `-V, --version` | — | — | 帮助 / 版本 |

同步请求（查询/写入）使用驱动内置的 1000 ms 应答超时，不另行开放。

## 命令

### `config list` — 查询全部配置

发 `ConfigQueryAll`，等待应答后按 ID 分组打印全部配置项。

```console
$ servo-robot-cli --mock config list
power switches:
  Servo Power............... on
  Battery Extra Output...... on
  5V Power.................. on
  Charge.................... on
  Servo Power Monitor....... on

current limits (mA):
  Bat Out1 Current Limit.... 50
  Bat Out2 Current Limit.... 0
  5V Out Current Limit...... 0
  Servo power out Current Limit 0
  Charge Min Current........ 0
  Charge Max Current........ 90

temperature limits (°C):
  Servo Temp Limit.......... 80.0
  5V Temp Limit............. 70.0
  Charge Temp Derating...... 60.0
  Charge Temp Limit......... 70.0

misc:
  Servo Baud Rate........... 115200
  Charge Stop Soc........... 100 %
  Charge Stop Voltage....... 168 mV
  TxLog Level............... INFO
  BMS IC.................... 0
  IMU IC.................... 0
```

### `config get <TYPE>` — 查询单项配置

`TYPE` 支持三种写法（大小写不敏感）：

- 变体名：`EnableCharge`、`Pwr5VOutCurrentLimitMa`
- 显示名：`Charge`、`Bat Out1 Current Limit`
- ID：十六进制 `0x25` 或十进制 `37`

```console
$ servo-robot-cli --mock config get enablecharge
EnableCharge = on
$ servo-robot-cli --mock config get ChargeMaxCurrentMa
ChargeMaxCurrentMa = 90 mA
$ servo-robot-cli --mock config get PwrServoTempLimit
PwrServoTempLimit = 80.0 °C
```

### `config set <TYPE> <VALUE>` — 写入单项配置

- 开关类：`on|off|true|false|1|0`。
- 数值类：整数或小数，按 `ConfigType` 值宽度（1/2/4 字节）校验范围；温度组按 **°C** 输入（写入时 ×10）。
- 结果来自板端 ACK：`success` → exit 0；ACK `success=false` → 输出 failed 并 exit 1。

```console
$ servo-robot-cli --mock config set ChargeMaxCurrentMa 80
ChargeMaxCurrentMa = 80 mA → ack: success
$ servo-robot-cli --mock config set PwrServoTempLimit 75.5
PwrServoTempLimit = 75.5 °C → ack: success
```

### `config types` — 列出全部配置类型

不依赖硬件；每行 `ID 变体名 单位 值字节数`，供人查阅与脚本参数生成。

```console
$ servo-robot-cli --mock config types
0x10  EnableBatOut1              switch 1
0x11  EnablePwrBatOut2           switch 1
0x12  EnablePwr5V                switch 1
0x13  EnableCharge               switch 1
0x14  EnableServoPwrMonitor      switch 1
0x20  BatOut1CurrentLimitMa      mA     2
...
0x40  ServoBaudRate                     4
0x41  ChargeStopSoc              %      1
0x42  ChargeStopVoltageMv        mV     2
0x43  TxLogLevel                        1
0x44  BMSIc                             1
0x45  IMUIc                             1
```

### `info` — 设备信息

同步 `DeviceInfo` 查询：设备 ID、UID、IMU 型号、固件/硬件版本、RAM 与 Flash 布局。

```console
$ servo-robot-cli --mock info
device_id : 0x4832
uid       : 0x12345678
imu_id    : 0x70
firmware  : 0.1.0
hardware  : 1.0.0
ram       : 128 KB
flash     : boot=16KB app=240KB ota=128KB user=128KB
```

（数值为 `MockTransport` `DeviceInfoSimulator` 默认值；实连时为板端真实值。）

### `get <KIND>` — 单次获取 STM32 上报的数据

`KIND` ∈ `imu | power | battery | event | diagnostic | log | all`。

启动驱动后轮询状态快照（`DriverState::snapshot()`），`--timeout` 内取该类型**最新一帧**打印；
超时未收到 → stderr 报错并 exit 1。`all` = `imu power battery event diagnostic` 五类高频帧，
在同一超时预算内等齐后分块打印。

- `log` 为稀疏触发帧（板端按需/低频发送，mock 每 20 s 一条），单独获取并独立计时。
- 配置快照请用 `config list`（请求-应答，确定性），不占用 `get`。

```console
$ servo-robot-cli --mock --timeout 5000 get all
=== imu ===
attitude : roll=+0.16° pitch=+4.97° yaw=-0.57°
gyro     : +0.312 +0.444 -0.097 °/s
accel    : -0.866 +0.026 +9.728 m/s²
ts       : 1011 ms

=== power ===
servo    : 8.5 V / 10.7 A
pd_in    : 0.0 V / 0.0 A
bat      : 17.0 V / -0.8 A
out1     : 8.6 A   out2 : 2.1 A
5v       : 0.5 V / 2.5 A

=== battery ===
soc      : 75 %   voltage : 15.58 V   current : 7.84 A
temp     : 28.7 °C
status   : Discharging   health : Good   present : yes
capacity : 4055 / 5000 mAh   serial : 0x3039
cells    : 4 × 3.91 V [3.91 3.89 3.89 3.90]

=== event ===
charge   : NotCharging
state    : FAN_ENABLED
protect  : (none)
error    : (none)

=== diagnostic ===
uptime   : 1 s   cpu : 37 %   heap : 137 KB   stack_min : 8 KB
errors   : i2c=0 spi=0 uart=0 usb=0
frames   : sent=1
pd       : 0.0 V / 0.0 A
temp     : servo_power=39.8 5v=34.6 mcu=30.2 charge=46.1 battery=27.0 °C
```

### `watch [KIND...]` — 连续流式输出

注册 `DriverCallback` 持续打印所选类型（省略 = 全部 6 类），`Ctrl-C` 退出（进程直接终止，无清理）。
文本模式每帧一行；`--json` 为 NDJSON。

```console
$ servo-robot-cli --mock watch imu
imu roll=-1.03 pitch=+1.13 yaw=-0.27 ts=12ms
imu roll=-2.08 pitch=+0.76 yaw=+0.12 ts=25ms

$ servo-robot-cli --mock watch power battery
power servo=8.6V/7.8A pd_in=0.0V/0.0A bat=16.5V/-1.3A out1=6.2A out2=1.5A 5v=0.5V/2.5A
battery soc=79% v=15.80V i=9.90A t=28.7°C status=Discharging

$ servo-robot-cli --mock --json watch battery
{"capacity_mah":4105,"cell_temperatures":[283,274,269,266],"cell_voltages_mv":[3965,3948,3947,3941],"charge_status":"Discharging","current_ma":9898,"design_capacity_mah":5000,"health":"Good","kind":"battery","percentage":79,"present":true,"serial_number":12345,"technology":"LiPo","temperature_c":28.7,"voltage_mv":15803}
```

## 输出与单位约定

- **文本**：工程单位 —— 电压 V、电流 A、温度 °C、电量 %、SOC %。
- **JSON**：字段名与协议层一致，取值为线上单位，例外：
  - `config` 类命令的 `value` 为展示单位（开关 0|1，温度 °C）；
  - 带 `_c` 后缀字段（`temperature_c`、`temperatures_c`）为换算后的 °C。

## 退出码

| 码 | 含义 |
|----|------|
| 0 | 成功 |
| 1 | 运行失败：打开串口失败 / 等待超时无数据 / 板端 ACK 失败 / 驱动错误 |
| 2 | 用法错误（参数、类型名、取值解析失败；clap 参数错误同为此码） |

## 实现设计

### 依赖与集成

- 根 `Cargo.toml`：`members` 含 `"crates/servo-robot-cli"`；`[workspace.dependencies]` 提供
  `clap`（4.x, derive+env）与 `serde_json`。
- `servo-robot-cli` 依赖 `servo-robot-driver`（`features = ["mock"]`，二进制非库，默认开启无下游影响）、
  `servo-robot-protocol`、`clap`、`serde_json`。

### 目录结构

```
crates/servo-robot-cli/
├── Cargo.toml
├── README.md / README_en.md
├── src/
│   ├── main.rs            入口：clap 解析 → 分发 → 统一退出码（CmdError: Usage=2 / Runtime=1）
│   ├── cli.rs             clap 定义 + 配置类型/取值解析（含单元测试）
│   ├── session.rs         传输层打开（Serial/Mock）、回调注册、驱动启停
│   ├── output.rs          文本/JSON 渲染、单位换算（温度 ×10、Power ×10、电池 mV/mA）
│   └── commands/
│       ├── config.rs      list / get / set / types（用法校验先于串口打开）
│       ├── info.rs        设备信息
│       ├── get.rs         单次获取（轮询快照 + 超时）
│       └── watch.rs       流式输出（WatchCallback 实现 DriverCallback）
└── tests/cli_smoke.rs     12 个 --mock 集成测试（驱动真实二进制，断言输出与退出码）
```

### 数据流

```
main.rs ──clap──▶ commands
                     │
                     ▼
   session.rs: SerialTransport::open(port, baud) | MockTransport::new()
               └─▶ Driver::new(transport) ─▶ [watch: register_callback] ─▶ driver.start()
                     │
      config/info ──▶ *_sync 请求-应答（驱动内置 1000ms 超时）
      get ──────────▶ 轮询 driver.state().snapshot() 至 --timeout
      watch ────────▶ 分发线程回调逐行输出，主线程 park 至 Ctrl-C
                     │
                     ▼
               driver.stop() ─▶ 退出码
```

### 协议层扩展（servo-robot-protocol）

- `enum_with_from_u8!` 宏新增 `ALL` / `from_name()`（变体名与显示名，大小写不敏感）/ `variant_name()`，
  配置类型解析与 `config types` 单一数据源。
- `ConfigType::is_switch()`：开关类判定（on/off 展示与解析）。
- 温度 ×10、Power ×10 等线上缩放在 CLI 渲染层换算（`output::is_temperature` / `display_to_wire`）。

### 测试计划（已实现）

- 单元测试 `src/cli.rs`：类型名/显示名/ID 解析、开关关键字、数值量程（u16 边界、温度 °C 上限）。
- 协议测试 `config.rs`：`ALL` 21 项自洽、`is_switch` 5 项、`from_name` 双向匹配。
- 集成测试 `tests/cli_smoke.rs`（`--mock` 驱动真实二进制）：types/list/get/set/info/get all/get json、
  退出码 2（未知类型、超量程、开关非法值）、退出码 1（缺 `--port`、`get log` 超时）、
  JSON 结构断言、`watch` NDJSON 首行（5s 超时兜底防挂死）。

## 设计决策（已确认）

| # | 决策 | 选择 |
|---|------|------|
| 1 | 参数解析 | `clap` 4.x（derive + env） |
| 2 | 输出格式 | 文本 + `--json` |
| 3 | `watch` 流式输出 | v1 实现 |
| 4 | 配置类型名解析 | 扩展协议 `enum_with_from_u8!` 宏（单一数据源） |

## 路线图

1. 系统命令 `system reset|shutdown|ota`。
2. 舵机转发 `servo forward`、固件更新 `firmware-update`。
3. 串口自动探测与 `--reconnect`。
4. JSON 输出 schema 版本化与文档化。
