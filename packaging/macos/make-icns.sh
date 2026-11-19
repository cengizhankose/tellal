#!/usr/bin/env bash
# Renders an SVG app icon into a macOS .icns: make-icns.sh <icon.svg> <out.icns>
# Uses sips (ImageMagick renders this SVG's gradients black) and iconutil, both part of macOS.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <icon.svg> <out.icns>" >&2
  exit 2
fi
svg="$1"
out="$2"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

master="$work/icon-1024.png"
sips -s format png -z 1024 1024 "$svg" --out "$master" >/dev/null

iconset="$work/icon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$master" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$master" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$iconset" -o "$out"
