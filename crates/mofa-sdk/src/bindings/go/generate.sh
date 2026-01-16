#!/bin/bash
# MoFA Go 绑定生成脚本
#
# Go 绑定使用社区维护的 uniffi-bindgen-go
# 项目地址: https://github.com/ArcticOJ/uniffi-bindgen-go
#
# 安装 uniffi-bindgen-go:
#   cargo install uniffi-bindgen-go --git https://github.com/ArcticOJ/uniffi-bindgen-go
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
echo "MoFA Go 绑定生成脚本"
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

# 检查 uniffi-bindgen-go 是否安装
if ! command -v uniffi-bindgen-go &> /dev/null; then
    echo "错误: uniffi-bindgen-go 未安装"
    echo ""
    echo "请先安装 uniffi-bindgen-go:"
    echo "  cargo install uniffi-bindgen-go --git https://github.com/ArcticOJ/uniffi-bindgen-go"
    echo ""
    echo "或者使用其他社区维护的 Go 绑定生成器:"
    echo "  - https://github.com/ArcticOJ/uniffi-bindgen-go"
    echo "  - https://github.com/nickolashucker/uniffi-bindgen-go"
    exit 1
fi

# 1. 构建库
echo "步骤 1: 构建 MoFA 库 (release 模式, features: $FEATURES)..."
cargo build --features "$FEATURES" --release

# 2. 生成 Go 绑定
echo ""
echo "步骤 2: 生成 Go 绑定..."

LIBRARY_PATH="target/release/libmofa.$LIB_EXT"
if [ ! -f "$LIBRARY_PATH" ]; then
    echo "错误: 找不到库文件 $LIBRARY_PATH"
    exit 1
fi

uniffi-bindgen-go \
    --library "$LIBRARY_PATH" \
    --out-dir bindings/go

# 3. 复制库文件
echo ""
echo "步骤 3: 复制库文件到 Go 绑定目录..."
cp "$LIBRARY_PATH" bindings/go/

# 4. 初始化 Go 模块
echo ""
echo "步骤 4: 初始化 Go 模块..."
cd bindings/go
if [ ! -f "go.mod" ]; then
    go mod init github.com/mofa/bindings/go
fi

echo ""
echo "================================================"
echo "Go 绑定生成完成!"
echo "================================================"
echo ""
echo "Dora Runtime: $DORA_STATUS"
echo ""
echo "生成的文件:"
ls -la
echo ""
echo "使用方法:"
echo "  cd bindings/go"
echo "  go build"
echo "  go run example.go"
echo ""
echo "在其他项目中使用:"
echo "  go get github.com/mofa/bindings/go"
echo ""
echo "检查 dora 是否可用: IsDoraAvailable()"
