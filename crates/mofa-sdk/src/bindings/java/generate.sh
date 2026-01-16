#!/bin/bash
# MoFA Java/Kotlin 绑定生成脚本
#
# UniFFI 原生支持 Kotlin，Kotlin 代码可以直接在 Java 项目中使用。
# 此脚本生成 Kotlin 绑定并设置项目结构。
#
# 用法:
#   ./generate.sh          # 不启用 dora runtime (默认)
#   ./generate.sh --dora   # 启用 dora runtime

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# 解析命令行参数
ENABLE_DORA=false
for arg in "$@"; do
    case $arg in
        --dora)
            ENABLE_DORA=true
            shift
            ;;
        --help|-h)
            echo "用法: $0 [选项]"
            echo ""
            echo "选项:"
            echo "  --dora    启用 dora runtime 功能"
            echo "  --help    显示帮助信息"
            exit 0
            ;;
    esac
done

# 设置 features
if [ "$ENABLE_DORA" = true ]; then
    FEATURES="uniffi,dora"
    DORA_STATUS="启用"
else
    FEATURES="uniffi"
    DORA_STATUS="禁用"
fi

echo "================================================"
echo "MoFA Java/Kotlin 绑定生成脚本"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""

cd "$PROJECT_ROOT"

# 检测操作系统和架构
case "$(uname -s)" in
    Darwin*)
        LIB_EXT="dylib"
        OS_NAME="darwin"
        ;;
    Linux*)
        LIB_EXT="so"
        OS_NAME="linux"
        ;;
    MINGW*|CYGWIN*|MSYS*)
        LIB_EXT="dll"
        OS_NAME="win32"
        ;;
    *)
        LIB_EXT="so"
        OS_NAME="linux"
        ;;
esac

case "$(uname -m)" in
    arm64|aarch64)  ARCH="aarch64" ;;
    x86_64|amd64)   ARCH="x86-64" ;;
    *)              ARCH="x86-64" ;;
esac

JNA_RESOURCE_DIR="$OS_NAME-$ARCH"
echo "检测到平台: $JNA_RESOURCE_DIR (.$LIB_EXT)"
echo ""

# 1. 构建库
echo "步骤 1: 构建 MoFA 库 (release 模式, features: $FEATURES)..."
cargo build --features "$FEATURES" --release

# 2. 查找库文件
echo ""
echo "步骤 2: 查找库文件..."

# 查找库文件（可能在不同目录）
LIBRARY_PATH=""
for search_path in "target/release/libaimos.$LIB_EXT" "/xdfapp/rust-target/release/libaimos.$LIB_EXT"; do
    if [ -f "$search_path" ]; then
        LIBRARY_PATH="$search_path"
        break
    fi
done

if [ -z "$LIBRARY_PATH" ]; then
    echo "错误: 找不到库文件"
    echo "尝试搜索..."
    find . -name "libaimos.$LIB_EXT" 2>/dev/null | head -5
    exit 1
fi

echo "找到库文件: $LIBRARY_PATH"

# 3. 生成 Kotlin 绑定
echo ""
echo "步骤 3: 生成 Kotlin 绑定..."

# 清理旧的绑定
rm -rf bindings/java/src/main/kotlin/uniffi

cargo run --features "$FEATURES" --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language kotlin \
    --out-dir bindings/java/src/main/kotlin

# 4. 复制库文件到多个位置
echo ""
echo "步骤 4: 复制库文件..."

# 复制到 libs 目录（用于 java.library.path）
mkdir -p bindings/java/libs
cp "$LIBRARY_PATH" bindings/java/libs/

# 复制到资源目录（用于 JNA 自动加载）
mkdir -p "bindings/java/src/main/resources/$JNA_RESOURCE_DIR"
cp "$LIBRARY_PATH" "bindings/java/src/main/resources/$JNA_RESOURCE_DIR/"

echo ""
echo "================================================"
echo "Java/Kotlin 绑定生成完成!"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""
echo "生成的文件:"
find bindings/java/src/main/kotlin -type f -name "*.kt" 2>/dev/null | head -20
echo ""
echo "库文件位置:"
echo "  - bindings/java/libs/libaimos.$LIB_EXT"
echo "  - bindings/java/src/main/resources/$JNA_RESOURCE_DIR/libaimos.$LIB_EXT"
echo ""
echo "使用方法 (Maven, 需要 Java 9+):"
echo "  cd bindings/java"
echo "  JAVA_HOME=/path/to/java11+ mvn compile"
echo "  JAVA_HOME=/path/to/java11+ mvn test"
echo "  JAVA_HOME=/path/to/java11+ mvn exec:java"
echo ""
echo "检查 dora 是否可用: isDoraAvailable()"
echo ""
