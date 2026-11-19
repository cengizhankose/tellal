#!/usr/bin/env python3
"""Builds docs/media from the recordings `tools/record-demo.sh <root> all` makes.

Usage: tools/build-media.py <root-dir> [<media-dir>] [--lang tr|en]
       (default tr → docs/media; en → docs/media/en, from runs recorded with `record-demo.sh … en`)

<root-dir> holds one run dir per recording: `demo-dark`, `demo-light` (TELLAL_SELFTEST=demo) and
one per README clip (`listen`, `summary`, `attention`, `reply`, `new-task`, `computer`, `theme`;
TELLAL_SELFTEST=clip:<name>). Each run dir holds panel.mov (window-only ProRes 4444 with alpha,
2x), recorder.jsonl (wall-clock time of the first frame) and logs/<run>/events.jsonl +
audio/*.wav from a silent (TELLAL_OUTPUT_VOLUME=0) run.

* Audio: every `audio.play` WAV is placed at its playback start (event time - play_ms), with the
  pauses of `toggle_pause` clicks on its thread cut in, mixed to one 48 kHz track.
* Captions: English. Turkish runs (`--lang tr`): from demo/fixture.json (`en`, `dictation_en`,
  `ack_en`, the new task's `en`s, a waiting step's `en` for the spoken "senden cevap bekliyor."),
  shown while the Turkish line plays. English runs (`--lang en`): the spoken line itself. Dictated
  text is captioned while it sits in the dock.
* Clips: a window of each clip run around its beats (`ui.selftest_action`), a title on top, a
  badge (Turkish runs: the Turkish UI words it shows, translated), captions below.
* Stills: frames of the recordings at `ui.selftest_state` times.

A run that received real clicks or keys (the window sits where a live panel would) is refused.
Needs ffmpeg/ffprobe, swift (to draw the app mark), Python 3.10+, numpy and Pillow.
"""

import difflib
import json
import re
import subprocess
import sys
import tempfile
import wave
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "demo/fixture.json"
MARK_SVG = REPO / "crates/app/assets/logo.svg"
FPS = 30
RATE = 48000
NAME = "Tellal"
TAGLINE = "A voice for your T3 Code agents."
FOOTNOTES = {"tr": "Turkish voice · runs on your Mac · open source",
             "en": "English & Turkish voice · runs on your Mac · open source"}
# `--lang` (main): the recordings' UI and speech language.
LANG = "tr"
REPO_URL = "github.com/cengizhankose/tellal"

BG_TOP = (24, 24, 28)
BG_BOTTOM = (10, 10, 12)
TEXT = (237, 237, 239)
MUTED = (161, 161, 170)
ACCENT = (232, 62, 140)
KEY_FILL = (44, 44, 50)

# demo.mp4: what each walkthrough beat shows (cue line above the caption).
CUES = {
    "listen": "Unmute a thread: a catch-up line, then live narration",
    "attention": "Another agent is waiting for you",
    "reply": {"tr": "Cevapla (Reply): answer by voice", "en": "Reply: answer by voice"},
    "send": "Sent. The agent answers out loud",
    "pause": "Pause and resume mid-sentence",
    "compact": "Compact: just the player",
    "drawer": "Metrics: everything runs on this Mac",
    "computer": {"tr": "An agent is using the screen; Durdur (Stop) ends it",
                 "en": "An agent is using the screen; Stop ends it"},
    "new_task": "+ starts a new task by voice",
}
# crates/core/src/extract.rs (catch-up line of a running thread, Turkish)
STILL_PREFIX = "Hâlâ çalışıyor."
# crates/core/src/live.rs WAIT_TEXT
WAIT_TEXT = "senden cevap bekliyor."
# Inputs a scripted run may log (`task`: the panel selects the thread a new task created);
# anything else came from a real pointer or keyboard.
SCRIPTED_SOURCES = {"selftest", "task"}

# The window's capture canvas (tools/record-demo.sh: 380 x 640 pt at 2x).
CANVAS_W, CANVAS_H = 760, 1280

# Draws an SVG with AppKit (NSImage reads SVG): `swift <this> <in.svg> <out.png> <px>`.
SVG_TO_PNG = """
import AppKit
let a = CommandLine.arguments
let img = NSImage(contentsOf: URL(fileURLWithPath: a[1]))!
let px = Int(a[3])!
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
    samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
img.draw(in: NSRect(x: 0, y: 0, width: px, height: px))
NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: a[2]))
"""


def run(*cmd: str) -> str:
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def unix_ms(ts: str) -> float:
    return datetime.fromisoformat(ts.replace("Z", "+00:00")).timestamp() * 1000


def font(size: int, weight: str = "Regular") -> ImageFont.FreeTypeFont:
    f = ImageFont.truetype("/System/Library/Fonts/SFNS.ttf", size)
    f.set_variation_by_name(weight)
    return f


def wrap(draw: ImageDraw.ImageDraw, text: str, f: ImageFont.FreeTypeFont, width: int) -> list[str]:
    lines: list[str] = []
    for word in text.split():
        if lines and draw.textlength(f"{lines[-1]} {word}", font=f) <= width:
            lines[-1] = f"{lines[-1]} {word}"
        else:
            lines.append(word)
    return lines


def norm(s: str) -> str:
    s = s.replace("🔊", " ")
    s = re.sub(r"[`*_]", "", s)
    return re.sub(r"\s+", " ", s).strip().lower()


@dataclass
class Caption:
    start: float
    end: float
    speaker: str
    text: str
    # "speech" (a spoken line), "you" (dictated text), "cue" (what a demo beat shows) or "badge"
    # (a clip's translation of the UI words it shows / its shortcut).
    lane: str = "speech"
    # Badge only: keycaps drawn after the text.
    keys: list[str] = field(default_factory=list)


@dataclass
class Run:
    root: Path
    events: list[dict]
    t0: float  # wall-clock ms of the first video frame
    duration: float  # seconds

    @staticmethod
    def load(root: Path) -> "Run":
        rec = [json.loads(line) for line in (root / "recorder.jsonl").read_text().splitlines() if line.strip()]
        start = next(r for r in rec if r["event"] == "start")
        events_file = next((root / "logs").glob("*/events.jsonl"))
        events = [json.loads(line) for line in events_file.read_text().splitlines() if line.strip()]
        foreign = [e for e in events if e["kind"] in ("ui.click", "ui.dblclick", "ui.key")
                   and e["in"].get("source") not in SCRIPTED_SOURCES]
        if foreign:
            raise SystemExit(f"{root}: real input during the recording ({foreign[0]['kind']} "
                             f"{json.dumps(foreign[0]['in'])}); record it again")
        # Runs from before TELLAL_LANG existed were Turkish.
        lang = next((e["in"]["config"].get("lang", "tr") for e in events if e["kind"] == "app.start"), "tr")
        if lang != LANG:
            raise SystemExit(f"{root}: recorded in {lang!r}, building --lang {LANG}; record it with "
                             f"`tools/record-demo.sh … {LANG}`")
        duration = float(run("ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                             str(root / "panel.mov")))
        return Run(root, events, start["first_frame_unix_ms"], duration)

    def t(self, e: dict) -> float:
        """Video time (s) of an event."""
        return (unix_ms(e["ts"]) - self.t0) / 1000

    def kind(self, kind: str) -> list[dict]:
        return [e for e in self.events if e["kind"] == kind]

    def states(self, name: str) -> list[float]:
        return [self.t(e) for e in self.kind("ui.selftest_state") if e["in"]["state"] == name]

    def state(self, name: str) -> float:
        return self.states(name)[0]

    def action(self, beat: str) -> float:
        e = next(e for e in self.kind("ui.selftest_action") if e["in"]["beat"] == beat)
        return self.t(e)


class Fixture:
    def __init__(self) -> None:
        data = json.loads(FIXTURE.read_text())
        self.titles = {t["id"]: t["title"] for t in data["threads"]}
        self.replies = {t["id"]: t["reply"] for t in data["threads"] if t.get("reply")}
        self.new_task = data.get("new_task") or {}
        self.waits = {t["id"]: s["en"] for t in data["threads"] for s in t.get("script", [])
                      if s.get("waiting") and s.get("en")}
        self.lines: list[tuple[str, str]] = []
        for t in data["threads"]:
            for m in t.get("messages", []):
                if m["role"] == "assistant" and m.get("en"):
                    self.lines.append((norm(m["text"]), m["en"]))
            for s in t.get("script", []):
                if s.get("text") and s.get("en"):
                    self.lines.append((norm(s["text"]), s["en"]))
            if t.get("reply"):
                self.lines.append((norm(t["reply"]["ack"]), t["reply"]["ack_en"]))
        if self.new_task:
            self.lines.append((norm(self.new_task["ack"]), self.new_task["ack_en"]))
            for s in self.new_task.get("steps", []):
                if s.get("text") and s.get("en"):
                    self.lines.append((norm(s["text"]), s["en"]))

    def title(self, thread: str) -> str:
        """A fixture thread's title; the thread a new task created has the new task's."""
        return self.titles.get(thread) or self.new_task.get("title", "")

    def english(self, spoken: str, thread: str) -> str:
        """English runs: the spoken line. Turkish runs: the fixture's English for it (catch-up
        lines get `Still working.`)."""
        if LANG == "en":
            return re.sub(r"\s+", " ", re.sub(r"[`*_]", "", spoken.replace("🔊", " "))).strip()
        if norm(spoken).endswith(norm(WAIT_TEXT)) and thread in self.waits:
            return self.waits[thread]
        prefix = ""
        if spoken.startswith(STILL_PREFIX):
            spoken = spoken[len(STILL_PREFIX):]
            prefix = "Still working. "
        s = norm(spoken)

        def score(tr: str) -> float:
            return 1.0 if s and s in tr else difflib.SequenceMatcher(None, s, tr).ratio()

        tr, en = max(self.lines, key=lambda line: score(line[0]))
        if score(tr) < 0.6:
            raise SystemExit(f"no English caption for spoken line {spoken!r}")
        return prefix + en


def read_wav(path: Path) -> np.ndarray:
    with wave.open(str(path)) as w:
        assert w.getsampwidth() == 2 and w.getnchannels() == 1 and w.getframerate() == RATE, path
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768


def build_audio(r: Run, fx: Fixture, out: Path) -> list[Caption]:
    """Writes the run's mixed track; returns the spoken captions."""
    track = np.zeros(int((r.duration + 1) * RATE), dtype=np.float32)
    pauses = [e for e in r.kind("ui.click") if e["in"].get("action") == "toggle_pause"]
    captions = []
    for e in r.kind("audio.play"):
        o = e["out"] or {}
        if not e["ok"] or not o.get("wav") or not o.get("play_ms"):
            continue
        end = r.t(e)
        start = end - o["play_ms"] / 1000
        samples = read_wav(Path(o["wav"]) if Path(o["wav"]).is_file() else r.root / "logs" /
                           Path(o["wav"]).parent.parent.name / "audio" / Path(o["wav"]).name)
        # Played audio = play time minus paused time (a dropped utterance stops early).
        played = int(round((o["play_ms"] - (o.get("paused_ms") or 0)) / 1000 * RATE))
        samples = samples[:played]
        toggles = sorted(r.t(p) for p in pauses if p["in"].get("thread") == e["in"]["thread"] and start < r.t(p) < end)
        # (video time, sample offset) segments: play, hold during each pause, play on.
        cursor, offset = start, 0
        for i in range(0, len(toggles) - 1, 2):
            n = int((toggles[i] - cursor) * RATE)
            mix(track, samples[offset:offset + n], cursor)
            offset += n
            cursor = toggles[i + 1]
        mix(track, samples[offset:], cursor)
        thread = e["in"]["thread"]
        captions.append(Caption(start, end, fx.title(thread), fx.english(e["in"]["text"], thread)))
    peak = float(np.abs(track).max()) or 1.0
    # Peak-normalize to -1 dBFS.
    track = track * (0.89 / peak)
    with wave.open(str(out), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes((track.clip(-1, 1) * 32767).astype("<i2").tobytes())
    return captions


def mix(track: np.ndarray, samples: np.ndarray, at: float) -> None:
    i = int(round(at * RATE))
    if i < 0:
        samples, i = samples[-i:], 0
    samples = samples[: max(0, len(track) - i)]
    track[i:i + len(samples)] += samples


def dictation_captions(r: Run, fx: Fixture, lane: str = "you") -> list[Caption]:
    """The dictated reply / task, from the moment it filled the dock until just after Gönder."""
    out = []
    filled = r.states("dictated")
    for send in r.kind("ui.reply_send") + r.kind("ui.task_send"):
        sent = r.t(send)
        fill = max((t for t in filled if t < sent), default=sent - 2.0) - 0.6
        if send["kind"] == "ui.task_send":
            text = fx.new_task["dictation_en"]
        else:
            text = fx.replies[send["in"]["thread"]]["dictation_en"]
        out.append(Caption(fill, sent + 0.8, "You · dictated", text, lane))
    return out


def cue_spans(r: Run) -> list[Caption]:
    acts = r.kind("ui.selftest_action")
    out = []
    for i, e in enumerate(acts):
        beat = e["in"]["beat"]
        if beat in CUES:
            end = r.t(acts[i + 1]) if i + 1 < len(acts) else r.duration
            if beat in ("listen", "attention", "reply", "computer", "new_task"):
                # These stay up until the next cue.
                nxt = next((a for a in acts[i + 1:] if a["in"]["beat"] in CUES), None)
                end = r.t(nxt) if nxt else r.duration
            cue = CUES[beat] if isinstance(CUES[beat], str) else CUES[beat][LANG]
            out.append(Caption(r.t(e), max(end, r.t(e) + 2.0), "", cue, "cue"))
    return out


def settle(captions: list[Caption]) -> list[Caption]:
    """Per lane: sorted, each caption ends by the next one's start, a short tail after speech."""
    out = []
    for lane in {c.lane for c in captions}:
        same = sorted((c for c in captions if c.lane == lane), key=lambda c: c.start)
        if lane in ("speech", "you"):
            for a, b in zip(same, same[1:]):
                a.end = min(a.end + 0.4, b.start)
            same[-1].end += 0.4
        out += same
    return sorted(out, key=lambda c: c.start)


# ----------------------------------------------------------------------------- drawing


def background(w: int, h: int) -> Image.Image:
    grad = np.linspace(0, 1, h)[:, None, None]
    top = np.array(BG_TOP, dtype=np.float32)[None, None, :]
    bottom = np.array(BG_BOTTOM, dtype=np.float32)[None, None, :]
    rgb = (top * (1 - grad) + bottom * grad).repeat(w, axis=1)
    return Image.fromarray(rgb.astype(np.uint8), "RGB").convert("RGBA")


def shadow(img: Image.Image, panel: tuple[int, int, int, int], radius: int) -> None:
    x, y, w, h = panel
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    ImageDraw.Draw(layer).rounded_rectangle((x, y + 12, x + w, y + h + 12), radius=radius, fill=(0, 0, 0, 150))
    img.alpha_composite(layer.filter(ImageFilter.GaussianBlur(28)))


class Mark:
    """The app mark (crates/app/assets/logo.svg) in the accent colour, drawn once by AppKit."""

    def __init__(self, tmp: Path) -> None:
        svg = tmp / "mark.svg"
        svg.write_text(MARK_SVG.read_text().replace("currentColor", "#%02X%02X%02X" % ACCENT))
        script = tmp / "svg-to-png.swift"
        script.write_text(SVG_TO_PNG)
        png = tmp / "mark.png"
        subprocess.run(["swift", str(script), str(svg), str(png), "512"], check=True, capture_output=True)
        self.image = Image.open(png).convert("RGBA")

    def at(self, px: int) -> Image.Image:
        return self.image.resize((px, px), Image.LANCZOS)


def wordmark(img: Image.Image, mark: Mark, x: float, y: int, px: int, center: bool = False) -> int:
    """Mark + bold `Tellal` with the mark's top at `y`; returns the right edge."""
    d = ImageDraw.Draw(img)
    f = font(px, "Bold")
    gap = int(px * 0.3)
    width = px + gap + d.textlength(NAME, font=f)
    if center:
        x -= width / 2
    img.alpha_composite(mark.at(px), (int(x), y))
    d.text((x + px + gap, y + px * 0.5), NAME, font=f, fill=TEXT, anchor="lm")
    return int(x + width)


def caption_png(size: tuple[int, int], c: Caption, path: Path, text_px: int, align: str) -> None:
    img = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if c.lane == "badge":
        draw_badge(img, c)
    elif c.lane == "cue":
        f = font(int(text_px * 0.62), "Semibold")
        lines = wrap(d, c.text, f, size[0])
        y = 0
        for line in lines:
            x = (size[0] - d.textlength(line, font=f)) / 2 if align == "center" else 0
            d.text((x, y), line, font=f, fill=ACCENT)
            y += int(text_px * 0.8)
    else:
        px = text_px
        # At most three lines below the speaker.
        while len(wrap(d, c.text, font(px, "Semibold"), size[0])) > 3:
            px -= 2
        fs = font(int(text_px * 0.6), "Medium")
        ft = font(px, "Semibold")
        lines = wrap(d, c.text, ft, size[0])
        y = 0
        if c.speaker:
            x = (size[0] - d.textlength(c.speaker, font=fs)) / 2 if align == "center" else 0
            d.text((x, y), c.speaker, font=fs, fill=ACCENT if c.speaker.startswith("You") else MUTED)
            y += int(text_px * 0.95)
        for line in lines:
            x = (size[0] - d.textlength(line, font=ft)) / 2 if align == "center" else 0
            d.text((x, y), line, font=ft, fill=TEXT, stroke_width=2, stroke_fill=(0, 0, 0, 200))
            y += int(px * 1.25)
    img.save(path)


def draw_badge(img: Image.Image, c: Caption) -> None:
    """A centred pill: text, then keycaps."""
    d = ImageDraw.Draw(img)
    w, h = img.size
    ft = font(28, "Semibold")
    fk = font(30, "Semibold")
    key_w, key_h, gap = 52, 44, 8
    text_w = d.textlength(c.text, font=ft)
    keys_w = len(c.keys) * key_w + max(0, len(c.keys) - 1) * gap
    inner = text_w + (20 + keys_w if c.keys else 0)
    x0 = (w - inner) / 2
    d.rounded_rectangle((x0 - 26, 0, x0 + inner + 26, h - 1), radius=h // 2, fill=(32, 32, 38, 235),
                        outline=(70, 70, 80, 255), width=2)
    d.text((x0, h / 2), c.text, font=ft, fill=TEXT, anchor="lm")
    kx = x0 + text_w + 20
    for k in c.keys:
        ky = (h - key_h) / 2
        d.rounded_rectangle((kx, ky, kx + key_w, ky + key_h), radius=9, fill=KEY_FILL, outline=MUTED, width=2)
        d.text((kx + key_w / 2, ky + key_h / 2), k, font=fk, fill=TEXT, anchor="mm")
        kx += key_w + gap


@dataclass
class Layout:
    size: tuple[int, int]
    panel_xy: tuple[int, int]
    panel_scale: float
    # Caption lane → box (x, y, w, h); lanes without a box are not shown.
    boxes: dict[str, tuple[int, int, int, int]]
    text_px: int
    align: str
    # Rows of the 2x capture canvas that are shown (the panel never grows below them).
    crop_h: int = CANVAS_H


def compose_video(r: Run, captions: list[Caption], audio: Path, lay: Layout, base: Image.Image, out: Path,
                  tmp: Path, span: tuple[float, float] | None = None) -> None:
    """Panel over `base`, captions on top, the mixed track; `span` = the part of the run to use."""
    start, end = span or (0.0, r.duration)
    dur = end - start
    base_png = tmp / f"{out.stem}-base.png"
    base.convert("RGB").save(base_png)
    pw = int(CANVAS_W * lay.panel_scale) // 2 * 2
    ph = int(lay.crop_h * lay.panel_scale) // 2 * 2
    inputs = ["-loop", "1", "-framerate", str(FPS), "-i", str(base_png),
              "-ss", f"{start:.3f}", "-t", f"{dur:.3f}", "-i", str(r.root / "panel.mov")]
    filters = [f"[1:v]fps={FPS},crop={CANVAS_W}:{lay.crop_h}:0:0,scale={pw}:{ph}:flags=lanczos,format=rgba[p]",
               f"[0:v][p]overlay={lay.panel_xy[0]}:{lay.panel_xy[1]}:format=auto:shortest=1[v0]"]
    label = "v0"
    n = 2
    shown = [c for c in captions if c.lane in lay.boxes and c.end > start and c.start < end - 0.1]
    for i, c in enumerate(shown):
        box = lay.boxes[c.lane]
        png = tmp / f"{out.stem}-cap{i:02}.png"
        caption_png((box[2], box[3]), c, png, lay.text_px, lay.align)
        a, b = max(0.0, c.start - start), min(dur, c.end - start)
        # Each caption is a stream of its own span only (decoding every image for the whole
        # video makes a long walkthrough take many minutes).
        inputs += ["-loop", "1", "-framerate", str(FPS), "-t", f"{b - a:.3f}", "-i", str(png)]
        filters.append(f"[{n}:v]setpts=PTS-STARTPTS+{a:.3f}/TB[c{n}]")
        filters.append(f"[{label}][c{n}]overlay={box[0]}:{box[1]}:eof_action=pass[v{n}]")
        label = f"v{n}"
        n += 1
    filters.append(f"[{label}]format=yuv420p[vout]")
    fade_out = max(0.0, dur - 0.2)
    filters.append(f"[{n}:a]afade=t=in:d=0.05,afade=t=out:st={fade_out:.3f}:d=0.2[aout]")
    cmd = ["ffmpeg", "-v", "error", "-y", *inputs, "-ss", f"{start:.3f}", "-t", f"{dur:.3f}", "-i", str(audio),
           "-filter_complex", ";".join(filters), "-map", "[vout]", "-map", "[aout]", "-t", f"{dur:.3f}",
           "-r", str(FPS), "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-profile:v", "high",
           "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-ar", str(RATE), "-ac", "2",
           "-movflags", "+faststart", str(out)]
    subprocess.run(cmd, check=True)


def content_height(r: Run, start: float, end: float) -> int:
    """Tallest the panel gets in [start, end], in canvas pixels (alpha of frames at 4 fps)."""
    raw = subprocess.run(["ffmpeg", "-v", "error", "-ss", f"{start:.3f}", "-t", f"{end - start:.3f}",
                          "-i", str(r.root / "panel.mov"), "-vf", "fps=4,alphaextract,scale=190:320",
                          "-f", "rawvideo", "-pix_fmt", "gray", "-"], check=True, capture_output=True).stdout
    frames = np.frombuffer(raw, dtype=np.uint8).reshape(-1, 320, 190)
    rows = np.nonzero((frames > 16).any(axis=(0, 2)))[0]
    return min(CANVAS_H, int(rows.max() + 1) * 4 + 8) // 2 * 2 if len(rows) else CANVAS_H


def gif_from_panel(r: Run, start: float, end: float, out: Path, tmp: Path) -> None:
    """Silent GIF of [start, end] at the panel's 1x size; the frame rate drops until ≤ 8 MB."""
    palette = tmp / "palette.png"
    for fps in (12, 10, 8, 6):
        vf = f"trim={start:.3f}:{end:.3f},setpts=PTS-STARTPTS,fps={fps},scale=380:-2:flags=lanczos,format=rgba"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(r.root / "panel.mov"), "-vf",
                        f"{vf},palettegen=max_colors=128:reserve_transparent=1:stats_mode=diff", str(palette)],
                       check=True)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(r.root / "panel.mov"), "-i", str(palette),
                        "-lavfi", f"{vf}[x];[x][1:v]paletteuse=dither=none:diff_mode=rectangle:alpha_threshold=128",
                        "-loop", "0", str(out)], check=True)
        if out.stat().st_size <= 8_000_000:
            return
    raise SystemExit(f"{out} stays above 8 MB")


def gif_from_video(video: Path, out: Path, tmp: Path) -> None:
    """Silent 480 px wide GIF of a clip; the frame rate drops until ≤ 4 MB."""
    palette = tmp / "palette.png"
    for fps in (12, 10, 8, 6, 5):
        vf = f"fps={fps},scale=480:-2:flags=lanczos"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(video), "-vf",
                        f"{vf},palettegen=max_colors=128:stats_mode=diff", str(palette)], check=True)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(video), "-i", str(palette), "-lavfi",
                        f"{vf}[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
                        "-loop", "0", str(out)], check=True)
        if out.stat().st_size <= 4_000_000:
            return
    raise SystemExit(f"{out} stays above 4 MB")


def frame(r: Run, t: float, path: Path) -> Image.Image:
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{t:.3f}", "-i", str(r.root / "panel.mov"),
                    "-frames:v", "1", "-pix_fmt", "rgba", str(path)], check=True)
    img = Image.open(path).convert("RGBA")
    # The window background is the canvas at 92 % (over the desktop blur, which a window-only
    # capture does not contain): stills get an opaque body and keep the rounded corners' edges.
    rgba = np.asarray(img).copy()
    rgba[..., 3] = np.clip(rgba[..., 3].astype(np.float32) / 0.92, 0, 255).astype(np.uint8)
    img = Image.fromarray(rgba, "RGBA")
    # Crop to the panel (the canvas below it is transparent).
    box = img.getchannel("A").getbbox()
    return img.crop(box) if box else img


def still(r: Run, t: float, out: Path, tmp: Path) -> Image.Image:
    img = frame(r, t, tmp / f"{out.stem}.png")
    img.save(out, optimize=True)
    return img


def social_card(panel: Image.Image, mark: Mark, out: Path) -> None:
    w, h = 1280, 640
    img = background(w, h)
    scale = (h - 80) / panel.height
    p = panel.resize((int(panel.width * scale), int(panel.height * scale)), Image.LANCZOS)
    x, y = 90, (h - p.height) // 2
    shadow(img, (x, y, p.width, p.height), 20)
    img.alpha_composite(p, (x, y))
    d = ImageDraw.Draw(img)
    tx = x + p.width + 80
    wordmark(img, mark, tx, 150, 84)
    y2 = 280
    for line in wrap(d, TAGLINE, font(46, "Semibold"), w - tx - 60):
        d.text((tx, y2), line, font=font(46, "Semibold"), fill=TEXT)
        y2 += 58
    d.text((tx, y2 + 22), FOOTNOTES[LANG], font=font(26, "Medium"), fill=MUTED)
    d.text((tx, h - 110), REPO_URL, font=font(30, "Semibold"), fill=ACCENT)
    img.convert("RGB").save(out, optimize=True)


# ----------------------------------------------------------------------------- clips


@dataclass
class Clip:
    name: str
    title: str
    # Window: (beat, offset s) for its start and end; a spoken line is never cut at the start.
    start: tuple[str, float]
    end: tuple[str, float]
    # tellal-social.mp4: (beat, offset s, seconds) of the clip's highlight. Besides beats:
    # `speech2` / `speech_last` (start of the 2nd / last spoken line), `dictated` (dock filled).
    highlight: tuple[str, float, float]


CLIPS = [
    # Ends as the third line ends (the run mutes the next one).
    Clip("listen", "Unmute a thread. Hear every step.", ("select", -0.8), ("heard", 0.0), ("speech2", -0.4, 5.0)),
    Clip("summary", "A run ends. You hear the gist.", ("select", -0.8), ("done", 0.5), ("speech_last", -0.4, 6.5)),
    Clip("attention", "Know when an agent needs you.", ("attention", -2.0), ("done", 0.5), ("attention", -1.2, 5.0)),
    Clip("reply", "Reply by voice. Hear it confirm.", ("attention_jump", -0.6), ("done", 0.5),
         ("dictated", -0.6, 6.5)),
    Clip("new-task", "Start a new task by voice.", ("new_task", -1.0), ("heard", 0.0), ("dictated", -0.6, 5.5)),
    # Ends once the toast has shown (the run then waits out the quiet).
    Clip("computer", "An agent is driving your screen? Stop it.", ("computer", -2.0), ("computer_stop", 2.8),
         ("computer", -0.5, 6.0)),
    Clip("theme", "Follows your T3 theme.", ("tune", -0.8), ("done", 0.5), ("t3_light", -0.3, 5.5)),
]
SOCIAL_INTRO = 2.0
SOCIAL_OUTRO = 2.5


def badges(clip: Clip, r: Run, end: float) -> list[Caption]:
    """Turkish runs: the Turkish UI words a clip shows, in English. Both: the computer-use
    shortcut and the T3 switch."""
    a = r.action
    en = LANG == "en"
    if clip.name == "attention":
        # Once the card has slid in.
        return [] if en else [Caption(a("attention") + 0.3, end, "", "seni bekliyor = is waiting for you", "badge")]
    if clip.name == "reply":
        return [Caption(a("reply") - 0.2, a("send"), "", "Reply: you dictate" if en else "Cevapla = Reply: you dictate",
                        "badge"),
                Caption(a("send"), end, "", "Send" if en else "Gönder = Send", "badge")]
    if clip.name == "new-task":
        return [Caption(a("new_task") - 0.2, a("task_send"), "", "+ = New task: you dictate", "badge"),
                Caption(a("task_send"), end, "", "Send" if en else "Gönder = Send", "badge")]
    if clip.name == "computer":
        stop = a("computer_stop") - 0.4
        return [Caption(a("computer") - 0.2, stop, "", "An agent is using your screen" if en else
                        "Ajan ekranını kullanıyor = an agent is using your screen", "badge"),
                Caption(stop, end, "", "Stop. From any app:" if en else "Durdur = Stop. From any app:", "badge",
                        ["⌃", "⌥", "⎋"])]
    if clip.name == "theme":
        return [Caption(a("t3_light"), a("t3_dark"), "", "In T3: appearance → Light", "badge"),
                Caption(a("t3_dark"), end, "", "In T3: appearance → Dark", "badge")]
    return []


def before_muted(speech: list[Caption], end: float) -> float:
    """`end`, moved before a line the run muted right away (it started just before the end beat)."""
    for c in speech:
        if end - 0.5 < c.start < end:
            end = c.start
    return end


def clip_anchor(r: Run, speech: list[Caption], beat: str) -> float:
    if beat == "speech2":
        return speech[1].start
    if beat == "speech_last":
        return speech[-1].start
    if beat == "dictated":
        return r.state("dictated") - 0.6
    return r.action(beat)


def build_clip(clip: Clip, r: Run, fx: Fixture, mark: Mark, out: Path, tmp: Path) -> tuple[float, float]:
    """Writes clips/<name>.mp4; returns its highlight (start, seconds) in clip time."""
    audio = tmp / f"{clip.name}.wav"
    speech = build_audio(r, fx, audio)
    start = r.action(clip.start[0]) + clip.start[1]
    for c in speech:
        if c.start < start < c.end:
            start = c.start - 0.3
    end = before_muted(speech, r.action(clip.end[0]) + clip.end[1])
    if not 6 <= end - start <= 20:
        raise SystemExit(f"clip {clip.name}: {end - start:.1f} s (want 6–20 s)")
    captions = settle(speech + dictation_captions(r, fx, "speech") + badges(clip, r, end))

    w, h = 1080, 1350
    top, bottom = 205, 1065
    crop_h = content_height(r, start, end)
    scale = min((bottom - top) / crop_h, 980 / CANVAS_W)
    pw, ph = int(CANVAS_W * scale), int(crop_h * scale)
    lay = Layout(size=(w, h), panel_xy=((w - pw) // 2, top + (bottom - top - ph) // 2), panel_scale=scale,
                 boxes={"speech": (60, 1090, 960, 196), "badge": (60, 128, 960, 58)}, text_px=40,
                 align="center", crop_h=crop_h)
    base = background(w, h)
    d = ImageDraw.Draw(base)
    d.text((w / 2, 70), clip.title, font=font(52, "Bold"), fill=TEXT, anchor="mm")
    fm = font(24, "Medium")
    foot = f"{TAGLINE}  ·  {REPO_URL}"
    right = wordmark(base, mark, (w - 30 - 8 - 72 - d.textlength(foot, font=fm)) / 2, 1298, 30)
    d.text((right + 14, 1313), foot, font=fm, fill=MUTED, anchor="lm")
    compose_video(r, captions, audio, lay, base, out, tmp, (start, end))
    for c in captions:
        if c.end > start and c.start < end:
            print(f"  {clip.name:9} {max(0.0, c.start - start):5.2f}-{min(end, c.end) - start:5.2f} {c.lane:6} {c.text}")
    beat, offset, seconds = clip.highlight
    hl = max(0.0, clip_anchor(r, speech, beat) + offset - start)
    return hl, min(seconds, end - start - hl)


def social_sizzle(clips: list[tuple[Path, float, float]], mark: Mark, out: Path, tmp: Path) -> None:
    """tellal-social.mp4: a title card, each clip's highlight, an end card (4:5, ≤ 45 s)."""
    w, h = 1080, 1350
    intro = background(w, h)
    wordmark(intro, mark, w / 2, 470, 120, center=True)
    d = ImageDraw.Draw(intro)
    d.text((w / 2, 680), TAGLINE, font=font(50, "Semibold"), fill=TEXT, anchor="mm")
    d.text((w / 2, 760), FOOTNOTES[LANG], font=font(30, "Medium"), fill=MUTED, anchor="mm")
    outro = background(w, h)
    wordmark(outro, mark, w / 2, 430, 110, center=True)
    d = ImageDraw.Draw(outro)
    d.text((w / 2, 630), TAGLINE, font=font(50, "Semibold"), fill=TEXT, anchor="mm")
    d.text((w / 2, 720), "macOS · Rust + GPUI · builds from source", font=font(30, "Medium"), fill=MUTED,
           anchor="mm")
    d.text((w / 2, 840), REPO_URL, font=font(40, "Semibold"), fill=ACCENT, anchor="mm")
    cards = []
    for name, img in (("intro", intro), ("outro", outro)):
        p = tmp / f"social-{name}.png"
        img.convert("RGB").save(p)
        cards.append(p)
    segments: list[tuple[Path, float | None, float]] = [(cards[0], None, SOCIAL_INTRO), *clips,
                                                         (cards[1], None, SOCIAL_OUTRO)]
    total = sum(s for _, _, s in segments)
    if total > 45:
        raise SystemExit(f"tellal-social.mp4 would be {total:.1f} s (max 45)")
    inputs: list[str] = []
    parts: list[str] = []
    filters: list[str] = []
    for i, (path, start, dur) in enumerate(segments):
        fade = f"fade=t=in:d=0.2,fade=t=out:st={dur - 0.2:.3f}:d=0.2"
        if start is None:
            inputs += ["-loop", "1", "-framerate", str(FPS), "-t", f"{dur:.3f}", "-i", str(path)]
            filters.append(f"[{i}:v]format=yuv420p,setsar=1,{fade}[v{i}]")
            filters.append(f"anullsrc=r={RATE}:cl=stereo,atrim=0:{dur:.3f}[a{i}]")
        else:
            inputs += ["-ss", f"{start:.3f}", "-t", f"{dur:.3f}", "-i", str(path)]
            filters.append(f"[{i}:v]setpts=PTS-STARTPTS,setsar=1,{fade}[v{i}]")
            filters.append(f"[{i}:a]asetpts=PTS-STARTPTS,afade=t=in:d=0.12,"
                           f"afade=t=out:st={dur - 0.25:.3f}:d=0.25[a{i}]")
        parts.append(f"[v{i}][a{i}]")
    filters.append(f"{''.join(parts)}concat=n={len(segments)}:v=1:a=1[vout][aout]")
    subprocess.run(["ffmpeg", "-v", "error", "-y", *inputs, "-filter_complex", ";".join(filters),
                    "-map", "[vout]", "-map", "[aout]", "-r", str(FPS), "-c:v", "libx264", "-preset", "slow",
                    "-crf", "18", "-profile:v", "high", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k",
                    "-ar", str(RATE), "-ac", "2", "-movflags", "+faststart", str(out)], check=True)


# ----------------------------------------------------------------------------- main


def main() -> None:
    global LANG
    args = sys.argv[1:]
    if "--lang" in args:
        i = args.index("--lang")
        LANG = args[i + 1] if i + 1 < len(args) else ""
        del args[i:i + 2]
    if LANG not in FOOTNOTES or len(args) not in (1, 2):
        sys.exit(__doc__)
    root = Path(args[0])
    media = Path(args[1]) if len(args) == 2 else REPO / "docs/media" / ("en" if LANG == "en" else "")
    (media / "clips").mkdir(parents=True, exist_ok=True)
    dark, light = Run.load(root / "demo-dark"), Run.load(root / "demo-light")
    runs = {c.name: Run.load(root / c.name) for c in CLIPS}
    fx = Fixture()
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        mark = Mark(tmp)
        audio = tmp / "audio.wav"
        speech = build_audio(dark, fx, audio)
        captions = settle(speech + dictation_captions(dark, fx) + cue_spans(dark))

        # demo.mp4: 1920x1080, panel left; cue, spoken line and dictated text on the right.
        scale = 1000 / CANVAS_H
        land = Layout(size=(1920, 1080), panel_xy=(170, 40), panel_scale=scale,
                      boxes={"cue": (900, 380, 900, 80), "speech": (900, 470, 900, 300), "you": (900, 790, 900, 170)},
                      text_px=50, align="left")
        base = background(*land.size)
        d = ImageDraw.Draw(base)
        wordmark(base, mark, 900, 82, 72)
        d.text((900, 195), TAGLINE, font=font(40, "Semibold"), fill=MUTED)
        d.text((900, 1010), FOOTNOTES[LANG], font=font(28, "Medium"), fill=MUTED)
        # Ends as the new task's acknowledgement ends (the run mutes what follows).
        end = before_muted(speech, dark.action("heard"))
        compose_video(dark, captions, audio, land, base, media / "demo.mp4", tmp, (0.0, end))

        # demo.gif: silent, the panel at 1x, from the attention card to compact mode.
        gif_from_panel(dark, dark.action("attention") - 1.5, dark.action("expand") + 0.8, media / "demo.gif", tmp)

        # Stills (each a settled state of a recording).
        panel = still(dark, dark.state("attention"), media / "panel-dark.png", tmp)
        still(light, light.state("attention"), media / "panel-light.png", tmp)
        still(dark, dark.state("listen") + 2.0, media / "now-playing.png", tmp)
        still(dark, dark.state("dictated") + 0.4, media / "reply.png", tmp)
        still(dark, dark.state("drawer"), media / "drawer.png", tmp)
        still(dark, dark.state("compact"), media / "compact.png", tmp)
        comp = runs["computer"]
        still(comp, comp.state("list"), media / "computer-permissions.png", tmp)
        still(comp, comp.action("computer_stop") - 0.3, media / "computer.png", tmp)
        still(runs["new-task"], runs["new-task"].state("dictated") + 0.4, media / "new-task.png", tmp)
        social_card(panel, mark, media / "social-card.png")

        # The README clips (4:5) and their GIFs, then the sizzle cut from their highlights.
        highlights = []
        for clip in CLIPS:
            out = media / "clips" / f"{clip.name}.mp4"
            start, seconds = build_clip(clip, runs[clip.name], fx, mark, out, tmp)
            gif_from_video(out, out.with_suffix(".gif"), tmp)
            highlights.append((out, start, seconds))
        social_sizzle(highlights, mark, media / "tellal-social.mp4", tmp)
    for c in captions:
        print(f"demo {c.start:6.2f}-{c.end:6.2f}  {c.lane:6} {c.speaker}: {c.text}")


if __name__ == "__main__":
    main()
