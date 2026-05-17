#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────────
# Download Kenney Racing Kit + Car Kit GLB models into public/assets/kenney/
#
# Assets are CC0 (public domain) — https://kenney.nl
# Run from anywhere inside the project:
#   bash tools/gta-builder/scripts/download-kenney-assets.sh
# ──────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEST="$SCRIPT_DIR/../public/assets/kenney"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

RACING_URL="https://kenney.nl/media/pages/assets/racing-kit/933b8fd9fd-1677580949/kenney_racing-kit.zip"
CARS_URL="https://kenney.nl/media/pages/assets/car-kit/1a312ec241-1775131960/kenney_car-kit.zip"

mkdir -p "$DEST/racing" "$DEST/cars"

echo "━━━ Kenney Racing Kit ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Downloading from kenney.nl…"
curl -L --progress-bar -o "$TMP/racing.zip" "$RACING_URL"

echo "Extracting GLBs…"
unzip -q "$TMP/racing.zip" -d "$TMP/racing_raw"
find "$TMP/racing_raw" -name "*.glb" | while read -r f; do
  cp "$f" "$DEST/racing/$(basename "$f")"
done
RACING_COUNT=$(ls "$DEST/racing/" | wc -l | tr -d ' ')
echo "✓ $RACING_COUNT racing kit models → public/assets/kenney/racing/"

echo ""
echo "━━━ Kenney Car Kit ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Downloading from kenney.nl…"
curl -L --progress-bar -o "$TMP/cars.zip" "$CARS_URL"

echo "Extracting GLBs…"
unzip -q "$TMP/cars.zip" -d "$TMP/cars_raw"
find "$TMP/cars_raw" -name "*.glb" | while read -r f; do
  cp "$f" "$DEST/cars/$(basename "$f")"
done
CARS_COUNT=$(ls "$DEST/cars/" | wc -l | tr -d ' ')
echo "✓ $CARS_COUNT car kit models → public/assets/kenney/cars/"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Done! Restart the gta-builder server then open any challenge → 3D View."
echo ""
echo "Key models installed:"
for f in roadStraight.glb roadRamp.glb overheadRound.glb lightPostLarge.glb; do
  [ -f "$DEST/racing/$f" ] && echo "  ✓ racing/$f" || echo "  ✗ racing/$f  (NOT FOUND)"
done
for f in race.glb hatchback-sports.glb sedan-sports.glb suv.glb; do
  [ -f "$DEST/cars/$f" ] && echo "  ✓ cars/$f" || echo "  ✗ cars/$f  (NOT FOUND)"
done
