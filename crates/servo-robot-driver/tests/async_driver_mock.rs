//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/6 23:01

//! AsyncDriver 集成测试（使用 AsyncMockTransport）

#![cfg(all(feature = "mock", feature = "async"))]

use servo_robot_driver::protocol::battery_state::{BatteryChargeStatus, BatteryState};
use servo_robot_driver::protocol::config::{Config, ConfigType};
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::power::PowerData;
use servo_robot_driver::{AsyncDriver, AsyncMockTransport, DriverCallback};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 测试回调收集器
struct TestCallback {
    imu_count: Arc<AtomicU64>,
    power_count: Arc<AtomicU64>,
    battery_count: Arc<AtomicU64>,
    last_imu: Arc<Mutex<Option<ImuData>>>,
    last_battery: Arc<Mutex<Option<BatteryState>>>,
}

impl TestCallback {
    fn new() -> (Self, CallbackStats) {
        let imu_count = Arc::new(AtomicU64::new(0));
        let power_count = Arc::new(AtomicU64::new(0));
        let battery_count = Arc::new(AtomicU64::new(0));
        let last_imu = Arc::new(Mutex::new(None));
        let last_battery = Arc::new(Mutex::new(None));

        let stats = CallbackStats {
            imu_count: imu_count.clone(),
            power_count: power_count.clone(),
            battery_count: battery_count.clone(),
            last_imu: last_imu.clone(),
            last_battery: last_battery.clone(),
        };

        let callback = TestCallback {
            imu_count,
            power_count,
            battery_count,
            last_imu,
            last_battery,
        };

        (callback, stats)
    }
}

struct CallbackStats {
    imu_count: Arc<AtomicU64>,
    power_count: Arc<AtomicU64>,
    battery_count: Arc<AtomicU64>,
    last_imu: Arc<Mutex<Option<ImuData>>>,
    last_battery: Arc<Mutex<Option<BatteryState>>>,
}

impl CallbackStats {
    fn imu_count(&self) -> u64 {
        self.imu_count.load(Ordering::Relaxed)
    }

    fn power_count(&self) -> u64 {
        self.power_count.load(Ordering::Relaxed)
    }

    fn battery_count(&self) -> u64 {
        self.battery_count.load(Ordering::Relaxed)
    }

    fn last_imu(&self) -> Option<ImuData> {
        self.last_imu.lock().unwrap().clone()
    }

    fn last_battery(&self) -> Option<BatteryState> {
        self.last_battery.lock().unwrap().clone()
    }
}

impl DriverCallback for TestCallback {
    fn on_imu_data(&mut self, data: &ImuData) {
        self.imu_count.fetch_add(1, Ordering::Relaxed);
        *self.last_imu.lock().unwrap() = Some(data.clone());
    }

    fn on_power_data(&mut self, _data: &PowerData) {
        self.power_count.fetch_add(1, Ordering::Relaxed);
    }

    fn on_battery_state(&mut self, state: &BatteryState) {
        self.battery_count.fetch_add(1, Ordering::Relaxed);
        *self.last_battery.lock().unwrap() = Some(state.clone());
    }
}

/// 等待直到条件满足或超时
async fn wait_until<F: Fn() -> bool>(timeout: Duration, check: F) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

#[tokio::test]
async fn test_async_driver_receives_imu_data() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().await.unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.imu_count() > 0).await;
    assert!(received, "Should receive IMU data within 2 seconds");

    let imu = stats.last_imu().unwrap();
    assert!(
        imu.accel[2] > 8.0 && imu.accel[2] < 12.0,
        "Z-axis accel should be ~9.81"
    );
    assert!(
        imu.quaternion[0].abs() <= 1.0,
        "Quaternion should be normalized"
    );

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_receives_multiple_data_types() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().await.unwrap();

    let received = wait_until(Duration::from_secs(3), || {
        stats.imu_count() > 0 && stats.power_count() > 0 && stats.battery_count() > 0
    })
    .await;
    assert!(received, "Should receive IMU, Power, and Battery data");

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_query_config_sync() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    driver.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = driver
        .query_config_sync(ConfigType::PowerServoCurrentLimitMa)
        .await;
    assert!(
        result.is_ok(),
        "Config query should succeed: {:?}",
        result.err()
    );

    let config = result.unwrap();
    assert_eq!(
        config.value(),
        5.0,
        "Default servo current limit should be 5.0A"
    );

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_query_all_configs_sync() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    driver.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = driver.query_all_configs_sync().await;
    assert!(
        result.is_ok(),
        "Query all configs should succeed: {:?}",
        result.err()
    );

    let config = result.unwrap();
    // Config values are u16 (*10), so 5.0A = 50, 80.0°C = 800
    assert_eq!(config.servo_current_limit, 50);
    assert_eq!(config.servo_temp_limit, 800);

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_write_config_sync() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    driver.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = driver
        .write_config_sync(Config::PowerServoCurrentLimitMa(10.0))
        .await;
    assert!(
        result.is_ok(),
        "Config write should succeed: {:?}",
        result.err()
    );
    assert!(result.unwrap(), "Config write should be acknowledged");

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_state_snapshot() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);
    driver.start().await.unwrap();

    let state = driver.state();

    wait_until(Duration::from_secs(2), || state.snapshot().imu.is_some()).await;

    let snap = state.snapshot();
    assert!(snap.connected, "Should be connected");
    assert!(snap.imu.is_some(), "Should have IMU data");

    let imu = snap.imu.unwrap();
    assert!(imu.accel[2] > 8.0, "Should have gravity component");

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_battery_monitoring() {
    let mock = AsyncMockTransport::new();
    let mut driver = AsyncDriver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().await.unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.battery_count() > 0).await;
    assert!(received, "Should receive battery data");

    let battery = stats.last_battery().unwrap();
    // percentage is u8 (1~100), voltage is u16 (mV)
    assert!(battery.percentage > 0 && battery.percentage <= 100);
    assert!(battery.voltage > 12000 && battery.voltage <= 16800);
    assert_eq!(
        battery.cell_voltages.len(),
        4,
        "Should have 4 cells (4S LiPo)"
    );

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_mock_initial_attitude() {
    let mut mock = AsyncMockTransport::new();
    mock.set_initial_attitude(45.0, 10.0, 90.0);

    let mut driver = AsyncDriver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().await.unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.imu_count() > 0).await;
    assert!(received, "Should receive IMU data");

    let imu = stats.last_imu().unwrap();
    assert!((imu.roll - 45.0).abs() < 5.0, "Roll should be near 45°");
    assert!((imu.pitch - 10.0).abs() < 5.0, "Pitch should be near 10°");
    assert!((imu.yaw - 90.0).abs() < 5.0, "Yaw should be near 90°");

    driver.stop().await.unwrap();
}

#[tokio::test]
async fn test_async_driver_mock_charging() {
    let mut mock = AsyncMockTransport::new();
    mock.set_charging(true);
    mock.set_battery_soc(50.0);

    let mut driver = AsyncDriver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().await.unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.battery_count() > 0).await;
    assert!(received, "Should receive battery data");

    let battery = stats.last_battery().unwrap();
    assert_eq!(battery.charge_status, BatteryChargeStatus::Charging);

    driver.stop().await.unwrap();
}
