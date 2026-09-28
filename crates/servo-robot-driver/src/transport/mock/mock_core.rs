//! Mock 传输层共享内核
//!
//! 包含所有模拟状态和逻辑，供 MockTransport 使用。

use super::mock_data::*;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::protocol::frame::{FrameType, FromPayload, RawFrame, ToPayload};
use crate::protocol::log::{LogLevel, LogMessage};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Mock 共享内核 —— 所有模拟状态和业务逻辑
pub(crate) struct MockCore {
    rx_queue: VecDeque<Vec<u8>>,
    priority_queue: VecDeque<Vec<u8>>,
    imu: ImuSimulator,
    power: PowerSimulator,
    // thermal 已合并到 Diagnostic
    battery: BatterySimulator,
    device_info: DeviceInfoSimulator,
    diagnostic: DiagnosticSimulator,
    event: EventSimulator,
    config: BoardConfigSnapshot,
    last_imu: Instant,
    last_power: Instant,
    last_battery: Instant,
    last_diagnostic: Instant,
    last_event: Instant,
    last_log: Instant,
    pub(crate) written_frames: Vec<Vec<u8>>,
    pub(crate) connected: bool,
    auto_disconnect_frames: Option<u64>,
    pub(crate) frame_count: u64,
}

impl MockCore {
    pub fn new() -> Self {
        let now = Instant::now();
        MockCore {
            rx_queue: VecDeque::new(),
            priority_queue: VecDeque::new(),
            imu: ImuSimulator::new(),
            power: PowerSimulator::new(),
            battery: BatterySimulator::new(),
            device_info: DeviceInfoSimulator::new(),
            diagnostic: DiagnosticSimulator::new(),
            event: EventSimulator::new(),
            config: BoardConfigSnapshot::default(),
            last_imu: now,
            last_power: now,
            last_battery: now,
            last_diagnostic: now,
            last_event: now,
            last_log: now,
            written_frames: Vec::new(),
            connected: true,
            auto_disconnect_frames: None,
            frame_count: 0,
        }
    }

    // ═══ 配置 setter ═══

    pub fn set_battery_soc(&mut self, percentage: f32) {
        // Keep as f32 (0~1) in simulator, convert to u8 (1~100) when generating
        self.battery.percentage = percentage.clamp(0.0, 1.0);
    }

    pub fn set_initial_attitude(&mut self, roll_deg: f32, pitch_deg: f32, yaw_deg: f32) {
        self.imu.set_attitude(roll_deg, pitch_deg, yaw_deg);
    }

    pub fn set_charging(&mut self, charging: bool) {
        self.power.charging = charging;
        self.battery.charging = charging;
        self.diagnostic.charging = charging;
        self.event.charging = charging;
    }

    pub fn set_charging_probability(&mut self, charging_probability: f64) {
        let charging = rand::RngExt::random_bool(&mut rand::rng(), charging_probability);
        self.set_charging(charging);
    }

    pub fn set_auto_disconnect(&mut self, after_frames: u64) {
        self.auto_disconnect_frames = Some(after_frames);
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn reconnect(&mut self) {
        self.connected = true;
    }

    pub fn written_frames(&self) -> &[Vec<u8>] {
        &self.written_frames
    }

    // ═══ 连接状态检查 ═══

    /// 检查是否需要自动断开，返回 true 表示已断开
    pub fn check_disconnect(&mut self) -> bool {
        if !self.connected {
            return true;
        }
        if let Some(threshold) = self.auto_disconnect_frames
            && self.frame_count >= threshold
        {
            self.connected = false;
            return true;
        }
        false
    }

    // ═══ 帧读取 ═══

    /// 尝试从队列取一帧（优先 ACK，再普通帧）
    pub fn try_read_frame(&mut self) -> Option<Vec<u8>> {
        // 优先返回 ACK 响应
        if let Some(frame) = self.priority_queue.pop_front() {
            self.frame_count += 1;
            return Some(frame);
        }

        // 从队列取出一帧
        if let Some(frame) = self.rx_queue.pop_front() {
            self.frame_count += 1;
            return Some(frame);
        }

        None
    }

    /// 生成模拟数据帧入队
    pub fn generate_frames(&mut self) {
        let now = Instant::now();

        // IMU 100Hz (10ms)
        if now.duration_since(self.last_imu) >= Duration::from_millis(10) {
            let data = self.imu.generate();
            let frame = RawFrame {
                frame_type: FrameType::Imu,
                payload: data.to_bytes(),
            };
            self.rx_queue.push_back(frame.encode());
            self.last_imu = now;
        }

        // Power 20Hz (50ms)
        if now.duration_since(self.last_power) >= Duration::from_millis(50) {
            let data = self.power.generate();
            let frame = RawFrame {
                frame_type: FrameType::Power,
                payload: data.to_bytes(),
            };
            self.rx_queue.push_back(frame.encode());
            self.last_power = now;
        }

        // Battery 10Hz (100ms)
        if now.duration_since(self.last_battery) >= Duration::from_millis(100) {
            let data = self.battery.generate();
            let frame = RawFrame {
                frame_type: FrameType::Battery,
                payload: data.to_bytes(),
            };
            self.rx_queue.push_back(frame.encode());
            self.last_battery = now;
        }

        // Diagnostic 1Hz (1000ms) - 运行时诊断数据
        if now.duration_since(self.last_diagnostic) >= Duration::from_millis(1000) {
            let dt = now.duration_since(self.last_diagnostic).as_secs_f32();
            let data = self.diagnostic.generate(dt);
            let frame = RawFrame {
                frame_type: FrameType::Diagnostic,
                payload: data.to_bytes(),
            };
            self.rx_queue.push_back(frame.encode());
            self.last_diagnostic = now;
        }

        // Event 1Hz (1000ms)
        if now.duration_since(self.last_event) >= Duration::from_millis(1000) {
            let data = self.event.generate();
            let frame = RawFrame {
                frame_type: FrameType::Event,
                payload: data.to_bytes(),
            };
            self.rx_queue.push_back(frame.encode());
            self.last_event = now;
        }

        // Log ~0.05Hz (20000ms)，随机日志等级，受 tx_log_level 过滤
        if now.duration_since(self.last_log) >= Duration::from_millis(20000) {
            let level = self.random_log_level();
            if self.should_emit_log(level) {
                self.push_log(
                    level,
                    "mock.rs",
                    "simulate",
                    &format!("random {} log #{}", level, self.frame_count),
                );
            }
            self.last_log = now;
        }
    }

    /// 生成随机日志等级
    fn random_log_level(&self) -> LogLevel {
        match rand::RngExt::random_range(&mut rand::rng(), 0u8..4) {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Warn,
            _ => LogLevel::Error,
        }
    }

    /// 根据配置的 tx_log_level 判断是否应该发送该等级的日志
    fn should_emit_log(&self, level: LogLevel) -> bool {
        (level as u8) >= (self.config.tx_log_level as u8)
    }

    // ═══ 帧写入 ═══

    /// 记录写入帧并生成 ACK 响应
    pub fn prepare_write(&mut self, frame: &[u8]) {
        self.written_frames.push(frame.to_vec());
        self.handle_write(frame);
    }

    fn handle_write(&mut self, frame: &[u8]) {
        use crate::protocol::request::{Request, RequestType};
        use crate::protocol::response::Response;

        if let Ok((raw, _)) = RawFrame::decode(frame) {
            if raw.frame_type != FrameType::Request {
                return;
            }
            let request = match Request::from_payload(&raw.payload) {
                Ok(r) => r,
                Err(_) => return,
            };

            match request.request_type {
                RequestType::ConfigQuery => {
                    if !request.data.is_empty() {
                        let config_type = ConfigType::from_u8(request.data[0]);
                        if let Some(ct) = config_type {
                            let config = Config::from_type_value(ct, self.get_config_value(ct));
                            let resp =
                                Response::new(RequestType::ConfigQuery, true, config.to_bytes());
                            let frame = RawFrame {
                                frame_type: FrameType::Response,
                                payload: resp.to_payload(),
                            };
                            self.priority_queue.push_back(frame.encode());
                            self.push_log(
                                LogLevel::Info,
                                "config.rs",
                                "handle_query",
                                &format!("queried {:?}", ct),
                            );
                        }
                    }
                }
                RequestType::ConfigQueryAll => {
                    let resp =
                        Response::new(RequestType::ConfigQueryAll, true, self.config.to_bytes());
                    let frame = RawFrame {
                        frame_type: FrameType::Response,
                        payload: resp.to_payload(),
                    };
                    self.priority_queue.push_back(frame.encode());
                    self.push_log(
                        LogLevel::Info,
                        "config.rs",
                        "handle_query",
                        "queried all configs",
                    );
                }
                RequestType::ConfigWrite => {
                    if let Ok(config) = Config::from_bytes(&request.data) {
                        self.update_config(config);
                        let resp = Response::simple(RequestType::ConfigWrite, true);
                        let frame = RawFrame {
                            frame_type: FrameType::Response,
                            payload: resp.to_payload(),
                        };
                        self.priority_queue.push_back(frame.encode());
                        self.publish_config();
                        self.push_log(
                            LogLevel::Info,
                            "config.rs",
                            "handle_write",
                            "config updated",
                        );
                    }
                }
                RequestType::Reset | RequestType::Shutdown | RequestType::Ota => {
                    let resp = Response::simple(request.request_type, true);
                    let frame = RawFrame {
                        frame_type: FrameType::Response,
                        payload: resp.to_payload(),
                    };
                    self.priority_queue.push_back(frame.encode());
                    self.push_log(
                        LogLevel::Info,
                        "command.rs",
                        "handle_command",
                        &format!("{} executed", request.request_type),
                    );
                }
                RequestType::DeviceInfo => {
                    let info = self.device_info.generate();
                    let resp = Response::new(RequestType::DeviceInfo, true, info.to_bytes());
                    let frame = RawFrame {
                        frame_type: FrameType::Response,
                        payload: resp.to_payload(),
                    };
                    self.priority_queue.push_back(frame.encode());
                }
                RequestType::ServoForward => {
                    // Mock: echo back empty servo response
                    let resp = Response::new(RequestType::ServoForward, true, Vec::new());
                    let frame = RawFrame {
                        frame_type: FrameType::Response,
                        payload: resp.to_payload(),
                    };
                    self.priority_queue.push_back(frame.encode());
                }
                RequestType::FirmwareUpdate => {
                    let mut data = Vec::with_capacity(4);
                    if request.data.len() >= 4 {
                        data.extend_from_slice(&request.data[..4]); // offset
                    }
                    let resp = Response::new(RequestType::FirmwareUpdate, true, data);
                    let frame = RawFrame {
                        frame_type: FrameType::Response,
                        payload: resp.to_payload(),
                    };
                    self.priority_queue.push_back(frame.encode());
                    self.push_log(
                        LogLevel::Info,
                        "command.rs",
                        "handle_firmware",
                        "firmware chunk written",
                    );
                }
            }
        }
    }

    /// 生成一条模拟板级日志并入队
    fn push_log(&mut self, level: LogLevel, file_name: &str, fun_name: &str, msg: &str) {
        let log_msg = LogMessage {
            level,
            file_name: file_name.into(),
            fun_name: fun_name.into(),
            msg: msg.into(),
        };
        let frame = RawFrame {
            frame_type: FrameType::Log,
            payload: log_msg.to_bytes(),
        };
        self.rx_queue.push_back(frame.encode());
    }

    // ═══ 配置管理 ═══

    /// 发布当前全量配置帧（模拟 STM32 在配置变更后主动上报）
    fn publish_config(&mut self) {
        let frame = RawFrame {
            frame_type: FrameType::Config,
            payload: self.config.to_bytes(),
        };
        self.rx_queue.push_back(frame.encode());
    }

    fn get_config_value(&self, ct: ConfigType) -> f32 {
        // Return protocol values as f32 for Config::from_type_value
        match ct {
            ConfigType::EnableBatOut1 => self.config.enable_bat_ou1 as u8 as f32,
            ConfigType::EnablePwrBatOut2 => self.config.enable_bat_out2 as u8 as f32,
            ConfigType::EnablePwr5V => self.config.enable_pwr_5v as u8 as f32,
            ConfigType::EnableCharge => self.config.enable_charge as u8 as f32,
            ConfigType::EnableServoPwrMonitor => self.config.enable_servo_pwr_monitor as u8 as f32,
            ConfigType::BatOut1CurrentLimitMa => self.config.bat_out1_current_limit_ma as f32,
            ConfigType::BatOut2CurrentLimitMa => self.config.bat_out2_current_limit_ma as f32,
            ConfigType::Pwr5VOutCurrentLimitMa => self.config.pwr_5v_out_current_limit_ma as f32,
            ConfigType::PwrServoCurrentLimitMa => self.config.servo_current_limit_ma as f32,
            ConfigType::ChargeMinCurrentMa => self.config.charge_min_current_ma as f32,
            ConfigType::ChargeMaxCurrentMa => self.config.charge_max_current_ma as f32,
            ConfigType::PwrServoTempLimit => self.config.pwr_servo_temp_limit as f32,
            ConfigType::Pwr5vTempLimit => self.config.pwr_5v_temp_limit as f32,
            ConfigType::ChargeTempDerating => self.config.charge_temp_derating as f32,
            ConfigType::ChargeTempLimit => self.config.charge_temp_limit as f32,
            ConfigType::ServoBaudRate => self.config.servo_baud_rate as f32,
            ConfigType::ChargeStopSoc => self.config.charge_stop_soc as f32,
            ConfigType::ChargeStopVoltageMv => self.config.charge_stop_voltage_mv as f32,
            ConfigType::TxLogLevel => self.config.tx_log_level as u8 as f32,
            ConfigType::BMSIc => self.config.bms_ic as f32,
            ConfigType::IMUIc => self.config.imu_ic as f32,
        }
    }

    fn update_config(&mut self, config: Config) {
        match config {
            Config::EnableBatOut1(on) => self.config.enable_bat_ou1 = on,
            Config::EnableBatOut2(on) => self.config.enable_bat_out2 = on,
            Config::EnablePwr5V(on) => self.config.enable_pwr_5v = on,
            Config::EnableCharge(on) => self.config.enable_charge = on,
            Config::EnableServoPwrMonitor(on) => self.config.enable_servo_pwr_monitor = on,
            Config::BatOut1CurrentLimitMa(v) => self.config.bat_out1_current_limit_ma = v,
            Config::BatOut2CurrentLimitMa(v) => self.config.bat_out2_current_limit_ma = v,
            Config::Pwr5VOutCurrentLimitMa(v) => self.config.pwr_5v_out_current_limit_ma = v,
            Config::PwrServoCurrentLimitMa(v) => self.config.servo_current_limit_ma = v,
            Config::ChargeMinCurrentMa(v) => self.config.charge_min_current_ma = v,
            Config::ChargeMaxCurrentMa(v) => self.config.charge_max_current_ma = v,
            Config::PwrServoTempLimit(v) => self.config.pwr_servo_temp_limit = v,
            Config::Pwr5vTempLimit(v) => self.config.pwr_5v_temp_limit = v,
            Config::ChargeTempDerating(v) => self.config.charge_temp_derating = v,
            Config::ChargeTempLimit(v) => self.config.charge_temp_limit = v,
            Config::ServoBaudRate(v) => self.config.servo_baud_rate = v,
            Config::ChargeStopSoc(v) => self.config.charge_stop_soc = v,
            Config::ChargeStopVoltageMv(v) => self.config.charge_stop_voltage_mv = v,
            Config::TxLogLevel(level) => self.config.tx_log_level = level,
            Config::BMSIc(v) => self.config.bms_ic = v,
            Config::IMUIc(v) => self.config.imu_ic = v,
        }
    }
}
