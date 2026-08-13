//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! Status snapshot (for TUI/ROS2 node)

use crate::error::DriverError;
use crate::protocol::battery_state::BatteryState;
use crate::protocol::config::BoardConfigSnapshot;
use crate::protocol::event::BoardEvent;
use crate::protocol::imu::ImuData;
use crate::protocol::power::PowerData;
use crate::protocol::system::SystemInfo;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;
use servo_robot_protocol::log::LogMessage;

/// 板级日志条目（带时间戳）
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub ts: u64,
    pub msg: LogMessage,
}

/// 日志缓冲区最大条数
const LOG_BUFFER_SIZE: usize = 100;

/// 线程安全的状态快照，供 TUI 轮询
pub struct DriverState {
    inner: Mutex<StateInner>,
}

struct StateInner {
    pub imu: Option<ImuData>,
    pub power: Option<PowerData>,
    // thermal 已合并到 SystemInfo
    pub battery: Option<BatteryState>,
    pub config: Option<BoardConfigSnapshot>,
    pub event: Option<BoardEvent>,
    pub system: Option<SystemInfo>,
    pub logs: VecDeque<LogEntry>,
    pub last_error: Option<DriverError>,
    pub connected: bool,
    pub frame_count: u64,
    pub frames_parsed: u64,  // 成功解析的帧数
    pub frames_dropped: u64, // 解析失败的帧数
    pub last_frame_time: Option<Instant>,
}

impl DriverState {
    pub fn new() -> Self {
        DriverState {
            inner: Mutex::new(StateInner {
                imu: None,
                power: None,
                battery: None,
                config: None,
                event: None,
                system: None,
                logs: VecDeque::with_capacity(LOG_BUFFER_SIZE),
                last_error: None,
                connected: false,
                frame_count: 0,
                frames_parsed: 0,
                frames_dropped: 0,
                last_frame_time: None,
            }),
        }
    }

    /// 获取所有数据的快照（一次性锁）
    pub fn snapshot(&self) -> StateSnapshot {
        let inner = self.inner.lock().unwrap();
        StateSnapshot {
            imu: inner.imu.clone(),
            power: inner.power.clone(),
            battery: inner.battery.clone(),
            config: inner.config.clone(),
            event: inner.event.clone(),
            system: inner.system.clone(),
            logs: inner.logs.clone(),
            connected: inner.connected,
            frame_count: inner.frame_count,
            frames_parsed: inner.frames_parsed,
            frames_dropped: inner.frames_dropped,
        }
    }

    /// 单独获取某项数据
    pub fn imu(&self) -> Option<ImuData> {
        self.inner.lock().unwrap().imu.clone()
    }

    pub fn power(&self) -> Option<PowerData> {
        self.inner.lock().unwrap().power.clone()
    }

    /// 获取温度数据（从 SystemInfo 中提取）
    pub fn thermal(&self) -> Option<crate::protocol::system::SystemInfo> {
        self.inner.lock().unwrap().system.clone()
    }

    pub fn battery(&self) -> Option<BatteryState> {
        self.inner.lock().unwrap().battery.clone()
    }

    pub fn config(&self) -> Option<BoardConfigSnapshot> {
        self.inner.lock().unwrap().config.clone()
    }

    pub fn event(&self) -> Option<BoardEvent> {
        self.inner.lock().unwrap().event.clone()
    }

    pub fn system(&self) -> Option<SystemInfo> {
        self.inner.lock().unwrap().system.clone()
    }

    /// 获取最新一条日志
    pub fn log(&self) -> Option<LogEntry> {
        self.inner.lock().unwrap().logs.back().cloned()
    }

    /// 获取所有日志（用于快照）
    pub fn logs(&self) -> VecDeque<LogEntry> {
        self.inner.lock().unwrap().logs.clone()
    }

    pub fn last_error(&self) -> Option<DriverError> {
        self.inner.lock().unwrap().last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.inner.lock().unwrap().connected
    }

    pub fn frame_count(&self) -> u64 {
        self.inner.lock().unwrap().frame_count
    }

    pub fn frames_parsed(&self) -> u64 {
        self.inner.lock().unwrap().frames_parsed
    }

    pub fn frames_dropped(&self) -> u64 {
        self.inner.lock().unwrap().frames_dropped
    }

    /// 成功解析帧计数
    pub(crate) fn increment_frames_parsed(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.frames_parsed += 1;
        inner.last_frame_time = Some(Instant::now());
    }

    /// 解析失败帧计数
    pub(crate) fn increment_frames_dropped(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.frames_dropped += 1;
    }

    /// 内部更新（由 Driver 调用）
    pub(crate) fn update_imu(&self, data: ImuData) {
        let mut inner = self.inner.lock().unwrap();
        inner.imu = Some(data);
        inner.frame_count += 1;
    }

    pub(crate) fn update_power(&self, data: PowerData) {
        let mut inner = self.inner.lock().unwrap();
        inner.power = Some(data);
        inner.frame_count += 1;
    }

    pub(crate) fn update_battery(&self, state: BatteryState) {
        let mut inner = self.inner.lock().unwrap();
        inner.battery = Some(state);
        inner.frame_count += 1;
    }

    pub(crate) fn update_config(&self, config: BoardConfigSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.config = Some(config);
        inner.frame_count += 1;
    }

    pub(crate) fn update_event(&self, event: BoardEvent) {
        let mut inner = self.inner.lock().unwrap();
        inner.event = Some(event);
        inner.frame_count += 1;
    }

    pub(crate) fn update_system(&self, info: SystemInfo) {
        let mut inner = self.inner.lock().unwrap();
        inner.system = Some(info);
        inner.frame_count += 1;
    }

    pub(crate) fn update_log(&self, ts: u64, info: LogMessage) {
        let mut inner = self.inner.lock().unwrap();
        if inner.logs.len() >= LOG_BUFFER_SIZE {
            inner.logs.pop_front();
        }
        inner.logs.push_back(LogEntry { ts, msg: info });
    }

    pub(crate) fn set_connected(&self, connected: bool) {
        self.inner.lock().unwrap().connected = connected;
    }

    pub(crate) fn set_error(&self, error: DriverError) {
        self.inner.lock().unwrap().last_error = Some(error);
    }
}

/// 一次性快照，不持有锁
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub imu: Option<ImuData>,
    pub power: Option<PowerData>,
    // thermal 已合并到 SystemInfo
    pub battery: Option<BatteryState>,
    pub config: Option<BoardConfigSnapshot>,
    pub event: Option<BoardEvent>,
    pub system: Option<SystemInfo>,
    pub logs: VecDeque<LogEntry>,
    pub connected: bool,
    pub frame_count: u64,
    pub frames_parsed: u64,
    pub frames_dropped: u64,
}
