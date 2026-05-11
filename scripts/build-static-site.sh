#!/usr/bin/env bash
# Render the traits.build site into ./dist/ as a fully static bundle for GitHub Pages.
# Requires a built ./target/release/traits binary.
set -euxo pipefail
cd "$(dirname "$0")/.."

BIN="${TRAITS_BIN:-./target/release/traits}"
OUT="${OUT_DIR:-dist}"

if [[ ! -x "$BIN" ]]; then
    echo "binary not found at $BIN — run: cargo build --release" >&2
    exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/traits" "$OUT/api" "$OUT/build"

# Pages return a JSON-encoded HTML string; strip the outer quoting with jq -r.
render_page() {
    local trait="$1" out="$2"
    local stderr_tmp
    stderr_tmp=$(mktemp)
    # Use 'if !' so set -e doesn't abort before we can print the error.
    if ! "$BIN" call "$trait" 2>"$stderr_tmp" | jq -r . > "$out"; then
        echo "render failed: $trait — pipeline failed" >&2
        echo "--- binary stderr ---" >&2
        cat "$stderr_tmp" >&2
        rm -f "$stderr_tmp"; exit 1
    fi
    rm -f "$stderr_tmp"
    if [[ ! -s "$out" ]]; then echo "render failed: $trait — empty output" >&2; exit 1; fi
}

render_page www.docs       "$OUT/traits/index.html"
render_page www.docs.api   "$OUT/api/index.html"
render_page www.build      "$OUT/build/index.html"

# Root → docs page (so visiting / lands on the documentation).
cp "$OUT/traits/index.html" "$OUT/index.html"
# 404 fallback also lands on docs.
cp "$OUT/traits/index.html" "$OUT/404.html"

# Static data the IDE and Redoc page fetch.
"$BIN" list                    > "$OUT/traits.json"
"$BIN" call sys.openapi 2>/dev/null > "$OUT/openapi.json"

# A copy at each subpath because pages reference both ./openapi.json and ./traits.json.
cp "$OUT/traits.json"  "$OUT/build/traits.json"
cp "$OUT/openapi.json" "$OUT/api/openapi.json"

# In-browser WASM kernel + terminal so the static /build IDE is fully live.
[[ -f static/wasm-runtime.js ]]     && cp static/wasm-runtime.js     "$OUT/wasm-runtime.js"
[[ -f static/terminal-runtime.js ]] && cp static/terminal-runtime.js "$OUT/terminal-runtime.js"

# Domain + sitemap.
[[ -f CNAME ]]       && cp CNAME       "$OUT/CNAME"
[[ -f sitemap.xml ]] && cp sitemap.xml "$OUT/sitemap.xml"
[[ -f robots.txt ]]  && cp robots.txt  "$OUT/robots.txt"

echo "✓ wrote $OUT/"
( cd "$OUT" && find . -maxdepth 2 -type f | sort )
