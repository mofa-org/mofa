#!/bin/bash
# MoFA Kotlin 绑定生成脚本
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
echo "MoFA Kotlin 绑定生成脚本"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""

cd "$PROJECT_ROOT"

# 检测操作系统
case "$(uname -s)" in
    Darwin*)    LIB_EXT="dylib" ;;
    Linux*)     LIB_EXT="so" ;;
    MINGW*|CYGWIN*|MSYS*) LIB_EXT="dll" ;;
    *)          LIB_EXT="so" ;;
esac

echo "检测到库扩展名: .$LIB_EXT"
echo ""

# 1. 构建库
echo "步骤 1: 构建 MoFA 库 (release 模式, features: $FEATURES)..."
cargo build --features "$FEATURES" --release

# 2. 生成 Kotlin 绑定
echo ""
echo "步骤 2: 生成 Kotlin 绑定..."

LIBRARY_PATH="target/release/libaimos.$LIB_EXT"
if [ ! -f "$LIBRARY_PATH" ]; then
    echo "错误: 找不到库文件 $LIBRARY_PATH"
    exit 1
fi

cargo run --features "$FEATURES" --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language kotlin \
    --out-dir bindings/kotlin

# 3. 复制库文件
echo ""
echo "步骤 3: 复制库文件到 Kotlin 绑定目录..."
cp "$LIBRARY_PATH" bindings/kotlin/

echo ""
echo "================================================"
echo "Kotlin 绑定生成完成!"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""
echo "生成的文件:"
ls -la bindings/kotlin/
echo ""
echo "编译和运行方法:"
echo "  cd bindings/kotlin"
echo "  kotlinc -include-runtime -d Example.jar Example.kt uniffi/mofa/*.kt"
echo "  java -Djava.library.path=. -jar Example.jar"
echo ""
echo "或者使用 Gradle (参见 build.gradle.kts 示例)"
echo ""
echo "检查 dora 是否可用: isDoraAvailable()"
