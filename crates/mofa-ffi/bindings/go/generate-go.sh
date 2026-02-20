#!/bin/bash
# MoFA Go Bindings Generation Script
#
# This script generates Go bindings from the compiled mofa-sdk library using uniffi-bindgen-go.
#
# Prerequisites:
# - Rust toolchain installed
# - cargo install uniffi-bindgen-go --git https://github.com/NordSecurity/uniffi-bindgen-go
# - Go toolchain installed
#
# Usage:
#   ./generate-go.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$SCRIPT_DIR/../.."
PROJECT_ROOT="$SCRIPT_DIR/../../../.."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect platform and library extension
detect_platform() {
    case "$(uname -s)" in
        Darwin*)
            LIB_EXT="dylib"
            ;;
        Linux*)
            LIB_EXT="so"
            ;;
        CYGWIN*|MINGW*|MSYS*)
            LIB_EXT="dll"
            ;;
        *)
            print_error "Unknown platform"
            exit 1
            ;;
    esac
}

# Build the library with uniffi and openai features
build_library() {
    print_info "Building mofa-sdk with uniffi and openai features..."
    cd "$PROJECT_ROOT"
    cargo build --release --features "uniffi,openai" -p mofa-sdk
    cd "$SCRIPT_DIR"
}

# Find the compiled library
find_library() {
    LIB_PATH="$PROJECT_ROOT/target/release/libmofa_sdk.$LIB_EXT"
    if [ ! -f "$LIB_PATH" ]; then
        print_error "Library not found at $LIB_PATH"
        print_info "Building library first..."
        build_library
    fi

    if [ ! -f "$LIB_PATH" ]; then
        print_error "Failed to build library"
        exit 1
    fi

    print_info "Using library: $LIB_PATH"
}

# Check if uniffi-bindgen-go is installed
check_bindgen() {
    if ! command -v uniffi-bindgen-go &> /dev/null; then
        print_error "uniffi-bindgen-go not found"
        print_info "Install it with: cargo install uniffi-bindgen-go --git https://github.com/NordSecurity/uniffi-bindgen-go"
        exit 1
    fi
}

# Generate Go bindings
generate_bindings() {
    print_info "Generating Go bindings..."

    uniffi-bindgen-go generate \
        --library "$LIB_PATH" \
        --out-dir "$SCRIPT_DIR"

    print_info "Go bindings generated at $SCRIPT_DIR"
}

# Main
main() {
    detect_platform
    check_bindgen
    find_library
    generate_bindings

    echo ""
    print_info "Done! Go bindings are in: $SCRIPT_DIR"
    print_info "You can now use the bindings in your Go project."
}

main "$@"
