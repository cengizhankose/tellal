#!/usr/bin/env bash
# Idempotently downloads the Turkish Piper (VITS) model in sherpa-onnx format into the
# default `Config::model_dir` (or $T3P_MODEL_DIR) and prints the resulting file sizes.
set -euo pipefail

NAME="vits-piper-tr_TR-dfki-medium"
URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/${NAME}.tar.bz2"
MODEL_DIR="${T3P_MODEL_DIR:-$HOME/Library/Application Support/t3-panel/models/$NAME}"
PARENT="$(dirname "$MODEL_DIR")"

if [[ -f "$MODEL_DIR/tr_TR-dfki-medium.onnx" && -f "$MODEL_DIR/tokens.txt" && -d "$MODEL_DIR/espeak-ng-data" ]]; then
  echo "model already present: $MODEL_DIR"
else
  mkdir -p "$PARENT"
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  echo "downloading $URL"
  curl -fL --retry 3 -o "$tmp/$NAME.tar.bz2" "$URL"
  tar -xjf "$tmp/$NAME.tar.bz2" -C "$tmp"
  rm -rf "$MODEL_DIR"
  mv "$tmp/$NAME" "$MODEL_DIR"
  echo "installed: $MODEL_DIR"
fi

echo "files:"
(cd "$MODEL_DIR" && find . -maxdepth 1 -type f -exec du -h {} + | sort -k2)
echo "espeak-ng-data: $(du -sh "$MODEL_DIR/espeak-ng-data" | cut -f1)"
echo "total: $(du -sh "$MODEL_DIR" | cut -f1)"
