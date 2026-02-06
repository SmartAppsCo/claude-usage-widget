#!/usr/bin/env bash
#
# release.sh - Bump version for a new release
#
# This script updates version numbers:
#   1. Validates the version number
#   2. Updates version in Cargo.toml
#   3. Updates the install tag in README.md
#   4. Adds a new section to CHANGELOG.md (with placeholders)
#
# After running, fill in CHANGELOG.md and commit manually.
#
# USAGE:
#   ./scripts/release.sh <version>
#   ./scripts/release.sh patch|minor|major
#   ./scripts/release.sh --allow-dirty <version>
#
# OPTIONS:
#   --allow-dirty   Skip the clean working tree check
#
# EXAMPLES:
#   ./scripts/release.sh 0.2.0          # Set specific version
#   ./scripts/release.sh patch          # 0.1.0 -> 0.1.1
#   ./scripts/release.sh minor          # 0.1.0 -> 0.2.0
#   ./scripts/release.sh major          # 0.1.0 -> 1.0.0
#   ./scripts/release.sh --allow-dirty patch  # Skip dirty check
#

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
fatal() { error "$*"; exit 1; }

# Change to repo root
cd "$(dirname "$0")/.."

# Files to update
CARGO_TOML="Cargo.toml"
README="README.md"
CHANGELOG="CHANGELOG.md"

# Get current version from Cargo.toml
get_current_version() {
    grep '^version = ' "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/'
}

# Parse version into components
parse_version() {
    local version="$1"
    echo "$version" | sed 's/\./ /g'
}

# Calculate next version based on bump type
bump_version() {
    local current="$1"
    local bump_type="$2"

    read -r major minor patch <<< "$(parse_version "$current")"

    case "$bump_type" in
        major)
            echo "$((major + 1)).0.0"
            ;;
        minor)
            echo "${major}.$((minor + 1)).0"
            ;;
        patch)
            echo "${major}.${minor}.$((patch + 1))"
            ;;
        *)
            fatal "Unknown bump type: $bump_type"
            ;;
    esac
}

# Validate version format
validate_version() {
    local version="$1"
    if ! echo "$version" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
        fatal "Invalid version format: $version (expected X.Y.Z or X.Y.Z-prerelease)"
    fi
}

# Update version in Cargo.toml
update_cargo_toml() {
    local file="$1"
    local version="$2"

    if [[ -f "$file" ]]; then
        sed -i "s/^version = \".*\"/version = \"$version\"/" "$file"
        success "Updated $file"
    else
        fatal "File not found: $file"
    fi
}

# Update --tag version in README.md
update_readme() {
    local file="$1"
    local version="$2"

    if [[ -f "$file" ]]; then
        sed -i "s/--tag v[0-9][0-9.a-zA-Z-]*/--tag v$version/" "$file"
        success "Updated $file"
    else
        warn "File not found: $file"
    fi
}

# Check changelog is valid for this version (call before any writes)
check_changelog() {
    local version="$1"

    if [[ ! -f "$CHANGELOG" ]]; then
        fatal "CHANGELOG.md not found"
    fi

    if grep -q "## \[$version\]" "$CHANGELOG"; then
        fatal "Version $version already exists in CHANGELOG.md"
    fi
}

# Update CHANGELOG.md (assumes check_changelog was called first)
update_changelog() {
    local version="$1"
    local date
    date=$(date +%Y-%m-%d)

    local tmp
    tmp=$(mktemp)

    awk -v version="$version" -v date="$date" '
        function print_version_section() {
            print "## [" version "] - " date
            print ""
            print "### Added"
            print ""
            print "- "
            print ""
            print "### Changed"
            print ""
            print "- "
            print ""
            print "### Fixed"
            print ""
            print "- "
            print ""
            print ""
        }
        /^## \[/ && !inserted {
            print ""
            print ""
            print_version_section()
            inserted = 1
            blank_count = 0
        }
        {
            if (/^$/) {
                blank_count++
            } else {
                for (i = 0; i < blank_count; i++) print ""
                blank_count = 0
                print
            }
        }
        END {
            if (!inserted) {
                print ""
                print ""
                print_version_section()
            }
        }
    ' "$CHANGELOG" > "$tmp"

    mv "$tmp" "$CHANGELOG"
    success "Updated $CHANGELOG (remember to fill in the changes!)"
}

# Check for uncommitted changes
check_clean_tree() {
    if ! git diff --quiet || ! git diff --cached --quiet; then
        fatal "Working tree has uncommitted changes. Commit or stash them first."
    fi
}

# Main
main() {
    local allow_dirty=false
    local input=""

    # Parse all arguments (flags can be in any position)
    for arg in "$@"; do
        case "$arg" in
            --allow-dirty)
                allow_dirty=true
                ;;
            -*)
                fatal "Unknown option: $arg"
                ;;
            *)
                if [[ -n "$input" ]]; then
                    fatal "Multiple version arguments: $input and $arg"
                fi
                input="$arg"
                ;;
        esac
    done

    if [[ -z "$input" ]]; then
        echo "Usage: $0 [--allow-dirty] <version|patch|minor|major>"
        echo ""
        echo "Current version: $(get_current_version)"
        exit 1
    fi
    local current_version
    local new_version

    current_version=$(get_current_version)
    info "Current version: $current_version"

    # Determine new version
    case "$input" in
        patch|minor|major)
            new_version=$(bump_version "$current_version" "$input")
            ;;
        *)
            new_version="$input"
            ;;
    esac

    validate_version "$new_version"
    info "New version: $new_version"

    # Confirm
    echo ""
    read -rp "Proceed with release $new_version? [y/N] " confirm
    case "$confirm" in
        [yY][eE][sS]|[yY]) ;;
        *) fatal "Aborted" ;;
    esac

    echo ""

    # === All checks before any writes ===

    if [[ "$allow_dirty" == true ]]; then
        warn "Skipping clean working tree check (--allow-dirty)"
    else
        check_clean_tree
    fi

    check_changelog "$new_version"

    # === All checks passed, now do writes ===

    info "Updating version files..."
    update_cargo_toml "$CARGO_TOML" "$new_version"
    update_readme "$README" "$new_version"

    echo ""
    info "Updating changelog..."
    update_changelog "$new_version"

    echo ""
    echo -e "${GREEN}=== Version Bumped ===${NC}"
    echo ""
    echo "Version files updated to $new_version."
    echo "CHANGELOG.md has placeholder entries - fill them in before committing."
    echo ""
    echo "Next steps:"
    echo "  1. Fill in CHANGELOG.md"
    echo "  2. git add -u && git commit -m 'Release v$new_version'"
    echo "  3. git tag v$new_version"
    echo "  4. git push origin main --tags"
    echo ""
    echo "The tag push triggers the GitHub Action to create a release with:"
    echo "  - Your CHANGELOG notes"
    echo "  - Auto-generated contributor callouts"
    echo ""
}

main "$@"
