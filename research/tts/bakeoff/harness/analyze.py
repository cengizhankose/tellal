#!/usr/bin/env python3
"""Read-back + loudness copies + silence stats for every raw/<run>.json; writes results.json (stdlib + ffmpeg + whisper-cli).

  python3 harness/analyze.py
Per clip: ffmpeg → 16 kHz mono for whisper; whisper-cli (ggml-large-v3-turbo-q8_0, -l tr) transcript cached in
raw/whisper/<run>__<s>.txt; WER vs the written text with digits spelled out as Turkish words on BOTH sides
(numberWord twin) and Turkish lowercasing (I→ı, İ→i), apostrophes dropped ("20'si" → "yirmisi");
loudnorm copy (-16 LUFS, single pass) → audio/norm/<run>/<s>.mp3; silencedetect (-40 dB, 0.25 s) on the raw wav.
"""
import json, re, statistics, subprocess, wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
WHISPER = Path.home() / ".cache/whisper/ggml-large-v3-turbo-q8_0.bin"
TMP = Path("/tmp/bakeoff16k")
META = json.loads((HERE / "meta.json").read_text())

ONES = ["", "bir", "iki", "üç", "dört", "beş", "altı", "yedi", "sekiz", "dokuz"]
TENS = ["", "on", "yirmi", "otuz", "kırk", "elli", "altmış", "yetmiş", "seksen", "doksan"]


def sayi(n):
    if n == 0:
        return "sıfır"
    def k3(k):
        h, r = divmod(k, 100)
        hs = "" if h == 0 else "yüz" if h == 1 else f"{ONES[h]} yüz"
        return " ".join(w for w in [hs, TENS[r // 10], ONES[r % 10]] if w)
    t, r = divmod(n, 1000)
    return " ".join(w for w in ["" if t == 0 else "bin" if t == 1 else f"{k3(t)} bin", k3(r)] if w)


def words(s):
    s = re.sub(r"\d{1,3}(?:\.\d{3})+|\d+", lambda m: f" {sayi(int(m.group().replace('.', '')))}" if len(m.group()) < 8 else m.group(), s)
    s = s.replace("I", "ı").replace("İ", "i").lower()
    s = re.sub(r"\s*['’`]\s*", "", s)  # suffix apostrophes: "yirmi'si" / "yirmi 'si" → "yirmisi"
    return re.sub(r"[^\wçğıöşü ]", " ", s).split()


def wer(ref, hyp):
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[-1] / len(ref)


def sh(*cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def silences(wav, dur):
    err = sh("ffmpeg", "-hide_banner", "-nostats", "-i", str(wav), "-af", "silencedetect=noise=-40dB:d=0.25", "-f", "null", "-").stderr
    st = [float(x) for x in re.findall(r"silence_start: (-?[\d.]+)", err)]
    en = [float(x) for x in re.findall(r"silence_end: ([\d.]+)", err)]
    spans = [(s, en[i] if i < len(en) else dur) for i, s in enumerate(st)]
    lead = next((e - max(s, 0) for s, e in spans if s <= 0.01), 0.0)
    trail = next((e - s for s, e in spans if e >= dur - 0.02 and s > 0.01), 0.0)
    inner = [e - s for s, e in spans if s > 0.01 and e < dur - 0.02]
    return round(lead, 3), round(trail, 3), round(max(inner, default=0.0), 3)


def peak(wav):
    """(peak 0..1, number of samples at |x| >= 0.999 full scale)."""
    import array
    with wave.open(str(wav)) as w:
        a = array.array("h", w.readframes(w.getnframes()))
    return max((abs(x) for x in a), default=0) / 32767, sum(1 for x in a if abs(x) >= 32735)


def cer(ref, hyp):
    """Character error rate with spaces removed (robust to whisper splitting "Durduruldu" → "Dur duruldu")."""
    return wer(list("".join(ref)), list("".join(hyp)))


EN_WORDS = {"d": ["pull", "request", "deploy"], "e": ["build", "ios"]}  # English tech words whisper must hear as such
SYL = re.compile(r"[aeıioöuü]")


def pass2(rid):
    """Timing-only repeat in another fresh process (run.py --pass2): load, TTFA p50, RTF, RSS."""
    p = ROOT / "raw" / "pass2" / f"{rid}.json"
    if not p.exists():
        return None
    r = json.loads(p.read_text())
    tt = [x.get("first_chunk_ms", x["synth_ms"]) for x in r["sentences"]]
    return {"load_ms": r["load_ms"], "ttfa_first_ms": tt[0], "ttfa_p50_ms": round(statistics.median(tt), 1),
            "rtf": round(sum(x["synth_ms"] for x in r["sentences"]) / sum(x["audio_ms"] for x in r["sentences"]), 4),
            "max_rss_mb": round(r["max_rss_bytes"] / 2**20) if r.get("max_rss_bytes") else None}


def seed_summary(rid, rows):
    """Seeds 2/3 from harness/seeds.py next to the main run (seed 1): WER and numbers check per seed."""
    p = ROOT / "raw" / "seeds.json"
    other = json.loads(p.read_text()).get(rid, {}) if p.exists() else {}
    if not other:
        return None
    allref = sum(len(words(r["text"])) for r in rows)
    out = {"1": {"wer": round(sum(r["wer"] * len(words(r["text"])) for r in rows) / allref, 3),
                 "numbers_ok": next(r["numbers_ok"] for r in rows if r["id"] == "c")}}
    out.update({k: {"wer": v["wer"], "numbers_ok": v["numbers_ok"], "heard_c": v["sentences"]["c"]["heard"]} for k, v in other.items()})
    out["wer_mean"] = round(sum(v["wer"] for k, v in out.items() if k != "wer_mean") / len(other.keys() | {"1"}), 3)
    out["numbers_ok_count"] = f"{sum(v['numbers_ok'] for k, v in out.items() if isinstance(v, dict))}/{len(other) + 1}"
    return out


def main():
    TMP.mkdir(exist_ok=True)
    (ROOT / "raw" / "whisper").mkdir(parents=True, exist_ok=True)
    reps = [json.loads(p.read_text()) for p in sorted((ROOT / "raw").glob("*.json")) if p.name != "seeds.json"]
    todo = []
    for rep in reps:
        rid = rep["run"]
        (ROOT / "audio" / "norm" / rid).mkdir(parents=True, exist_ok=True)
        for s in rep["sentences"]:
            raw = ROOT / "audio" / "raw" / rid / f"{s['id']}.wav"
            norm = ROOT / "audio" / "norm" / rid / f"{s['id']}.mp3"
            if not norm.exists():
                sh("ffmpeg", "-y", "-hide_banner", "-i", str(raw), "-af", "loudnorm=I=-16:TP=-1.5:LRA=11",
                   "-ar", str(min(int(rep["sample_rate"]), 48000)), "-c:a", "libmp3lame", "-q:a", "2", str(norm))
            txt = ROOT / "raw" / "whisper" / f"{rid}__{s['id']}.txt"
            if not txt.exists():
                w16 = TMP / f"{rid}__{s['id']}.wav"
                sh("ffmpeg", "-y", "-hide_banner", "-i", str(raw), "-ar", "16000", "-ac", "1", str(w16))
                todo.append((w16, txt))
    for i in range(0, len(todo), 40):  # one whisper process per 40 clips (model load once)
        batch = todo[i:i + 40]
        sh("whisper-cli", "-m", str(WHISPER), "-l", "tr", "-nt", "-np", "-otxt", *[str(w) for w, _ in batch])
        for w, txt in batch:
            out = Path(str(w) + ".txt")
            txt.write_text(out.read_text().strip() if out.exists() else "")
        print(f"whisper {i + len(batch)}/{len(todo)}", flush=True)

    engines = []
    for rep in reps:
        rid = rep["run"]
        meta = META["runs"].get(rid, {})
        fam = META["families"][rep["config"]["engine"]]
        rows = []
        for s in rep["sentences"]:
            raw = ROOT / "audio" / "raw" / rid / f"{s['id']}.wav"
            heard = (ROOT / "raw" / "whisper" / f"{rid}__{s['id']}.txt").read_text().strip()
            ref, hyp = words(s["text"]), words(heard)
            lead, trail, gap = silences(raw, s["audio_ms"] / 1000)
            speech = max(s["audio_ms"] / 1000 - lead - trail, 0.01)
            syl = len(SYL.findall(" ".join(ref)))
            flags = []
            if lead > 0.5: flags.append(f"lead silence {lead:.2f}s")
            if trail > 1.0: flags.append(f"trail silence {trail:.2f}s")
            if gap > 0.8: flags.append(f"inner gap {gap:.2f}s")
            if syl / speech > 8.5: flags.append(f"fast {syl / speech:.1f} syl/s")
            if syl / speech < 3.0: flags.append(f"slow/padded {syl / speech:.1f} syl/s")
            pk, nclip = peak(raw)
            if nclip > 10: flags.append(f"clipped ({nclip} samples at full scale)")
            row = {**s, "ttfa_ms": s.get("first_chunk_ms", s["synth_ms"]), "heard": heard, "wer": round(wer(ref, hyp), 3), "cer": round(cer(ref, hyp), 3),
                   "lead_s": lead, "trail_s": trail, "max_gap_s": gap, "syl_per_s": round(syl / speech, 2), "peak": round(pk, 3),
                   "flags": flags, "audio": f"audio/norm/{rid}/{s['id']}.mp3"}
            if s["id"] == "c":
                row["numbers_ok"] = all(w in hyp for w in ["seksen", "yirmisi", "otuzu"])
            if s["id"] in EN_WORDS:
                row["english_ok"] = [w for w in EN_WORDS[s["id"]] if w in hyp]
            rows.append(row)
        ttfa = [r["ttfa_ms"] for r in rows]
        tot_s, tot_a = sum(r["synth_ms"] for r in rows), sum(r["audio_ms"] for r in rows)
        allref = sum(len(words(r["text"])) for r in rows)
        engines.append({
            "id": rid, "family": rep["config"]["engine"], "label": meta.get("label", rid), "baseline": rid == "piper-dfki",
            "config": rep["config"], "command": rep.get("command"), "device": rep["device"], "sample_rate": rep["sample_rate"],
            "import_ms": rep["import_ms"], "load_ms": rep["load_ms"], "ttfa_first_ms": rows[0]["ttfa_ms"],
            "ttfa_p50_ms": round(statistics.median(ttfa), 1), "ttfa_max_ms": max(ttfa),
            "streams": any("first_chunk_ms" in r for r in rows), "rtf": round(tot_s / tot_a, 4),
            "max_rss_mb": round(rep["max_rss_bytes"] / 2**20) if rep.get("max_rss_bytes") else None,
            "peak_footprint_mb": round(rep["peak_footprint_bytes"] / 2**20) if rep.get("peak_footprint_bytes") else None,
            "wall_s": rep.get("wall_s"),
            "wer": round(sum(r["wer"] * len(words(r["text"])) for r in rows) / allref, 3),
            "cer": round(sum(r["cer"] * len("".join(words(r["text"]))) for r in rows) / sum(len("".join(words(r["text"]))) for r in rows), 3),
            "lead_p50_s": round(statistics.median(r["lead_s"] for r in rows), 3),
            "perceived_ttfa_p50_ms": round(statistics.median(r["ttfa_ms"] + 1000 * r["lead_s"] for r in rows), 1),
            "numbers_ok": next(r["numbers_ok"] for r in rows if r["id"] == "c"),
            "english_ok": f"{sum(len(r['english_ok']) for r in rows if 'english_ok' in r)}/{sum(len(v) for v in EN_WORDS.values())}",
            "flags": sum(len(r["flags"]) for r in rows),
            "pass2": pass2(rid),
            "seeds": seed_summary(rid, rows),
            "license": meta.get("license", fam["license"]), "rust": meta.get("rust", fam["rust"]), "note": meta.get("note", ""), "sentences": rows,
        })
    corpus = json.loads((HERE / "corpus.json").read_text())
    out = {"generated": __import__("datetime").datetime.now().isoformat(timespec="seconds"), "machine": META["machine"],
           "method": META["method"], "corpus": corpus, "engines": engines, "failures": META.get("failures", []),
           "skipped": META.get("skipped", [])}
    for p in sorted((ROOT / "raw").glob("*.error.txt")):
        if not any(f["id"] == p.name[:-10] for f in out["failures"]):
            out["failures"].append({"id": p.name[:-10], "error": p.read_text()[-1500:]})
    (ROOT / "results.json").write_text(json.dumps(out, ensure_ascii=False, indent=1))
    print(f"{len(engines)} engines → results.json")


if __name__ == "__main__":
    main()
