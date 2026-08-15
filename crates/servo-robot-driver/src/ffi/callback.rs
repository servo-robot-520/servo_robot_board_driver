//! C 回调表适配器 — 将 C 函数指针表桥接到 `DriverCallback` trait
//!
//! 线程模型: 回调在驱动内部的分发线程上触发。C 侧回调必须线程安全,
//! 且回调内禁止调用任何 `sr_driver_*` 函数(尤其 `sr_driver_free`,会自死锁)。

use super::{
    err_code, SrBoardConfig, SrBoardEvent, SrBatteryState, SrConfig, SrImu, SrLogMessage,
    SrPower, SrSystemInfo,
};
use crate::dispatch::callback::DriverCallback;
use crate::error::DriverError;
use crate::protocol::battery_state::BatteryState;
use crate::protocol::config::{BoardConfigSnapshot, Config};
use crate::protocol::event::BoardEvent;
use crate::protocol::imu::ImuData;
use crate::protocol::log::LogMessage;
use crate::protocol::power::PowerData;
use crate::protocol::servo::ServoCmdWrapper;
use crate::protocol::system::SystemInfo;
use std::ffi::{c_void, CString};
use std::sync::{Arc, Mutex};

/// C 回调表 — 与 `include/servo_robot_driver.h` 的 `sr_callbacks` 严格一致。
///
/// `Option<extern "C" fn>` 由 NPO 保证布局与可空函数指针一致(C NULL ↔ Rust None)。
/// 所有回调第一个参数均为 `userdata`。
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SrCallbacks {
    pub userdata: *mut c_void,
    pub on_imu_data: Option<extern "C" fn(*mut c_void, *const SrImu)>,
    pub on_power_data: Option<extern "C" fn(*mut c_void, *const SrPower)>,
    pub on_battery_state: Option<extern "C" fn(*mut c_void, *const SrBatteryState)>,
    pub on_config_snapshot: Option<extern "C" fn(*mut c_void, *const SrBoardConfig)>,
    pub on_board_event: Option<extern "C" fn(*mut c_void, *const SrBoardEvent)>,
    pub on_system_info: Option<extern "C" fn(*mut c_void, *const SrSystemInfo)>,
    pub on_log: Option<extern "C" fn(*mut c_void, *const SrLogMessage)>,
    pub on_ack_cfg_write: Option<extern "C" fn(*mut c_void, u8)>,
    pub on_ack_cfg_query: Option<extern "C" fn(*mut c_void, *const SrConfig)>,
    pub on_ack_cfg_query_all: Option<extern "C" fn(*mut c_void, *const SrBoardConfig)>,
    pub on_ack_servo_cmd: Option<extern "C" fn(*mut c_void, *const u8, usize)>,
    pub on_ack_command: Option<extern "C" fn(*mut c_void, u8)>,
    pub on_ack_firmware_update: Option<extern "C" fn(*mut c_void, u8, u32)>,
    pub on_error: Option<extern "C" fn(*mut c_void, i32)>,
}

/// 可空回调表容器。`userdata` 是裸指针(非 Send),由 C 调用方契约保证
/// 其跨线程安全(回调本就在分发线程触发),故显式标记 Send + Sync。
pub struct CallbackTable(pub Option<SrCallbacks>);

unsafe impl Send for CallbackTable {}
unsafe impl Sync for CallbackTable {}

/// `DriverCallback` 的 C 适配器 — 每次事件从表里取对应函数指针调用。
pub struct CffiCallback {
    table: Arc<Mutex<CallbackTable>>,
}

impl CffiCallback {
    pub fn new(table: Arc<Mutex<CallbackTable>>) -> Self {
        CffiCallback { table }
    }

    /// 取回调表并执行闭包;表为空或锁中毒时静默跳过
    fn with_table<R>(&self, f: impl FnOnce(&SrCallbacks) -> R) {
        if let Ok(guard) = self.table.lock() {
            if let Some(cb) = guard.0.as_ref() {
                f(cb);
            }
        }
    }
}

/// String → NUL 结尾 CString(含内嵌 NUL 时降级为占位串)
fn cstr_owned(s: &str) -> CString {
    match CString::new(s) {
        Ok(c) => c,
        Err(_) => CString::new("<invalid utf8>").expect("static string is NUL-free"),
    }
}

impl DriverCallback for CffiCallback {
    fn on_imu_data(&mut self, data: &ImuData) {
        let sr = SrImu {
            accel: data.accel,
            gyro: data.gyro,
            quaternion: data.quaternion,
            timestamp_ms: data.timestamp_ms,
            roll: data.roll,
            pitch: data.pitch,
            yaw: data.yaw,
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_imu_data {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_power_data(&mut self, data: &PowerData) {
        let sr = SrPower {
            servo_voltage_mv: data.servo_voltage_mv,
            servo_current_ma: data.servo_current_ma,
            charge_in_voltage_mv: data.charge_in_voltage_mv,
            charge_in_current_ma: data.charge_in_current_ma,
            bat_voltage_mv: data.bat_voltage_mv,
            bat_current_ma: data.bat_current_ma,
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_power_data {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_battery_state(&mut self, state: &BatteryState) {
        let sr = SrBatteryState {
            voltage_mv: state.voltage_mv,
            current_ma: state.current_ma,
            capacity_mah: state.capacity_mah,
            design_capacity_mah: state.design_capacity_mah,
            percentage: state.percentage,
            temperature: state.temperature,
            charge_status: state.charge_status as u8,
            health: state.health as u8,
            technology: state.technology as u8,
            present: state.present as u8,
            serial_number: state.serial_number,
            cell_voltages_mv: state.cell_voltages_mv.as_ptr(),
            cell_count: state.cell_voltages_mv.len() as u32,
            cell_temperatures: state.cell_temperatures.as_ptr(),
            cell_temp_count: state.cell_temperatures.len() as u32,
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_battery_state {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_config_snapshot(&mut self, config: &BoardConfigSnapshot) {
        let sr = super::to_sr_board_config(config.clone());
        self.with_table(|cb| {
            if let Some(f) = cb.on_config_snapshot {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_board_event(&mut self, event: &BoardEvent) {
        let sr = SrBoardEvent {
            charge_phase: event.charge_phase as u8,
            state_change_flags: event.state_change_flags.bits(),
            protection_flags: event.protection_flags.bits(),
            error_flags: event.error_flags.bits(),
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_board_event {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_system_info(&mut self, info: &SystemInfo) {
        let sr = SrSystemInfo {
            device_id: info.device_id,
            uid: info.uid,
            imu_id: info.imu_id,
            uptime_s: info.uptime_s,
            cpu_usage_percent: info.cpu_usage_percent,
            free_heap_kb: info.free_heap_kb,
            stack_watermark_min_kb: info.stack_watermark_min_kb,
            i2c_error_count: info.i2c_error_count,
            spi_error_count: info.spi_error_count,
            uart_error_count: info.uart_error_count,
            usb_error_count: info.usb_error_count,
            frames_sent_total: info.frames_sent_total,
            pd_request_voltage_mv: info.pd_request_voltage_mv,
            pd_request_current_ma: info.pd_request_current_ma,
            fw_major: info.firmware_version.major,
            fw_minor: info.firmware_version.minor,
            fw_patch: info.firmware_version.patch,
            temp_servo_power: info.temp_servo_power,
            temp_5v_power: info.temp_5v_power,
            temp_mcu: info.temp_mcu,
            temp_charge: info.temp_charge,
            temp_battery: info.temp_battery,
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_system_info {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        let file = cstr_owned(&log_msg.file_name);
        let fun = cstr_owned(&log_msg.fun_name);
        let msg = cstr_owned(&log_msg.msg);
        let sr = SrLogMessage {
            ts_ms: ts,
            level: log_msg.level as u8,
            file_name: file.as_ptr(),
            fun_name: fun.as_ptr(),
            msg: msg.as_ptr(),
        };
        self.with_table(|cb| {
            if let Some(f) = cb.on_log {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_ack_cfg_write(&mut self, success: bool) {
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_cfg_write {
                f(cb.userdata, success as u8)
            }
        });
    }

    fn on_ack_cfg_query(&mut self, config: &Config) {
        let sr = super::to_sr_config(*config);
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_cfg_query {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_ack_cfg_query_all(&mut self, config: &BoardConfigSnapshot) {
        let sr = super::to_sr_board_config(config.clone());
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_cfg_query_all {
                f(cb.userdata, &sr)
            }
        });
    }

    fn on_ack_servo_cmd(&mut self, cmd: &ServoCmdWrapper) {
        let data = cmd.data();
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_servo_cmd {
                f(cb.userdata, data.as_ptr(), data.len())
            }
        });
    }

    fn on_ack_command(&mut self, success: bool) {
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_command {
                f(cb.userdata, success as u8)
            }
        });
    }

    fn on_ack_firmware_update(&mut self, success: bool, offset: u32) {
        self.with_table(|cb| {
            if let Some(f) = cb.on_ack_firmware_update {
                f(cb.userdata, success as u8, offset)
            }
        });
    }

    fn on_error(&mut self, error: &DriverError) {
        let code = err_code(error);
        self.with_table(|cb| {
            if let Some(f) = cb.on_error {
                f(cb.userdata, code)
            }
        });
    }
}
