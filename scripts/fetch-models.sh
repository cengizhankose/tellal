#!/usr/bin/env bash
# Downloads Tellal's voices into $TELLAL_MODELS_DIR (default: the app's
# ~/Library/Application Support/tellal/models), the same pinned files and SHA-256
# sums the app uses (crates/core/src/voices.rs):
#   tr  EMA Lightning (Turkish, Apache-2.0)        → ema-lightning-onnx/
#   en  Supertonic 2, voice M1 (English, OpenRAIL-M) → supertonic-2-en/
# Usage: scripts/fetch-models.sh [tr|en|all]   (default: all)
# Idempotent: a voice whose files are all present with the right size is skipped.
# Files land in <dir>.part and are moved into place only after every checksum matches.
set -euo pipefail

ROOT="${TELLAL_MODELS_DIR:-$HOME/Library/Application Support/tellal/models}"
EMA="https://huggingface.co/ozcancelik/ema-lightning-onnx/resolve/13c431db0356b2f7fafb1247cd823ec0d777c820"
SUPERTONE="https://huggingface.co/Supertone/supertonic-2/resolve/75e6727618a02f323c720cba9478152d4bc16ca4"
SHERPA="https://huggingface.co/csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06/resolve/5dec5897b86e2b293790eedb5e1f561a3295651f"

# name|url|size|sha256
TR_FILES=(
  "meta.json|$EMA/meta.json|357|76627201405723633f5319a208ed7dea63c42acbf3ca997671bccdca9d9186b2"
  "text.onnx|$EMA/text.onnx|4806400|ad5d4885b5d218f63d976a6e562334ea0194aec54847a6c6482d328e784089f0"
  "sound.onnx|$EMA/sound.onnx|18120655|958364217360cecaa92dc5ff2170d19f3c9f6995fd5e61241ea4068971a46a59"
  "decoder.onnx|$EMA/decoder.onnx|12031945|396c06a8e5711fd40bfd2c3f4f33851feb2ad07a86485833844e0ae8ff637b17"
  "LICENSE|$EMA/LICENSE|11356|5c9817c129b98e7bb966bca028c43c19107102ef8e03fe799bffb4354f4ef015"
  "NOTICE|$EMA/NOTICE|1170|498684f051de6523c86c08b6f182f2d69b65df873ed260b96dbe802be4df94d3"
)
EN_FILES=(
  "tts.json|$SUPERTONE/onnx/tts.json|8699|ee531d9af9b80438a2ed703e22155ee6c83b12595ab22fd3bb6de94c7502fe96"
  "unicode_indexer.json|$SUPERTONE/onnx/unicode_indexer.json|262196|b7662a73a0703f43b97c0f2e089f8e8325e26f5d841aca393b5a54c509c92df1"
  "M1.json|$SUPERTONE/voice_styles/M1.json|420510|a04c823cbda6dd1c7de131ec68fea83bbb70d7f29d61623304eb871e3b83b5a1"
  "duration_predictor.onnx|$SUPERTONE/onnx/duration_predictor.onnx|1521526|6d556b3691165c364be91dc0bd894656b5949f5acd2750d8ec2f954010845011"
  "text_encoder.onnx|$SUPERTONE/onnx/text_encoder.onnx|27431318|dd5f535ed629f7df86071043e15f541ce1b2ab7f1bdbce4c7892b307bca79fa3"
  "vector_estimator.int8.onnx|$SHERPA/vector_estimator.int8.onnx|40686657|9cce89801068dc998719fbc91777f99f3f4d16ee36592281f8703f32e305daa2"
  "vocoder.int8.onnx|$SHERPA/vocoder.int8.onnx|25977132|db99c36fb871fbfb128b903662995f2e22213e875f2434f319f8d10ee0f10e90"
  "LICENSE|$SUPERTONE/LICENSE|15007|0d944a9110fed9a9602d60e0423a272903e7bd21ab060490774efc77c2275e9f"
)

fetch() { # <dir> <files...>
  local dir="$ROOT/$1"; shift
  local ok=1 entry name url size sha
  for entry in "$@"; do
    IFS='|' read -r name url size sha <<<"$entry"
    [[ "$(stat -f %z "$dir/$name" 2>/dev/null || echo -1)" == "$size" ]] || ok=0
  done
  if ((ok)); then
    echo "$(basename "$dir"): installed"
  else
    local part="$ROOT/.$(basename "$dir").part"
    rm -rf "$part" && mkdir -p "$part"
    for entry in "$@"; do
      IFS='|' read -r name url size sha <<<"$entry"
      echo "downloading $(basename "$dir")/$name"
      curl -fsSL --proto '=https' --retry 3 -o "$part/$name" "$url"
      got="$(shasum -a 256 "$part/$name" | cut -d' ' -f1)"
      if [[ "$got" != "$sha" ]]; then
        echo "$name: sha256 mismatch (got $got, expected $sha)" >&2
        rm -rf "$part"
        exit 1
      fi
    done
    rm -rf "$dir.old"
    [[ -d "$dir" ]] && mv "$dir" "$dir.old"
    mv "$part" "$dir"
    rm -rf "$dir.old"
  fi
  (cd "$dir" && du -h -- *)
}

case "${1:-all}" in
  tr) fetch ema-lightning-onnx "${TR_FILES[@]}" ;;
  en) fetch supertonic-2-en "${EN_FILES[@]}" ;;
  all) fetch ema-lightning-onnx "${TR_FILES[@]}"; fetch supertonic-2-en "${EN_FILES[@]}" ;;
  *) echo "usage: $0 [tr|en|all]" >&2; exit 2 ;;
esac
echo "models: $ROOT ($(du -sh "$ROOT" | cut -f1))"
