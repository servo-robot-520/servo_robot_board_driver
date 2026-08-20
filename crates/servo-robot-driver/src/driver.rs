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
use crate::protocol::command::Command;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
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

    // ═══ 写入/查询（不等待应答）═══

    /// 写入配置到 STM32（不等待应答）
    pub fn write_config(&self, config: Config) -> Result<(), DriverError> {
        let encoded = driver_common::encode_cfg_write(&config);
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
        let encoded = driver_common::encode_cfg_query(config_type);
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
        let encoded = driver_common::encode_cfg_query_all();
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
        let encoded = driver_common::encode_servo_forward(cmd);
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
        self.query_config(config_type)?;
        self.wait_for_ack_cfg_query(DEFAULT_TIMEOUT)
    }

    /// 查询所有配置并等待响应
    pub fn query_all_configs_sync(&self) -> Result<BoardConfigSnapshot, DriverError> {
        self.query_all_configs()?;
        self.wait_for_ack_cfg_query_all(DEFAULT_TIMEOUT)
    }

    /// 写入配置并等待确认
    pub fn write_config_sync(&self, config: Config) -> Result<bool, DriverError> {
        self.write_config(config)?;
        self.wait_for_ack_cfg_write(DEFAULT_TIMEOUT)
    }

    /// 转发舵机命令并等待响应
    pub fn forward_servo_sync(
        &self,
        cmd: &ServoCmdWrapper,
    ) -> Result<ServoCmdWrapper, DriverError> {
        self.forward_servo(cmd)?;
        self.wait_for_ack_servo_cmd(DEFAULT_TIMEOUT)
    }

    /// 发送命令（不等待应答）
    pub fn send_command(&self, cmd: &Command) -> Result<(), DriverError> {
        let encoded = driver_common::encode_command(cmd);
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

    /// 发送命令并等待响应
    pub fn send_command_sync(&self, cmd: &Command) -> Result<bool, DriverError> {
        self.send_command(cmd)?;
        self.wait_for_ack_command(DEFAULT_TIMEOUT)
    }

    /// 发送固件更新数据（不等待应答）
    pub fn firmware_update(&self, offset: u32, data: &[u8]) -> Result<(), DriverError> {
        let encoded = driver_common::encode_firmware_update(offset, data);
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
        self.firmware_update(offset, data)?;
        self.wait_for_ack_firmware_update(DEFAULT_TIMEOUT)
    }

    // ═══ 等待应答（使用 recv_timeout，不再 busy-poll）═══

    fn wait_for_ack_cfg_query(&self, timeout: Duration) -> Result<Config, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckCfgQuery(config) => return Ok(config),
                _ => continue,
            }
        }
    }

    fn wait_for_ack_cfg_query_all(
        &self,
        timeout: Duration,
    ) -> Result<BoardConfigSnapshot, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckCfgQueryAll(config) => return Ok(config),
                _ => continue,
            }
        }
    }

    fn wait_for_ack_cfg_write(&self, timeout: Duration) -> Result<bool, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckCfgWrite { success } => return Ok(success),
                _ => continue,
            }
        }
    }

    fn wait_for_ack_servo_cmd(&self, timeout: Duration) -> Result<ServoCmdWrapper, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckServoCmd(cmd) => return Ok(cmd),
                _ => continue,
            }
        }
    }

    fn wait_for_ack_command(&self, timeout: Duration) -> Result<bool, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckCommand { success } => return Ok(success),
                _ => continue,
            }
        }
    }

    fn wait_for_ack_firmware_update(&self, timeout: Duration) -> Result<bool, DriverError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(DriverError::Timeout);
            }
            match self.bus.recv_ack_timeout(remaining)? {
                DriverEvent::AckFirmwareUpdate { success, .. } => return Ok(success),
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
                        continue;
                    }
                    Err(DriverError::TransportClosed) => {
                        // 连接断开
                        log::warn!("Connection lost");
                        state.set_connected(false);
                        *transport_guard = None;

                        // 尝试重连
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
                    Err(e) => {
                        state.set_error(e.clone());
                        let _ = bus.sender().send(DriverEvent::Error(e));
                        continue;
                    }
                }
            };

            // 解码、解析、更新状态
            let event = match driver_common::decode_and_dispatch(&frame_data, &state) {
                Some(event) => event,
                None => continue,
            };

            // 检查是否是 ACK 事件
            let is_ack = matches!(
                event,
                DriverEvent::AckCfgQuery(_)
                    | DriverEvent::AckCfgQueryAll(_)
                    | DriverEvent::AckCfgWrite { .. }
                    | DriverEvent::AckServoCmd(_)
                    | DriverEvent::AckCommand { .. }
                    | DriverEvent::AckFirmwareUpdate { .. }
            );

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
