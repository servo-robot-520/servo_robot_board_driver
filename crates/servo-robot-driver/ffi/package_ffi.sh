#!/usr/bin/env bash
# 打包 servo-robot-driver FFI 产物: .so + 头文件 + 示例 + CMakeLists → 独立目录
#
# 用法:
#   package_ffi.sh [输出目录]
#     默认输出到仓库根 ffi-dist/
#
# 打包完成后整个目录可复制进任意 C/C++ 项目:
#   cd <输出目录> && cmake -S . -B build && cmake --build build
#   ./build/cpp_example /dev/ttyUSB0 115200
set -euo pipefail

# 仓库根(脚本位于 crates/servo-robot-driver/ffi/)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CRATE_DIR="$REPO_ROOT/crates/servo-robot-driver"
OUT_DIR="${1:-$REPO_ROOT/ffi-dist}"

echo "==> [1/4] cargo build --release --features ffi"
(cd "$REPO_ROOT" && cargo build --release --features ffi)

SO_FILE="$REPO_ROOT/target/release/libservo_robot_driver.so"
if [ ! -f "$SO_FILE" ]; then
    echo "ERROR: build failed, $SO_FILE not found" >&2
    exit 1
fi

echo "==> [2/4] assembling package at $OUT_DIR"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/include" "$OUT_DIR/lib" "$OUT_DIR/examples"

cp "$SO_FILE" "$OUT_DIR/lib/"
cp "$CRATE_DIR/include/servo_robot_driver.h" "$OUT_DIR/include/"
cp "$CRATE_DIR/ffi/cpp_example.cpp" "$CRATE_DIR/ffi/smoke_test.c" "$OUT_DIR/examples/"

echo "==> [3/4] writing CMakeLists.txt"
cat > "$OUT_DIR/CMakeLists.txt" << 'EOF'
cmake_minimum_required(VERSION 3.16)
project(servo_robot_driver_example C CXX)

# 导入已编译好的共享库(不重新编译 Rust 代码)
add_library(servo_robot_driver SHARED IMPORTED GLOBAL)
set_target_properties(servo_robot_driver PROPERTIES
    IMPORTED_LOCATION            "${CMAKE_CURRENT_SOURCE_DIR}/lib/libservo_robot_driver.so"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
)

# C++ 示例(完整生命周期: RAII + 回调 + 配置读写)
add_executable(cpp_example examples/cpp_example.cpp)
target_link_libraries(cpp_example PRIVATE servo_robot_driver)

# C 冒烟示例
add_executable(smoke_test examples/smoke_test.c)
target_link_libraries(smoke_test PRIVATE servo_robot_driver)

# 运行时直接找到 .so,无需 LD_LIBRARY_PATH
set_target_properties(cpp_example smoke_test PROPERTIES
    BUILD_RPATH  "${CMAKE_CURRENT_SOURCE_DIR}/lib"
    INSTALL_RPATH "${CMAKE_CURRENT_SOURCE_DIR}/lib"
)
EOF

echo "==> [4/4] writing README.md"
cat > "$OUT_DIR/README.md" << 'EOF'
# servo-robot-driver FFI 包

预编译的 Rust 驱动共享库 + C/C++ 头文件 + 示例。

## 构建

```bash
cmake -S . -B build
cmake --build build
```

## 运行

```bash
./build/cpp_example /dev/ttyUSB0 115200   # C++ 完整示例
./build/smoke_test                        # C 冒烟测试(错误路径/NULL 安全)
```

## 集成到你的项目

把 `include/` 与 `lib/` 复制进项目,链接 `libservo_robot_driver.so`:

```cmake
add_library(servo_robot_driver SHARED IMPORTED)
set_target_properties(servo_robot_driver PROPERTIES
    IMPORTED_LOCATION "${CMAKE_CURRENT_SOURCE_DIR}/lib/libservo_robot_driver.so"
    INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/include"
)
target_link_libraries(your_target PRIVATE servo_robot_driver)
```

或直接编译:

```bash
gcc your_prog.c -I include -L lib -lservo_robot_driver -Wl,-rpath,$PWD/lib -o your_prog
```

## 红线(详见 servo_robot_driver.h 顶部)

- 回调在驱动分发线程执行,回调内禁止调用任何 sr_driver_*(尤其 sr_driver_free)
- 回调参数指针仅在回调执行期间有效
- 禁止有其他线程调用句柄时 sr_driver_free
- 同步函数阻塞 ≤1s

重新打包请回到仓库运行 `crates/servo-robot-driver/ffi/package_ffi.sh`。
EOF

echo
echo "打包完成: $OUT_DIR"
echo "  ├── CMakeLists.txt"
echo "  ├── README.md"
echo "  ├── include/servo_robot_driver.h"
echo "  ├── lib/libservo_robot_driver.so"
echo "  └── examples/{cpp_example.cpp, smoke_test.c}"
echo
echo "下一步:"
echo "  cd $OUT_DIR"
echo "  cmake -S . -B build && cmake --build build"
echo "  ./build/cpp_example /dev/ttyUSB0 115200"
