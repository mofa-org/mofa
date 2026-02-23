#!/bin/bash
# Build MoFA documentation using mdbook

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DOC_DIR="$PROJECT_ROOT/docs/mofa-doc"

# Parse arguments
CLEAN=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --clean|-c)
            CLEAN=true
            shift
            ;;
        *)
            echo "Usage: $0 [--clean|-c]"
            echo "  --clean, -c  Clean build directory before building"
            exit 1
            ;;
    esac
done

echo "Building MoFA documentation..."

cd "$DOC_DIR"

# Check if mdbook is installed
if ! command -v mdbook &> /dev/null; then
    echo "Error: mdbook is not installed."
    echo "Install it with: cargo install mdbook"
    exit 1
fi

# Check for mdbook-mermaid preprocessor
if ! command -v mdbook-mermaid &> /dev/null; then
    echo "Warning: mdbook-mermaid is not installed. Mermaid diagrams may not render."
    echo "Install it with: cargo install mdbook-mermaid"
fi

# Clean build directory if requested
if [ "$CLEAN" = true ]; then
    echo "Cleaning build directory..."
    rm -rf "$DOC_DIR/book"
fi

# Build the documentation
mdbook build

echo ""
echo "Documentation built successfully!"
echo "Open $DOC_DIR/book/html/index.html to view."
