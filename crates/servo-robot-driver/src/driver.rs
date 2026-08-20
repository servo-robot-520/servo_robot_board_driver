//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:39

//! Driver

use crate::dispatch::callback::DriverCallback;
use crate::dispatch::{DriverEvent, EventBus};
use crate::driver_common;
use crate::error::DriverError;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::protocol::frame::ToPayload;
use crate::protocol::request::{Request, RequestType};
use crate::protocol::response::Response;
use crate::protocol::servo::ServoCmdWrapper;
use crate::reconnect::ReconnectConfig;
use crate::state::DriverState;
use crate::transport::{Transport, TransportFactory};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// 默认超时时间
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

/// 主驱动结构体
pub struct Driver {
    /// 传输层
    transport: Arc<Mutex<Option<Box<dyn Transport>>>>,
    /// 传输层工厂（用于重连）
    transport_factory: Option<Arc<dyn TransportFactory>>,
    /// 重连配置
    reconnect_config: Option<ReconnectConfig>,
    /// 事件总线
    bus: Arc<EventBus>,
    /// 状态快照
    state: Arc<DriverState>,
    /// 读取线程句柄
    read_handle: Option<JoinHandle<()>>,
    /// 分发线程句柄
    dispatch_handle: Option<JoinHandle<()>>,
    /// 运行标志
    running: Arc<AtomicBool>,
    /// 同步请求-响应互斥:保证同一时刻只有一个 `*_sync` 在等待应答,
    /// 避免多线程并发时互相吞掉彼此的 ACK(共享 ACK 通道无请求关联 ID)。
    sync_lock: Mutex<()>,
}

impl Driver {
    /// 创建新驱动实例（不支持自动重连）
    pub fn new(transport: impl Transport) -> Self {
        Driver {
            transport: Arc::new(Mutex::new(Some(Box::new(transport)))),
            transport_factory: None,
            reconnect_config: None,
            bus: Arc::new(EventBus::new()),
            state: Arc::new(DriverState::new()),
            read_handle: None,
            dispatch_handle: None,
            running: Arc::new(AtomicBool::new(false)),
            sync_lock: Mutex::new(()),
        }
    }

    /// 创建支持自动重连的驱动实例
    ///
    /// # Arguments
    /// * `factory` - 传输层工厂，每次重连时调用
    /// * `reconnect_config` - 重连配置
    pub fn new_with_reconnect(
        factory: impl TransportFactory,
        reconnect_config: ReconnectConfig,
    ) -> Self {
        // 创建初始连接
        let initial_transport = factory.create();

        Driver {
            transport: Arc::new(Mutex::new(initial_transport.ok())),
            transport_factory: Some(Arc::new(factory)),
            reconnect_config: Some(reconnect_config),
            bus: Arc::new(EventBus::new()),
            state: Arc::new(DriverState::new()),
            read_handle: None,
            dispatch_handle: None,
            running: Arc::new(AtomicBool::new(false)),
            sync_lock: Mutex::new(()),
        }
    }

    /// 注册 trait 回调
    pub fn register_callback(&self, cb: impl DriverCallback) {
        self.bus.register_callback(cb);
    }

    /// 启动驱动（开启读取线程 + 分发线程）
    pub fn start(&mut self) -> Result<(), DriverError> {
        if self.running.load(Ordering::Relaxed) {
            return Err(DriverError::AlreadyStarted);
        }

        // 检查是否有可用的传输层
        {
            let transport = self
                .transport
                .lock()
                .map_err(|_| DriverError::LockPoisoned)?;
            if transport.is_none() && self.transport_factory.is_none() {
                return Err(DriverError::TransportClosed);
            }
        }

        self.running.store(true, Ordering::Relaxed);
        self.state.set_connected(true);

        let transport = Arc::clone(&self.transport);
        let transport_factory = self.transport_factory.clone();
        let reconnect_config = self.reconnect_config.clone();
        let bus = Arc::clone(&self.bus);
        let state = Arc::clone(&self.state);
        let running = Arc::clone(&self.running);

        // 读取线程：只做 I/O + 状态更新 + 发送事件到通道
        let read_handle = std::thread::spawn(move || {
            Self::read_loop(
                transport,
                transport_factory,
                bus,
                state,
                running,
                reconnect_config,
            );
        });

        let bus = Arc::clone(&self.bus);
        let running = Arc::clone(&self.running);

        // 分发线程：从通道消费事件，触发回调
        let dispatch_handle = std::thread::spawn(move || {
            Self::dispatch_loop(bus, running);
        });

        self.read_handle = Some(read_handle);
        self.dispatch_handle = Some(dispatch_handle);
        Ok(())
    }

    /// 停止驱动
    pub fn stop(&mut self) -> Result<(), DriverError> {
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.read_handle.take() {
            handle.join().map_err(|_| DriverError::LockPoisoned)?;
        }
        if let Some(handle) = self.dispatch_handle.take() {
            handle.join().map_err(|_| DriverError::LockPoisoned)?;
        }

        self.state.set_connected(false);
        Ok(())
    }

    /// 重新连接到指定串口
    ///
    /// 先打开新串口,成功后再停旧连接、替换传输层、重新启动。
    /// 打开新串口失败时驱动保持原状(连接与运行状态都不变),
    /// 适用于上层自行实现重连逻辑的场景(如 FFI 调用方)。
    ///
    /// # Arguments
    /// * `port` - 串口设备路径
    /// * `baud_rate` - 波特率
    pub fn connect(&mut self, port: &str, baud_rate: u32) -> Result<(), DriverError> {
        let was_running = self.running.load(Ordering::Relaxed);

        // 先尝试打开新连接:失败直接返回,驱动状态不变
        let transport = crate::transport::serial::SerialTransport::open(port, baud_rate)
            .map_err(|e| DriverError::Serial(e.to_string()))?;

        // 打开成功后才停旧连接
        if was_running {
            self.stop()?;
        }

        // 替换传输层
        {
            let mut guard = self
                .transport
                .lock()
                .map_err(|_| DriverError::LockPoisoned)?;
            *guard = Some(Box::new(transport));
        }

        // 重新启动
        if was_running {
            self.start()?;
        }

        Ok(())
    }

    // ═══ 写入/查询（不等待应答）═══

    /// 写入配置到 STM32（不等待应答）
    pub fn write_config(&self, config: Config) -> Result<(), DriverError> {
        let request = Request::new(RequestType::ConfigWrite, config.to_bytes());
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 查询单个配置（不等待应答）
    pub fn query_config(&self, config_type: ConfigType) -> Result<(), DriverError> {
        let request = Request::new(RequestType::ConfigQuery, vec![config_type as u8]);
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 查询所有配置（不等待应答）
    pub fn query_all_configs(&self) -> Result<(), DriverError> {
        let request = Request::simple(RequestType::ConfigQueryAll);
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 查询设备信息（不等待应答）
    pub fn query_device_info(&self) -> Result<(), DriverError> {
        let request = Request::simple(RequestType::DeviceInfo);
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 转发舵机命令（不等待应答）
    pub fn forward_servo(&self, cmd: &ServoCmdWrapper) -> Result<(), DriverError> {
        let request = Request::new(RequestType::ServoForward, cmd.to_payload());
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    // ═══ 同步发送（等待应答）═══

    /// 查询单个配置并等待响应
    pub fn query_config_sync(&self, config_type: ConfigType) -> Result<Config, DriverError> {
        let _guard = self.begin_sync()?;
        self.query_config(config_type)?;
        let resp = self.wait_for_response(RequestType::ConfigQuery, DEFAULT_TIMEOUT)?;
        Config::from_bytes(&resp.data).map_err(|_| {
            DriverError::Frame(crate::error::FrameError::PayloadDecode("Config decode"))
        })
    }

    /// 查询所有配置并等待响应
    pub fn query_all_configs_sync(&self) -> Result<BoardConfigSnapshot, DriverError> {
        let _guard = self.begin_sync()?;
        self.query_all_configs()?;
        let resp = self.wait_for_response(RequestType::ConfigQueryAll, DEFAULT_TIMEOUT)?;
        BoardConfigSnapshot::from_bytes(&resp.data).map_err(|_| {
            DriverError::Frame(crate::error::FrameError::PayloadDecode(
                "ConfigSnapshot decode",
            ))
        })
    }

    /// 查询设备信息并等待响应
    pub fn query_device_info_sync(
        &self,
    ) -> Result<crate::protocol::device_info::DeviceInfo, DriverError> {
        let _guard = self.begin_sync()?;
        self.query_device_info()?;
        let resp = self.wait_for_response(RequestType::DeviceInfo, DEFAULT_TIMEOUT)?;
        crate::protocol::device_info::DeviceInfo::from_bytes(&resp.data).map_err(|_| {
            DriverError::Frame(crate::error::FrameError::PayloadDecode("DeviceInfo decode"))
        })
    }

    /// 写入配置并等待确认
    pub fn write_config_sync(&self, config: Config) -> Result<bool, DriverError> {
        let _guard = self.begin_sync()?;
        self.write_config(config)?;
        self.wait_for_response(RequestType::ConfigWrite, DEFAULT_TIMEOUT)
            .map(|r| r.success)
    }

    /// 转发舵机命令并等待响应
    pub fn forward_servo_sync(
        &self,
        cmd: &ServoCmdWrapper,
    ) -> Result<ServoCmdWrapper, DriverError> {
        let _guard = self.begin_sync()?;
        self.forward_servo(cmd)?;
        let resp = self.wait_for_response(RequestType::ServoForward, DEFAULT_TIMEOUT)?;
        Ok(ServoCmdWrapper::new(resp.data))
    }

    /// 发送系统控制命令（Reset/Shutdown/Ota，不等待应答）
    pub fn send_command(&self, kind: RequestType) -> Result<(), DriverError> {
        let request = Request::simple(kind);
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 发送系统控制命令
    ///
    /// 对于 fire-and-forget 命令（Reset/Shutdown/Ota），发送后立即返回 Ok(true)。
    /// 对于需要应答的命令，等待 Response 并返回 success 状态。
    pub fn send_command_sync(&self, kind: RequestType) -> Result<bool, DriverError> {
        let _guard = self.begin_sync()?;
        self.send_command(kind)?;
        if kind.expects_response() {
            self.wait_for_response(kind, DEFAULT_TIMEOUT)
                .map(|r| r.success)
        } else {
            Ok(true)
        }
    }

    /// 发送固件更新数据（不等待应答）
    pub fn firmware_update(&self, offset: u32, data: &[u8]) -> Result<(), DriverError> {
        let mut payload = Vec::with_capacity(4 + data.len());
        payload.extend_from_slice(&offset.to_le_bytes());
        payload.extend_from_slice(data);
        let request = Request::new(RequestType::FirmwareUpdate, payload);
        let encoded = driver_common::encode_request(&request)?;
        let mut transport = self
            .transport
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded)?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    /// 发送固件更新数据并等待响应
    pub fn firmware_update_sync(&self, offset: u32, data: &[u8]) -> Result<bool, DriverError> {
        let _guard = self.begin_sync()?;
        self.firmware_update(offset, data)?;
        self.wait_for_response(RequestType::FirmwareUpdate, DEFAULT_TIMEOUT)
            .map(|r| r.success)
    }

    // ═══ 等待应答 ═══

    /// 同步操作入口:串行化 + 排空陈旧 ACK。
    ///
    /// 持有 `sync_lock` 直到等待结束,保证多线程 `*_sync` 不会互相吞应答。
    /// 排空 ACK 通道中所有无人等待的遗留应答(先前超时的迟到 ACK、
    /// fire-and-forget 发送触发的应答),否则下一次同类型同步等待会
    /// 立即消费陈旧 ACK,返回上一条请求的结果。
    fn begin_sync(&self) -> Result<std::sync::MutexGuard<'_, ()>, DriverError> {
        let guard = self
            .sync_lock
            .lock()
            .map_err(|_| DriverError::LockPoisoned)?;
        while let Ok(Some(_)) = self.bus.try_recv_ack() {}
        Ok(guard)
    }

    /// 等待指定 RequestType 的 Response
    fn wait_for_response(
        &self,
        kind: RequestType,
        timeout: Duration,
    ) -> Result<Response, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::Response(resp) if resp.request_type == kind => return Ok(resp),
                _ => continue,
            }
        }
    }

    /// 获取状态快照（给 TUI 用）
    pub fn state(&self) -> Arc<DriverState> {
        Arc::clone(&self.state)
    }

    /// 分发线程：从事件通道消费事件，触发所有注册的回调
    fn dispatch_loop(bus: Arc<EventBus>, running: Arc<AtomicBool>) {
        while running.load(Ordering::Relaxed) {
            // 阻塞接收,超时 100ms 以便检查 running 标志
            match bus.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    bus.dispatch(&event);
                }
                Err(DriverError::Timeout) => {
                    // 通道空,继续循环检查 running 标志
                }
                Err(_) => {
                    // 通道已断开
                    break;
                }
            }
        }
        log::info!("Dispatch loop exited");
    }

    /// 内部读取循环 — 只做 I/O + 状态更新，不触发回调
    fn read_loop(
        transport: Arc<Mutex<Option<Box<dyn Transport>>>>,
        transport_factory: Option<Arc<dyn TransportFactory>>,
        bus: Arc<EventBus>,
        state: Arc<DriverState>,
        running: Arc<AtomicBool>,
        reconnect_config: Option<ReconnectConfig>,
    ) {
        let mut retry_count = 0;

        while running.load(Ordering::Relaxed) {
            // 读取一帧
            let frame_data = {
                let mut transport_guard = match transport.lock() {
                    Ok(t) => t,
                    Err(_) => {
                        state.set_error(DriverError::LockPoisoned);
                        break;
                    }
                };

                let current_transport = match transport_guard.as_mut() {
                    Some(t) => t,
                    None => {
                        // 传输层不可用，尝试重连
                        state.set_connected(false);
                        drop(transport_guard);
                        if let Some(ref factory) = transport_factory {
                            if !Self::attempt_reconnect(
                                &transport,
                                factory.as_ref(),
                                &state,
                                reconnect_config.as_ref(),
                                &mut retry_count,
                            ) {
                                let _ = bus
                                    .sender()
                                    .send(DriverEvent::Error(DriverError::TransportClosed));
                                break;
                            }
                        } else {
                            state.set_error(DriverError::TransportClosed);
                            let _ = bus
                                .sender()
                                .send(DriverEvent::Error(DriverError::TransportClosed));
                            break;
                        }
                        continue;
                    }
                };

                match current_transport.read_frame() {
                    Ok(data) => {
                        retry_count = 0; // 成功读取，重置重试计数
                        data
                    }
                    Err(DriverError::IoTimeout) => {
                        // 空闲超时:回到循环头检查 running,stop() 才能 join 退出
                        continue;
                    }
                    Err(e) => {
                        // 除超时外,所有读错误(TransportClosed / Io / 其他)一律视为连接断开:
                        // 清空传输层并重连/退出。若只 continue,持续错误会在无 sleep 的热循环里
                        // 空转(占用 CPU + 刷错误事件),且无工厂时永远无法退出。
                        state.set_error(e.clone());
                        let _ = bus.sender().send(DriverEvent::Error(e));
                        log::warn!("Read error, treating as connection loss");
                        state.set_connected(false);
                        *transport_guard = None;

                        if let Some(ref factory) = transport_factory {
                            drop(transport_guard);
                            if !Self::attempt_reconnect(
                                &transport,
                                factory.as_ref(),
                                &state,
                                reconnect_config.as_ref(),
                                &mut retry_count,
                            ) {
                                let _ = bus
                                    .sender()
                                    .send(DriverEvent::Error(DriverError::TransportClosed));
                                break;
                            }
                        } else {
                            let _ = bus
                                .sender()
                                .send(DriverEvent::Error(DriverError::TransportClosed));
                            break;
                        }
                        continue;
                    }
                }
            };

            // 解码、解析、更新状态
            let event = match driver_common::decode_and_dispatch(&frame_data, &state) {
                Some(event) => event,
                None => continue,
            };

            // 检查是否是应答事件
            let is_ack = matches!(event, DriverEvent::Response(_));

            // 发送到主事件通道（bounded，满时丢弃——try_send 不阻塞读线程，
            // 否则慢分发会反向卡死串口读取导致线速下 MCU 侧缓冲溢出）
            if bus.sender().try_send(event.clone()).is_err() {
                log::debug!("Event channel full, event dropped");
            }

            // ACK 事件同时发送到 ACK 通道（bounded，无等待者时丢弃）
            if is_ack {
                let _ = bus.ack_sender().try_send(event);
            }
        }

        log::info!("Read loop exited");
    }

    /// 尝试重连;返回 `false` 表示重试耗尽,调用方应退出读循环(避免空转)
    fn attempt_reconnect(
        transport: &Arc<Mutex<Option<Box<dyn Transport>>>>,
        factory: &dyn TransportFactory,
        state: &Arc<DriverState>,
        config: Option<&ReconnectConfig>,
        retry_count: &mut u32,
    ) -> bool {
        let config = match config {
            Some(c) => c,
            None => return false,
        };

        if *retry_count >= config.max_retries {
            log::error!("Max retries ({}) reached", config.max_retries);
            state.set_error(DriverError::TransportClosed);
            return false;
        }

        let delay = config.delay_for_retry(*retry_count);
        log::info!(
            "Reconnecting in {:?} (attempt {}/{})",
            delay,
            *retry_count + 1,
            config.max_retries
        );

        std::thread::sleep(delay);

        match factory.create() {
            Ok(new_transport) => match transport.lock() {
                Ok(mut guard) => {
                    *guard = Some(new_transport);
                    state.set_connected(true);
                    log::info!("Reconnected successfully");
                }
                Err(e) => {
                    log::error!("Failed to acquire transport lock: {}", e);
                }
            },
            Err(e) => {
                log::warn!("Reconnect failed: {}", e);
                *retry_count += 1;
            }
        }
        true
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use super::*;
    use crate::transport::MockTransport;
    use crate::protocol::response::Response;

    /// 同步操作入口必须排空遗留 ACK(超时迟到 / fire-and-forget 应答),
    /// 否则下一次同类型同步等待会立即消费陈旧应答、返回上一条请求的结果。
    #[test]
    fn test_begin_sync_drains_stale_acks() {
        let driver = Driver::new(MockTransport::new());
        for _ in 0..3 {
            driver
                .bus
                .ack_sender()
                .send(DriverEvent::Response(Response::simple(
                    RequestType::ConfigWrite,
                    true,
                )))
                .unwrap();
        }
        let _guard = driver.begin_sync().unwrap();
        assert!(
            matches!(driver.bus.try_recv_ack(), Ok(None)),
            "stale ACKs must be drained before sync wait"
        );
    }

    /// 空闲传输层(只返回 IoTimeout)下 stop() 必须能 join 退出,不能挂死。
    /// 曾因串口帧头扫描循环吞掉超时,读线程永不返回 → join 永久阻塞。
    #[test]
    fn test_stop_with_idle_transport() {
        struct IdleTransport;
        impl Transport for IdleTransport {
            fn read_frame(&mut self) -> Result<Vec<u8>, DriverError> {
                std::thread::sleep(Duration::from_millis(20));
                Err(DriverError::IoTimeout)
            }
            fn write_frame(&mut self, _frame: &[u8]) -> Result<(), DriverError> {
                Ok(())
            }
            fn close(&mut self) -> Result<(), DriverError> {
                Ok(())
            }
        }

        let mut driver = Driver::new(IdleTransport);
        driver.start().unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(driver.stop());
        });
        rx.recv_timeout(Duration::from_secs(2))
            .expect("stop() must return within 2s on idle transport")
            .unwrap();
        handle.join().unwrap();
    }

    /// 无应答的传输层(只回 IoTimeout):同步查询必须超时,不能无限等待
    #[test]
    fn test_sync_timeout_when_no_ack() {
        struct NoAckTransport;
        impl Transport for NoAckTransport {
            fn read_frame(&mut self) -> Result<Vec<u8>, DriverError> {
                std::thread::sleep(Duration::from_millis(10));
                Err(DriverError::IoTimeout)
            }
            fn write_frame(&mut self, _frame: &[u8]) -> Result<(), DriverError> {
                Ok(())
            }
            fn close(&mut self) -> Result<(), DriverError> {
                Ok(())
            }
        }

        let mut driver = Driver::new(NoAckTransport);
        driver.start().unwrap();
        let start = std::time::Instant::now();
        let r = driver.query_config_sync(ConfigType::SwitchServoPower);
        assert!(matches!(r, Err(DriverError::Timeout)));
        // 等待上限 1s:确认按超时返回而非挂死(上限放宽以容忍并行测试负载)
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "sync wait must time out, not hang"
        );
        driver.stop().unwrap();
    }

    /// payload 超协议上限(255B)的请求必须被拒绝,不能生成 LEN 截断的损坏帧。
    #[test]
    fn test_encode_request_payload_limit() {
        // 254B data + 1B kind = 255 = 上限,允许
        let ok = Request::new(RequestType::ServoForward, vec![0u8; 254]);
        assert!(driver_common::encode_request(&ok).is_ok());
        // 255B data + 1B kind = 256 > 上限,拒绝
        let too_big = Request::new(RequestType::ServoForward, vec![0u8; 255]);
        assert!(matches!(
            driver_common::encode_request(&too_big),
            Err(DriverError::PayloadTooLarge { max: 255, .. })
        ));
    }
}
