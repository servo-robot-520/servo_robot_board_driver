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

    /* 1. 打开不存在的串口: 期望 NULL + err_buf 有内容(验证 ABI、错误路径、panic 边界) */
    d = sr_driver_open("/dev/nonexistent_ffi_test", 115200, err_buf, sizeof(err_buf));
    CHECK(d == NULL, "open nonexistent port returns NULL");
    CHECK(err_buf[0] != '\0', "err_buf populated on failure");

    /* 2. NULL 句柄安全检查 */
    CHECK(sr_driver_start(NULL) == SR_ERR_NULL, "start(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_stop(NULL) == SR_ERR_NULL, "stop(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_write_config(NULL, (sr_config){0}) == SR_ERR_NULL,
          "write_config(NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_set_callbacks(NULL, NULL) == SR_ERR_NULL,
          "set_callbacks(NULL,NULL) -> SR_ERR_NULL");
    CHECK(sr_driver_last_error(NULL, err_buf, sizeof(err_buf)) == SR_ERR_NULL,
          "last_error(NULL) -> SR_ERR_NULL");
    sr_driver_free(NULL); /* 不应崩溃 */

    /* 3. 非法参数 */
    CHECK(sr_driver_write_config(NULL, (sr_config){.typ = 0x99, .value = 0}) == SR_ERR_NULL,
          "invalid cfg + NULL handle -> SR_ERR_NULL (handle check first)");

    /* 4. 回调表结构体可用(memset 0 全 NULL 表) */
    sr_callbacks cbs;
    memset(&cbs, 0, sizeof(cbs));
    CHECK(cbs.on_imu_data == NULL, "zeroed callbacks table is valid (all NULL)");

    if (failures == 0) {
        printf("\nAll smoke tests passed.\n");
        return 0;
    }
    printf("\n%d smoke test(s) failed.\n", failures);
    return 1;
}
