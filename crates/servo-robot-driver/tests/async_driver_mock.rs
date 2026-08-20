//! AsyncDriver 集成测试 — 薄门面(同步 MockTransport + spawn_blocking 包装)

#![cfg(all(feature = "mock", feature = "async"))]

use servo_robot_driver::protocol::config::Config;
use servo_robot_driver::protocol::request::RequestKind;
use servo_robot_driver::protocol::servo::ServoCmdWrapper;
use servo_robot_driver::{AsyncDriver, DriverCallback, MockTransport};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// 计数回调:统计 IMU / Power 回调次数(计数 Arc 共享,克隆体计数同一组)
#[derive(Clone, Default)]
struct CountingCallback {
    imu_count: Arc<AtomicU64>,
    power_count: Arc<AtomicU64>,
}

impl CountingCallback {
    fn new() -> (Self, Arc<Self>) {
        let cb = Arc::new(CountingCallback {
            imu_count: Arc::new(AtomicU64::new(0)),
            power_count: Arc::new(AtomicU64::new(0)),
        });
        ((*cb).clone(), cb)
    }
}

impl DriverCallback for CountingCallback {
    fn on_imu_data(&mut self, _data: &servo_robot_driver::protocol::imu::ImuData) {
        self.imu_count.fetch_add(1, Ordering::Relaxed);
    }
    fn on_power_data(&mut self, _data: &servo_robot_driver::protocol::power::PowerData) {
        self.power_count.fetch_add(1, Ordering::Relaxed);
    }
}

fn mock_driver() -> AsyncDriver {
    AsyncDriver::new(MockTransport::new())
}

#[tokio::test]
async fn test_start_stop() {
    let driver = mock_driver();
    assert!(!driver.state().snapshot().connected);

    driver.start().await.expect("start should succeed");
    assert!(driver.state().snapshot().connected);

    driver.stop().await.expect("stop should succeed");
    assert!(!driver.state().snapshot().connected);
}

#[tokio::test]
async fn test_query_all_configs_sync() {
    let driver = mock_driver();
    driver.start().await.unwrap();

    let cfg = driver
        .query_all_configs_sync()
        .await
        .expect("query all configs should succeed");
    // mock 默认配置
    assert_eq!(cfg.servo_baud_rate, 115200);
    assert!(cfg.power_servo_on);

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_write_config_sync_roundtrip() {
    let driver = mock_driver();
    driver.start().await.unwrap();

    let ok = driver
        .write_config_sync(Config::ServoBaudRate(1000000))
        .await
        .expect("write config should not error");
    assert!(ok, "mock should ACK cfg write");

    let cfg = driver.query_all_configs_sync().await.unwrap();
    assert_eq!(cfg.servo_baud_rate, 1000000);

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_callback_fires() {
    let driver = mock_driver();
    let (cb, stats) = CountingCallback::new();
    driver.register_callback(cb);
    driver.start().await.unwrap();

    // mock 每 10ms 发 IMU 帧,50ms 发 Power 帧;轮询等待分发线程触发
    let mut saw_imu = false;
    let mut saw_power = false;
    for _ in 0..50 {
        if stats.imu_count.load(Ordering::Relaxed) > 0 {
            saw_imu = true;
        }
        if stats.power_count.load(Ordering::Relaxed) > 0 {
            saw_power = true;
        }
        if saw_imu && saw_power {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(saw_imu, "IMU callback should fire from dispatch thread");
    assert!(saw_power, "Power callback should fire from dispatch thread");

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_send_command_sync_success() {
    let driver = mock_driver();
    driver.start().await.unwrap();

    // mock 对 Request 帧回 Response(Reset, success=true)
    let ok = driver
        .send_command_sync(RequestKind::Reset)
        .await
        .expect("mock ACKs command frames");
    assert!(ok);

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_queries() {
    let driver = mock_driver();
    driver.start().await.unwrap();

    // 并发同步查询:内部锁串行化,不应 panic 或死锁
    let (a, b) = tokio::join!(
        driver.query_all_configs_sync(),
        driver.query_all_configs_sync()
    );
    let a = a.expect("query a");
    let b = b.expect("query b");
    assert_eq!(a.servo_baud_rate, b.servo_baud_rate);

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_forward_servo_fire_and_forget() {
    let driver = mock_driver();
    driver.start().await.unwrap();

    // 不等待应答的舵机透传:mock 记录写入帧,返回 Ok
    let cmd = ServoCmdWrapper::new(vec![0x01, 0x02]);
    driver
        .forward_servo(cmd)
        .await
        .expect("fire-and-forget forward should not error");

    driver.stop().await.unwrap();
}
