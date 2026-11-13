#!/usr/bin/env bash
# Idempotently downloads the EMA Lightning ONNX voice (Turkish, Apache-2.0,
# huggingface.co/ozcancelik/ema-lightning-onnx) into the default `Config::model_dir`
# (or $TELLAL_MODEL_DIR) and prints the resulting file sizes.
set -euo pipefail

REPO="https://huggingface.co/ozcancelik/ema-lightning-onnx/resolve/main"
FILES=(meta.json text.onnx sound.onnx decoder.onnx LICENSE NOTICE)
MODEL_DIR="${TELLAL_MODEL_DIR:-$HOME/Library/Application Support/tellal/models/ema-lightning-onnx}"

mkdir -p "$MODEL_DIR"
for f in "${FILES[@]}"; do
  if [[ -s "$MODEL_DIR/$f" ]]; then
    continue
  fi
  echo "downloading $f"
  curl -fsSL --retry 3 -o "$MODEL_DIR/$f.part" "$REPO/$f"
  mv "$MODEL_DIR/$f.part" "$MODEL_DIR/$f"
done

echo "files:"
(cd "$MODEL_DIR" && du -h "${FILES[@]}")
echo "total: $(du -sh "$MODEL_DIR" | cut -f1)"
