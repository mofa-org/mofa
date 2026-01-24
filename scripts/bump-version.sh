#!/bin/bash
# MoFA Version Bump Script
# This script updates the version number across all crates in the workspace
#
# Usage: ./scripts/bump-version.sh [major|minor|patch|pre|VERSION]
#
# Examples:
#   ./scripts/bump-version.sh patch       # 0.1.0 -> 0.1.1
#   ./scripts/bump-version.sh minor       # 0.1.0 -> 0.2.0
#   ./scripts/bump-version.sh major       # 0.1.0 -> 1.0.0
#   ./scripts/bump-version.sh pre         # 0.1.0 -> 0.2.0-rc.1
#   ./scripts/bump-version.sh 1.2.3       # Set specific version

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CARGO_TOML="$PROJECT_ROOT/Cargo.toml"

# Get current version
get_current_version() {
    grep "^version = " "$CARGO_TOML" | head -1 | sed 's/version = "\([^"]*\)"/\1/'
}

# Parse semver
parse_version() {
    local version="$1"
    # Remove 'v' prefix if present
    version="${version#v}"

    # Check for pre-release version
    if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)-(.+)$ ]]; then
        echo "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" "${BASH_REMATCH[4]}"
    elif [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
        echo "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}" ""
    else
        echo -e "${RED}Error: Invalid version format: $version${NC}" >&2
        exit 1
    fi
}

# Bump version based on type
bump_version() {
    local current="$1"
    local bump_type="$2"

    read -r major minor patch prerelease <<< $(parse_version "$current")

    case "$bump_type" in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            prerelease=""
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            prerelease=""
            ;;
        patch)
            patch=$((patch + 1))
            prerelease=""
            ;;
        pre|rc)
            minor=$((minor + 1))
            patch=0
            if [[ -n "$prerelease" ]]; then
                # Increment pre-release number
                if [[ "$prerelease" =~ rc\.([0-9]+)$ ]]; then
                    rc_num=$((BASH_REMATCH[1] + 1))
                    prerelease="rc.$rc_num"
                else
                    prerelease="rc.1"
                fi
            else
                prerelease="rc.1"
            fi
            ;;
        *)
            echo -e "${RED}Error: Invalid bump type: $bump_type${NC}" >&2
            echo "Valid types: major, minor, patch, pre" >&2
            exit 1
            ;;
    esac

    if [[ -n "$prerelease" ]]; then
        echo "${major}.${minor}.${patch}-${prerelease}"
    else
        echo "${major}.${minor}.${patch}"
    fi
}

# Update version in Cargo.toml files
update_version() {
    local new_version="$1"
    local current_version="$2"

    echo -e "${BLUE}Updating version: ${current_version} -> ${new_version}${NC}"
    echo ""

    # Update workspace Cargo.toml
    echo -e "${YELLOW}Updating workspace Cargo.toml...${NC}"
    sed -i '' "s/^version = \"${current_version}\"/version = \"${new_version}\"/" "$CARGO_TOML"

    # Update all crate Cargo.toml files
    for crate_toml in "$PROJECT_ROOT"/crates/*/Cargo.toml; do
        echo -e "${YELLOW}Updating $(basename $(dirname $crate_toml))/Cargo.toml...${NC}"
        sed -i '' "s/^version = \"${current_version}\"/version = \"${new_version}\"/" "$crate_toml"
    done

    # Update workspace dependency references
    echo -e "${YELLOW}Updating mofa-sdk dependency references...${NC}"
    find "$PROJECT_ROOT/crates" -name "Cargo.toml" -exec sed -i '' "s/mofa-sdk = \"${current_version}\"/mofa-sdk = \"${new_version}\"/g" {} +

    # Update CLI version in main.rs if it exists
    main_rs="$PROJECT_ROOT/crates/mofa-cli/src/main.rs"
    if [[ -f "$main_rs" ]]; then
        echo -e "${YELLOW}Updating CLI version in main.rs...${NC}"
        sed -i '' "s/env!(\"CARGO_PKG_VERSION\")/\"${new_version}\"/" "$main_rs"
    fi

    echo ""
    echo -e "${GREEN}Version updated successfully!${NC}"
}

# Show changes
show_changes() {
    local current_version="$1"
    local new_version="$2"

    echo ""
    echo -e "${BLUE}Changes to be committed:${NC}"
    echo ""
    git diff --stat
    echo ""
    echo -e "${YELLOW}Showing diff of Cargo.toml files...${NC}"
    git diff "$PROJECT_ROOT/Cargo.toml" "$PROJECT_ROOT"/crates/*/Cargo.toml
}

# Main
main() {
    local bump_type="$1"

    # Get current version
    local current_version
    current_version=$(get_current_version)

    echo -e "${BLUE}Current version: ${current_version}${NC}"
    echo ""

    # Determine new version
    local new_version
    if [[ -z "$bump_type" ]]; then
        echo "Usage: $0 [major|minor|patch|pre|VERSION]"
        echo ""
        echo "Examples:"
        echo "  $0 patch       # Bump patch version (0.1.0 -> 0.1.1)"
        echo "  $0 minor       # Bump minor version (0.1.0 -> 0.2.0)"
        echo "  $0 major       # Bump major version (0.1.0 -> 1.0.0)"
        echo "  $0 pre         # Create pre-release (0.1.0 -> 0.2.0-rc.1)"
        echo "  $0 1.2.3       # Set specific version"
        exit 0
    fi

    # Check if it's a specific version or a bump type
    if [[ "$bump_type" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
        new_version="$bump_type"
    else
        new_version=$(bump_version "$current_version" "$bump_type")
    fi

    echo -e "${BLUE}New version: ${new_version}${NC}"
    echo ""

    # Confirm
    read -p "Continue with this version? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi

    # Update version
    update_version "$new_version" "$current_version"

    # Show changes
    show_changes "$current_version" "$new_version"

    # Prompt to commit
    echo ""
    read -p "Commit changes? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git add -A
        git commit -m "chore: bump version to ${new_version}"
        echo -e "${GREEN}Changes committed!${NC}"
        echo ""
        echo "To create a tag:"
        echo "  git tag -a v${new_version} -m 'Release v${new_version}'"
        echo "  git push origin v${new_version}"
    else
        echo ""
        echo "Changes not committed. Commit manually when ready:"
        echo "  git add -A"
        echo "  git commit -m 'chore: bump version to ${new_version}'"
    fi
}

main "$@"
