#!/usr/bin/env python3
"""Builds docs/media/clips/real-t3.mp4 (+ real-t3.png) from a run made by tools/record-real.sh.

Usage: tools/build-real.py <run-dir> [<clips-dir>]   (default docs/media/clips)

The run dir holds tellal.mov / tellal.jsonl (the recording panel, window-only, 2x),
browser.mov / browser.jsonl (the BrowserOS window with T3's web UI, 1.5x), browser.json (its
geometry), run.jsonl, logs/<run>/events.jsonl + audio/*.wav, and edit.json, written by hand
after looking at the run:

    {"captions": {"<spoken Turkish text>": "<English>", ...},    every spoken line (required)
     "t3_crop": [x, y, w, h],          part of T3's page viewport to show, in points (optional)
     "redact": [{"x": .., "y": .., "w": .., "h": .., "from": s, "to": s}, ...]}   (optional)

`redact` boxes are filled with T3's background in browser.mov pixels over browser.mov time
(seconds; `to` may be null = until the end), for personal details T3 itself prints (e.g. an
absolute path in the agent's final message).

* Layout: 1920 x 1080; T3 left (only the page: tabs, toolbar and BrowserOS's side panel are
  cropped away), Tellal right, a small title, English captions under Tellal while each line
  plays. Until the new thread is open, the T3 pane is the blank page the window was parked on.
* Audio: every `audio.play` WAV at its playback start (event time - play_ms), as in
  tools/build-media.py.
* Cuts: the clip keeps the typed task and Gönder, the thread appearing in T3, and every spoken
  line with a lead-in and tail; stretches without a spoken line longer than MAX_GAP are cut.
  Every kept frame is the recording itself.

Needs ffmpeg/ffprobe, Python 3.10+, numpy and Pillow.
"""

import json
import subprocess
import sys
import tempfile
import wave
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parent.parent
FPS = 30
RATE = 48000
W, H = 1920, 1080
TITLE = "Real T3 Code run · Tellal narrates"
BG = (12, 12, 14)
TEXT = (237, 237, 239)
MUTED = (161, 161, 170)
ACCENT = (232, 62, 140)
BROWSER_SCALE = 1.5  # record-real.sh: --scale 1.5
PANEL_W, PANEL_H = 380, 640  # record-real.sh: the panel's canvas, points (2x in tellal.mov)
# T3 pane: top-left corner and height (the width follows t3_crop's aspect).
T3_X, T3_Y, T3_H = 60, 96, 956
# Tellal pane (the canvas scaled to this width) and the caption box under it.
TELLAL_X, TELLAL_Y, TELLAL_W = 790, 96, 640
CAPTION_X, CAPTION_Y, CAPTION_W = 790, 720, 1070
# Cut padding around spoken lines (s), the longest stretch without one kept inside a segment,
# and the hold after the last line.
LEAD, TAIL, MAX_GAP, END_HOLD = 0.8, 0.7, 6.0, 2.0


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


def jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def duration(path: Path) -> float:
    return float(run("ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)))


@dataclass
class Line:
    start: float  # wall clock, s
    end: float
    text: str
    english: str
    wav: Path
    samples: int


class Run:
    def __init__(self, root: Path) -> None:
        self.root = root
        run_dir = next(p for p in (root / "logs").iterdir() if p.is_dir() and not p.is_symlink())
        self.events = jsonl(run_dir / "events.jsonl")
        self.audio_dir = run_dir / "audio"

        def start(name: str) -> float:
            return next(r for r in jsonl(root / f"{name}.jsonl") if r["event"] == "start")["first_frame_unix_ms"] / 1000

        self.t0_tellal, self.t0_browser = start("tellal"), start("browser")
        self.end_tellal = self.t0_tellal + duration(root / "tellal.mov")
        self.end_browser = self.t0_browser + duration(root / "browser.mov")
        self.geometry = json.loads((root / "browser.json").read_text())
        self.info = {k: v for r in jsonl(root / "run.jsonl") for k, v in r.items()}
        self.edit = json.loads((root / "edit.json").read_text())

    def t(self, e: dict) -> float:
        return unix_ms(e["ts"]) / 1000

    def kind(self, kind: str) -> list[dict]:
        return [e for e in self.events if e["kind"] == kind]

    def action(self, beat: str) -> float:
        return self.t(next(e for e in self.kind("ui.selftest_action") if e["in"]["beat"] == beat))

    def lines(self) -> list[Line]:
        captions = self.edit["captions"]
        out = []
        for e in self.kind("audio.play"):
            o = e["out"] or {}
            if not e["ok"] or not o.get("wav") or not o.get("play_ms"):
                continue
            text = e["in"]["text"]
            if text not in captions:
                raise SystemExit(f"edit.json has no English caption for {text!r}")
            end = self.t(e)
            played = int(round((o["play_ms"] - (o.get("paused_ms") or 0)) / 1000 * RATE))
            out.append(Line(end - o["play_ms"] / 1000, end, text, captions[text],
                            self.audio_dir / Path(o["wav"]).name, played))
        return out


def segments(r: Run, lines: list[Line]) -> list[tuple[float, float]]:
    """Wall-clock spans to keep, in order, merged where the gap is at most MAX_GAP."""
    spans = [(r.action("new_task") - 1.2, r.action("task_send") + 1.6)]
    shown = r.info.get("thread_shown_ms")
    if shown:
        spans.append((shown / 1000 - 0.3, shown / 1000 + 2.2))
    spans += [(ln.start - LEAD, ln.end + TAIL) for ln in lines]
    if lines:
        spans.append((lines[-1].end, lines[-1].end + END_HOLD))
    lo = max(r.t0_tellal, r.t0_browser)
    hi = min(r.end_tellal, r.end_browser)
    spans = sorted((max(a, lo), min(b, hi)) for a, b in spans if b > lo and a < hi)
    merged: list[list[float]] = []
    for a, b in spans:
        if merged and a <= merged[-1][1] + MAX_GAP:
            merged[-1][1] = max(merged[-1][1], b)
        else:
            merged.append([a, b])
    return [(a, b) for a, b in merged]


def cut_time(t: float, spans: list[tuple[float, float]]) -> float | None:
    """Clip time of wall-clock `t` (None if `t` was cut)."""
    offset = 0.0
    for a, b in spans:
        if a <= t <= b:
            return offset + t - a
        offset += b - a
    return None


def read_wav(path: Path) -> np.ndarray:
    with wave.open(str(path)) as w:
        assert w.getsampwidth() == 2 and w.getnchannels() == 1 and w.getframerate() == RATE, path
        return np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768


def mix_audio(lines: list[Line], spans: list[tuple[float, float]], out: Path) -> None:
    """One track over the clip timeline: each line at its playback start."""
    total = sum(b - a for a, b in spans)
    track = np.zeros(int((total + 1) * RATE), dtype=np.float32)
    for ln in lines:
        at = cut_time(ln.start, spans)
        if at is None:
            raise SystemExit(f"line {ln.text!r} starts inside a cut")
        i = int(round(at * RATE))
        s = read_wav(ln.wav)[: ln.samples]
        s = s[: max(0, len(track) - i)]
        track[i:i + len(s)] += s
    track *= 0.89 / (float(np.abs(track).max()) or 1.0)
    with wave.open(str(out), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes((track.clip(-1, 1) * 32767).astype("<i2").tobytes())


def background(t3_w: int) -> Image.Image:
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    d.text((T3_X, 26), TITLE, font=font(34, "Semibold"), fill=TEXT)
    label = font(22, "Medium")
    d.text((T3_X + t3_w - d.textlength("T3 Code · web UI", font=label), 62), "T3 Code · web UI", font=label, fill=MUTED)
    d.text((TELLAL_X, 62), "Tellal · speaks Turkish", font=label, fill=MUTED)
    d.rounded_rectangle((T3_X - 2, T3_Y - 2, T3_X + t3_w + 1, T3_Y + T3_H + 1), radius=8, outline=(48, 48, 54), width=2)
    return img


def note_png(text: str, size: tuple[int, int], path: Path) -> None:
    """Muted note centered in the T3 pane while it still shows the blank page."""
    img = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    f = font(28, "Medium")
    d.text(((size[0] - d.textlength(text, font=f)) / 2, size[1] / 2 - 16), text, font=f, fill=MUTED)
    img.save(path)


def caption_png(english: str, summary: bool, path: Path) -> None:
    probe = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
    ft, fs = font(40, "Semibold"), font(22, "Semibold")
    lines = wrap(probe, english, ft, CAPTION_W - 40)
    img = Image.new("RGBA", (CAPTION_W, 60 + len(lines) * 52), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.text((0, 0), "SPOKEN SUMMARY (TURKISH) · EN" if summary else "TELLAL SAYS (TURKISH) · EN", font=fs, fill=ACCENT)
    y = 40
    for line in lines:
        d.text((0, y), line, font=ft, fill=TEXT)
        y += 52
    img.save(path)


def main() -> None:
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    r = Run(Path(sys.argv[1]))
    clips = Path(sys.argv[2]) if len(sys.argv) == 3 else REPO / "docs/media/clips"
    clips.mkdir(parents=True, exist_ok=True)
    lines = r.lines()
    spans = segments(r, lines)
    total = sum(b - a for a, b in spans)
    print(f"{len(spans)} segment(s), {total:.1f} s:",
          ", ".join(f"{a - spans[0][0]:.1f}+{b - a:.1f}" for a, b in spans))
    if not 30 <= total <= 60:
        raise SystemExit(f"cut length {total:.1f} s is outside 30-60 s; adjust LEAD/TAIL/MAX_GAP")

    # T3: page viewport (right of BrowserOS's side panel, below its toolbar) → t3_crop, in
    # browser.mov pixels.
    s = BROWSER_SCALE
    side, top = r.geometry["chrome"]
    vw, vh = r.geometry["inner"]
    cx, cy, cw, ch = r.edit.get("t3_crop", [0, 0, vw, vh])
    crop = [round(v * s) for v in (cw, ch, side + cx, top + cy)]
    t3_w = round(T3_H * cw / ch) // 2 * 2
    pw = TELLAL_W // 2 * 2
    ph = round(pw * PANEL_H / PANEL_W) // 2 * 2

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)
        audio = tmp / "audio.wav"
        mix_audio(lines, spans, audio)
        bg = tmp / "bg.png"
        background(t3_w).save(bg)

        def cut(src: str, t0: float, name: str) -> str:
            """`src` trimmed to the spans (its own time base starts at t0) and concatenated."""
            n = len(spans)
            split = f"[{src}]split={n}" + "".join(f"[{name}s{i}]" for i in range(n)) if n > 1 else None
            parts = [f"[{name}s{i}]" if n > 1 else f"[{src}]" for i in range(n)]
            trims = [f"{parts[i]}trim={a - t0:.3f}:{b - t0:.3f},setpts=PTS-STARTPTS[{name}{i}]"
                     for i, (a, b) in enumerate(spans)]
            joined = "".join(f"[{name}{i}]" for i in range(n)) + f"concat=n={n}:v=1:a=0[{name}]"
            return ";".join(([split] if split else []) + trims + [joined])

        boxes = "".join(
            f",drawbox=x={b['x']}:y={b['y']}:w={b['w']}:h={b['h']}:color=black:t=fill"
            f":enable='between(t,{b['from']},{b['to'] if b['to'] is not None else 1e9})'"
            for b in r.edit.get("redact", []))
        filters = [
            f"[1:v]fps={FPS}{boxes},crop={crop[0]}:{crop[1]}:{crop[2]}:{crop[3]},"
            f"scale={t3_w}:{T3_H}:flags=lanczos[bsrc]",
            cut("bsrc", r.t0_browser, "b"),
            f"[2:v]fps={FPS},scale={pw}:{ph}:flags=lanczos,format=rgba[psrc]",
            cut("psrc", r.t0_tellal, "p"),
            f"[0:v][b]overlay={T3_X}:{T3_Y}:shortest=1[v0]",
            f"[v0][p]overlay={TELLAL_X}:{TELLAL_Y}:format=auto[v1]",
        ]
        inputs = ["-loop", "1", "-framerate", str(FPS), "-i", str(bg),
                  "-i", str(r.root / "browser.mov"), "-i", str(r.root / "tellal.mov")]
        label, n = "v1", 3
        shown = r.info.get("thread_shown_ms")
        shown_at = cut_time(shown / 1000, spans) if shown else None
        if shown_at:
            note = tmp / "note.png"
            note_png("T3 Code opens the new thread here", (t3_w, T3_H), note)
            inputs += ["-loop", "1", "-framerate", str(FPS), "-i", str(note)]
            filters.append(f"[{label}][{n}:v]overlay={T3_X}:{T3_Y}:enable='lt(t,{shown_at - 0.6:.3f})':shortest=1[v{n}]")
            label, n = f"v{n}", n + 1
        cues = []
        for i, ln in enumerate(lines):
            start = cut_time(ln.start, spans)
            nxt = cut_time(lines[i + 1].start, spans) if i + 1 < len(lines) else None
            end = min(start + (ln.end - ln.start) + 0.4, nxt if nxt is not None else total)
            png = tmp / f"cap{i:02}.png"
            caption_png(ln.english, i == len(lines) - 1, png)
            inputs += ["-loop", "1", "-framerate", str(FPS), "-i", str(png)]
            filters.append(f"[{label}][{n}:v]overlay={CAPTION_X}:{CAPTION_Y}:enable='between(t,{start:.3f},{end:.3f})'"
                           f":shortest=1[v{n}]")
            cues.append((start, end, ln.text, ln.english))
            label, n = f"v{n}", n + 1
        filters.append(f"[{label}]format=yuv420p[vout]")
        out = clips / "real-t3.mp4"
        subprocess.run(["ffmpeg", "-v", "error", "-y", *inputs, "-i", str(audio),
                        "-filter_complex", ";".join(filters), "-map", "[vout]", "-map", f"{n}:a",
                        "-t", f"{total:.3f}", "-r", str(FPS), "-c:v", "libx264", "-preset", "slow", "-crf", "18",
                        "-profile:v", "high", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k",
                        "-ar", str(RATE), "-ac", "2", "-movflags", "+faststart", str(out)], check=True)
        # Poster: the spoken one-sentence summary (the last line), mid-sentence.
        start, end = cues[-1][0], cues[-1][1]
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", f"{(start + end) / 2:.3f}", "-i", str(out),
                        "-frames:v", "1", str(clips / "real-t3.png")], check=True)
    for start, end, text, english in cues:
        print(f"{start:6.2f}-{end:6.2f}  {text}\n               {english}")


if __name__ == "__main__":
    main()
