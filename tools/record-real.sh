#!/usr/bin/env bash
# Records the real-T3 hero clip: ONE real agent run, started from a separate Tellal's new-task
# dock (TELLAL_SELFTEST=hero types the task and presses Gönder → the real `start_task`), with
# T3 Code's web UI in a dedicated BrowserOS window next to it. Both windows are captured
# window-only (tools/record-window.swift); tools/build-real.py turns the run into
# docs/media/clips/real-t3.mp4.
#
# Usage: tools/record-real.sh <out-dir> <project-id-or-path> [task text]
#
# Costs one agent run on the user's T3 (the project's default model). Silent: the voice is
# synthesized and saved as WAVs, never played. The recording panel only lists <project>.
# BrowserOS must be running (MCP on 127.0.0.1:9000). The BrowserOS window is brought to the
# front for the run (macOS switches to its Space: a window on another Space yields no frames);
# afterwards T3 Code's app is activated again, without touching any of its threads.
#
# Output: <out-dir>/tellal.mov, tellal.jsonl (recorder events), browser.mov, browser.jsonl,
#         browser.json (window geometry), run.json (thread, switch times), logs/<run>/events.jsonl
#         + audio/*.wav
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:?usage: tools/record-real.sh <out-dir> <project-id-or-path> [task text]}"
project="${2:?usage: tools/record-real.sh <out-dir> <project-id-or-path> [task text]}"
task="${3:-npm test kırmızı: hatayı bul, düzelt ve testleri yeniden çalıştır.}"
target="$repo/target/hero-dev"
panel="$target/release/tellal"
t3="$HOME/.t3/bin/t3"
# Run from inside a T3 agent thread, the t3 CLI would act as that thread (its MCP bearer and
# thread id): the new task would become its child and skip the project's default model.
unset T3CODE_THREAD_ID T3_ACP_MCP_AUTHORIZATION T3_ACP_MCP_ENDPOINT T3_ACP_MCP_ENTRYPOINT T3_ACP_MCP_NODE
t3_url="http://127.0.0.1:3773"
t3_app="T3 Code (Nightly)"
# T3 page viewport (points) in the BrowserOS window.
view_w=1100
view_h=812

(cd "$repo" && CARGO_TARGET_DIR="$target" cargo build -p tellal --release)
mkdir -p "$target"
for tool in record-window find-window; do
  if [[ ! -x "$target/$tool" || "$repo/tools/$tool.swift" -nt "$target/$tool" ]]; then
    swiftc -O "$repo/tools/$tool.swift" -o "$target/$tool"
  fi
done

rm -rf "$out"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
state="$out/browser.json"
browser() { python3 "$repo/tools/hero-browser.py" "$state" "$@"; }
# Neutral log path (no user name), moved into <out-dir>/logs afterwards.
logs="/tmp/tellal-hero/$(basename "$out")"
rm -rf "$logs"
mkdir -p "$logs"
now_ms() { python3 -c 'import time; print(int(time.time() * 1000))'; }

pairing=""
pid=""
rec_panel=""
rec_browser=""
switched_at=""
cleanup() {
  set +e
  [[ -n "$rec_browser" ]] && kill -INT "$rec_browser" 2>/dev/null && wait "$rec_browser"
  [[ -n "$rec_panel" ]] && kill -INT "$rec_panel" 2>/dev/null && wait "$rec_panel"
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null && wait "$pid"
  [[ -f "$state" ]] && browser close
  if [[ -n "$switched_at" ]]; then
    open -a "$t3_app"
    echo "{\"space_switch_ms\": $(( $(now_ms) - switched_at ))}" >>"$out/run.jsonl"
  fi
  # The one-time token (if unused) and the browser session it created.
  [[ -n "$pairing" ]] && "$t3" auth pairing revoke "$pairing" >/dev/null 2>&1
  "$t3" auth session list --json | python3 -c '
import json, sys
for s in json.load(sys.stdin):
    if (s.get("client") or {}).get("label") == "tellal-hero":
        print(s["sessionId"])' | while read -r session; do "$t3" auth session revoke "$session"; done
  [[ -d "$logs" ]] && mv "$logs" "$out/logs"
}
trap cleanup EXIT

# events.jsonl of the recording panel: first event of <kind> (optionally with in.<key> == <value>).
event() {
  python3 - "$logs" "$@" <<'PY'
import json, pathlib, sys
root, kind = sys.argv[1], sys.argv[2]
for events in pathlib.Path(root).glob("*/events.jsonl"):
    for line in events.read_text().splitlines():
        e = json.loads(line)
        if e["kind"] == kind and (len(sys.argv) < 5 or str(e["in"].get(sys.argv[3])) == sys.argv[4]):
            print(json.dumps(e, ensure_ascii=False))
            sys.exit()
PY
}

# 1. T3 web UI in a dedicated BrowserOS window, paired with a short-lived one-time link.
pair_json="$("$t3" auth pairing create --ttl 30m --label tellal-hero --base-url "$t3_url" --json)"
pairing="$(jq -r .id <<<"$pair_json")"
browser open "$(jq -r .pairUrl <<<"$pair_json")" >/dev/null
switched_at="$(now_ms)"
echo "{\"space_switch_start_ms\": $switched_at}" >>"$out/run.jsonl"
browser activate
sleep 1
browser layout "$view_w" "$view_h" >/dev/null
read -r bx by bw bh < <(jq -r '.bounds | "\(.left) \(.top) \(.width) \(.height)"' "$state")
window_b="$("$target/find-window" BrowserOS | jq -r --argjson x "$bx" --argjson y "$by" --argjson w "$bw" --argjson h "$bh" \
  'select(.x == $x and .y == $y and .width == $w and .height == $h and .onscreen) | .id' | head -1)"
[[ -n "$window_b" ]] || { echo "BrowserOS window not on screen at $bx,$by ${bw}x$bh" >&2; exit 1; }

# 2. The recording panel: only <project> listed, new tasks go there, silent, no global hotkeys.
env TELLAL_PROJECTS="$project" TELLAL_TASK_PROJECT="$project" TELLAL_OUTPUT_VOLUME=0 \
  TELLAL_HOTKEY=off TELLAL_STOP_HOTKEY=off TELLAL_SELFTEST=hero TELLAL_SELFTEST_TASK="$task" \
  TELLAL_LOG_ROOT="$logs" "$panel" >"$out/panel.stdout" 2>"$out/panel.stderr" &
pid=$!
window_p=""
for _ in $(seq 1 100); do
  window_p="$(event ui.ready | jq -r '.in.window_number // empty')"
  [[ -n "$window_p" ]] && break
  kill -0 "$pid" 2>/dev/null || { echo "panel exited early; see $out/panel.stderr" >&2; exit 1; }
  sleep 0.05
done
[[ -n "$window_p" ]] || { echo "no ui.ready window_number in $logs" >&2; exit 1; }

# 3. Both windows at once. 380 x 640 pt = the panel's width and maximum height.
"$target/record-window" --window "$window_p" --pid "$pid" --out "$out/tellal.mov" --width 380 --height 640 \
  --fps 30 >"$out/tellal.jsonl" &
rec_panel=$!
# Two ScreenCaptureKit streams starting at the same instant interrupt each other: one at a time.
for _ in $(seq 1 50); do
  grep -q '"start"' "$out/tellal.jsonl" && break
  sleep 0.1
done
"$target/record-window" --window "$window_b" --out "$out/browser.mov" --width "$bw" --height "$bh" \
  --fps 30 --scale 1.5 >"$out/browser.jsonl" &
rec_browser=$!

# 4. Once `start_task` answered, the browser shows the new thread.
started=""
for _ in $(seq 1 1200); do
  started="$(event task.start)"
  [[ -n "$started" ]] && break
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.1
done
[[ -n "$started" ]] || { echo "no task.start" >&2; exit 1; }
thread="$(jq -r '.out.thread // empty' <<<"$started")"
[[ -n "$thread" ]] || { echo "task.start failed: $(jq -c '{error, out}' <<<"$started")" >&2; exit 1; }
# From here on the agent runs (and costs): failures only warn, the panel keeps recording.
link=""
for _ in $(seq 1 30); do
  link="$("$t3" thread list --project "$project" --json | jq -r --arg id "$thread" '.[] | select(.threadId == $id) | .link')"
  [[ -n "$link" ]] && break
  sleep 0.5
done
# https://app.t3.codes/threads/<environment>/<thread> → the local web UI's /<environment>/<thread>.
url="$t3_url/${link#*/threads/}"
if [[ -z "$link" ]]; then
  echo "warning: thread $thread not in t3 thread list; the browser stays blank" >&2
elif ! browser thread "$url" "$(jq -r '.in.title' <<<"$started")" >"$out/thread-page.json"; then
  echo "warning: the browser could not show $url" >&2
fi
echo "{\"thread\": \"$thread\", \"url\": \"$url\", \"thread_shown_ms\": $(now_ms)}" >>"$out/run.jsonl"

# 5. The panel quits once the run ended and its narration went quiet.
wait "$pid" || true
pid=""
wait "$rec_panel" || true
rec_panel=""
sleep 1
kill -INT "$rec_browser"
wait "$rec_browser" || true
rec_browser=""
cat "$out/tellal.jsonl" "$out/browser.jsonl" "$out/run.jsonl"
