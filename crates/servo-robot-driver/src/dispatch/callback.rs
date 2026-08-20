//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! DriverCallback trait 定义

use crate::error::DriverError;
use crate::protocol::battery_state::BatteryState;
use crate::protocol::config::BoardConfigSnapshot;
use crate::protocol::device_info::DeviceInfo;
use crate::protocol::diagnostic::Diagnostic;
use crate::protocol::event::BoardEvent;
use crate::protocol::imu::ImuData;
use crate::protocol::log::{LogLevel, LogMessage};
use crate::protocol::power::PowerData;
use crate::protocol::response::Response;

/// 回调 trait — 实现感兴趣的回调，其余用默认空实现
///
/// 回调在独立的分发线程上触发，不会阻塞读线程。
///
/// # Example
///
/// ```rust
/// use servo_robot_driver::DriverCallback;
/// use servo_robot_driver::protocol::imu::ImuData;
///
/// struct MyCallback {
///     imu_count: u64,
/// }
///
/// impl DriverCallback for MyCallback {
///     fn on_imu_data(&mut self, data: &ImuData) {
///         self.imu_count += 1;
///         println!("IMU #{}: roll={:.1}", self.imu_count, data.roll);
///     }
/// }
/// ```
pub trait DriverCallback: Send + 'static {
    fn on_imu_data(&mut self, _data: &ImuData) {}
    fn on_power_data(&mut self, _data: &PowerData) {}
    fn on_battery_state(&mut self, _state: &BatteryState) {}
    fn on_config_snapshot(&mut self, _config: &BoardConfigSnapshot) {}
    fn on_board_event(&mut self, _event: &BoardEvent) {}
    fn on_device_info(&mut self, _info: &DeviceInfo) {}
    fn on_diagnostic(&mut self, _diag: &Diagnostic) {}

    /// 板级日志回调
    ///
    /// `ts` 为 Unix 时间戳（毫秒），在读线程解码帧时采集。
    /// 默认实现通过 `log` 库输出，带 `[ServoRobotBoard]` 前缀。
    /// TUI/ROS2 可覆盖此方法自行处理日志。
    fn on_log(&mut self, ts: u64, log_msg: &LogMessage) {
        let total_s = ts / 1000;
        let ms = ts % 1000;
        let h = (total_s % 86400) / 3600;
        let m = (total_s % 3600) / 60;
        let s = total_s % 60;
        let prefix = "[ServoRobotBoard]";
        match log_msg.level {
            LogLevel::Error => log::error!(
                "{} [{:02}:{:02}:{:02}.{:03}] {}::{}: {}",
                prefix,
                h,
                m,
                s,
                ms,
                log_msg.file_name,
                log_msg.fun_name,
                log_msg.msg
            ),
            LogLevel::Warn => log::warn!(
                "{} [{:02}:{:02}:{:02}.{:03}] {}::{}: {}",
                prefix,
                h,
                m,
                s,
                ms,
                log_msg.file_name,
                log_msg.fun_name,
                log_msg.msg
            ),
            LogLevel::Info | LogLevel::OFF => log::info!(
                "{} [{:02}:{:02}:{:02}.{:03}] {}::{}: {}",
                prefix,
                h,
                m,
                s,
                ms,
                log_msg.file_name,
                log_msg.fun_name,
                log_msg.msg
            ),
            LogLevel::Debug => log::debug!(
                "{} [{:02}:{:02}:{:02}.{:03}] {}::{}: {}",
                prefix,
                h,
                m,
                s,
                ms,
                log_msg.file_name,
                log_msg.fun_name,
                log_msg.msg
            ),
        }
    }

    /// 统一应答回调（替代原 6 个 on_ack_* 回调）
    ///
    /// `response.request_kind` 表示应答哪个请求，`response.success` 表示是否成功，
    /// `response.data` 携带附加数据（如 Config、BoardConfigSnapshot、DeviceInfo 等）。
    fn on_response(&mut self, _response: &Response) {}

    fn on_error(&mut self, _error: &DriverError) {}
}
