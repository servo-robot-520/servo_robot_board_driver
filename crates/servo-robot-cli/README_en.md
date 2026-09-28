# servo-robot-cli

[简体中文](README.md) | English

Command-line tool for the ServoRobotBoard: read/write board **configuration** and fetch **data reported by the STM32** over the serial port.

## Features

- Configuration: query all items, query/write a single item, list all config types; a type is accepted as variant name, display name, or ID (case-insensitive).
- Data: one-shot `get` and streaming `watch`, covering the 7 uplink frame kinds (IMU / Power / Battery / Event / Diagnostic / Log plus a device-info query).
- Units: text output uses engineering units (V / A / °C / %); temperatures are entered and displayed in °C (the ×10 wire factor is handled by the tool).
- `--mock`: run against the driver's built-in `MockTransport`, no hardware needed.
- `--json`: structured output; `watch --json` emits NDJSON (one frame per line).

## Build & Run

```bash
cargo build -p servo-robot-cli --release
# real serial port
target/release/servo-robot-cli -p /dev/ttyUSB0 config list
# hardware-free
cargo run -q -p servo-robot-cli -- --mock get all
```

## Global Options

| Option | Env | Default | Description |
|--------|-----|---------|-------------|
| `-p, --port <DEV>` | `SR_PORT` | none (required for serial mode) | serial device path, e.g. `/dev/ttyUSB0` |
| `-b, --baud <N>` | `SR_BAUD` | `115200` | baud rate |
| `-t, --timeout <MS>` | — | `3000` | how long `get` waits for an uplink frame |
| `--mock` | — | off | use `MockTransport`, ignores `--port/--baud` |
| `--json` | — | off | JSON output (`watch` = NDJSON, `get all` = array) |
| `-h, --help` / `-V, --version` | — | — | help / version |

Synchronous requests (query/write) use the driver's built-in 1000 ms response timeout and are not configurable.

## Commands

### `config list` — query all configuration

Sends `ConfigQueryAll` and prints every item grouped by ID once the response arrives.

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

### `config get <TYPE>` — query a single config item

`TYPE` accepts three forms (case-insensitive):

- Variant name: `EnableCharge`, `Pwr5VOutCurrentLimitMa`
- Display name: `Charge`, `Bat Out1 Current Limit`
- ID: hex `0x25` or decimal `37`

```console
$ servo-robot-cli --mock config get enablecharge
EnableCharge = on
$ servo-robot-cli --mock config get ChargeMaxCurrentMa
ChargeMaxCurrentMa = 90 mA
$ servo-robot-cli --mock config get PwrServoTempLimit
PwrServoTempLimit = 80.0 °C
```

### `config set <TYPE> <VALUE>` — write a config item

- Switches: `on|off|true|false|1|0`.
- Numeric values: integer or decimal, range-validated against the `ConfigType` value width (1/2/4 bytes);
  the temperature group is entered in **°C** (×10 on the wire).
- Result comes from the board ACK: `success` → exit 0; ACK `success=false` → printed as failed, exit 1.

```console
$ servo-robot-cli --mock config set ChargeMaxCurrentMa 80
ChargeMaxCurrentMa = 80 mA → ack: success
$ servo-robot-cli --mock config set PwrServoTempLimit 75.5
PwrServoTempLimit = 75.5 °C → ack: success
```

### `config types` — list all config types

Hardware-free; one line per type: `ID variant-name unit value-bytes`.

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

### `info` — device information

Synchronous `DeviceInfo` query: device id, uid, IMU model, firmware/hardware version, RAM and flash layout.

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

(Values are the `MockTransport` `DeviceInfoSimulator` defaults; over a real serial port these come from the board.)

### `get <KIND>` — one-shot fetch of STM32 telemetry

`KIND` ∈ `imu | power | battery | event | diagnostic | log | all`.

Starts the driver, polls the state snapshot (`DriverState::snapshot()`), prints the **latest frame** of that kind
within `--timeout`; on timeout it errors to stderr and exits 1. `all` = `imu power battery event diagnostic`
(the five high-frequency kinds), printed as separate blocks once all have been received within the same budget.

- `log` is a sparse, triggered frame (board sends it on demand/low-rate; mock every 20 s) — fetched on its own with its own timer.
- The config snapshot is fetched via `config list` (request/response, deterministic) and is not part of `get`.

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

### `watch [KIND...]` — streaming output

Registers a `DriverCallback` and keeps printing the selected kinds (omitted = all 6); exit with `Ctrl-C`
(process terminates immediately, no cleanup). Text mode is one line per frame; `--json` is NDJSON.

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

## Output & Unit Conventions

- **Text**: engineering units — volts V, current A, temperature °C, capacity/SOC %.
- **JSON**: field names follow the protocol layer, values are wire units, except:
  - `value` of the `config` commands is in display units (switches 0|1, temperature °C);
  - `_c`-suffixed fields (`temperature_c`, `temperatures_c`) are converted °C.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | success |
| 1 | runtime failure: cannot open port / timeout with no data / board ACK failed / driver error |
| 2 | usage error (arguments, type name, value parsing; clap errors use this code as well) |

## Implementation Design

### Dependencies and integration

- Root `Cargo.toml`: `members` includes `"crates/servo-robot-cli"`; `[workspace.dependencies]` provides
  `clap` (4.x, derive+env) and `serde_json`.
- `servo-robot-cli` depends on `servo-robot-driver` (`features = ["mock"]` — binary crate, default-on has no
  downstream impact), `servo-robot-protocol`, `clap`, `serde_json`.

### Layout

```
crates/servo-robot-cli/
├── Cargo.toml
├── README.md / README_en.md
├── src/
│   ├── main.rs            entry: clap parse → dispatch → unified exit codes (CmdError: Usage=2 / Runtime=1)
│   ├── cli.rs             clap definitions + config type/value parsing (unit tests)
│   ├── session.rs         transport setup (Serial/Mock), callback registration, driver lifecycle
│   ├── output.rs          text/JSON rendering, unit conversion (temp ×10, power ×10, battery mV/mA)
│   └── commands/
│       ├── config.rs      list / get / set / types (usage validation before opening the port)
│       ├── info.rs        device info
│       ├── get.rs         one-shot fetch (snapshot polling + timeout)
│       └── watch.rs       streaming (WatchCallback implements DriverCallback)
└── tests/cli_smoke.rs     12 --mock integration tests (drive the real binary, assert output and exit codes)
```

### Data flow

```
main.rs ──clap──▶ commands
                     │
                     ▼
   session.rs: SerialTransport::open(port, baud) | MockTransport::new()
               └─▶ Driver::new(transport) ─▶ [watch: register_callback] ─▶ driver.start()
                     │
      config/info ──▶ *_sync request/response (driver's built-in 1000 ms timeout)
      get ──────────▶ poll driver.state().snapshot() until --timeout
      watch ────────▶ dispatch-thread callbacks print per frame; main thread parks until Ctrl-C
                     │
                     ▼
               driver.stop() ─▶ exit code
```

### Protocol-layer extensions (servo-robot-protocol)

- `enum_with_from_u8!` macro now emits `ALL` / `from_name()` (variant or display name, case-insensitive) /
  `variant_name()` — single source of truth for type parsing and `config types`.
- `ConfigType::is_switch()`: switch detection (on/off rendering and parsing).
- Wire scalars (temperature ×10, power ×10) are converted in the CLI rendering layer
  (`output::is_temperature` / `display_to_wire`).

### Test Plan (implemented)

- Unit tests in `src/cli.rs`: name/display-name/ID parsing, switch keywords, numeric ranges
  (u16 boundary, °C upper bound for the ×10 group).
- Protocol tests in `config.rs`: `ALL` self-consistency over 21 items, `is_switch` over 5 switches,
  bidirectional `from_name`.
- Integration tests in `tests/cli_smoke.rs` (`--mock`, real binary): types/list/get/set/info/get all/get json,
  exit code 2 (unknown type, out of range, invalid switch value), exit code 1 (missing `--port`,
  `get log` timeout), JSON structure assertions, first `watch` NDJSON line (5 s timeout guard against hangs).

## Design Decisions (confirmed)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Argument parsing | `clap` 4.x (derive + env) |
| 2 | Output format | text + `--json` |
| 3 | `watch` streaming | implemented in v1 |
| 4 | Config type name resolution | extend the protocol `enum_with_from_u8!` macro (single source of truth) |

## Roadmap

1. System commands `system reset|shutdown|ota`.
2. `servo forward`, `firmware-update`.
3. Port auto-discovery and `--reconnect`.
4. Versioned, documented JSON output schema.
