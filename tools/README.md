# tools: recording the demo media

Everything in [`docs/media`](../docs/media) is made from the demo fixture
([`demo/fixture.json`](../demo/fixture.json)), never from real T3 threads, and only the panel's own
window is captured.

| File | What it does |
|---|---|
| `record-window.swift` | Records one window by `CGWindowID` with ScreenCaptureKit (`SCContentFilter(desktopIndependentWindow:)`): nothing else on screen can enter the frame. Up to 60 fps, ProRes 4444 with alpha, 2x. The panel changes height while it runs, so every frame is placed top-left in a fixed 380 × 640 pt canvas. Stops when the given pid exits. |
| `record-demo.sh` | Runs `tellal` with `TELLAL_DEMO=demo/fixture.json TELLAL_OUTPUT_VOLUME=0 TELLAL_HOTKEY=off TELLAL_SELFTEST=<mode>`, waits for `ui.ready` (window number) and records the window until the walkthrough quits. Silent: the voice is synthesized with normal timing and saved as WAVs, but not played. |
| `light-app-data.py` | Private copy of T3's theme settings (`Local Storage/leveldb`) with `t3code:theme-appearance-mode` set to `light`, for `TELLAL_T3_APP_DATA`. T3's own files are only read. `record-demo.sh … light` uses it and deletes the copy afterwards. |
| `build-media.py` | Builds every file in `docs/media` from a dark and a light recording (see below). |

## Re-recording

Needs macOS 14+, Xcode command line tools (`swiftc`), `ffmpeg`/`ffprobe`, Python 3.10+ with
`numpy` and `Pillow`, the voice model (`scripts/fetch-models.sh`), and Screen Recording permission
for the terminal that runs the scripts (System Settings → Privacy & Security → Screen Recording).

```sh
CARGO_TARGET_DIR=target/ui-dev cargo build -p tellal --release
tools/record-demo.sh target/ui-dev/media/dark demo dark     # ~50 s, window appears top right
tools/record-demo.sh target/ui-dev/media/light demo light
tools/build-media.py target/ui-dev/media/dark target/ui-dev/media/light   # → docs/media
```

A panel that is already running is left alone (the demo run turns its global shortcut off). The
demo window sits at the same spot as a normal panel; window-only capture makes overlap irrelevant.

### The walkthrough (`TELLAL_SELFTEST=demo`)

`crates/app/src/panel/selftest.rs` drives the UI through its normal code paths, one beat at a
time; each beat logs `ui.selftest_action` and, once settled, `ui.selftest_state`:

list → select the first running thread → unmute it (catch-up line, then live narration) → wait
until a thread waits for the user (attention card) → the card's jump → Cevapla (the fixture's
dictation fills the dock) → Gönder → the agent's spoken `Tamam, …` acknowledgement → pause /
resume → compact → back → metrics drawer → done. The run never takes the keyboard focus.

### What `build-media.py` makes

| File | From |
|---|---|
| `demo.mp4` | 1920 × 1080: dark recording, audio, English captions, cue lines |
| `demo-social.mp4` | 1080 × 1350 (4:5): same run, title and tagline, captions |
| `demo.gif` | silent, 380 px wide, attention card → compact; frame rate lowered until ≤ 8 MB |
| `panel-dark.png`, `panel-light.png` | `attention` state of the dark / light run |
| `now-playing.png`, `reply.png`, `drawer.png`, `compact.png` | `listen` (+2 s), `dictated`, `drawer`, `compact` states |
| `social-card.png` | 1280 × 640: `panel-dark.png` + title + repo URL |

- **Audio.** Each `audio.play` event carries the utterance text, its play time and its WAV
  (`<run>/audio/<seq>.wav`); the WAV is placed at `event time − play_ms`, with the gaps of
  `toggle_pause` clicks on the same thread cut in, mixed and normalized.
- **Captions.** The English text comes from the fixture (`en` on messages and script steps,
  `dictation_en`, `ack_en`), matched to the spoken Turkish line; a catch-up line
  (`Hâlâ çalışıyor. …`) is captioned `Still working. …`. A caption shows while its line plays; the
  dictated reply is captioned while it sits in the dock.
- **Sync.** `recorder.jsonl`'s `start` event has the wall-clock time of the first video frame;
  every event time is mapped onto the video with it.
- **Encoding.** H.264 High (yuv420p, CRF 18) + AAC 160 kb/s stereo 48 kHz, 30 fps, `+faststart`;
  fine for X and LinkedIn (both under 2:20).

Check the result before publishing: `ffprobe docs/media/demo.mp4`, and look at frames, e.g.
`ffmpeg -i docs/media/demo-social.mp4 -vf "fps=9/46,scale=540:-2,tile=3x3" -frames:v 1 grid.png`.
