# tools: recording the demo media

Everything in [`docs/media`](../docs/media) except the real-T3 hero clip (`clips/real-t3.*`) and the
app icon is made from the demo fixture ([`demo/fixture.json`](../demo/fixture.json)), never from
real T3 threads, and only the panel's own window is captured. The icon is drawn by hand in
`icon.svg`; `icon.png` is its 512 px render: `sips -s format png docs/media/icon.svg --out /tmp/icon.png`
(ImageMagick's built-in SVG renderer draws it black), then `magick /tmp/icon.png -resize 512x512 -strip docs/media/icon.png`.

| File | What it does |
|---|---|
| `record-window.swift` | Records one window by `CGWindowID` with ScreenCaptureKit (`SCContentFilter(desktopIndependentWindow:)`): nothing else on screen can enter the frame. Up to 60 fps, ProRes 4444 with alpha, 2x. The panel changes height while it runs, so every frame is placed top-left in a fixed 380 × 640 pt canvas. Stops when the given pid exits. |
| `record-demo.sh` | Runs `tellal` with `TELLAL_DEMO=demo/fixture.json TELLAL_OUTPUT_VOLUME=0 TELLAL_HOTKEY=off TELLAL_STOP_HOTKEY=off TELLAL_SELFTEST=<mode>`, waits for `ui.ready` (window number) and records the window until the run quits. Silent: the voice is synthesized with normal timing and saved as WAVs, but not played. `all` records every run `build-media.py` needs. |
| `t3-appearance.py` | Private copy of T3's theme settings (`Local Storage/leveldb`) with `t3code:theme-appearance-mode` forced to `light` or `dark`, for `TELLAL_T3_APP_DATA`; called again on the same copy it switches the appearance (the panel follows within ~2 s). T3's own files are only read. `record-demo.sh` makes one per run and deletes it afterwards. |
| `build-media.py` | Builds every file in `docs/media` (except `clips/real-t3.*` and `icon.*`) from those recordings (see below). |

## Re-recording

Needs macOS 14+, Xcode command line tools (`swiftc`, `swift`), `ffmpeg`/`ffprobe`, Python 3.10+
with `numpy` and `Pillow`, the voice model (`scripts/fetch-models.sh`), and Screen Recording
permission for the terminal that runs the scripts (System Settings → Privacy & Security → Screen
Recording).

```sh
export CARGO_TARGET_DIR=target/ui-dev
cargo build -p tellal --release
tools/record-demo.sh target/ui-dev/media all       # ~8 min: 2 walkthroughs + 7 clips, window top right
tools/build-media.py target/ui-dev/media           # → docs/media
```

A panel that is already running is left alone (the recorded run turns both global shortcuts off).
The recorded window sits where a normal panel would and ignores the mouse, so clicks and hover
reach whatever is below it; `build-media.py` still refuses a run that logged a real click or key.

One recording at a time (`build-media.py` needs them all, so rebuild afterwards):

| Run dir | Command |
|---|---|
| `demo-dark` | `tools/record-demo.sh target/ui-dev/media/demo-dark demo dark` |
| `demo-light` | `tools/record-demo.sh target/ui-dev/media/demo-light demo light` |
| `listen` | `tools/record-demo.sh target/ui-dev/media/listen clip:listen:t-checkout` |
| `summary` | `tools/record-demo.sh target/ui-dev/media/summary clip:summary:t-batch` |
| `attention` | `tools/record-demo.sh target/ui-dev/media/attention clip:attention:t-backfill` |
| `reply` | `tools/record-demo.sh target/ui-dev/media/reply clip:reply` |
| `new-task` | `tools/record-demo.sh target/ui-dev/media/new-task clip:new-task` |
| `computer` | `tools/record-demo.sh target/ui-dev/media/computer clip:computer:t-notes` |
| `theme` | `tools/record-demo.sh target/ui-dev/media/theme clip:theme:t-onboarding` |

### The walkthrough (`TELLAL_SELFTEST=demo`)

`crates/app/src/panel/selftest.rs` drives the UI through its normal code paths, one beat at a
time; each beat logs `ui.selftest_action` and, once settled, `ui.selftest_state`:

list → grant computer-use permissions → select the first running thread → unmute it (catch-up
line, then live narration) → wait until a thread waits for the user (attention card) → the card's
jump → Cevapla (the fixture's dictation fills the dock) → Gönder → the agent's spoken `Tamam, …`
acknowledgement → pause / resume → compact → back → metrics drawer → wait for a computer-use
session (banner) → Durdur → `+` (dictated task) → Gönder → the new thread selected and narrated →
done. The run never takes the keyboard focus.

### The clips (`TELLAL_SELFTEST=clip:<name>[:<thread>]`)

Same machinery, one feature per run. Every clip first grants the demo's computer-use permissions
and waits out the toast, so the story starts on a clean list. `<thread>` is the fixture thread the
clip selects or tunes in to (`record-demo.sh all` passes the ones above).

| Clip | Beats | Window in the clip |
|---|---|---|
| `listen` | select → unmute → catch-up + 2 live steps heard → mute | select − 0.8 s … the third line's end |
| `summary` | select the batch thread → unmute (catch-up) → its run ends → its `🔊` line spoken → quiet | select − 0.8 s … done |
| `attention` | tune in to the billing thread without selecting it → it waits for approval (attention card, spoken) → jump → quiet | card − 2 s … done |
| `reply` | attention card → jump → Cevapla (dictation) → Gönder → `Tamam, …` spoken → quiet | jump − 0.6 s … done |
| `new-task` | `+` (dictation) → Gönder → `Görev başladı`, the new thread selected → its spoken acknowledgement heard → mute | `+` − 1 s … the acknowledgement's end |
| `computer` | tune in to the notes thread → sessions start (banner as the real cua-driver shows it: client and last-action time) → Durdur (banner clears, toast) → quiet | banner − 2 s … Durdur + 2.8 s |
| `theme` | select + unmute → `t3_light` → the panel turns light → `t3_dark` → dark again → quiet | unmute − 0.8 s … done |

`clip:theme` asks for each switch with a `t3_light` / `t3_dark` beat; `record-demo.sh` watches the
run's `events.jsonl` and rewrites its private copy with `t3-appearance.py`, and the next beat
waits until the panel's theme has followed. The fixture's timeline (`at_s`) is shared by every
run: the batch summary lands at 15 s, the approval request at 24 s (after the `listen` and
`summary` clips end), computer-use sessions start at 46 s (after every clip that must not show
the banner).

To change a clip: its beats are `Clip::beats` in `selftest.rs`; its title, window, badges and
social highlight are `CLIPS` / `badges` in `build-media.py`; its spoken lines and English
captions are in the fixture.

### What `build-media.py` makes

| File | From |
|---|---|
| `clips/<clip>.mp4` | 1080 × 1350 (4:5), 6–20 s: the clip's window of its run, title on top, a badge translating the Turkish UI words it shows (⌃⌥⎋ keycaps in `computer`, the T3 switch in `theme`), English captions below, the Turkish voice |
| `clips/<clip>.gif` | the same clip, silent, 480 px wide; frame rate lowered until ≤ 4 MB |
| `tellal-social.mp4` | 1080 × 1350, ≤ 45 s: a title card, each clip's highlight (`CLIPS[].highlight`), an end card |
| `demo.mp4` | 1920 × 1080: the dark walkthrough, audio, English captions, cue lines |
| `demo.gif` | silent, 380 px wide, walkthrough from the attention card to compact; frame rate lowered until ≤ 8 MB |
| `panel-dark.png`, `panel-light.png` | `attention` state of the dark / light walkthrough |
| `now-playing.png`, `reply.png`, `drawer.png`, `compact.png` | walkthrough `listen` (+2 s), `dictated`, `drawer`, `compact` states |
| `computer-permissions.png`, `computer.png` | `computer` clip: before the grant; the banner just before Durdur |
| `new-task.png` | `new-task` clip: `dictated` state |
| `social-card.png` | 1280 × 640: `panel-dark.png` + mark, name, tagline, repo URL |

- **Audio.** Each `audio.play` event carries the utterance text, its play time and its WAV
  (`<run>/audio/<seq>.wav`); the WAV is placed at `event time − play_ms`, with the gaps of
  `toggle_pause` clicks on the same thread cut in, mixed and normalized. A clip cuts its window
  out of that track (never in the middle of a line at its start).
- **Captions.** The English text comes from the fixture (`en` on messages and script steps,
  `dictation_en`, `ack_en`, the new task's `en`s), matched to the spoken Turkish line; a catch-up
  line (`Hâlâ çalışıyor. …`) is captioned `Still working. …`, the spoken `senden cevap bekliyor.`
  with the waiting step's `en`. A caption shows while its line plays; dictated text is captioned
  while it sits in the dock.
- **Sync.** `recorder.jsonl`'s `start` event has the wall-clock time of the first video frame;
  every event time is mapped onto the video with it.
- **Mark.** The app mark is drawn from `crates/app/assets/logo.svg` with AppKit (`swift`).
- **Encoding.** H.264 High (yuv420p, CRF 18) + AAC 160 kb/s stereo 48 kHz, 30 fps, `+faststart`;
  fine for X and LinkedIn.

Check the result before publishing: `ffprobe` every file, look at frames, e.g.
`ffmpeg -i docs/media/clips/listen.mp4 -vf "fps=9/$(ffprobe -v error -show_entries format=duration -of csv=p=0 docs/media/clips/listen.mp4),scale=360:-2,tile=3x3" -frames:v 1 grid.png`,
and read the audio back: `ffmpeg -i docs/media/clips/summary.mp4 -ar 16000 -ac 1 s.wav && whisper-cli -m ~/.cache/whisper/ggml-large-v3-turbo-q8_0.bin -l tr s.wav`.

### The real-T3 hero clip (`docs/media/clips/real-t3.mp4`)

One real agent run on the user's T3, recorded as it happens: a separate Tellal (`TELLAL_SELFTEST=hero`) types the task into its new-task dock and presses Gönder (the real `start_task`), and T3 Code's web UI shows the new thread in a dedicated BrowserOS window.

| File | What it does |
|---|---|
| `record-real.sh <out-dir> <project-id> [task]` | Builds into `target/hero-dev`, mints a 30-minute T3 pairing link (label `tellal-hero`), opens a new BrowserOS window through its MCP server, pairs, collapses T3's sidebar, brings the window to the front (macOS switches to its Space: windows on another Space yield no frames), runs the panel with `TELLAL_PROJECTS`/`TELLAL_TASK_PROJECT=<project-id>` (silent, hotkeys off) and records both windows; once `task.start` returns, the browser opens the thread. The panel quits when the run ended and its narration went quiet. Always: closes the window, revokes the pairing token and the browser session, re-activates T3 Code's app. Costs one agent run. |
| `hero-browser.py` | The BrowserOS side (`open`, `activate`, `layout`, `thread`, `close`), only ever addressing its own window. |
| `find-window.swift` | Lists an app's windows (CGWindowID, bounds, on-screen) for `record-window`. |
| `build-real.py <out-dir>` | 1920 × 1080: T3 left (page only; BrowserOS chrome and T3's empty right panel cropped), Tellal right, title, English captions while each line plays, WAVs mixed at their `audio.play` times. Needs `<out-dir>/edit.json` written by hand after the take: English for every spoken line (`captions`), the T3 crop (`t3_crop`, points) and `redact` boxes for anything personal T3 itself prints (the take's final message ended with an absolute path). Cuts stretches without speech longer than 6 s; the 2026-11-13 take needed none (32.9 s). Poster: the spoken summary, mid-sentence. |

```sh
tools/record-real.sh target/hero-dev/real <project-id>   # ~45 s, one run
# write target/hero-dev/real/edit.json, then
tools/build-real.py target/hero-dev/real
```

Run it from a normal terminal: inside a T3 agent thread the `t3` CLI would act as that thread (the script unsets `T3CODE_THREAD_ID`/`T3_ACP_MCP_*`). Two ScreenCaptureKit streams started at the same instant interrupt each other, so the script starts the browser recorder after the panel's first frame.
