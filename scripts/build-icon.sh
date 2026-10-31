#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
resources="apps/railway-forwarder/resources/icons"
icon_work="$(mktemp -d "${TMPDIR:-/tmp}/railway-forwarder-icon.XXXXXX")"
trap 'rm -rf "$icon_work"' EXIT
iconset="$icon_work/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips --resampleHeightWidth "$size" "$size" "$resources/AppIcon.png" \
    --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina_size=$((size * 2))
  sips --resampleHeightWidth "$retina_size" "$retina_size" "$resources/AppIcon.png" \
    --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil --convert icns "$iconset" --output "$resources/AppIcon.icns"
printf 'Built %s/AppIcon.icns\n' "$resources"
