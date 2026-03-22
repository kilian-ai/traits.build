#!/bin/bash
# Build the kernel binary + all cdylib trait plugins, then distribute.
#
# Usage:
#   ./scripts/build.sh            # build everything
#   ./scripts/build.sh kernel     # build only the kernel binary
#   ./scripts/build.sh dylibs     # build only the cdylib plugins
#   ./scripts/build.sh install    # build + install kernel to ~/.traits/bin
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

TARGET_DIR="$ROOT_DIR/target/release"
DYLIB_EXT="dylib"
[[ "$(uname)" == "Linux" ]] && DYLIB_EXT="so"

# ── Sync versions: .trait.toml → Cargo.toml ──
# Format: {epoch}.{YYMMDD}.{HHMMSS}  (semver-compatible)
# epoch = manually-bumped milestone number (0, 1, 2, ...)
# date part = auto-generated from trait snapshot timestamps

sync_one_version() {
    local cargo_toml="$1"
    local trait_toml="$2"

    local epoch version date_part semver
    epoch=$(grep '^epoch' "$trait_toml" | head -1 | sed 's/[^0-9]//g')
    version=$(grep '^version' "$trait_toml" | head -1 | sed 's/.*= *"//' | sed 's/".*//')

    [[ -z "$epoch" ]] && epoch=0

    # Strip leading 'v': v260320.142947 → 260320.142947
    date_part="${version#v}"

    if [[ "$date_part" == *.* ]]; then
        semver="${epoch}.${date_part}"
    else
        semver="${epoch}.${date_part}.0"
    fi

    sed -i '' "s/^version = \".*\"/version = \"$semver\"/" "$cargo_toml"
    echo "    $(basename "$(dirname "$cargo_toml")")/Cargo.toml → $semver"
}

sync_versions() {
    echo "==> Syncing versions (.trait.toml → Cargo.toml)..."

    # Root kernel: use serve trait as version source
    sync_one_version "$ROOT_DIR/Cargo.toml" "$ROOT_DIR/traits/kernel/serve/serve.trait.toml"

    # All other workspace Cargo.tomls: find companion .trait.toml in same dir
    while IFS= read -r -d '' cargo_toml; do
        [[ "$cargo_toml" == "$ROOT_DIR/Cargo.toml" ]] && continue
        local crate_dir trait_toml=""
        crate_dir="$(dirname "$cargo_toml")"
        for f in "$crate_dir"/*.trait.toml; do
            [[ -f "$f" ]] && trait_toml="$f" && break
        done
        if [[ -n "$trait_toml" ]]; then
            sync_one_version "$cargo_toml" "$trait_toml"
        else
            echo "    SKIP: no .trait.toml for $(basename "$crate_dir")"
        fi
    done < <(find "$ROOT_DIR" -name "Cargo.toml" -not -path "*/target/*" -print0)
}

# ── Build kernel binary ──
build_kernel() {
    echo "==> Building kernel binary..."
    cargo build --release -p traits
    echo "    Kernel: $TARGET_DIR/traits"
}

# ── Build all cdylib trait plugins ──
build_dylibs() {
    echo "==> Building cdylib trait plugins..."

    # Find all workspace cdylib members by looking for Cargo.toml with crate-type = ["cdylib"]
    local count=0
    while IFS= read -r -d '' cargo_toml; do
        local crate_dir
        crate_dir="$(dirname "$cargo_toml")"

        # Check if it's a cdylib (match crate-type line, not descriptions)
        if grep -q 'crate-type.*cdylib' "$cargo_toml" 2>/dev/null; then
            local crate_name
            crate_name=$(grep '^name' "$cargo_toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

            echo "    Building: $crate_name"
            cargo build --release -p "$crate_name"

            # Distribute: copy the dylib to the trait's directory
            distribute_dylib "$crate_name" "$crate_dir"
            count=$((count + 1))
        fi
    done < <(find "$ROOT_DIR/traits" -name "Cargo.toml" -print0)

    echo "    Built $count trait plugin(s)"
}

# ── Copy a built dylib to its trait directory ──
distribute_dylib() {
    local crate_name="$1"
    local crate_dir="$2"

    # Cargo output: lib<crate_name_with_hyphens_as_underscores>.<ext>
    local lib_name
    lib_name="lib$(echo "$crate_name" | tr '-' '_').$DYLIB_EXT"
    local src="$TARGET_DIR/$lib_name"

    if [[ -f "$src" ]]; then
        # Get the directory name (used as the canonical dylib name)
        local dir_name
        dir_name="$(basename "$crate_dir")"
        local dst="$crate_dir/lib${dir_name}.$DYLIB_EXT"
        cp "$src" "$dst"
        echo "    Distributed: $dst"
    else
        echo "    WARNING: $src not found, skipping distribution"
    fi
}

# ── Install kernel to ~/.traits/bin ──
install_kernel() {
    local install_dir="$HOME/.traits/bin"
    mkdir -p "$install_dir"
    cp "$TARGET_DIR/traits" "$install_dir/traits"
    echo "==> Installed kernel to $install_dir/traits"
    echo "    Add to PATH: export PATH=\"\$HOME/.traits/bin:\$PATH\""
}

# ── Main ──
case "${1:-all}" in
    kernel)
        sync_versions
        build_kernel
        ;;
    dylibs)
        sync_versions
        build_dylibs
        ;;
    install)
        sync_versions
        build_kernel
        build_dylibs
        install_kernel
        ;;
    sync)
        sync_versions
        ;;
    all|*)
        sync_versions
        build_kernel
        build_dylibs
        echo "==> Done. Kernel + plugins built successfully."
        ;;
esac
