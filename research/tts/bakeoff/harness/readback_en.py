#!/usr/bin/env python3
"""English pass read-back: whisper-cli (ggml-large-v3-turbo-q8_0, -l en) per clip, WER, lead silence, speed numbers.

  BAKEOFF_OUT=<dir> python3 harness/readback_en.py [<out.json>]   # default out: en-results.json next to harness/
Reads <dir>/raw/<run>.json (from run.py with BAKEOFF_RUNS=runs.en.json BAKEOFF_CORPUS=corpus.en.json) and the
wavs under <dir>/audio/raw/<run>/. WER compares words after the same canonical form on both sides: commit hashes
dropped, paths reduced to the file name, lowercase, digits and clock times spelled out ("3:45" → "three forty
five"), "p.m." → "pm", unit words merged with their symbols ("milliseconds" → "ms"), punctuation dropped.
Also reads a `tellal-bench --voice en` run dir (its bench.json lists every wav):
  python3 harness/readback_en.py --tellal <run dir> <out.json>
"""
import json, os, re, statistics, subprocess, sys
from pathlib import Path

from analyze import silences, wer  # noqa: E402  (same WER and silence detection as the Turkish pass)

HERE = Path(__file__).resolve().parent
WHISPER = Path.home() / ".cache/whisper/ggml-large-v3-turbo-q8_0.bin"
TMP = Path("/tmp/bakeoff16k-en")
ONES = "zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen " \
       "seventeen eighteen nineteen".split()
TENS = "_ _ twenty thirty forty fifty sixty seventy eighty ninety".split()
SAME = {"milliseconds": "ms", "millisecond": "ms", "pm": "pm", "p m": "pm", "okay": "ok", "o k": "ok",
        "megabytes": "mb", "megabyte": "mb", "version": "v", "percent": "", "npm": "n p m"}


def say(n):
    if n < 20:
        return ONES[n]
    if n < 100:
        return TENS[n // 10] + ("" if n % 10 == 0 else " " + ONES[n % 10])
    if n < 1000:
        return ONES[n // 100] + " hundred" + ("" if n % 100 == 0 else " " + say(n % 100))
    return say(n // 1000) + " thousand" + ("" if n % 1000 == 0 else " " + say(n % 1000))


def words(s):
    s = re.sub(r"\b(?=[0-9a-f]*\d)(?=[0-9a-f]*[a-f])[0-9a-f]{7,}\b", " ", s)  # commit hashes are not spoken
    s = re.sub(r"\S*/([^\s/]+)", r"\1", s)  # paths are spoken as their file name
    s = s.lower().replace("p.m.", "pm").replace("a.m.", "am")
    s = re.sub(r"\b(\d{1,2}):(\d{2})\b", lambda m: f" {say(int(m[1]))} " + ("" if m[2] == "00" else
               ("oh " + ONES[int(m[2])] if m[2][0] == "0" else say(int(m[2])))), s)
    s = re.sub(r"\d+", lambda m: f" {say(int(m[0]))} " if len(m[0]) < 7 else m[0], s)
    s = re.sub(r"[^a-z0-9' ]", " ", s).replace("'", "")
    s = " " + " ".join(s.split()) + " "
    for k, v in SAME.items():
        s = s.replace(f" {k} ", f" {v} ")
    return s.split()


def clips_bakeoff(root):
    for p in sorted((root / "raw").glob("*.json")):
        rep = json.loads(p.read_text())
        yield rep, [(s, root / "audio" / "raw" / rep["run"] / f"{s['id']}.wav") for s in rep["sentences"]]


def clips_tellal(run_dir):
    """tellal-bench run dir: bench.json (voice, load_ms, peak_rss_mb, sentences with their wav paths)."""
    rep = json.loads((run_dir / "bench.json").read_text())
    yield rep, [(s, Path(s["wav"])) for s in rep["sentences"]]


def transcribe(pairs):
    TMP.mkdir(exist_ok=True)
    todo = []
    for key, wav in pairs:
        w16 = TMP / f"{key}.wav"
        subprocess.run(["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-i", str(wav), "-ar", "16000", "-ac", "1",
                        str(w16)], check=True)
        todo.append(w16)
    subprocess.run(["whisper-cli", "-m", str(WHISPER), "-l", "en", "-nt", "-np", "-otxt", *map(str, todo)],
                   capture_output=True, check=True)
    return {w.stem: Path(str(w) + ".txt").read_text().strip() for w in todo}


def main():
    args = sys.argv[1:]
    if args and args[0] == "--tellal":
        reps, out = list(clips_tellal(Path(args[1]))), Path(args[2])
    else:
        reps = list(clips_bakeoff(Path(os.environ["BAKEOFF_OUT"])))
        out = Path(args[0]) if args else HERE.parent / "en-results.json"
    run_id = lambda rep: rep.get("run") or rep.get("voice")  # noqa: E731
    heard = transcribe([(f"{run_id(rep)}__{s['id']}", wav) for rep, clips in reps for s, wav in clips])
    engines = []
    for rep, clips in reps:
        rows = []
        for s, wav in clips:
            h = heard[f"{run_id(rep)}__{s['id']}"]
            ref, hyp = words(s.get("spoken_ref") or s["text"]), words(h)
            lead, _, _ = silences(wav, s["audio_ms"] / 1000)
            rows.append({"id": s["id"], "text": s["text"], "heard": h, "wer": round(wer(ref, hyp), 3),
                         "synth_ms": s["synth_ms"], "audio_ms": s["audio_ms"], "lead_s": lead})
        nref = sum(len(words(r["text"])) for r in rows)
        engines.append({
            "id": run_id(rep), "config": rep.get("config"), "device": rep.get("device"),
            "load_ms": rep.get("load_ms"), "first_synth_ms": rows[0]["synth_ms"],
            "synth_p50_ms": statistics.median(r["synth_ms"] for r in rows),
            "rtf": round(sum(r["synth_ms"] for r in rows) / sum(r["audio_ms"] for r in rows), 4),
            "max_rss_mb": round(rep["max_rss_bytes"] / 2**20) if rep.get("max_rss_bytes") else rep.get("peak_rss_mb"),
            "wer": round(sum(r["wer"] * len(words(r["text"])) for r in rows) / nref, 3),
            "lead_p50_s": statistics.median(r["lead_s"] for r in rows), "sentences": rows})
    out.write_text(json.dumps({"whisper": f"{WHISPER.name} -l en", "engines": engines}, ensure_ascii=False, indent=1))
    for e in engines:
        print(f"{e['id']}: load {e['load_ms']} ms, p50 {e['synth_p50_ms']} ms, rtf {e['rtf']}, rss {e['max_rss_mb']} MB, "
              f"wer {e['wer']:.3f}, lead {e['lead_p50_s']} s")
        for r in e["sentences"]:
            if r["wer"]:
                print(f"   {r['id']} wer {r['wer']}: {r['heard']}")


if __name__ == "__main__":
    main()
