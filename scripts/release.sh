#!/usr/bin/env bash
#
# Release traits.build to GitHub.
#
# Builds the project, reads the generated version, tags, pushes, and
# creates a GitHub release. Intended to run as the first build of the
# day (shortly after midnight UTC) so the version is a clean vYYMMDD.
#
# Usage:
#   ./scripts/release.sh                  # auto-detect version from build
#   ./scripts/release.sh v260322          # override version tag
#   ./scripts/release.sh --dry-run        # show what would happen
#
set -euo pipefail
cd "$(dirname "$0")/.."

DRY_RUN=false
VERSION=""

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    v*)        VERSION="$arg" ;;
    *)         echo "Usage: $0 [--dry-run] [vYYMMDD]"; exit 1 ;;
  esac
done

# ── Build ──────────────────────────────────────────────────────────
echo "==> Building release..."
cargo build --release

# ── Read version ───────────────────────────────────────────────────
if [[ -z "$VERSION" ]]; then
  VERSION=$(grep '^version' traits/sys/version/version.trait.toml \
    | head -1 | sed 's/.*"\(.*\)"/\1/')
fi

if [[ -z "$VERSION" ]]; then
  echo "ERROR: Could not determine version" >&2
  exit 1
fi

echo "==> Version: $VERSION"

# Warn if intraday suffix present
if [[ "$VERSION" == *.* ]]; then
  echo "WARNING: Version has intraday suffix ($VERSION)."
  echo "  For a clean release, build as the first build of the day (after midnight UTC)."
  read -rp "  Continue anyway? [y/N] " yn
  [[ "$yn" =~ ^[Yy]$ ]] || exit 0
fi

# ── Git status ─────────────────────────────────────────────────────
if [[ -n "$(git status --porcelain)" ]]; then
  echo "==> Uncommitted changes detected, committing..."
  if $DRY_RUN; then
    echo "  [dry-run] git add -A && git commit"
  else
    git add -A
    git commit -m "release: $VERSION"
  fi
fi

# ── Push ───────────────────────────────────────────────────────────
echo "==> Pushing to GitHub..."
if $DRY_RUN; then
  echo "  [dry-run] git push origin main"
else
  git push origin main
fi

# ── Tag ────────────────────────────────────────────────────────────
if git rev-parse "$VERSION" >/dev/null 2>&1; then
  echo "==> Tag $VERSION already exists, replacing..."
  if $DRY_RUN; then
    echo "  [dry-run] delete + recreate tag $VERSION"
  else
    git tag -d "$VERSION"
    git push origin ":refs/tags/$VERSION" 2>/dev/null || true
    gh release delete "$VERSION" --yes 2>/dev/null || true
  fi
fi

echo "==> Tagging $VERSION..."
if $DRY_RUN; then
  echo "  [dry-run] git tag $VERSION && git push origin $VERSION"
else
  git tag "$VERSION"
  git push origin "$VERSION"
fi

# ── Release notes ──────────────────────────────────────────────────
NOTES_FILE=$(mktemp)
cat > "$NOTES_FILE" <<EOF
## traits.build $VERSION

Pure Rust composable function kernel.

### Changes since last release

$(git log --oneline "$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || git rev-list --max-parents=0 HEAD)"..HEAD)
EOF

echo "==> Creating GitHub release..."
if $DRY_RUN; then
  echo "  [dry-run] gh release create $VERSION"
  echo "  Release notes:"
  cat "$NOTES_FILE"
else
  gh release create "$VERSION" --title "$VERSION" --notes-file "$NOTES_FILE"
fi

rm -f "$NOTES_FILE"

echo "==> Done: https://github.com/kilian-ai/traits.build/releases/tag/$VERSION"
