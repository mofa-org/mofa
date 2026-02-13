#!/bin/bash
# MoFA UniFFI Binding Generation Script
#
# This script generates language bindings from the compiled mofa-ffi library.
#
# Prerequisites:
# - Rust toolchain installed
# - cargo install uniffi-bindgen-cli (for Python, Kotlin, Swift)
# - For Java: cargo install uniffi-bindgen-java
#
# Usage:
#   ./generate-bindings.sh [python|kotlin|swift|java|all]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE_DIR="$SCRIPT_DIR"
PROJECT_ROOT="$SCRIPT_DIR/../.."
BINDINGS_DIR="$CRATE_DIR/bindings"

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

# Build the library with uniffi feature
build_library() {
    print_info "Building mofa-ffi with uniffi feature..."
    cd "$PROJECT_ROOT"
    cargo build --release --features "uniffi" -p mofa-ffi
    cd "$CRATE_DIR"
}

# Find the compiled library
find_library() {
    LIB_PATH="$PROJECT_ROOT/target/release/libmofa_ffi.$LIB_EXT"
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

# Generate Python bindings
generate_python() {
    print_info "Generating Python bindings..."
    mkdir -p "$BINDINGS_DIR/python"

    uniffi-bindgen generate \
        --library "$LIB_PATH" \
        --language python \
        --out-dir "$BINDINGS_DIR/python"

    # Create __init__.py for package import
    echo "from .mofa import *" > "$BINDINGS_DIR/python/__init__.py"

    print_info "Python bindings generated at $BINDINGS_DIR/python"
}

# Generate Kotlin bindings
generate_kotlin() {
    print_info "Generating Kotlin bindings..."
    mkdir -p "$BINDINGS_DIR/kotlin"

    uniffi-bindgen generate \
        --library "$LIB_PATH" \
        --language kotlin \
        --out-dir "$BINDINGS_DIR/kotlin"

    print_info "Kotlin bindings generated at $BINDINGS_DIR/kotlin"
}

# Generate Swift bindings
generate_swift() {
    print_info "Generating Swift bindings..."
    mkdir -p "$BINDINGS_DIR/swift"

    uniffi-bindgen generate \
        --library "$LIB_PATH" \
        --language swift \
        --out-dir "$BINDINGS_DIR/swift"

    print_info "Swift bindings generated at $BINDINGS_DIR/swift"
}

# Generate Java bindings
generate_java() {
    print_info "Generating Java bindings..."
    mkdir -p "$BINDINGS_DIR/java"

    # Check if uniffi-bindgen-java is installed
    if ! command -v uniffi-bindgen-java &> /dev/null; then
        print_warn "uniffi-bindgen-java not found"
        print_info "Install it with: cargo install uniffi-bindgen-java"
        print_info "Then re-run this script with 'java' argument"
        return 1
    fi

    uniffi-bindgen-java \
        --library "$LIB_PATH" \
        --out-dir "$BINDINGS_DIR/java" \
        --package "org.mofa"

    print_info "Java bindings generated at $BINDINGS_DIR/java"
}

# Print usage
print_usage() {
    echo "Usage: $0 [python|kotlin|swift|java|all]"
    echo ""
    echo "Commands:"
    echo "  python  - Generate Python bindings"
    echo "  kotlin  - Generate Kotlin bindings"
    echo "  swift   - Generate Swift bindings"
    echo "  java    - Generate Java bindings (requires uniffi-bindgen-java)"
    echo "  all     - Generate all bindings"
    echo ""
    echo "Prerequisites:"
    echo "  - Build library: cargo build --release --features 'uniffi' -p mofa-ffi"
    echo "  - Install uniffi-bindgen: cargo install uniffi-bindgen-cli"
    echo "  - For Java: cargo install uniffi-bindgen-java"
}

# Main
main() {
    detect_platform

    case "${1:-all}" in
        python)
            find_library
            generate_python
            ;;
        kotlin)
            find_library
            generate_kotlin
            ;;
        swift)
            find_library
            generate_swift
            ;;
        java)
            find_library
            generate_java
            ;;
        all)
            find_library
            generate_python
            generate_kotlin
            generate_swift
            generate_java || true  # Don't fail if Java bindgen is missing
            ;;
        help|--help|-h)
            print_usage
            ;;
        *)
            print_error "Unknown command: $1"
            print_usage
            exit 1
            ;;
    esac

    echo ""
    print_info "Done! Bindings are in: $BINDINGS_DIR"
}

main "$@"
