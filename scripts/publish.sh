#!/bin/bash
# Publish workspace members to crates.io in dependency order.
# Usage: bash scripts/publish.sh [--dry-run]
set -euo pipefail

DRY_RUN=""
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN="--dry-run"
    echo "=== DRY RUN (only leaf crates testable — deps need real publish) ==="
fi

# Order matters: leaf deps first, then dependents.
# 1. plugin_api   — no workspace deps
# 2. kernel-logic — no workspace deps
# 3. traits       — depends on kernel-logic (--no-verify: index lag)
# 4. trait-sys-checksum — depends on plugin_api (--no-verify: index lag)
# (trait-www-traits-build has publish = false)
# (trait-sys-ps removed: now a builtin compiled into the kernel binary)

# Crates with workspace deps need --no-verify because the crates.io sparse
# index takes time to reflect newly published versions. We've already tested
# the full build via build.sh, so verification is redundant.
LEAF_CRATES=("traits-plugin-api" "kernel-logic")
DEP_CRATES=("traits" "trait-sys-checksum")

wait_for_index() {
    local crate="$1"
    local version="$2"
    if [ -n "$DRY_RUN" ]; then return 0; fi

    echo "  Waiting for crates.io index to list $crate@$version..."
    for i in $(seq 1 30); do
        sleep 5
        # Check if the version appears in the crates.io API
        if curl -fsSL "https://crates.io/api/v1/crates/$crate/$version" 2>/dev/null | grep -q '"version"'; then
            echo "  ✓ $crate@$version is live on crates.io"
            # Give the sparse index a moment to catch up
            sleep 3
            return 0
        fi
        echo "  ... attempt $i/30"
    done
    echo "  ✗ Timed out waiting for $crate@$version"
    exit 1
}

get_version() {
    local crate="$1"
    cargo metadata --format-version 1 --no-deps 2>/dev/null \
        | grep -o "\"name\":\"$crate\",\"version\":\"[^\"]*\"" \
        | head -1 \
        | sed -E 's/.*"version":"([^"]+)".*/\1/'
}

# Clear cargo's crates.io cache so it picks up freshly published versions
clear_index_cache() {
    if [ -n "$DRY_RUN" ]; then return 0; fi
    echo "  Clearing sparse index cache..."
    cargo update 2>/dev/null || true
}

echo ""
echo "Publishing workspace members to crates.io"
echo "=========================================="

# Phase 1: Publish leaf crates (no workspace deps — full verify)
for crate in "${LEAF_CRATES[@]}"; do
    VERSION="$(get_version "$crate")"
    echo ""
    echo "── $crate v$VERSION ──"

    if cargo publish -p "$crate" --allow-dirty $DRY_RUN 2>&1; then
        echo "  ✓ Published $crate v$VERSION"
        wait_for_index "$crate" "$VERSION"
        clear_index_cache
    else
        echo "  ✗ Failed to publish $crate"
        exit 1
    fi
done

# Phase 2: Publish dependent crates (--no-verify: index may lag)
# In dry-run mode, these will fail because leaf deps aren't on crates.io yet.
for crate in "${DEP_CRATES[@]}"; do
    VERSION="$(get_version "$crate")"
    echo ""
    echo "── $crate v$VERSION (--no-verify) ──"

    if cargo publish -p "$crate" --no-verify --allow-dirty $DRY_RUN 2>&1; then
        echo "  ✓ Published $crate v$VERSION"
    else
        if [ -n "$DRY_RUN" ]; then
            echo "  ⚠ Skipped (dry-run can't resolve unpublished workspace deps)"
        else
            echo "  ✗ Failed to publish $crate"
            exit 1
        fi
    fi
done

echo ""
echo "✓ All crates published successfully"
