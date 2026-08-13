//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 21:43

//! Async driver (experimental, not full test)

use crate::dispatch::callback::DriverCallback;
use crate::dispatch::{DriverEvent, EventBus};
use crate::driver_common;
use crate::error::DriverError;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::protocol::servo::ServoCmdWrapper;
use crate::reconnect::ReconnectConfig;
use crate::state::DriverState;
use crate::transport::async_trait::{AsyncTransport, AsyncTransportFactory};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// 默认超时时间
const DEFAULT_TIMEOUT: Duration = Duration::from_millis(1000);

/// 异步驱动结构体
#[cfg(feature = "async")]
pub struct AsyncDriver {
    /// 传输层
    transport: Arc<Mutex<Option<Box<dyn AsyncTransport>>>>,
    /// 传输层工厂（用于重连）
    transport_factory: Option<Arc<dyn AsyncTransportFactory>>,
    /// 重连配置
    reconnect_config: Option<ReconnectConfig>,
    /// 事件总线
    bus: Arc<EventBus>,
    /// 状态快照
    state: Arc<DriverState>,
    /// 读取任务句柄
    read_handle: Option<JoinHandle<()>>,
    /// 分发任务句柄
    dispatch_handle: Option<JoinHandle<()>>,
    /// 运行标志
    running: Arc<AtomicBool>,
}

#[cfg(feature = "async")]
impl AsyncDriver {
    /// 创建新的异步驱动实例
    pub fn new(transport: impl AsyncTransport) -> Self {
        AsyncDriver {
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

    /// 创建支持自动重连的异步驱动实例
    pub fn new_with_reconnect(
        factory: impl AsyncTransportFactory,
        reconnect_config: ReconnectConfig,
    ) -> Self {
        AsyncDriver {
            transport: Arc::new(Mutex::new(None)),
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

    /// 启动异步驱动
    pub async fn start(&mut self) -> Result<(), DriverError> {
        if self.running.load(Ordering::Relaxed) {
            return Err(DriverError::NotRunning);
        }

        // 如果有工厂，创建初始连接
        if let Some(ref factory) = self.transport_factory {
            let transport = factory.create().await?;
            let mut guard = self.transport.lock().await;
            *guard = Some(transport);
        }

        self.running.store(true, Ordering::Relaxed);
        self.state.set_connected(true);

        let transport = self.transport.clone();
        let transport_factory = self.transport_factory.clone();
        let reconnect_config = self.reconnect_config.clone();
        let bus = self.bus.clone();
        let state = self.state.clone();
        let running = self.running.clone();

        // 读取任务：只做 I/O + 状态更新
        let read_handle = tokio::spawn(Self::read_loop(
            transport,
            transport_factory,
            bus.clone(),
            state,
            running.clone(),
            reconnect_config,
        ));

        let bus = self.bus.clone();
        let running = self.running.clone();

        // 分发任务：从通道消费事件，触发回调
        let dispatch_handle = tokio::spawn(async move {
            Self::dispatch_loop(bus, running).await;
        });

        self.read_handle = Some(read_handle);
        self.dispatch_handle = Some(dispatch_handle);
        Ok(())
    }

    /// 停止异步驱动
    pub async fn stop(&mut self) -> Result<(), DriverError> {
        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.read_handle.take() {
            handle.await.map_err(|_| DriverError::LockPoisoned)?;
        }
        if let Some(handle) = self.dispatch_handle.take() {
            handle.await.map_err(|_| DriverError::LockPoisoned)?;
        }

        self.state.set_connected(false);
        Ok(())
    }

    // ═══ 写入/查询（不等待应答）═══

    pub async fn write_config(&self, config: Config) -> Result<(), DriverError> {
        let encoded = driver_common::encode_cfg_write(&config);
        let mut transport = self.transport.lock().await;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded).await?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    pub async fn query_config(&self, config_type: ConfigType) -> Result<(), DriverError> {
        let encoded = driver_common::encode_cfg_query(config_type);
        let mut transport = self.transport.lock().await;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded).await?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    pub async fn query_all_configs(&self) -> Result<(), DriverError> {
        let encoded = driver_common::encode_cfg_query_all();
        let mut transport = self.transport.lock().await;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded).await?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    pub async fn forward_servo(&self, cmd: &ServoCmdWrapper) -> Result<(), DriverError> {
        let encoded = driver_common::encode_servo_forward(cmd);
        let mut transport = self.transport.lock().await;
        match transport.as_mut() {
            Some(t) => t.write_frame(&encoded).await?,
            None => return Err(DriverError::TransportClosed),
        }
        Ok(())
    }

    // ═══ 同步发送（等待应答）═══

    pub async fn query_config_sync(&self, config_type: ConfigType) -> Result<Config, DriverError> {
        self.query_config(config_type).await?;
        self.wait_for_ack_cfg_query(DEFAULT_TIMEOUT).await
    }

    pub async fn query_all_configs_sync(&self) -> Result<BoardConfigSnapshot, DriverError> {
        self.query_all_configs().await?;
        self.wait_for_ack_cfg_query_all(DEFAULT_TIMEOUT).await
    }

    pub async fn write_config_sync(&self, config: Config) -> Result<bool, DriverError> {
        self.write_config(config).await?;
        self.wait_for_ack_cfg_write(DEFAULT_TIMEOUT).await
    }

    pub async fn forward_servo_sync(&self, cmd: &ServoCmdWrapper) -> Result<ServoCmdWrapper, DriverError> {
        self.forward_servo(cmd).await?;
        self.wait_for_ack_servo_cmd(DEFAULT_TIMEOUT).await
    }

    async fn wait_for_ack_cfg_query(&self, timeout: Duration) -> Result<Config, DriverError> {
        match tokio::time::timeout(timeout, self.bus.recv_ack_async()).await {
            Ok(Ok(event)) => {
                if let DriverEvent::AckCfgQuery(config) = event {
                    Ok(config)
                } else {
                    Err(DriverError::Timeout)
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DriverError::Timeout),
        }
    }

    async fn wait_for_ack_cfg_query_all(
        &self,
        timeout: Duration,
    ) -> Result<BoardConfigSnapshot, DriverError> {
        match tokio::time::timeout(timeout, self.bus.recv_ack_async()).await {
            Ok(Ok(event)) => {
                if let DriverEvent::AckCfgQueryAll(config) = event {
                    Ok(config)
                } else {
                    Err(DriverError::Timeout)
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DriverError::Timeout),
        }
    }

    async fn wait_for_ack_cfg_write(&self, timeout: Duration) -> Result<bool, DriverError> {
        match tokio::time::timeout(timeout, self.bus.recv_ack_async()).await {
            Ok(Ok(event)) => {
                if let DriverEvent::AckCfgWrite { success } = event {
                    Ok(success)
                } else {
                    Err(DriverError::Timeout)
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DriverError::Timeout),
        }
    }

    async fn wait_for_ack_servo_cmd(&self, timeout: Duration) -> Result<ServoCmdWrapper, DriverError> {
        match tokio::time::timeout(timeout, self.bus.recv_ack_async()).await {
            Ok(Ok(event)) => {
                if let DriverEvent::AckServoCmd(cmd) = event {
                    Ok(cmd)
                } else {
                    Err(DriverError::Timeout)
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(DriverError::Timeout),
        }
    }

    /// 获取状态快照
    pub fn state(&self) -> Arc<DriverState> {
        self.state.clone()
    }

    /// 分发任务：从事件通道消费事件，触发回调
    async fn dispatch_loop(bus: Arc<EventBus>, running: Arc<AtomicBool>) {
        while running.load(Ordering::Relaxed) {
            // 使用 recv_async + timeout 实现高效的异步等待
            match tokio::time::timeout(Duration::from_millis(100), bus.recv_async()).await {
                Ok(Ok(event)) => {
                    bus.dispatch(&event);
                }
                Ok(Err(_)) => {
                    // 通道断开
                    break;
                }
                Err(_) => {
                    // 超时，继续循环检查 running 标志
                    continue;
                }
            }
        }
        log::info!("Async dispatch loop exited");
    }

    /// 异步读取循环 — 只做 I/O + 状态更新，不触发回调
    async fn read_loop(
        transport: Arc<Mutex<Option<Box<dyn AsyncTransport>>>>,
        transport_factory: Option<Arc<dyn AsyncTransportFactory>>,
        bus: Arc<EventBus>,
        state: Arc<DriverState>,
        running: Arc<AtomicBool>,
        reconnect_config: Option<ReconnectConfig>,
    ) {
        let mut retry_count = 0;

        while running.load(Ordering::Relaxed) {
            // 读取一帧
            let frame_data = {
                let mut transport_guard = transport.lock().await;

                let current_transport = match transport_guard.as_mut() {
                    Some(t) => t,
                    None => {
                        // 传输层不可用，尝试重连
                        drop(transport_guard);
                        if let Some(ref factory) = transport_factory {
                            Self::attempt_reconnect(
                                &transport,
                                factory.as_ref(),
                                &state,
                                reconnect_config.as_ref(),
                                &mut retry_count,
                            )
                            .await;
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

                match current_transport.read_frame().await {
                    Ok(data) => {
                        retry_count = 0;
                        data
                    }
                    Err(DriverError::TransportClosed) => {
                        log::warn!("Async connection lost");
                        state.set_connected(false);
                        *transport_guard = None;

                        if let Some(ref factory) = transport_factory {
                            drop(transport_guard);
                            Self::attempt_reconnect(
                                &transport,
                                factory.as_ref(),
                                &state,
                                reconnect_config.as_ref(),
                                &mut retry_count,
                            )
                            .await;
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
            );

            // 发送到主事件通道
            let _ = bus.sender().send(event.clone());

            // ACK 事件同时发送到 ACK 通道
            if is_ack {
                let _ = bus.ack_sender().send(event);
            }
        }

        log::info!("Async read loop exited");
    }

    /// 尝试重连
    async fn attempt_reconnect(
        transport: &Arc<Mutex<Option<Box<dyn AsyncTransport>>>>,
        factory: &dyn AsyncTransportFactory,
        state: &Arc<DriverState>,
        config: Option<&ReconnectConfig>,
        retry_count: &mut u32,
    ) {
        let config = match config {
            Some(c) => c,
            None => return,
        };

        if *retry_count >= config.max_retries {
            log::error!("Max retries ({}) reached", config.max_retries);
            state.set_error(DriverError::TransportClosed);
            return;
        }

        let delay = config.delay_for_retry(*retry_count);
        log::info!(
            "Reconnecting in {:?} (attempt {}/{})",
            delay,
            *retry_count + 1,
            config.max_retries
        );

        tokio::time::sleep(delay).await;

        match factory.create().await {
            Ok(new_transport) => {
                let mut guard = transport.lock().await;
                *guard = Some(new_transport);
                state.set_connected(true);
                log::info!("Reconnected successfully");
            }
            Err(e) => {
                log::warn!("Reconnect failed: {}", e);
                *retry_count += 1;
            }
        }
    }
}

#[cfg(feature = "async")]
impl Drop for AsyncDriver {
    fn drop(&mut self) {
        // 注意：在 async 上下文中无法直接调用 async stop()
        // 用户应该在 drop 前显式调用 stop().await
        // 设置 running=false 以便 tokio task 自行退出
        self.running.store(false, Ordering::Relaxed);
    }
}
