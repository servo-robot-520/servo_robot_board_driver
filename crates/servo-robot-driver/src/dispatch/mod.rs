//! 事件分发系统

pub mod callback;

use crate::error::DriverError;
use callback::DriverCallback;
use flume::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 事件类型，内部使用
#[derive(Debug, Clone)]
pub enum DriverEvent {
    // ═══ 上行数据 ═══
    ImuData(crate::protocol::imu::ImuData),
    PowerData(crate::protocol::power::PowerData),
    BatteryState(crate::protocol::battery_state::BatteryState),
    ConfigSnapshot(crate::protocol::config::BoardConfigSnapshot),
    BoardEvent(crate::protocol::event::BoardEvent),
    Diagnostic(crate::protocol::diagnostic::Diagnostic),
    /// 日志事件 (时间戳: Unix 毫秒, 日志内容) 
    Log(u64, crate::protocol::log::LogMessage),

    // ═══ 应答事件 ═══
    Response(crate::protocol::response::Response),

    // ═══ 错误 ═══
    Error(DriverError),
}

/// 事件总线容量
const EVENT_CHANNEL_CAPACITY: usize = 1024;
/// ACK 通道容量(有界:无同步等待者时 ACK 可丢弃,防止长期运行内存增长)
const ACK_CHANNEL_CAPACITY: usize = 64;

/// 事件总线 — 连接传输层和回调系统
///
/// - 主事件通道（bounded）：读线程发送事件，分发线程消费并触发回调；
///   满时读线程用 try_send 丢弃(上行数据可丢,不能反向阻塞读线程)
/// - ACK 通道（bounded）：供同步请求-响应等待；无等待者时 try_send 丢弃
pub struct EventBus {
    /// 事件发送端（bounded，满时丢弃最旧事件）
    tx: Sender<DriverEvent>,
    /// 事件接收端
    rx: Receiver<DriverEvent>,
    /// ACK 事件发送端（用于同步等待）
    ack_tx: Sender<DriverEvent>,
    /// ACK 事件接收端（用于同步等待）
    ack_rx: Receiver<DriverEvent>,
    /// 回调注册表
    callbacks: Arc<Mutex<Vec<Box<dyn DriverCallback>>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, rx) = flume::bounded(EVENT_CHANNEL_CAPACITY);
        let (ack_tx, ack_rx) = flume::bounded(ACK_CHANNEL_CAPACITY);
        EventBus {
            tx,
            rx,
            ack_tx,
            ack_rx,
            callbacks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 注册 trait 回调
    pub fn register_callback(&self, cb: impl DriverCallback) {
        let mut callbacks = self.callbacks.lock().unwrap_or_else(|e| e.into_inner());
        callbacks.push(Box::new(cb));
    }

    /// 获取事件发送端（给读线程用）
    pub fn sender(&self) -> Sender<DriverEvent> {
        self.tx.clone()
    }

    /// 获取 ACK 事件发送端（用于同步等待）
    pub fn ack_sender(&self) -> Sender<DriverEvent> {
        self.ack_tx.clone()
    }

    /// 尝试接收 ACK 事件（非阻塞）
    pub fn try_recv_ack(&self) -> Result<Option<DriverEvent>, DriverError> {
        match self.ack_rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => Err(DriverError::TransportClosed),
        }
    }

    /// 接收 ACK 事件（带超时）
    pub fn recv_ack_timeout(&self, timeout: Duration) -> Result<DriverEvent, DriverError> {
        self.ack_rx.recv_timeout(timeout).map_err(|e| match e {
            flume::RecvTimeoutError::Timeout => DriverError::Timeout,
            flume::RecvTimeoutError::Disconnected => DriverError::TransportClosed,
        })
    }

    /// 接收一个事件（阻塞）
    pub fn recv(&self) -> Result<DriverEvent, DriverError> {
        self.rx.recv().map_err(|_| DriverError::TransportClosed)
    }

    /// 接收一个事件（带超时;超时返回 `DriverError::Timeout`）
    pub fn recv_timeout(&self, timeout: Duration) -> Result<DriverEvent, DriverError> {
        self.rx.recv_timeout(timeout).map_err(|e| match e {
            flume::RecvTimeoutError::Timeout => DriverError::Timeout,
            flume::RecvTimeoutError::Disconnected => DriverError::TransportClosed,
        })
    }

    /// 尝试接收一个事件（非阻塞）
    pub fn try_recv(&self) -> Result<Option<DriverEvent>, DriverError> {
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(flume::TryRecvError::Empty) => Ok(None),
            Err(flume::TryRecvError::Disconnected) => Err(DriverError::TransportClosed),
        }
    }

    /// 分发事件给所有注册的回调
    ///
    /// 单个回调 panic 被 catch_unwind 拦截:回调运行在驱动分发线程上,
    /// 一次 panic 不应杀死整个分发线程(否则驱动静默停止派发)。
    pub fn dispatch(&self, event: &DriverEvent) {
        let mut callbacks = self.callbacks.lock().unwrap_or_else(|e| e.into_inner());
        for cb in callbacks.iter_mut() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match event {
                DriverEvent::ImuData(d) => cb.on_imu_data(d),
                DriverEvent::PowerData(d) => cb.on_power_data(d),
                DriverEvent::BatteryState(d) => cb.on_battery_state(d),
                DriverEvent::ConfigSnapshot(d) => cb.on_config_snapshot(d),
                DriverEvent::BoardEvent(d) => cb.on_board_event(d),
                DriverEvent::Diagnostic(d) => cb.on_diagnostic(d),
                DriverEvent::Log(ts, d) => cb.on_log(*ts, d),
                DriverEvent::Response(r) => cb.on_response(r),
                DriverEvent::Error(e) => cb.on_error(e),
            }));
        }
    }
}
