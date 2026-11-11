#!/usr/bin/env bash
# Runs the panel on the demo fixture under a T3P_SELFTEST walkthrough and records ONLY its window
# (ScreenCaptureKit, tools/record-window.swift). Silent: T3P_OUTPUT_VOLUME=0; the voice is still
# synthesized and saved as WAVs in the run dir, so tools/build-media.py can rebuild the audio.
#
# Usage: tools/record-demo.sh <out-dir> [demo|shots] [dark|light]
# Output: <out-dir>/panel.mov (ProRes 4444 with alpha, 2x), <out-dir>/recorder.jsonl,
#         <out-dir>/logs/<run>/events.jsonl + audio/*.wav
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:?usage: tools/record-demo.sh <out-dir> [demo|shots] [dark|light]}"
mode="${2:-demo}"
appearance="${3:-dark}"
target="$repo/target/ui-dev"
panel="$target/release/t3-panel"
recorder="$target/record-window"

if [[ ! -x "$panel" ]]; then
  (cd "$repo" && CARGO_TARGET_DIR="$target" cargo build -p t3-panel --release)
fi
if [[ ! -x "$recorder" || "$repo/tools/record-window.swift" -nt "$recorder" ]]; then
  mkdir -p "$target"
  swiftc -O "$repo/tools/record-window.swift" -o "$recorder"
fi

rm -rf "$out"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
# The metrics drawer shows the run dir: keep it a neutral path (no user name), moved into
# <out-dir>/logs afterwards.
logs="/tmp/t3-panel-demo/$(basename "$out")"
rm -rf "$logs"
mkdir -p "$logs"

env_extra=()
if [[ "$appearance" == light ]]; then
  # A private copy of T3's theme settings with the appearance forced to light; the real T3
  # settings are only read.
  python3 "$repo/tools/light-app-data.py" "$out/t3-app-data"
  env_extra+=(T3P_T3_APP_DATA="$out/t3-app-data")
fi

# T3P_HOTKEY=off: never compete with a panel that is already running for the global shortcut.
env ${env_extra[@]+"${env_extra[@]}"} \
  T3P_DEMO="$repo/demo/fixture.json" T3P_OUTPUT_VOLUME=0 T3P_SELFTEST="$mode" T3P_HOTKEY=off \
  T3P_LOG_ROOT="$logs" "$panel" >"$out/panel.stdout" 2>"$out/panel.stderr" &
pid=$!

# The window number appears in `ui.ready` once the first frame rendered.
window=""
for _ in $(seq 1 100); do
  window="$(python3 - "$logs" <<'PY'
import json, pathlib, sys
for events in pathlib.Path(sys.argv[1]).glob("*/events.jsonl"):
    for line in events.read_text().splitlines():
        e = json.loads(line)
        if e["kind"] == "ui.ready" and e["in"].get("window_number"):
            print(e["in"]["window_number"])
            sys.exit()
PY
)"
  [[ -n "$window" ]] && break
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "panel exited before its window appeared; see $out/panel.stderr" >&2
    exit 1
  fi
  sleep 0.05
done
if [[ -z "$window" ]]; then
  kill "$pid"
  echo "no ui.ready window_number in $logs" >&2
  exit 1
fi

# 380 x 640 pt = the panel's width and maximum height (crates/app/src/panel.rs WIDTH, MAX_H).
"$recorder" --window "$window" --pid "$pid" --out "$out/panel.mov" --width 380 --height 640 --fps 60 \
  >"$out/recorder.jsonl" &
rec=$!
wait "$pid" || true
wait "$rec"
mv "$logs" "$out/logs"
rm -rf "$out/t3-app-data"
cat "$out/recorder.jsonl"
