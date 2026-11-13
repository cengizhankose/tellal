#!/usr/bin/env bash
# Runs the panel on the demo fixture under a TELLAL_SELFTEST walkthrough and records ONLY its window
# (ScreenCaptureKit, tools/record-window.swift). Silent: TELLAL_OUTPUT_VOLUME=0; the voice is still
# synthesized and saved as WAVs in the run dir, so tools/build-media.py can rebuild the audio.
#
# Usage: tools/record-demo.sh <out-dir> [demo|shots|clip:<name>[:<thread>]] [dark|light]
#        tools/record-demo.sh <root-dir> all    # everything tools/build-media.py needs:
#                                               # <root-dir>/{demo-dark,demo-light,<clip>...}
# Output: <out-dir>/panel.mov (ProRes 4444 with alpha, 2x), <out-dir>/recorder.jsonl,
#         <out-dir>/logs/<run>/events.jsonl + audio/*.wav
# Binary: $CARGO_TARGET_DIR/release/tellal (default target/ui-dev), built if missing.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
usage="usage: tools/record-demo.sh <out-dir> [demo|shots|clip:<name>[:<thread>]] [dark|light] | <root-dir> all"
out="${1:?$usage}"
mode="${2:-demo}"
appearance="${3:-dark}"

# The README clips (crates/app/src/panel/selftest.rs `Clip`) and the fixture thread each one
# selects or tunes in to.
clips=(listen:t-checkout summary:t-batch attention:t-backfill reply new-task computer:t-notes theme:t-onboarding)

if [[ "$mode" == all ]]; then
  "$0" "$out/demo-dark" demo dark
  "$0" "$out/demo-light" demo light
  for clip in "${clips[@]}"; do
    "$0" "$out/${clip%%:*}" "clip:$clip" dark
  done
  exit
fi

target="${CARGO_TARGET_DIR:-target/ui-dev}"
[[ "$target" == /* ]] || target="$repo/$target"
panel="$target/release/tellal"
recorder="$target/record-window"

if [[ ! -x "$panel" ]]; then
  (cd "$repo" && CARGO_TARGET_DIR="$target" cargo build -p tellal --release)
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
logs="/tmp/tellal-demo/$(basename "$out")"
rm -rf "$logs"
mkdir -p "$logs"

# A private copy of T3's theme settings with the appearance forced, so every run looks the same;
# the real T3 settings are only read. `clip:theme` switches this copy while it runs.
app_data="$out/t3-app-data"
python3 "$repo/tools/t3-appearance.py" "$app_data" "$appearance" >/dev/null

# TELLAL_HOTKEY / TELLAL_STOP_HOTKEY off: never compete with a panel that is already running for
# the global shortcuts.
TELLAL_T3_APP_DATA="$app_data" TELLAL_DEMO="$repo/demo/fixture.json" TELLAL_OUTPUT_VOLUME=0 \
  TELLAL_SELFTEST="$mode" TELLAL_HOTKEY=off TELLAL_STOP_HOTKEY=off TELLAL_LOG_ROOT="$logs" \
  "$panel" >"$out/panel.stdout" 2>"$out/panel.stderr" &
pid=$!

# The window number appears in `ui.ready` once the first frame rendered.
window=""
for _ in $(seq 1 100); do
  window="$(python3 - "$logs" <<'PY'
import json, pathlib, sys
for events in pathlib.Path(sys.argv[1]).glob("*/events.jsonl"):
    for line in events.read_text().splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
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

if [[ "$mode" == clip:theme* ]]; then
  # The theme clip asks for each switch with a `t3_light` / `t3_dark` beat.
  python3 - "$logs" "$app_data" "$repo/tools/t3-appearance.py" "$pid" >"$out/appearance.log" <<'PY' &
import json, os, pathlib, subprocess, sys, time

logs, data, tool, pid = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
asks = {"t3_light": "light", "t3_dark": "dark"}
done = set()
while True:
    try:
        os.kill(pid, 0)
    except OSError:
        break
    for events in pathlib.Path(logs).glob("*/events.jsonl"):
        for line in events.read_text().splitlines():
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            beat = e["in"].get("beat") if e["kind"] == "ui.selftest_action" else None
            if beat in asks and beat not in done:
                done.add(beat)
                subprocess.run([sys.executable, tool, data, asks[beat]], check=True, capture_output=True)
                print(f"{time.time():.3f} {asks[beat]}", flush=True)
    time.sleep(0.1)
PY
fi

# 380 x 640 pt = the panel's width and maximum height (crates/app/src/panel.rs WIDTH, MAX_H).
"$recorder" --window "$window" --pid "$pid" --out "$out/panel.mov" --width 380 --height 640 --fps 60 \
  >"$out/recorder.jsonl" &
rec=$!
wait "$pid" || true
wait "$rec"
wait
mv "$logs" "$out/logs"
rm -rf "$app_data"
cat "$out/recorder.jsonl"
