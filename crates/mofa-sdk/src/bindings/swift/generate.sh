#!/bin/bash
# MoFA Swift 绑定生成脚本
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
echo "MoFA Swift 绑定生成脚本"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""

cd "$PROJECT_ROOT"

# 检测操作系统
case "$(uname -s)" in
    Darwin*)    LIB_EXT="dylib" ;;
    Linux*)     LIB_EXT="so" ;;
    *)
        echo "错误: Swift 绑定仅支持 macOS 和 Linux"
        exit 1
        ;;
esac

echo "检测到库扩展名: .$LIB_EXT"
echo ""

# 1. 构建库
echo "步骤 1: 构建 MoFA 库 (release 模式, features: $FEATURES)..."
cargo build --features "$FEATURES" --release

# 2. 生成 Swift 绑定
echo ""
echo "步骤 2: 生成 Swift 绑定..."

LIBRARY_PATH="target/release/libaimos.$LIB_EXT"
if [ ! -f "$LIBRARY_PATH" ]; then
    echo "错误: 找不到库文件 $LIBRARY_PATH"
    exit 1
fi

cargo run --features "$FEATURES" --bin uniffi-bindgen generate \
    --library "$LIBRARY_PATH" \
    --language swift \
    --out-dir bindings/swift

# 3. 复制库文件
echo ""
echo "步骤 3: 复制库文件到 Swift 绑定目录..."
cp "$LIBRARY_PATH" bindings/swift/

# 4. 生成模块映射（用于 Swift Package Manager）
echo ""
echo "步骤 4: 生成模块映射..."
cat > bindings/swift/module.modulemap << 'EOF'
module AimosFFI {
    header "aimosFFI.h"
    link "mofa"
    export *
}
EOF

echo ""
echo "================================================"
echo "Swift 绑定生成完成!"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""
echo "生成的文件:"
ls -la bindings/swift/
echo ""
echo "使用方法:"
echo "  1. 将生成的文件添加到你的 Xcode 项目"
echo "  2. 确保 libaimos.$LIB_EXT 在正确的搜索路径中"
echo "  3. 导入模块: import Aimos"
echo ""
echo "或者使用 Swift Package Manager (参见 Package.swift 示例)"
echo ""
echo "检查 dora 是否可用: isDoraAvailable()"
