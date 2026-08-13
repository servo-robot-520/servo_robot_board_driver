//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 12:50

//! Reconnection configuration

use std::time::Duration;

/// 重连配置
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// 最大重试次数（0 表示不重连）
    pub max_retries: u32,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 退避倍数（每次重试后间隔乘以此值）
    pub backoff_multiplier: f32,
    /// 最大重试间隔
    pub max_retry_interval: Duration,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        ReconnectConfig {
            max_retries: 3,
            retry_interval: Duration::from_secs(1),
            backoff_multiplier: 1.5,
            max_retry_interval: Duration::from_secs(10),
        }
    }
}

impl ReconnectConfig {
    /// 创建新的重连配置
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// 设置重试间隔
    pub fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = interval;
        self
    }

    /// 设置退避倍数
    pub fn with_backoff_multiplier(mut self, multiplier: f32) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// 设置最大重试间隔
    pub fn with_max_retry_interval(mut self, interval: Duration) -> Self {
        self.max_retry_interval = interval;
        self
    }

    /// 计算第 n 次重试的等待时间
    pub fn delay_for_retry(&self, retry_count: u32) -> Duration {
        let delay =
            self.retry_interval.as_secs_f32() * self.backoff_multiplier.powi(retry_count as i32);
        let delay = Duration::from_secs_f32(delay);
        delay.min(self.max_retry_interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ReconnectConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_interval, Duration::from_secs(1));
    }

    #[test]
    fn test_delay_calculation() {
        let config = ReconnectConfig::new(5)
            .with_retry_interval(Duration::from_secs(1))
            .with_backoff_multiplier(2.0)
            .with_max_retry_interval(Duration::from_secs(10));

        assert_eq!(config.delay_for_retry(0), Duration::from_secs(1));
        assert_eq!(config.delay_for_retry(1), Duration::from_secs(2));
        assert_eq!(config.delay_for_retry(2), Duration::from_secs(4));
        assert_eq!(config.delay_for_retry(3), Duration::from_secs(8));
        assert_eq!(config.delay_for_retry(4), Duration::from_secs(10)); // capped at max
    }
}
