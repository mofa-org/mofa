#!/bin/bash
# MoFA Pre-Release Check Script
# This script performs various checks before a release
#
# Usage: ./scripts/check-release.sh [version]
#
# This script will check:
#   - All tests pass
#   - Code compiles without errors
#   - Documentation builds correctly
#   - Changelog is updated
#   - Version numbers are consistent
#   - No uncommitted changes

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Counters
CHECKS_PASSED=0
CHECKS_FAILED=0
CHECKS_WARNED=0

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((CHECKS_PASSED++))
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((CHECKS_FAILED++))
}

log_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((CHECKS_WARNED++))
}

check_command() {
    if command -v "$1" &> /dev/null; then
        return 0
    else
        return 1
    fi
}

check_version_consistency() {
    local expected_version="$1"

    if [[ -z "$expected_version" ]]; then
        log_info "No version specified, skipping version consistency check"
        log_warning "Specify a version to check: $0 <version>"
        return 0
    fi

    log_info "Checking version consistency for $expected_version..."

    local inconsistencies=0

    # Check workspace Cargo.toml
    local workspace_version
    workspace_version=$(grep "^version = " "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/version = "\([^"]*\)"/\1/')
    if [[ "$workspace_version" != "$expected_version" ]]; then
        log_error "Workspace version mismatch: $workspace_version (expected $expected_version)"
        ((inconsistencies++))
    fi

    # Check each crate
    for crate_toml in "$PROJECT_ROOT"/crates/*/Cargo.toml; do
        local crate_name
        crate_name=$(basename "$(dirname "$crate_toml")")
        local crate_version
        crate_version=$(grep "^version = " "$crate_toml" | sed 's/version = "\([^"]*\)"/\1/')

        if [[ "$crate_version" != "$expected_version" ]]; then
            log_error "$crate_name version mismatch: $crate_version (expected $expected_version)"
            ((inconsistencies++))
        fi
    done

    if [[ $inconsistencies -eq 0 ]]; then
        log_success "All version numbers are consistent ($expected_version)"
    else
        log_error "Found $inconsistencies version inconsistencies"
    fi
}

check_git_clean() {
    log_info "Checking git working directory status..."

    if [[ -n "$(git status --porcelain)" ]]; then
        log_error "Working directory is not clean"
        echo ""
        echo "Uncommitted changes:"
        git status --short
        echo ""
        log_warning "Please commit or stash changes before releasing"
    else
        log_success "Working directory is clean"
    fi
}

check_git_branch() {
    log_info "Checking git branch..."

    local branch
    branch=$(git rev-parse --abbrev-ref HEAD)

    if [[ "$branch" == "main" || "$branch" == "master" ]]; then
        log_success "On release branch: $branch"
    else
        log_warning "Not on main/master branch (current: $branch)"
    fi
}

check_tests() {
    log_info "Running tests..."

    if cargo test --workspace 2>&1 | tee /tmp/test-output.txt; then
        log_success "All tests passed"
    else
        log_error "Some tests failed"
        echo ""
        echo "Test output:"
        cat /tmp/test-output.txt
    fi
}

check_build() {
    log_info "Checking build..."

    if cargo build --workspace 2>&1 | tee /tmp/build-output.txt; then
        log_success "Project builds successfully"
    else
        log_error "Build failed"
        echo ""
        echo "Build output:"
        cat /tmp/build-output.txt
    fi
}

check_build_release() {
    log_info "Checking release build..."

    if cargo build --release --workspace 2>&1 | tee /tmp/build-release-output.txt; then
        log_success "Release build successful"
    else
        log_error "Release build failed"
        echo ""
        echo "Build output:"
        cat /tmp/build-release-output.txt
    fi
}

check_doc() {
    log_info "Checking documentation build..."

    if cargo doc --workspace --no-deps 2>&1 | tee /tmp/doc-output.txt; then
        log_success "Documentation builds successfully"
    else
        log_error "Documentation build failed"
        echo ""
        echo "Doc output:"
        cat /tmp/doc-output.txt
    fi
}

check_formatting() {
    log_info "Checking code formatting..."

    if cargo fmt -- --check 2>&1 | tee /tmp/fmt-output.txt; then
        log_success "Code is properly formatted"
    else
        log_error "Code formatting issues found"
        echo ""
        echo "Run 'cargo fmt' to fix:"
        cat /tmp/fmt-output.txt
    fi
}

check_lint() {
    log_info "Running clippy..."

    if cargo clippy --workspace -- -D warnings 2>&1 | tee /tmp/clippy-output.txt; then
        log_success "No clippy warnings"
    else
        log_warning "Clippy found warnings (non-blocking)"
        echo ""
        cat /tmp/clippy-output.txt
    fi
}

check_changelog() {
    log_info "Checking CHANGELOG.md..."

    local changelog="$PROJECT_ROOT/CHANGELOG.md"

    if [[ ! -f "$changelog" ]]; then
        log_warning "No CHANGELOG.md found"
        return 0
    fi

    # Check if there's an unreleased section
    if grep -q "## \[Unreleased\]" "$changelog"; then
        log_success "CHANGELOG.md has [Unreleased] section"
    else
        log_warning "No [Unreleased] section in CHANGELOG.md"
    fi

    # Check if there's a section for the target version
    if [[ -n "$1" ]]; then
        local version="$1"
        if grep -q "## \[${version}\]" "$changelog" || grep -q "## \[${version#v}\]" "$changelog"; then
            log_success "CHANGELOG.md has section for $version"
        else
            log_warning "No section for $version in CHANGELOG.md"
        fi
    fi
}

check_dependencies() {
    log_info "Checking required tools..."

    local required_tools=("cargo" "git" "rustc")
    local missing_tools=()

    for tool in "${required_tools[@]}"; do
        if check_command "$tool"; then
            log_success "$tool is installed"
        else
            log_error "$tool is not installed"
            missing_tools+=("$tool")
        fi
    done

    if [[ ${#missing_tools[@]} -gt 0 ]]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        return 1
    fi
}

check_examples() {
    log_info "Checking examples build..."

    if cargo build --examples 2>&1 | tee /tmp/examples-output.txt; then
        log_success "All examples build successfully"
    else
        log_error "Some examples failed to build"
        cat /tmp/examples-output.txt
    fi
}

# Main
main() {
    local version="$1"

    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  MoFA Pre-Release Check${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""

    if [[ -n "$version" ]]; then
        log_info "Target version: $version"
    else
        log_info "No version specified"
    fi
    echo ""

    # Run checks
    check_dependencies
    check_git_clean
    check_git_branch
    check_version_consistency "$version"
    check_formatting
    check_lint
    check_build
    check_build_release
    check_tests
    check_doc
    check_changelog "$version"
    check_examples

    # Summary
    echo ""
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Summary${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo ""
    echo -e "${GREEN}Passed:  $CHECKS_PASSED${NC}"
    echo -e "${YELLOW}Warnings: $CHECKS_WARNED${NC}"
    echo -e "${RED}Failed:   $CHECKS_FAILED${NC}"
    echo ""

    if [[ $CHECKS_FAILED -gt 0 ]]; then
        echo -e "${RED}Release check FAILED${NC}"
        echo "Please fix the failures before releasing"
        exit 1
    elif [[ $CHECKS_WARNED -gt 0 ]]; then
        echo -e "${YELLOW}Release check passed with warnings${NC}"
        echo "You may want to review the warnings before releasing"
        exit 0
    else
        echo -e "${GREEN}Release check PASSED${NC}"
        echo "All checks passed! Ready for release."
        exit 0
    fi
}

main "$@"
