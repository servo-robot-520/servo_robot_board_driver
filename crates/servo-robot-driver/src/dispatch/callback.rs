//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! DriverCallback trait 定义

use crate::error::DriverError;
use crate::protocol::battery_state::BatteryState;
use crate::protocol::config::{BoardConfigSnapshot, Config};
use crate::protocol::device_info::DeviceInfo;
use crate::protocol::diagnostic::Diagnostic;
use crate::protocol::event::BoardEvent;
use crate::protocol::imu::ImuData;
use crate::protocol::log::{LogLevel, LogMessage};
use crate::protocol::power::PowerData;
use crate::protocol::request::RequestType;
use crate::protocol::response::Response;
use crate::protocol::servo::ServoCmdWrapper;

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

    /// 统一应答回调（默认实现自动分解到具体 on_ack_* 回调）
    ///
    /// 每个 Response 都会调用此回调。默认实现根据 `request_type` 解析数据
    /// 并调用对应的具体回调。覆盖此方法可接管全部应答处理逻辑。
    fn on_response(&mut self, response: &Response) {
        match response.request_type {
            // 数据型应答:成功且可解析才交付具体回调,否则一律走 on_ack_failed
            // (NACK 或数据损坏都不应静默,否则上层无法区分"查询被拒"与"没收到")
            RequestType::DeviceInfo => {
                match (response.success, DeviceInfo::from_bytes(&response.data)) {
                    (true, Ok(info)) => self.on_ack_device_info(&info),
                    _ => self.on_ack_failed(response),
                }
            }
            RequestType::ConfigQuery => {
                match (response.success, Config::from_bytes(&response.data)) {
                    (true, Ok(config)) => self.on_ack_cfg_query(&config),
                    _ => self.on_ack_failed(response),
                }
            }
            RequestType::ConfigQueryAll => {
                match (response.success, BoardConfigSnapshot::from_bytes(&response.data)) {
                    (true, Ok(snapshot)) => self.on_ack_cfg_query_all(&snapshot),
                    _ => self.on_ack_failed(response),
                }
            }
            RequestType::ConfigWrite => {
                self.on_ack_cfg_write(response.success);
            }
            RequestType::ServoForward => {
                let cmd = ServoCmdWrapper::new(response.data.clone());
                self.on_ack_servo_cmd(&cmd);
            }
            RequestType::Reset | RequestType::Shutdown | RequestType::Ota => {
                self.on_ack_command(response.success);
            }
            RequestType::FirmwareUpdate => {
                let offset = if response.data.len() >= 4 {
                    u32::from_le_bytes([
                        response.data[0],
                        response.data[1],
                        response.data[2],
                        response.data[3],
                    ])
                } else {
                    0
                };
                self.on_ack_firmware_update(response.success, offset);
            }
        }
    }

    // ═══ 具体应答回调（dispatch 层根据 request_type 自动分发）═══

    /// 设备信息应答（DeviceInfo 查询响应）
    fn on_ack_device_info(&mut self, _info: &DeviceInfo) {}

    /// 配置写入确认
    fn on_ack_cfg_write(&mut self, _success: bool) {}

    /// 单个配置查询响应
    fn on_ack_cfg_query(&mut self, _config: &Config) {}

    /// 所有配置查询响应
    fn on_ack_cfg_query_all(&mut self, _config: &BoardConfigSnapshot) {}

    /// 舵机命令响应
    fn on_ack_servo_cmd(&mut self, _cmd: &ServoCmdWrapper) {}

    /// 系统命令确认（Reset/Shutdown/Ota）
    fn on_ack_command(&mut self, _success: bool) {}

    /// 固件更新确认
    fn on_ack_firmware_update(&mut self, _success: bool, _offset: u32) {}

    /// 数据型应答失败通知（DeviceInfo / ConfigQuery / ConfigQueryAll）
    ///
    /// 应答为 `success=false`(板子 NACK)或数据无法解析时调用;此类应答没有
    /// 可交付的数据,因此不会调用对应的 `on_ack_*` 具体回调。覆盖此方法可
    /// 区分"查询被拒"与"根本没收到应答"(后者表现为无任何回调)。
    fn on_ack_failed(&mut self, _response: &Response) {}

    fn on_error(&mut self, _error: &DriverError) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::config::Config;
    use crate::protocol::request::RequestType;

    struct Cb {
        failed: Vec<RequestType>,
        cfg_query_called: bool,
    }

    impl DriverCallback for Cb {
        fn on_ack_failed(&mut self, response: &Response) {
            self.failed.push(response.request_type);
        }
        fn on_ack_cfg_query(&mut self, _config: &Config) {
            self.cfg_query_called = true;
        }
    }

    /// 数据型应答失败(NACK / 数据损坏)必须走 on_ack_failed,
    /// 不能静默也不调用具体回调。
    #[test]
    fn test_on_response_failure_invokes_on_ack_failed() {
        let mut cb = Cb {
            failed: Vec::new(),
            cfg_query_called: false,
        };

        // NACK:无数据可交付 → on_ack_failed
        cb.on_response(&Response::simple(RequestType::ConfigQuery, false));
        assert_eq!(cb.failed, vec![RequestType::ConfigQuery]);
        assert!(!cb.cfg_query_called);

        // success 但数据无法解析 → on_ack_failed
        cb.on_response(&Response::new(RequestType::ConfigQuery, true, vec![0x99, 0x99]));
        assert_eq!(cb.failed.len(), 2);
        assert!(!cb.cfg_query_called);

        // 成功且可解析 → 具体回调,不触发 on_ack_failed
        let config = Config::ChargeStopSoc(80);
        cb.on_response(&Response::new(
            RequestType::ConfigQuery,
            true,
            config.to_bytes(),
        ));
        assert_eq!(cb.failed.len(), 2, "success path must not fire on_ack_failed");
        assert!(cb.cfg_query_called);
    }
}
