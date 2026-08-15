//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/8/15

//! AsyncDriver — 同步 `Driver` 的薄异步门面
//!
//! 设计:不复制读循环/重连/等待逻辑。`AsyncDriver` 持 `Arc<Mutex<Driver>>`,
//! 每个操作通过 `tokio::task::spawn_blocking` 在阻塞线程池上短暂持锁调用
//! 同步 `Driver`,从而让 tokio 调用方可以 `.await` 而不占住 runtime worker 线程。
//! 读线程/分发线程/回调/重连语义全部由同步 `Driver` 唯一提供。
//!
//! 注意:调用方仍需负责在应用退出前 `stop().await`(或 drop 后由 Driver 自行回收)。

use crate::dispatch::callback::DriverCallback;
use crate::driver::Driver;
use crate::error::DriverError;
use crate::protocol::command::Command;
use crate::protocol::config::{BoardConfigSnapshot, Config, ConfigType};
use crate::protocol::servo::ServoCmdWrapper;
use crate::reconnect::ReconnectConfig;
use crate::state::DriverState;
use crate::transport::{Transport, TransportFactory};
use std::sync::{Arc, Mutex};

/// 异步门面 — 可 Clone 共享,方法均可在 tokio 任务中 await
#[derive(Clone)]
pub struct AsyncDriver {
    inner: Arc<Mutex<Driver>>,
}

impl AsyncDriver {
    /// 创建新驱动实例(不支持自动重连)
    pub fn new(transport: impl Transport + 'static) -> Self {
        AsyncDriver {
            inner: Arc::new(Mutex::new(Driver::new(transport))),
        }
    }

    /// 创建支持自动重连的驱动实例
    pub fn new_with_reconnect(
        factory: impl TransportFactory,
        reconnect_config: ReconnectConfig,
    ) -> Self {
        AsyncDriver {
            inner: Arc::new(Mutex::new(Driver::new_with_reconnect(
                factory,
                reconnect_config,
            ))),
        }
    }

    /// 注册 trait 回调(在驱动分发线程上触发)
    pub fn register_callback(&self, cb: impl DriverCallback + 'static) {
        if let Ok(guard) = self.inner.lock() {
            guard.register_callback(cb);
        }
    }

    /// 在阻塞线程池上执行一次同步 Driver 调用
    async fn call<T, F>(&self, f: F) -> Result<T, DriverError>
    where
        T: Send + 'static,
        F: FnOnce(&mut Driver) -> Result<T, DriverError> + Send + 'static,
    {
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = inner.lock().map_err(|_| DriverError::LockPoisoned)?;
            f(&mut guard)
        })
        .await
        .map_err(|e| DriverError::Io(e.to_string()))?
    }

    /// 启动驱动(开启读线程 + 分发线程)
    pub async fn start(&self) -> Result<(), DriverError> {
        self.call(|d| d.start()).await
    }

    /// 停止驱动(join 读/分发线程)
    pub async fn stop(&self) -> Result<(), DriverError> {
        self.call(|d| d.stop()).await
    }

    /// 写入配置到 STM32(不等待应答)
    pub async fn write_config(&self, config: Config) -> Result<(), DriverError> {
        self.call(move |d| d.write_config(config)).await
    }

    /// 查询单个配置(不等待应答)
    pub async fn query_config(&self, config_type: ConfigType) -> Result<(), DriverError> {
        self.call(move |d| d.query_config(config_type)).await
    }

    /// 查询所有配置(不等待应答)
    pub async fn query_all_configs(&self) -> Result<(), DriverError> {
        self.call(|d| d.query_all_configs()).await
    }

    /// 转发舵机命令(不等待应答)
    pub async fn forward_servo(&self, cmd: ServoCmdWrapper) -> Result<(), DriverError> {
        self.call(move |d| d.forward_servo(&cmd)).await
    }

    /// 查询单个配置并等待响应(阻塞 ≤1s,在阻塞池执行)
    pub async fn query_config_sync(&self, config_type: ConfigType) -> Result<Config, DriverError> {
        self.call(move |d| d.query_config_sync(config_type)).await
    }

    /// 查询所有配置并等待响应
    pub async fn query_all_configs_sync(
        &self,
    ) -> Result<BoardConfigSnapshot, DriverError> {
        self.call(|d| d.query_all_configs_sync()).await
    }

    /// 写入配置并等待确认
    pub async fn write_config_sync(&self, config: Config) -> Result<bool, DriverError> {
        self.call(move |d| d.write_config_sync(config)).await
    }

    /// 转发舵机命令并等待响应
    pub async fn forward_servo_sync(
        &self,
        cmd: ServoCmdWrapper,
    ) -> Result<ServoCmdWrapper, DriverError> {
        self.call(move |d| d.forward_servo_sync(&cmd)).await
    }

    /// 发送板级命令(不等待应答)
    pub async fn send_command(&self, cmd: Command) -> Result<(), DriverError> {
        self.call(move |d| d.send_command(&cmd)).await
    }

    /// 发送板级命令并等待响应
    pub async fn send_command_sync(&self, cmd: Command) -> Result<bool, DriverError> {
        self.call(move |d| d.send_command_sync(&cmd)).await
    }

    /// 发送固件更新数据(不等待应答)
    pub async fn firmware_update(&self, offset: u32, data: Vec<u8>) -> Result<(), DriverError> {
        self.call(move |d| d.firmware_update(offset, &data)).await
    }

    /// 发送固件更新数据并等待响应
    pub async fn firmware_update_sync(
        &self,
        offset: u32,
        data: Vec<u8>,
    ) -> Result<bool, DriverError> {
        self.call(move |d| d.firmware_update_sync(offset, &data))
            .await
    }

    /// 获取状态快照(不需要 await)
    pub fn state(&self) -> Arc<DriverState> {
        self.inner.lock().map(|g| g.state()).unwrap_or_else(|_| {
            log::warn!("AsyncDriver state lock poisoned, returning empty state");
            // 锁中毒:返回一个空状态,避免 panic 跨出门面
            Arc::new(DriverState::new())
        })
    }
}
