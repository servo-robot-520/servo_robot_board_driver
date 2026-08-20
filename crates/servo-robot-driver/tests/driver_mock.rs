//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/4 15:39

//! Driver 集成测试（使用 MockTransport）

#![cfg(feature = "mock")]

use servo_robot_driver::protocol::battery_state::{BatteryChargeStatus, BatteryState};
use servo_robot_driver::protocol::config::Config;
use servo_robot_driver::protocol::config::ConfigType;
use servo_robot_driver::protocol::imu::ImuData;
use servo_robot_driver::protocol::power::PowerData;
use servo_robot_driver::{Driver, DriverCallback, MockTransport};
use servo_robot_protocol::log::LogLevel::OFF;
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
fn wait_until<F: Fn() -> bool>(timeout: Duration, check: F) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn test_driver_receives_imu_data() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.imu_count() > 0);
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

    driver.stop().unwrap();
}

#[test]
fn test_driver_receives_multiple_data_types() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    let received = wait_until(Duration::from_secs(3), || {
        stats.imu_count() > 0 && stats.power_count() > 0 && stats.battery_count() > 0
    });
    assert!(received, "Should receive IMU, Power, and Battery data");

    driver.stop().unwrap();
}

#[test]
fn test_driver_write_config_sync() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    // 等待驱动启动并接收一些数据
    wait_until(Duration::from_secs(1), || stats.imu_count() > 0);

    // 发送命令并等待确认
    let result = driver.write_config_sync(Config::TxLogLevel(OFF));
    assert!(result.is_ok(), "Command should succeed: {:?}", result.err());
    assert!(result.unwrap(), "Command should be acknowledged as success");

    driver.stop().unwrap();
}

#[test]
fn test_driver_query_config_sync() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    driver.start().unwrap();

    // 等待驱动启动
    std::thread::sleep(Duration::from_millis(100));

    let result = driver.query_config_sync(ConfigType::PowerServoCurrentLimitMa);
    assert!(
        result.is_ok(),
        "Config query should succeed: {:?}",
        result.err()
    );

    let config = result.unwrap();
    assert_eq!(
        config.value(),
        50.0,
        "Default servo current limit should be 50mA"
    );

    driver.stop().unwrap();
}

#[test]
fn test_driver_query_all_configs_sync() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    driver.start().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let result = driver.query_all_configs_sync();
    assert!(
        result.is_ok(),
        "Query all configs should succeed: {:?}",
        result.err()
    );

    let config = result.unwrap();
    // Config values are u16 (*10), so 5.0A = 50, 80.0°C = 800
    assert_eq!(config.servo_current_limit_ma, 50);
    assert_eq!(config.servo_temp_limit, 800);

    driver.stop().unwrap();
}

#[test]
fn test_driver_write_config_value_sync() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    driver.start().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let result = driver.write_config_sync(Config::PowerServoCurrentLimitMa(100));
    assert!(
        result.is_ok(),
        "Config write should succeed: {:?}",
        result.err()
    );
    assert!(result.unwrap(), "Config write should be acknowledged");

    driver.stop().unwrap();
}

#[test]
fn test_driver_state_snapshot() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);
    driver.start().unwrap();

    let state = driver.state();

    wait_until(Duration::from_secs(2), || state.snapshot().imu.is_some());

    let snap = state.snapshot();
    assert!(snap.connected, "Should be connected");
    assert!(snap.imu.is_some(), "Should have IMU data");

    let imu = snap.imu.unwrap();
    assert!(imu.accel[2] > 8.0, "Should have gravity component");

    driver.stop().unwrap();
}

#[test]
fn test_driver_battery_monitoring() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.battery_count() > 0);
    assert!(received, "Should receive battery data");

    let battery = stats.last_battery().unwrap();
    // percentage is u8 (1~100), voltage is u16 (mV)
    assert!(battery.percentage > 0 && battery.percentage <= 100);
    assert!(battery.voltage_mv > 12000 && battery.voltage_mv <= 16800);
    assert_eq!(
        battery.cell_voltages_mv.len(),
        4,
        "Should have 4 cells (4S LiPo)"
    );

    driver.stop().unwrap();
}

#[test]
fn test_driver_mock_initial_attitude() {
    let mut mock = MockTransport::new();
    mock.set_initial_attitude(45.0, 10.0, 90.0);

    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.imu_count() > 0);
    assert!(received, "Should receive IMU data");

    let imu = stats.last_imu().unwrap();
    assert!((imu.roll - 45.0).abs() < 5.0, "Roll should be near 45°");
    assert!((imu.pitch - 10.0).abs() < 5.0, "Pitch should be near 10°");
    assert!((imu.yaw - 90.0).abs() < 5.0, "Yaw should be near 90°");

    driver.stop().unwrap();
}

#[test]
fn test_driver_mock_charging() {
    let mut mock = MockTransport::new();
    mock.set_charging(true);
    mock.set_battery_soc(50.0);

    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);
    driver.start().unwrap();

    let received = wait_until(Duration::from_secs(2), || stats.battery_count() > 0);
    assert!(received, "Should receive battery data");

    let battery = stats.last_battery().unwrap();
    assert_eq!(battery.charge_status, BatteryChargeStatus::Charging);

    driver.stop().unwrap();
}

#[test]
fn test_driver_stop_and_restart() {
    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);

    let (callback, stats) = TestCallback::new();
    driver.register_callback(callback);

    driver.start().unwrap();
    wait_until(Duration::from_secs(1), || stats.imu_count() > 0);
    let count1 = stats.imu_count();
    assert!(count1 > 0);

    driver.stop().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    driver.start().unwrap();
    wait_until(Duration::from_secs(1), || stats.imu_count() > count1);
    let count2 = stats.imu_count();
    assert!(count2 > count1, "Should receive more data after restart");

    driver.stop().unwrap();
}

#[test]
fn test_driver_auto_reconnect() {
    use servo_robot_driver::protocol::request::RequestKind;
    use servo_robot_driver::transport::FnTransportFactory;
    use servo_robot_driver::{ReconnectConfig, Transport};

    // 初始连接 3 帧后自动断开;工厂再次调用返回全新的正常 mock
    let calls = Arc::new(AtomicU64::new(0));
    let factory = {
        let calls = calls.clone();
        FnTransportFactory::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            let mut m = MockTransport::new();
            // 仅第一个实例开启自动断开
            if calls.load(Ordering::SeqCst) == 1 {
                m.set_auto_disconnect(3);
            }
            Ok(Box::new(m) as Box<dyn Transport>)
        })
    };
    let config = ReconnectConfig::new(3).with_retry_interval(Duration::from_millis(10));
    let mut driver = Driver::new_with_reconnect(factory, config);

    driver.start().unwrap();
    // 初始连接已建立
    assert!(wait_until(Duration::from_secs(2), || driver
        .state()
        .frame_count()
        > 0));

    // 3 帧后断开 → 自动重连 → 恢复连接
    assert!(
        wait_until(Duration::from_secs(3), || !driver.state().is_connected()),
        "should observe disconnect"
    );
    assert!(
        wait_until(Duration::from_secs(3), || driver.state().is_connected()),
        "should reconnect"
    );
    assert!(
        calls.load(Ordering::SeqCst) >= 2,
        "factory must be re-invoked for reconnect"
    );

    // 重连后仍能收帧
    let before = driver.state().frame_count();
    assert!(wait_until(Duration::from_secs(2), || driver
        .state()
        .frame_count()
        > before));

    // 恢复的实例上命令同步往返正常
    let ok = driver.send_command_sync(RequestKind::Reset).unwrap();
    assert!(ok, "command ACK after reconnect");

    driver.stop().unwrap();
}

#[test]
fn test_driver_reconnect_gives_up() {
    use servo_robot_driver::ReconnectConfig;
    use servo_robot_driver::transport::FnTransportFactory;

    // 工厂永远失败,max_retries=1 → 重试耗尽后读循环退出,不再空转
    let factory = FnTransportFactory::new(|| Err(servo_robot_driver::DriverError::TransportClosed));
    let config = ReconnectConfig::new(1).with_retry_interval(Duration::from_millis(10));
    let mut driver = Driver::new_with_reconnect(factory, config);

    driver.start().unwrap();
    // 初始连接失败 + 重试耗尽 → 状态为断开且保持
    assert!(
        wait_until(Duration::from_secs(2), || !driver.state().is_connected()),
        "should give up and stay disconnected"
    );
    // 重试耗尽后读循环记录 TransportClosed 错误并退出
    assert!(
        wait_until(Duration::from_secs(2), || driver
            .state()
            .last_error()
            .is_some()),
        "give-up should record TransportClosed error"
    );
    driver.stop().unwrap();
}

#[test]
fn test_driver_command_and_firmware_sync() {
    use servo_robot_driver::protocol::request::RequestKind;

    let mock = MockTransport::new();
    let mut driver = Driver::new(mock);
    driver.start().unwrap();

    // mock 回 Response(Reset, success=true)
    let ok = driver.send_command_sync(RequestKind::Reset).unwrap();
    assert!(ok);

    // mock 回 Response(FirmwareUpdate, success=true)
    let data = vec![0x11u8, 0x22, 0x33, 0x44];
    let ok = driver.firmware_update_sync(0x1000, &data).unwrap();
    assert!(ok, "firmware chunk ACK should succeed");

    driver.stop().unwrap();
}
