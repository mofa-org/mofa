#!/bin/bash
# MoFA Release Script
# This script automates the release process for the MoFA project
#
# Usage: ./scripts/release.sh [version] [options]
#
# Options:
#   --dry-run         Show what would be done without making changes
#   --skip-tests      Skip running tests
#   --skip-build      Skip building release binaries
#   --publish         Publish Rust crates to crates.io
#   --publish-pypi    Publish Python package to PyPI
#   --publish-maven   Publish Java package to Maven Central
#   --publish-go      Publish Go module (create and push git tag)
#   --publish-all     Publish to all registries (crates.io + PyPI + Maven + Go)
#   --git-tag         Create and push git tag
#   --help            Show this help message
#
# Examples:
#   ./scripts/release.sh 1.0.0 --dry-run
#   ./scripts/release.sh 1.0.0 --publish-all --git-tag
#   ./scripts/release.sh 1.0.0 --publish-pypi --publish-maven

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Default options
DRY_RUN=false
SKIP_TESTS=false
SKIP_BUILD=false
PUBLISH_TO_CRATES_IO=false
PUBLISH_TO_PYPI=false
PUBLISH_TO_MAVEN=false
PUBLISH_TO_GO=false
CREATE_GIT_TAG=false
VERSION=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-tests)
            SKIP_TESTS=true
            shift
            ;;
        --skip-build)
            SKIP_BUILD=true
            shift
            ;;
        --publish)
            PUBLISH_TO_CRATES_IO=true
            shift
            ;;
        --publish-pypi)
            PUBLISH_TO_PYPI=true
            shift
            ;;
        --publish-maven)
            PUBLISH_TO_MAVEN=true
            shift
            ;;
        --publish-go)
            PUBLISH_TO_GO=true
            shift
            ;;
        --publish-all)
            PUBLISH_TO_CRATES_IO=true
            PUBLISH_TO_PYPI=true
            PUBLISH_TO_MAVEN=true
            PUBLISH_TO_GO=true
            shift
            ;;
        --git-tag)
            CREATE_GIT_TAG=true
            shift
            ;;
        --help)
            sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# //g' | sed 's/^#//g'
            exit 0
            ;;
        -*)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"
            else
                echo -e "${RED}Unexpected argument: $1${NC}"
                exit 1
            fi
            shift
            ;;
    esac
done

# Validate version
if [[ -z "$VERSION" ]]; then
    echo -e "${RED}Error: Version number is required${NC}"
    echo "Usage: $0 [version] [options]"
    exit 1
fi

# Validate semver format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo -e "${RED}Error: Invalid version format. Expected semver (e.g., 1.0.0 or 1.0.0-rc.1)${NC}"
    exit 1
fi

# Functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

run_cmd() {
    local cmd="$1"
    if [[ "$DRY_RUN" == true ]]; then
        echo -e "${YELLOW}[DRY RUN]${NC} $cmd"
    else
        log_info "Running: $cmd"
        eval "$cmd"
    fi
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "Required command not found: $1"
        exit 1
    fi
}

check_clean_git() {
    if [[ -n "$(git status --porcelain)" ]]; then
        log_error "Working directory is not clean. Please commit or stash changes first."
        exit 1
    fi
}

check_on_main_branch() {
    local branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$branch" != "main" && "$branch" != "master" ]]; then
        log_warning "Not on main/master branch (current: $branch)"
        read -p "Continue anyway? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# Main release process
main() {
    log_info "MoFA Release Process v$VERSION"
    echo ""

    # Check prerequisites
    log_info "Checking prerequisites..."
    check_command "cargo"
    check_command "git"

    if [[ "$PUBLISH_TO_CRATES_IO" == true ]]; then
        check_command "cargo-publish-workspace"
    fi

    if [[ "$PUBLISH_TO_PYPI" == true ]]; then
        check_command "maturin"
        check_command "twine"
    fi

    if [[ "$PUBLISH_TO_MAVEN" == true ]]; then
        check_command "mvn"
        check_command "gpg"
    fi

    if [[ "$PUBLISH_TO_GO" == true ]]; then
        check_command "git"
    fi

    check_clean_git
    check_on_main_branch

    log_success "Prerequisites check passed"
    echo ""

    # Step 1: Update version in Cargo.toml files
    log_info "Step 1: Updating version to $VERSION..."

    # Update workspace version
    run_cmd "sed -i '' 's/^version = \"[^\"]*\"/version = \"$VERSION\"/' '$PROJECT_ROOT/Cargo.toml'"

    # Update package versions in each crate
    for crate in crates/*/Cargo.toml; do
        run_cmd "sed -i '' 's/^version = \"[^\"]*\"/version = \"$VERSION\"/' '$crate'"
    done

    # Update workspace dependency versions
    run_cmd "find '$PROJECT_ROOT/crates' -name 'Cargo.toml' -exec sed -i '' 's/mofa-sdk = \"[^\"]*\"/mofa-sdk = \"$VERSION\"/g' {} +"

    log_success "Version updated"
    echo ""

    # Step 2: Run tests
    if [[ "$SKIP_TESTS" == false ]]; then
        log_info "Step 2: Running tests..."
        run_cmd "cd '$PROJECT_ROOT' && cargo test --workspace --all-features"
        log_success "All tests passed"
        echo ""
    else
        log_warning "Skipping tests (--skip-tests)"
        echo ""
    fi

    # Step 3: Build release binaries
    if [[ "$SKIP_BUILD" == false ]]; then
        log_info "Step 3: Building release binaries..."
        run_cmd "cd '$PROJECT_ROOT' && cargo build --release --workspace"
        log_success "Release binaries built successfully"
        echo ""
    else
        log_warning "Skipping build (--skip-build)"
        echo ""
    fi

    # Step 4: Build CLI binary for multiple targets
    log_info "Step 4: Building CLI for multiple platforms..."

    if [[ "$SKIP_BUILD" == false ]]; then
        PLATFORMS=(
            "x86_64-apple-darwin"
            "aarch64-apple-darwin"
            "x86_64-unknown-linux-gnu"
            "x86_64-pc-windows-msvc"
        )

        for platform in "${PLATFORMS[@]}"; do
            log_info "Building for $platform..."
            run_cmd "cd '$PROJECT_ROOT' && cargo build --release --target $platform -p mofa-cli"
        done

        # Create release directory
        RELEASE_DIR="$PROJECT_ROOT/target/release-$VERSION"
        run_cmd "mkdir -p '$RELEASE_DIR'"

        # Copy binaries to release directory
        for platform in "${PLATFORMS[@]}"; do
            bin_name="mofa"
            if [[ "$platform" == *"windows"* ]]; then
                bin_name="mofa.exe"
            fi

            src_bin="$PROJECT_ROOT/target/$platform/release/$bin_name"
            dest_name="mofa-$platform"

            if [[ "$platform" == *"windows"* ]]; then
                dest_name="mofa-$platform.exe"
            fi

            run_cmd "cp '$src_bin' '$RELEASE_DIR/$dest_name'"
            run_cmd "chmod +x '$RELEASE_DIR/$dest_name' 2>/dev/null || true"
        done

        # Create checksums
        run_cmd "cd '$RELEASE_DIR' && shasum -a 256 * > SHA256SUMS"

        log_success "Multi-platform binaries built in $RELEASE_DIR"
        echo ""
    fi

    # Step 5: Commit version changes
    log_info "Step 5: Committing version changes..."
    run_cmd "cd '$PROJECT_ROOT' && git add -A"
    run_cmd "cd '$PROJECT_ROOT' && git commit -m 'chore: bump version to $VERSION'"
    log_success "Version changes committed"
    echo ""

    # Step 6: Create git tag
    if [[ "$CREATE_GIT_TAG" == true ]]; then
        log_info "Step 6: Creating git tag v$VERSION..."
        run_cmd "cd '$PROJECT_ROOT' && git tag -a 'v$VERSION' -m 'Release v$VERSION'"
        run_cmd "cd '$PROJECT_ROOT' && git push origin main"
        run_cmd "cd '$PROJECT_ROOT' && git push origin 'v$VERSION'"
        log_success "Git tag created and pushed"
        echo ""
    else
        log_warning "Skipping git tag creation (--git-tag not specified)"
        echo ""
    fi

    # Step 7: Publish to crates.io
    if [[ "$PUBLISH_TO_CRATES_IO" == true ]]; then
        log_info "Step 7: Publishing to crates.io..."

        # Check if cargo-publish-workspace is installed
        if ! command -v cargo-publish-workspace &> /dev/null; then
            log_warning "cargo-publish-workspace not found. Installing..."
            run_cmd "cargo install cargo-publish-workspace"
        fi

        # Publish in correct order (SDK first, then others)
        log_info "Publishing mofa-sdk first..."
        run_cmd "cd '$PROJECT_ROOT/crates/mofa-sdk' && cargo publish"

        log_info "Waiting for mofa-sdk to be available on crates.io..."
        sleep 30

        log_info "Publishing remaining crates..."
        run_cmd "cd '$PROJECT_ROOT' && cargo publish-workspace --no-dev-dependencies --skip mofa-sdk --skip mofa-macros"

        log_success "All crates published to crates.io"
        echo ""
    else
        log_warning "Skipping crates.io publishing (--publish not specified)"
        echo ""
    fi

    # Step 7.5: Generate bindings for all languages
    if [[ "$PUBLISH_TO_PYPI" == true || "$PUBLISH_TO_MAVEN" == true || "$PUBLISH_TO_GO" == true ]]; then
        log_info "Step 7.5: Generating language bindings..."
        SDK_DIR="$PROJECT_ROOT/crates/mofa-sdk"

        # Generate Python, Kotlin, Swift, Java bindings
        run_cmd "cd '$SDK_DIR' && ./generate-bindings.sh all"

        # Generate Go bindings separately (different toolchain)
        if [[ -f "$SDK_DIR/bindings/go/generate-go.sh" ]]; then
            run_cmd "cd '$SDK_DIR/bindings/go' && ./generate-go.sh"
        fi

        log_success "Language bindings generated"
        echo ""
    fi

    # Step 8: Publish to PyPI
    if [[ "$PUBLISH_TO_PYPI" == true ]]; then
        log_info "Step 8: Publishing to PyPI..."
        PYTHON_DIR="$PROJECT_ROOT/crates/mofa-sdk/bindings/python"

        # Update version in pyproject.toml
        run_cmd "sed -i '' 's/^version = \"[^\"]*\"/version = \"$VERSION\"/' '$PYTHON_DIR/pyproject.toml'"

        # Build Python wheel with maturin
        run_cmd "cd '$PYTHON_DIR' && maturin build --release --strip --out dist/"

        # Check if we should publish or just show what would be done
        if [[ "$DRY_RUN" == false ]]; then
            # Publish to PyPI
            log_info "Uploading to PyPI..."
            run_cmd "cd '$PYTHON_DIR' && twine upload dist/*"
        else
            log_warning "Skipping actual PyPI upload (dry-run mode)"
        fi

        log_success "Python package published to PyPI"
        echo ""
    else
        log_warning "Skipping PyPI publishing (--publish-pypi not specified)"
        echo ""
    fi

    # Step 9: Publish to Maven Central
    if [[ "$PUBLISH_TO_MAVEN" == true ]]; then
        log_info "Step 9: Publishing to Maven Central..."
        JAVA_DIR="$PROJECT_ROOT/crates/mofa-sdk/bindings/java"

        # Update version in pom.xml
        run_cmd "sed -i '' 's/<version>[^<]*<\\/version>/<version>$VERSION<\\/version>/g' '$JAVA_DIR/pom.xml'"

        # Build and deploy with Maven
        if [[ "$DRY_RUN" == false ]]; then
            run_cmd "cd '$JAVA_DIR' && mvn clean deploy -P release"
        else
            log_warning "Skipping actual Maven deployment (dry-run mode)"
        fi

        log_success "Java package published to Maven Central"
        echo ""
    else
        log_warning "Skipping Maven Central publishing (--publish-maven not specified)"
        echo ""
    fi

    # Step 10: Publish Go module
    if [[ "$PUBLISH_TO_GO" == true ]]; then
        log_info "Step 10: Publishing Go module..."
        GO_DIR="$PROJECT_ROOT/crates/mofa-sdk/bindings/go"

        # Update version in go.mod
        run_cmd "sed -i '' 's/mofa-go v[^\"]*/mofa-go v$VERSION/g' '$GO_DIR/go.mod'"

        # Go modules are auto-discovered via git tags
        if [[ "$DRY_RUN" == false ]]; then
            # Create a Go-specific version tag
            run_cmd "cd '$PROJECT_ROOT' && git tag -a 'go/v$VERSION' -m 'Go release v$VERSION'"
            run_cmd "cd '$PROJECT_ROOT' && git push origin 'go/v$VERSION'"
        else
            log_warning "Skipping actual Go tag creation (dry-run mode)"
        fi

        log_success "Go module published (tag: go/v$VERSION)"
        echo ""
    else
        log_warning "Skipping Go module publishing (--publish-go not specified)"
        echo ""
    fi

    # Summary
    echo ""
    log_success "Release v$VERSION completed successfully!"
    echo ""
    echo "Summary:"
    echo "  - Version: $VERSION"
    echo "  - Binaries: $RELEASE_DIR"
    echo "  - Git tag: ${CREATE_GIT_TAG:-Not created}"
    echo "  - Crates.io: ${PUBLISH_TO_CRATES_IO:-Not published}"
    echo "  - PyPI: ${PUBLISH_TO_PYPI:-Not published}"
    echo "  - Maven Central: ${PUBLISH_TO_MAVEN:-Not published}"
    echo "  - Go module: ${PUBLISH_TO_GO:-Not published}"
    echo ""
    echo "Next steps:"
    if [[ "$CREATE_GIT_TAG" == false ]]; then
        echo "  1. Review changes: git log"
        echo "  2. Create tag: git tag -a v$VERSION -m 'Release v$VERSION'"
        echo "  3. Push tag: git push origin v$VERSION"
    fi
    if [[ "$PUBLISH_TO_CRATES_IO" == false ]]; then
        echo "  1. Publish to crates.io: cargo publish"
    fi
    if [[ "$PUBLISH_TO_PYPI" == false ]]; then
        echo "  1. Publish to PyPI: cd crates/mofa-sdk/bindings/python && maturin publish"
    fi
    if [[ "$PUBLISH_TO_MAVEN" == false ]]; then
        echo "  1. Publish to Maven Central: cd crates/mofa-sdk/bindings/java && mvn deploy"
    fi
    if [[ "$PUBLISH_TO_GO" == false ]]; then
        echo "  1. Tag Go module: git tag -a go/v$VERSION -m 'Go release v$VERSION' && git push origin go/v$VERSION"
    fi
    echo "  2. Create GitHub release with binaries from: $RELEASE_DIR"
}

# Run main function
main
