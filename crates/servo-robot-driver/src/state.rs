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
use crate::protocol::device_info::DeviceInfo;
use crate::protocol::diagnostic::Diagnostic;
use crate::protocol::event::BoardEvent;
use crate::protocol::imu::ImuData;
use crate::protocol::power::PowerData;
use servo_robot_protocol::log::LogMessage;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

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
    // thermal 已合并到 Diagnostic
    pub battery: Option<BatteryState>,
    pub config: Option<BoardConfigSnapshot>,
    pub event: Option<BoardEvent>,
    pub device_info: Option<DeviceInfo>,
    pub diagnostic: Option<Diagnostic>,
    pub logs: VecDeque<LogEntry>,
    pub last_error: Option<DriverError>,
    pub connected: bool,
    pub frame_count: u64,
    pub frames_parsed: u64,  // 成功解析的帧数
    pub frames_dropped: u64, // 解析失败的帧数
    pub last_frame_time: Option<Instant>,
}

impl Default for DriverState {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverState {
    /// 获取内部状态;锁中毒时恢复(读取方不应因其他线程的一次 panic 连带 panic)
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, StateInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn new() -> Self {
        DriverState {
            inner: Mutex::new(StateInner {
                imu: None,
                power: None,
                battery: None,
                config: None,
                event: None,
                device_info: None,
                diagnostic: None,
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
            device_info: inner.device_info.clone(),
            diagnostic: inner.diagnostic.clone(),
            logs: inner.logs.clone(),
            connected: inner.connected,
            frame_count: inner.frame_count,
            frames_parsed: inner.frames_parsed,
            frames_dropped: inner.frames_dropped,
        }
    }

    /// 单独获取某项数据
    pub fn imu(&self) -> Option<ImuData> {
        self.lock_inner().imu.clone()
    }

    pub fn power(&self) -> Option<PowerData> {
        self.lock_inner().power.clone()
    }

    /// 获取诊断数据快照（运行时 CPU/内存/温度等）
    pub fn diagnostic(&self) -> Option<Diagnostic> {
        self.lock_inner().diagnostic.clone()
    }

    pub fn battery(&self) -> Option<BatteryState> {
        self.lock_inner().battery.clone()
    }

    pub fn config(&self) -> Option<BoardConfigSnapshot> {
        self.lock_inner().config.clone()
    }

    pub fn event(&self) -> Option<BoardEvent> {
        self.lock_inner().event.clone()
    }

    pub fn device_info(&self) -> Option<DeviceInfo> {
        self.lock_inner().device_info.clone()
    }

    /// 获取最新一条日志
    pub fn log(&self) -> Option<LogEntry> {
        self.lock_inner().logs.back().cloned()
    }

    /// 获取所有日志（用于快照）
    pub fn logs(&self) -> VecDeque<LogEntry> {
        self.lock_inner().logs.clone()
    }

    pub fn last_error(&self) -> Option<DriverError> {
        self.lock_inner().last_error.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.lock_inner().connected
    }

    pub fn frame_count(&self) -> u64 {
        self.lock_inner().frame_count
    }

    pub fn frames_parsed(&self) -> u64 {
        self.lock_inner().frames_parsed
    }

    pub fn frames_dropped(&self) -> u64 {
        self.lock_inner().frames_dropped
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

    pub(crate) fn update_device_info(&self, info: DeviceInfo) {
        let mut inner = self.inner.lock().unwrap();
        inner.device_info = Some(info);
        inner.frame_count += 1;
    }

    pub(crate) fn update_diagnostic(&self, diag: Diagnostic) {
        let mut inner = self.inner.lock().unwrap();
        inner.diagnostic = Some(diag);
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
        self.lock_inner().connected = connected;
    }

    pub(crate) fn set_error(&self, error: DriverError) {
        self.lock_inner().last_error = Some(error);
    }
}

/// 一次性快照，不持有锁
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    pub imu: Option<ImuData>,
    pub power: Option<PowerData>,
    // thermal 已合并到 Diagnostic
    pub battery: Option<BatteryState>,
    pub config: Option<BoardConfigSnapshot>,
    pub event: Option<BoardEvent>,
    pub device_info: Option<DeviceInfo>,
    pub diagnostic: Option<Diagnostic>,
    pub logs: VecDeque<LogEntry>,
    pub connected: bool,
    pub frame_count: u64,
    pub frames_parsed: u64,
    pub frames_dropped: u64,
}
