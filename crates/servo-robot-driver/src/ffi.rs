//! C FFI 包装层 — 以 C ABI 暴露 `Driver`,供 C/C++ 调用
//!
//! 头文件: `include/servo_robot_driver.h`(字段顺序/签名与本模块严格一致)
//!
//! 约定:
//! - 所有函数空指针安全;入参非法返回 `SR_ERR_NULL`/`SR_ERR_INVALID_ARG`
//! - 所有函数用 `catch_unwind` 包裹,Rust panic 不会跨 FFI 传播
//! - 同步函数阻塞 ≤1s(驱动默认超时)
//! - 回调在驱动内部的分发线程触发;回调内禁止调用任何 `sr_driver_*`(会死锁)

pub mod callback;

use crate::driver::Driver;
use crate::error::DriverError;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::protocol::request::RequestKind;
use crate::protocol::servo::ServoCmdWrapper;
use crate::transport::serial::SerialTransport;
use callback::{CallbackTable, CffiCallback, SrCallbacks};
use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex};

// ═══ 错误码(与头文件 SR_ERR_* 严格一致)═══

pub const SR_OK: i32 = 0;
pub const SR_ERR_SERIAL: i32 = -1;
pub const SR_ERR_IO: i32 = -2;
pub const SR_ERR_FRAME: i32 = -3;
pub const SR_ERR_TRANSPORT_CLOSED: i32 = -4;
pub const SR_ERR_TIMEOUT: i32 = -5;
pub const SR_ERR_CRC: i32 = -6;
pub const SR_ERR_PAYLOAD_TOO_SHORT: i32 = -7;
pub const SR_ERR_UNKNOWN_FRAME: i32 = -8;
pub const SR_ERR_NOT_RUNNING: i32 = -9;
pub const SR_ERR_LOCK_POISONED: i32 = -10;
pub const SR_ERR_NULL: i32 = -11;
pub const SR_ERR_INVALID_ARG: i32 = -12;
pub const SR_ERR_PANIC: i32 = -13;
pub const SR_ERR_ALREADY_STARTED: i32 = -14;

// ═══ 不透明句柄 ═══

/// C 侧不透明句柄(头文件 `typedef struct sr_driver sr_driver;`)
///
/// `inner` 用 `Mutex` 包裹:FFI 层所有操作通过 `&SrDriver` + 内部锁访问,
/// 避免多线程并发调用时对裸指针产生别名 `&mut`(UB)。
pub struct SrDriver {
    pub(crate) inner: Mutex<Driver>,
    pub(crate) last_error: Mutex<String>,
    pub(crate) callbacks: Arc<Mutex<CallbackTable>>,
}

// ═══ C 数据结构(字段与协议层逐一对应)═══

/// 配置值 — type 为 ConfigType(0x10~0x37),value 语义同 `Config::value()`(f32)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrConfig {
    pub typ: u8,
    pub value: f32,
}

/// 板级配置全量快照(24 字节 payload 的镜像)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrBoardConfig {
    pub power_servo_on: u8,
    pub power_5v_on: u8,
    pub charge_on: u8,
    pub bat_ext_out_on: u8,
    pub charge_stop_percentage: u8,
    pub tx_log_level: u8,
    pub servo_current_limit_ma: u16,
    pub servo_temp_limit: u16,
    pub temp_5v_limit: u16,
    pub charge_max_current_ma: u16,
    pub charge_temp_derating: u16,
    pub charge_temp_limit: u16,
    pub charge_stop_voltage_mv: u16,
    pub servo_baud_rate: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrImu {
    pub accel: [f32; 3],
    pub gyro: [f32; 3],
    pub quaternion: [f32; 4], // w, x, y, z
    pub timestamp_ms: u32,
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrPower {
    pub servo_voltage_mv: u16,
    pub servo_current_ma: u16,
    pub charge_in_voltage_mv: u16,
    pub charge_in_current_ma: u16,
    pub bat_voltage_mv: u16,
    pub bat_current_ma: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrBatteryState {
    pub voltage_mv: u16,
    pub current_ma: i16,
    pub capacity_mah: u16,
    pub design_capacity_mah: u16,
    pub percentage: u8,
    pub temperature: i16,
    pub charge_status: u8,
    pub health: u8,
    pub technology: u8,
    pub present: u8,
    pub serial_number: u16,
    /// 指向电芯电压数组,仅在回调执行期间有效
    pub cell_voltages_mv: *const u16,
    pub cell_count: u32,
    /// 指向电芯温度数组,仅在回调执行期间有效
    pub cell_temperatures: *const i16,
    pub cell_temp_count: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrBoardEvent {
    pub charge_phase: u8,
    pub state_change_flags: u16,
    pub protection_flags: u16,
    pub error_flags: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrDeviceInfo {
    pub device_id: u16,
    pub uid: u32,
    pub imu_id: u8,
    pub fw_major: u8,
    pub fw_minor: u8,
    pub fw_patch: u8,
    pub ram_kb: u16,
    pub flash_boot_kb: u16,
    pub flash_app_kb: u16,
    pub flash_ota_kb: u16,
    pub flash_user_kb: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrDiagnostic {
    pub uptime_s: u32,
    pub cpu_usage_percent: u8,
    pub free_heap_kb: u16,
    pub stack_watermark_min_kb: u16,
    pub i2c_error_count: u16,
    pub spi_error_count: u16,
    pub uart_error_count: u16,
    pub usb_error_count: u16,
    pub frames_sent_total: u32,
    pub pd_request_voltage_mv: u16,
    pub pd_request_current_ma: u16,
    pub temp_servo_power: i16,
    pub temp_5v_power: i16,
    pub temp_mcu: i16,
    pub temp_charge: i16,
    pub temp_battery: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrLogMessage {
    /// Unix 时间戳(毫秒),读线程解码时采集
    pub ts_ms: u64,
    pub level: u8,
    /// NUL 结尾字符串,仅在回调执行期间有效
    pub file_name: *const c_char,
    pub fun_name: *const c_char,
    pub msg: *const c_char,
}

// ═══ 内部辅助 ═══

fn err_code(e: &DriverError) -> i32 {
    match e {
        DriverError::Serial(_) => SR_ERR_SERIAL,
        DriverError::Io(_) => SR_ERR_IO,
        DriverError::Frame(_) => SR_ERR_FRAME,
        DriverError::TransportClosed => SR_ERR_TRANSPORT_CLOSED,
        DriverError::Timeout => SR_ERR_TIMEOUT,
        DriverError::IoTimeout => SR_ERR_TIMEOUT,
        DriverError::CrcMismatch { .. } => SR_ERR_CRC,
        DriverError::PayloadTooShort { .. } => SR_ERR_PAYLOAD_TOO_SHORT,
        DriverError::UnknownFrameType(_) => SR_ERR_UNKNOWN_FRAME,
        DriverError::NotRunning => SR_ERR_NOT_RUNNING,
        DriverError::AlreadyStarted => SR_ERR_ALREADY_STARTED,
        DriverError::LockPoisoned => SR_ERR_LOCK_POISONED,
    }
}

/// NUL 截断拷贝消息到 C buffer
fn write_err_buf(buf: *mut c_char, len: usize, msg: &str) {
    if buf.is_null() || len == 0 {
        return;
    }
    let bytes = msg.as_bytes();
    let n = bytes.len().min(len - 1);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, n);
        *buf.add(n) = 0;
    }
}

fn set_last_error(d: *mut SrDriver, msg: &str) {
    if let Ok(mut le) = unsafe { &*d }.last_error.lock() {
        le.clear();
        le.push_str(msg);
    }
}

/// 统一入口:null 检查 + catch_unwind + 错误码/错误信息落盘。
/// 闭包接收 `&mut Driver`(内部锁已持有),禁止在闭包内再锁 handle。
fn guard(d: *mut SrDriver, f: impl FnOnce(&mut Driver) -> Result<(), DriverError>) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    match catch_unwind(AssertUnwindSafe(|| {
        let mut inner = unsafe { &*d }
            .inner
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        f(&mut inner)
    })) {
        Ok(Ok(())) => SR_OK,
        Ok(Err(e)) => {
            let code = err_code(&e);
            set_last_error(d, &e.to_string());
            code
        }
        Err(_) => {
            set_last_error(d, "panic in driver call");
            SR_ERR_PANIC
        }
    }
}

/// SrConfig → Config;非法 type 返回 None
fn config_from_sr(c: SrConfig) -> Option<Config> {
    let ct = ConfigType::from_u8(c.typ)?;
    Some(Config::from_type_value(ct, c.value))
}

fn to_sr_config(c: Config) -> SrConfig {
    SrConfig {
        typ: c.config_type() as u8,
        value: c.value(),
    }
}

fn to_sr_board_config(c: BoardConfigSnapshot) -> SrBoardConfig {
    SrBoardConfig {
        power_servo_on: c.power_servo_on as u8,
        power_5v_on: c.power_5v_on as u8,
        charge_on: c.charge_on as u8,
        bat_ext_out_on: c.bat_ext_out_on as u8,
        charge_stop_percentage: c.charge_stop_percentage,
        tx_log_level: c.tx_log_level as u8,
        servo_current_limit_ma: c.servo_current_limit_ma,
        servo_temp_limit: c.servo_temp_limit,
        temp_5v_limit: c.temp_5v_limit,
        charge_max_current_ma: c.charge_max_current_ma,
        charge_temp_derating: c.charge_temp_derating,
        charge_temp_limit: c.charge_temp_limit,
        charge_stop_voltage_mv: c.charge_stop_voltage_mv,
        servo_baud_rate: c.servo_baud_rate,
    }
}

fn to_sr_device_info(d: DeviceInfo) -> SrDeviceInfo {
    SrDeviceInfo {
        device_id: info.device_id,
        uid: info.uid,
        imu_id: info.imu_id,
        fw_major: info.firmware_version.major,
        fw_minor: info.firmware_version.minor,
        fw_patch: info.firmware_version.patch,
        ram_kb: info.ram_kb,
        flash_boot_kb: info.flash_boot_kb,
        flash_app_kb: info.flash_app_kb,
        flash_ota_kb: info.flash_ota_kb,
        flash_user_kb: info.flash_user_kb,
    }
}

/// 驱动版本号（与 Cargo.toml version 一致）
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SrVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

/// 获取驱动版本号
///
/// @return 版本结构体，始终成功
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_version() -> SrVersion {
    SrVersion {
        major: env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0),
        minor: env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0),
        patch: env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0),
    }
}

/// 打开串口并创建驱动句柄;失败返回 NULL,错误描述写入 err_buf
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_open(
    port: *const c_char,
    baud_rate: u32,
    err_buf: *mut c_char,
    err_buf_len: usize,
) -> *mut SrDriver {
    let res = catch_unwind(AssertUnwindSafe(|| -> Result<*mut SrDriver, String> {
        if port.is_null() {
            return Err("port is NULL".to_string());
        }
        let port_name = unsafe { CStr::from_ptr(port) }
            .to_string_lossy()
            .into_owned();
        let transport = SerialTransport::open(&port_name, baud_rate).map_err(|e| e.to_string())?;
        Ok(build_sr_driver(Driver::new(transport)))
    }));
    match res {
        Ok(Ok(d)) => d,
        Ok(Err(msg)) => {
            write_err_buf(err_buf, err_buf_len, &msg);
            ptr::null_mut()
        }
        Err(_) => {
            write_err_buf(err_buf, err_buf_len, "panic in sr_driver_open");
            ptr::null_mut()
        }
    }
}

/// 重新连接到指定串口（上层实现重连逻辑时使用）
///
/// 如果驱动正在运行，会先停止当前连接，打开新串口，再重新启动。
/// 失败时返回错误码，驱动状态不变。
///
/// @param d         驱动句柄
/// @param port      新的串口设备路径
/// @param baud_rate 波特率
/// @return SR_OK 成功;其他见 sr_error_code
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_connect(d: *mut SrDriver, port: *const c_char, baud_rate: u32) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    if port.is_null() {
        return SR_ERR_NULL;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let driver = unsafe { &*d };
        let mut inner = driver.inner.lock().map_err(|_| DriverError::LockPoisoned)?;
        let port_name = unsafe { CStr::from_ptr(port) }
            .to_string_lossy()
            .into_owned();
        inner.connect(&port_name, baud_rate)
    }));
    match result {
        Ok(Ok(())) => SR_OK,
        Ok(Err(e)) => {
            let code = err_code(&e);
            let _ = set_last_error(unsafe { &mut *d }, &e.to_string());
            code
        }
        Err(_) => {
            let _ = set_last_error(unsafe { &mut *d }, "panic in sr_driver_connect");
            SR_ERR_PANIC
        }
    }
}

/// 构造句柄并注册 C 回调适配器
fn build_sr_driver(inner: Driver) -> *mut SrDriver {
    let callbacks = Arc::new(Mutex::new(CallbackTable(None)));
    inner.register_callback(CffiCallback::new(Arc::clone(&callbacks)));
    Box::into_raw(Box::new(SrDriver {
        inner: Mutex::new(inner),
        last_error: Mutex::new(String::new()),
        callbacks,
    }))
}

/// 释放句柄(内部 stop + join 读/分发线程);NULL 安全
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_free(d: *mut SrDriver) {
    if d.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(d)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_start(d: *mut SrDriver) -> i32 {
    guard(d, |d| d.start())
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_stop(d: *mut SrDriver) -> i32 {
    guard(d, |d| d.stop())
}

// ═══ 配置 ═══

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_write_config(d: *mut SrDriver, cfg: SrConfig) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    let Some(config) = config_from_sr(cfg) else {
        return SR_ERR_INVALID_ARG;
    };
    guard(d, |d| d.write_config(config))
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_write_config_sync(
    d: *mut SrDriver,
    cfg: SrConfig,
    out_success: *mut u8,
) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    let Some(config) = config_from_sr(cfg) else {
        return SR_ERR_INVALID_ARG;
    };
    if out_success.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |d| {
        let ok = d.write_config_sync(config)?;
        unsafe { *out_success = ok as u8 }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_query_config(d: *mut SrDriver, typ: u8, out: *mut SrConfig) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    let Some(ct) = ConfigType::from_u8(typ) else {
        return SR_ERR_INVALID_ARG;
    };
    if out.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |d| {
        let cfg = d.query_config_sync(ct)?;
        unsafe { *out = to_sr_config(cfg) }
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_query_all_configs(d: *mut SrDriver, out: *mut SrBoardConfig) -> i32 {
    if out.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |d| {
        let snap = d.query_all_configs_sync()?;
        unsafe { *out = to_sr_board_config(snap) }
        Ok(())
    })
}

/// 查询设备信息（同步，阻塞 ≤1s）
///
/// @param d   句柄
/// @param out [out] 设备信息
/// @return SR_OK 成功;SR_ERR_TIMEOUT 超时;其他见 sr_error_code
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_query_device_info(d: *mut SrDriver, out: *mut SrDeviceInfo) -> i32 {
    if out.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |d| {
        let info = d.query_device_info_sync()?;
        unsafe {
            *out = to_sr_device_info(info);
        }
        Ok(())
    })
}

// ═══ 舵机 ═══

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_forward_servo(d: *mut SrDriver, data: *const u8, len: usize) -> i32 {
    if len > 0 && data.is_null() {
        return SR_ERR_NULL;
    }
    let data = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    let cmd = ServoCmdWrapper::new(data.to_vec());
    guard(d, |d| d.forward_servo(&cmd))
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_forward_servo_sync(
    d: *mut SrDriver,
    data: *const u8,
    len: usize,
    out: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> i32 {
    if out_len.is_null() || (cap > 0 && out.is_null()) || (len > 0 && data.is_null()) {
        return SR_ERR_NULL;
    }
    let data = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    let cmd = ServoCmdWrapper::new(data.to_vec());
    guard(d, |d| {
        let ack = d.forward_servo_sync(&cmd)?;
        let ack_data = ack.data();
        if ack_data.len() > cap {
            // 先写回实际长度,调用方可据此扩容重试
            unsafe { *out_len = ack_data.len() };
            return Err(DriverError::PayloadTooShort {
                expected: ack_data.len(),
                got: cap,
            });
        }
        unsafe {
            ptr::copy_nonoverlapping(ack_data.as_ptr(), out, ack_data.len());
            *out_len = ack_data.len();
        }
        Ok(())
    })
}

// ═══ 板级命令 ═══

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_send_command(d: *mut SrDriver, cmd: u8) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    let Some(kind) = RequestKind::from_u8(cmd) else {
        return SR_ERR_INVALID_ARG;
    };
    guard(d, |d| d.send_command(kind))
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_send_command_sync(
    d: *mut SrDriver,
    cmd: u8,
    out_success: *mut u8,
) -> i32 {
    if d.is_null() {
        return SR_ERR_NULL;
    }
    let Some(kind) = RequestKind::from_u8(cmd) else {
        return SR_ERR_INVALID_ARG;
    };
    if out_success.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |d| {
        let ok = d.send_command_sync(kind)?;
        unsafe { *out_success = ok as u8 }
        Ok(())
    })
}

// ═══ 固件更新 ═══

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_firmware_update(
    d: *mut SrDriver,
    offset: u32,
    data: *const u8,
    len: usize,
) -> i32 {
    if len > 0 && data.is_null() {
        return SR_ERR_NULL;
    }
    let data = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    guard(d, |d| d.firmware_update(offset, data))
}

#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_firmware_update_sync(
    d: *mut SrDriver,
    offset: u32,
    data: *const u8,
    len: usize,
    out_success: *mut u8,
) -> i32 {
    if len > 0 && data.is_null() {
        return SR_ERR_NULL;
    }
    if out_success.is_null() {
        return SR_ERR_NULL;
    }
    let data = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    };
    guard(d, |d| {
        let ok = d.firmware_update_sync(offset, data)?;
        unsafe { *out_success = ok as u8 }
        Ok(())
    })
}

// ═══ 回调 ═══

/// 设置/替换 C 回调表(NULL 指针的槽位被忽略);任意时刻可调用
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_set_callbacks(d: *mut SrDriver, cbs: *const SrCallbacks) -> i32 {
    if d.is_null() || cbs.is_null() {
        return SR_ERR_NULL;
    }
    guard(d, |_| {
        let table = unsafe { *cbs };
        let mut slot = unsafe { &*d }
            .callbacks
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        slot.0 = Some(table);
        Ok(())
    })
}

/// 拷贝最近一次错误的描述(截断 + NUL 结尾)
#[unsafe(no_mangle)]
pub extern "C" fn sr_driver_last_error(d: *mut SrDriver, buf: *mut c_char, len: usize) -> i32 {
    if d.is_null() || buf.is_null() || len == 0 {
        return SR_ERR_NULL;
    }
    let msg = match unsafe { &*d }.last_error.lock() {
        Ok(m) => m.clone(),
        Err(_) => "lock poisoned".to_string(),
    };
    write_err_buf(buf, len, &msg);
    SR_OK
}

// ═══ 单元测试(mock 传输层)═══

#[cfg(all(test, feature = "mock"))]
mod tests {
    use super::*;
    use crate::MockTransport;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn test_driver() -> SrDriver {
        let inner = Driver::new(MockTransport::new());
        let callbacks = Arc::new(Mutex::new(CallbackTable(None)));
        inner.register_callback(CffiCallback::new(Arc::clone(&callbacks)));
        SrDriver {
            inner: Mutex::new(inner),
            last_error: Mutex::new(String::new()),
            callbacks,
        }
    }

    fn boxed_ptr() -> *mut SrDriver {
        Box::into_raw(Box::new(test_driver()))
    }

    #[test]
    fn test_config_roundtrip() {
        // 四类取值: bool / u8 / u16 / u32
        let cases: &[(SrConfig, f32)] = &[
            (
                SrConfig {
                    typ: 0x10,
                    value: 1.0,
                },
                1.0,
            ), // SwitchPowerServo
            (
                SrConfig {
                    typ: 0x20,
                    value: 80.0,
                },
                80.0,
            ), // ChargeStopSoc
            (
                SrConfig {
                    typ: 0x30,
                    value: 500.0,
                },
                500.0,
            ), // PowerServoCurrentLimitMa
            (
                SrConfig {
                    typ: 0x37,
                    value: 1000000.0,
                },
                1000000.0,
            ), // ServoBaudRate
        ];
        for (src, expected) in cases {
            let cfg = config_from_sr(*src).expect("valid config type");
            let back = to_sr_config(cfg);
            assert_eq!(back.typ, src.typ);
            assert_eq!(back.value, *expected);
        }
        // 非法 type
        assert!(
            config_from_sr(SrConfig {
                typ: 0x99,
                value: 0.0
            })
            .is_none()
        );
        assert_eq!(
            sr_driver_write_config(
                ptr::null_mut(),
                SrConfig {
                    typ: 0x99,
                    value: 0.0
                }
            ),
            SR_ERR_NULL
        );
    }

    #[test]
    fn test_open_null_port() {
        let mut err_buf = [0 as c_char; 64];
        let d = sr_driver_open(ptr::null(), 115200, err_buf.as_mut_ptr(), err_buf.len());
        assert!(d.is_null());
        let msg = unsafe { CStr::from_ptr(err_buf.as_ptr()) }.to_string_lossy();
        assert!(!msg.is_empty(), "err_buf should be populated");
    }

    #[test]
    fn test_connect_null_args() {
        let d = boxed_ptr();
        // NULL driver
        assert_eq!(
            sr_driver_connect(
                ptr::null_mut(),
                b"/dev/ttyUSB0\0".as_ptr() as *const c_char,
                115200
            ),
            SR_ERR_NULL
        );
        // NULL port
        assert_eq!(sr_driver_connect(d, ptr::null(), 115200), SR_ERR_NULL);
        sr_driver_free(d);
    }

    #[test]
    fn test_start_stop_free() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_query_all_configs_sync() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        let mut out = SrBoardConfig {
            servo_baud_rate: 0,
            ..unsafe { std::mem::zeroed() }
        };
        assert_eq!(sr_driver_query_all_configs(d, &mut out), SR_OK);
        // mock 默认配置
        assert_eq!(out.servo_baud_rate, 115200);
        assert_eq!(out.power_servo_on, 1);
        assert_eq!(out.charge_stop_percentage, 100);
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_write_config_sync_roundtrip() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        let mut success = 0u8;
        let rc = sr_driver_write_config_sync(
            d,
            SrConfig {
                typ: 0x37,
                value: 1000000.0,
            },
            &mut success,
        );
        assert_eq!(rc, SR_OK);
        assert_eq!(success, 1);
        // 写回后查询验证(mock 同步更新内部配置)
        let mut out = SrConfig { typ: 0, value: 0.0 };
        assert_eq!(sr_driver_query_config(d, 0x37, &mut out), SR_OK);
        assert_eq!(out.value, 1000000.0);
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_query_config_sync() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        let mut out = SrConfig { typ: 0, value: 0.0 };
        assert_eq!(sr_driver_query_config(d, 0x10, &mut out), SR_OK);
        assert_eq!(out.value, 1.0); // mock 默认 power_servo_on = true
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_query_config_invalid_type() {
        let d = boxed_ptr();
        let mut out = SrConfig { typ: 0, value: 0.0 };
        assert_eq!(
            sr_driver_query_config(d, 0x99, &mut out),
            SR_ERR_INVALID_ARG
        );
        sr_driver_free(d);
    }

    #[test]
    fn test_forward_servo_sync_timeout() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        let data = [0x01u8, 0x02];
        let mut out = [0u8; 64];
        let mut out_len = 0usize;
        // mock 不对舵机帧回 ACK → 1s 超时
        assert_eq!(
            sr_driver_forward_servo_sync(
                d,
                data.as_ptr(),
                data.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len
            ),
            SR_ERR_TIMEOUT
        );
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_guard_panic() {
        let d = boxed_ptr();
        let code = guard(d, |_| -> Result<(), DriverError> { panic!("boom") });
        assert_eq!(code, SR_ERR_PANIC);
        // last_error 有记录
        let mut buf = [0 as c_char; 64];
        assert_eq!(sr_driver_last_error(d, buf.as_mut_ptr(), buf.len()), SR_OK);
        assert!(
            !unsafe { CStr::from_ptr(buf.as_ptr()) }
                .to_string_lossy()
                .is_empty()
        );
        sr_driver_free(d);
    }

    #[test]
    fn test_set_callbacks_null_and_valid() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_set_callbacks(d, ptr::null()), SR_ERR_NULL);
        let cbs = SrCallbacks::default();
        assert_eq!(sr_driver_set_callbacks(d, &cbs), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_last_error() {
        let d = boxed_ptr();
        assert_eq!(sr_driver_start(d), SR_OK);
        // 重复 start → AlreadyStarted
        assert_eq!(sr_driver_start(d), SR_ERR_ALREADY_STARTED);
        let mut buf = [0 as c_char; 64];
        assert_eq!(sr_driver_last_error(d, buf.as_mut_ptr(), buf.len()), SR_OK);
        let msg = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_string_lossy();
        assert!(!msg.is_empty());
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }

    #[test]
    fn test_callback_invocation() {
        static IMU_CALLED: AtomicBool = AtomicBool::new(false);
        extern "C" fn on_imu(_u: *mut std::ffi::c_void, _d: *const SrImu) {
            IMU_CALLED.store(true, Ordering::SeqCst);
        }
        let d = boxed_ptr();
        let mut cbs = SrCallbacks::default();
        cbs.on_imu_data = Some(on_imu);
        assert_eq!(sr_driver_set_callbacks(d, &cbs), SR_OK);
        assert_eq!(sr_driver_start(d), SR_OK);
        // mock 每 10ms 发一帧 IMU;等分发线程触发 C 回调
        for _ in 0..50 {
            if IMU_CALLED.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            IMU_CALLED.load(Ordering::SeqCst),
            "C callback should fire from dispatch thread"
        );
        assert_eq!(sr_driver_stop(d), SR_OK);
        sr_driver_free(d);
    }
}
