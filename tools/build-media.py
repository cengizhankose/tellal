#!/usr/bin/env python3
"""Builds docs/media from recordings made by tools/record-demo.sh.

Usage: tools/build-media.py <dark-run-dir> <light-run-dir> [<media-dir>]   (default docs/media)

Each run dir holds panel.mov (window-only ProRes 4444 with alpha, 2x), recorder.jsonl (wall-clock
time of the first frame) and logs/<run>/events.jsonl + audio/*.wav from a silent
(T3P_OUTPUT_VOLUME=0) T3P_SELFTEST=demo run.

* Audio: every `audio.play` WAV is placed at its playback start (event time - play_ms), with the
  pauses of `toggle_pause` clicks on its thread cut in, mixed to one 48 kHz track.
* Captions: English, from demo/fixture.json (`en`, `dictation_en`, `ack_en`), shown while the
  Turkish line plays; the dictated reply is captioned while its text sits in the dock. Short cue
  lines name each walkthrough beat (`ui.selftest_action`).
* Stills: frames of the recording at `ui.selftest_state` times.

Needs ffmpeg/ffprobe, Python 3.10+, numpy and Pillow.
"""

import difflib
import json
import re
import subprocess
import sys
import tempfile
import wave
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

REPO = Path(__file__).resolve().parent.parent
FIXTURE = REPO / "demo/fixture.json"
FPS = 30
RATE = 48000
TITLE = "T3 Panel — listen to your coding agents"
TAGLINE = "local Turkish voice · Rust + GPUI · open source"
REPO_URL = "github.com/cengizhankose/t3-panel"

BG_TOP = (24, 24, 28)
BG_BOTTOM = (10, 10, 12)
TEXT = (237, 237, 239)
MUTED = (161, 161, 170)
ACCENT = (232, 62, 140)

# What each walkthrough beat shows (cue line above the caption).
CUES = {
    "listen": "Unmute a thread: a catch-up line, then live narration",
    "attention": "Another agent is waiting for you",
    "reply": "Cevapla: reply by voice",
    "send": "Sent. The agent answers out loud",
    "pause": "Pause and resume mid-sentence",
    "compact": "Compact: just the player",
    "drawer": "Metrics: everything runs on this Mac",
}
STILL_PREFIX = "Hâlâ çalışıyor."


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
    # "speech" (a spoken line), "you" (the dictated reply) or "cue" (what the beat shows).
    lane: str = "speech"


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
        duration = float(run("ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0",
                             str(root / "panel.mov")))
        return Run(root, events, start["first_frame_unix_ms"], duration)

    def t(self, e: dict) -> float:
        """Video time (s) of an event."""
        return (unix_ms(e["ts"]) - self.t0) / 1000

    def kind(self, kind: str) -> list[dict]:
        return [e for e in self.events if e["kind"] == kind]

    def state(self, name: str) -> float:
        e = next(e for e in self.kind("ui.selftest_state") if e["in"]["state"] == name)
        return self.t(e)

    def action(self, beat: str) -> float:
        e = next(e for e in self.kind("ui.selftest_action") if e["in"]["beat"] == beat)
        return self.t(e)


class Fixture:
    def __init__(self) -> None:
        data = json.loads(FIXTURE.read_text())
        self.titles = {t["id"]: t["title"] for t in data["threads"]}
        self.replies = {t["id"]: t["reply"] for t in data["threads"] if t.get("reply")}
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

    def english(self, spoken: str) -> str:
        """The fixture's English for a spoken Turkish line (catch-up lines get `Still working.`)."""
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
    """Writes the mixed track; returns the spoken captions."""
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
        captions.append(Caption(start, end, fx.titles.get(e["in"]["thread"], ""), fx.english(e["in"]["text"])))
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


def dictation_caption(r: Run, fx: Fixture) -> Caption:
    send = next(e for e in r.kind("ui.reply_send"))
    thread = send["in"]["thread"]
    filled = r.state("dictated") - 0.6
    return Caption(filled, r.t(send) + 0.8, "You · dictated", fx.replies[thread]["dictation_en"], "you")


def cue_spans(r: Run) -> list[Caption]:
    acts = r.kind("ui.selftest_action")
    out = []
    for i, e in enumerate(acts):
        beat = e["in"]["beat"]
        if beat in CUES:
            end = r.t(acts[i + 1]) if i + 1 < len(acts) else r.duration
            if beat in ("listen", "attention", "reply"):
                # These stay up until the next cue.
                nxt = next((a for a in acts[i + 1:] if a["in"]["beat"] in CUES), None)
                end = r.t(nxt) if nxt else r.duration
            out.append(Caption(r.t(e), max(end, r.t(e) + 2.0), "", CUES[beat], "cue"))
    return out


def settle(captions: list[Caption]) -> list[Caption]:
    """Per lane: sorted, each caption ends by the next one's start, a short tail after speech."""
    out = []
    for lane in {c.lane for c in captions}:
        same = sorted((c for c in captions if c.lane == lane), key=lambda c: c.start)
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


def caption_png(size: tuple[int, int], c: Caption, path: Path, text_px: int, align: str) -> None:
    img = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if c.lane == "cue":
        f = font(int(text_px * 0.62), "Semibold")
        lines = wrap(d, c.text, f, size[0])
        y = 0
        for line in lines:
            x = (size[0] - d.textlength(line, font=f)) / 2 if align == "center" else 0
            d.text((x, y), line, font=f, fill=ACCENT)
            y += int(text_px * 0.8)
    else:
        fs = font(int(text_px * 0.6), "Medium")
        ft = font(text_px, "Semibold")
        lines = wrap(d, c.text, ft, size[0])
        y = 0
        if c.speaker:
            x = (size[0] - d.textlength(c.speaker, font=fs)) / 2 if align == "center" else 0
            d.text((x, y), c.speaker, font=fs, fill=MUTED)
            y += int(text_px * 0.95)
        for line in lines:
            x = (size[0] - d.textlength(line, font=ft)) / 2 if align == "center" else 0
            d.text((x, y), line, font=ft, fill=TEXT, stroke_width=2, stroke_fill=(0, 0, 0, 200))
            y += int(text_px * 1.25)
    img.save(path)


@dataclass
class Layout:
    size: tuple[int, int]
    panel_xy: tuple[int, int]
    panel_scale: float
    # Caption lane → box (x, y, w, h); lanes without a box are not shown.
    boxes: dict[str, tuple[int, int, int, int]]
    text_px: int
    align: str


def compose_video(r: Run, captions: list[Caption], audio: Path, lay: Layout, base: Image.Image, out: Path,
                  tmp: Path) -> None:
    base_png = tmp / f"{out.stem}-base.png"
    base.convert("RGB").save(base_png)
    pw = int(760 * lay.panel_scale) // 2 * 2
    ph = int(1280 * lay.panel_scale) // 2 * 2
    inputs = ["-loop", "1", "-framerate", str(FPS), "-i", str(base_png), "-i", str(r.root / "panel.mov")]
    filters = [f"[1:v]fps={FPS},scale={pw}:{ph}:flags=lanczos,format=rgba[p]",
               f"[0:v][p]overlay={lay.panel_xy[0]}:{lay.panel_xy[1]}:format=auto:shortest=1[v0]"]
    label = "v0"
    n = 2
    for i, c in enumerate(c for c in captions if c.lane in lay.boxes):
        box = lay.boxes[c.lane]
        png = tmp / f"{out.stem}-cap{i:02}.png"
        caption_png((box[2], box[3]), c, png, lay.text_px, lay.align)
        inputs += ["-loop", "1", "-framerate", str(FPS), "-i", str(png)]
        filters.append(f"[{label}][{n}:v]overlay={box[0]}:{box[1]}:enable='between(t,{c.start:.3f},{c.end:.3f})'"
                       f":shortest=1[v{n}]")
        label = f"v{n}"
        n += 1
    filters.append(f"[{label}]format=yuv420p[vout]")
    cmd = ["ffmpeg", "-v", "error", "-y", *inputs, "-i", str(audio), "-filter_complex", ";".join(filters),
           "-map", "[vout]", "-map", f"{n}:a", "-t", f"{r.duration:.3f}", "-r", str(FPS),
           "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-profile:v", "high", "-pix_fmt", "yuv420p",
           "-c:a", "aac", "-b:a", "160k", "-ar", str(RATE), "-ac", "2", "-movflags", "+faststart", str(out)]
    subprocess.run(cmd, check=True)


def gif(r: Run, start: float, end: float, out: Path, tmp: Path) -> None:
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


def social_card(panel: Image.Image, out: Path) -> None:
    w, h = 1280, 640
    img = background(w, h)
    scale = (h - 80) / panel.height
    p = panel.resize((int(panel.width * scale), int(panel.height * scale)), Image.LANCZOS)
    x, y = 90, (h - p.height) // 2
    shadow(img, (x, y, p.width, p.height), 20)
    img.alpha_composite(p, (x, y))
    d = ImageDraw.Draw(img)
    tx = x + p.width + 80
    d.text((tx, 170), "T3 Panel", font=font(84, "Bold"), fill=TEXT)
    y2 = 280
    for line in wrap(d, "Listen to your coding agents.", font(44, "Semibold"), w - tx - 60):
        d.text((tx, y2), line, font=font(44, "Semibold"), fill=TEXT)
        y2 += 56
    d.text((tx, y2 + 20), TAGLINE, font=font(28, "Medium"), fill=MUTED)
    d.text((tx, h - 110), REPO_URL, font=font(30, "Semibold"), fill=ACCENT)
    img.convert("RGB").save(out, optimize=True)


def main() -> None:
    if len(sys.argv) not in (3, 4):
        sys.exit(__doc__)
    dark, light = Run.load(Path(sys.argv[1])), Run.load(Path(sys.argv[2]))
    media = Path(sys.argv[3]) if len(sys.argv) == 4 else REPO / "docs/media"
    media.mkdir(parents=True, exist_ok=True)
    fx = Fixture()
    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        audio = tmp / "audio.wav"
        captions = settle(build_audio(dark, fx, audio) + [dictation_caption(dark, fx)] + cue_spans(dark))

        # demo.mp4: 1920x1080, panel left; cue, spoken line and dictated reply on the right.
        scale = 1000 / 1280
        land = Layout(size=(1920, 1080), panel_xy=(170, 40), panel_scale=scale,
                      boxes={"cue": (900, 380, 900, 80), "speech": (900, 470, 900, 300), "you": (900, 790, 900, 170)},
                      text_px=50, align="left")
        base = background(*land.size)
        d = ImageDraw.Draw(base)
        d.text((900, 90), "T3 Panel", font=font(64, "Bold"), fill=TEXT)
        d.text((900, 175), "Listen to your coding agents", font=font(40, "Semibold"), fill=MUTED)
        d.text((900, 1010), TAGLINE, font=font(28, "Medium"), fill=MUTED)
        compose_video(dark, captions, audio, land, base, media / "demo.mp4", tmp)

        # demo-social.mp4: 1080x1350 (4:5), panel centered, title top, captions + tagline bottom.
        scale = 922 / 1280
        pw = int(760 * scale)
        social = Layout(size=(1080, 1350), panel_xy=((1080 - pw) // 2, 125), panel_scale=scale,
                        boxes={"speech": (60, 1062, 960, 120), "you": (60, 1188, 960, 85)}, text_px=32,
                        align="center")
        base = background(*social.size)
        d = ImageDraw.Draw(base)
        ft = font(46, "Bold")
        d.text(((1080 - d.textlength(TITLE, font=ft)) / 2, 55), TITLE, font=ft, fill=TEXT)
        ftag = font(30, "Medium")
        d.text(((1080 - d.textlength(TAGLINE, font=ftag)) / 2, 1288), TAGLINE, font=ftag, fill=MUTED)
        compose_video(dark, captions, audio, social, base, media / "demo-social.mp4", tmp)

        # demo.gif: silent, the panel at 1x, from the attention card to compact mode.
        gif(dark, dark.action("attention") - 1.5, dark.action("expand") + 0.8, media / "demo.gif", tmp)

        # Stills (each a settled state of the walkthrough).
        panel = still(dark, dark.state("attention"), media / "panel-dark.png", tmp)
        still(light, light.state("attention"), media / "panel-light.png", tmp)
        still(dark, dark.state("listen") + 2.0, media / "now-playing.png", tmp)
        still(dark, dark.state("dictated") + 0.4, media / "reply.png", tmp)
        still(dark, dark.state("drawer"), media / "drawer.png", tmp)
        still(dark, dark.state("compact"), media / "compact.png", tmp)
        social_card(panel, media / "social-card.png")
    for c in captions:
        print(f"{c.start:6.2f}-{c.end:6.2f}  {c.speaker}: {c.text}")


if __name__ == "__main__":
    main()
