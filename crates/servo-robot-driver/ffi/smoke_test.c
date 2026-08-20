/*
 * servo-robot-driver C FFI 冒烟测试
 *
 * 编译运行:
 *   cargo build --release --features ffi
 *   gcc -Wall -Wextra -I../include smoke_test.c -o smoke_test \
 *       -L ../../../target/release -lservo_robot_driver \
 *       -Wl,-rpath,$PWD/../../../target/release
 *   ./smoke_test
 */
#include <stdio.h>
#include <string.h>

#include "servo_robot_driver.h"

static int failures = 0;

#define CHECK(cond, name)                                                        \
    do {                                                                         \
        if (cond) {                                                              \
            printf("PASS: %s\n", name);                                          \
        } else {                                                                 \
            printf("FAIL: %s\n", name);                                          \
            failures++;                                                          \
        }                                                                        \
    } while (0)

int main(void) {
    char err_buf[256];
    sr_driver* d;

    /* 1. 版本查询（始终成功，无需句柄） */
    sr_version ver = sr_driver_version();
    CHECK(ver.major > 0 || ver.minor > 0 || ver.patch > 0,
          "version() returns non-zero");
    printf("       driver version: %u.%u.%u\n", ver.major, ver.minor, ver.patch);

    /* 2. 打开不存在的串口: 期望 NULL + err_buf 有内容 */
    d = sr_driver_open("/dev/nonexistent_ffi_test", 115200, err_buf, sizeof(err_buf));
    CHECK(d == NULL, "open nonexistent port returns NULL");
    CHECK(err_buf[0] != '\0', "err_buf populated on failure");

    /* 3. NULL 句柄安全检查 */
    CHECK(sr_driver_start(NULL) == SR_ERR_NULL, "start(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_stop(NULL) == SR_ERR_NULL, "stop(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_write_config(NULL, (sr_config){0}) == SR_ERR_NULL,
          "write_config(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_set_callbacks(NULL, NULL) == SR_ERR_NULL,
          "set_callbacks(NULL,NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_last_error(NULL, err_buf, sizeof(err_buf)) == SR_ERR_NULL,
          "last_error(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_connect(NULL, "/dev/ttyUSB0", 115200) == SR_ERR_NULL,
          "connect(NULL,...) -> SR_ERR_NULL");
    sr_driver_free(NULL); /* 不应崩溃 */

    /* 4. connect NULL port 检查（需要有效句柄，这里用 NULL 测试句柄检查优先） */
    CHECK(sr_driver_connect(NULL, NULL, 115200) == SR_ERR_NULL,
          "connect(NULL handle, NULL port) -> SR_ERR_NULL (handle check first)");

    /* 5. 回调表结构体可用(memset 0 全 NULL 表) */
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof(cbs));
    CHECK(cbs.on_imu_data == NULL, "zeroed callbacks: on_imu_data is NULL");
    CHECK(cbs.on_diagnostic == NULL, "zeroed callbacks: on_diagnostic is NULL");
    CHECK(cbs.on_ack_device_info == NULL, "zeroed callbacks: on_ack_device_info is NULL");

    if (failures == 0) {
        printf("\nAll smoke tests passed.\n");
        return 0;
    }
    printf("\n%d smoke test(s) failed.\n", failures);
    return 1;
}
