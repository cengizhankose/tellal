#!/usr/bin/env python3
"""Seed robustness for the shortlisted stochastic engines: re-synthesize the corpus with seeds 2 and 3 (audio to
/tmp), whisper read-back, WER + numbers check → raw/seeds.json. Seed 1 is the main run. stdlib + analyze.py helpers.

  python3 harness/seeds.py run_id [run_id ...]
"""
import json, os, subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import analyze as A  # noqa: E402

HERE, ROOT = A.HERE, A.ROOT
RUNS = json.loads((HERE / "runs.json").read_text())
CORPUS = json.loads((HERE / "corpus.json").read_text())
OUT = ROOT / "raw" / "seeds.json"


def main(ids, seeds=(2, 3)):
    res = json.loads(OUT.read_text()) if OUT.exists() else {}
    for rid in ids:
        for seed in seeds:
            if str(seed) in res.get(rid, {}):
                continue
            e = RUNS[rid]
            cfg = {**e["config"], "seed": seed}
            if cfg["engine"] == "mlx":
                cfg["kwargs"] = {**cfg.get("kwargs", {}), "seed": seed}
            base = Path(f"/tmp/bakeoff-seeds/s{seed}")
            p = subprocess.run([os.path.expanduser(e["python"]), str(HERE / "bench.py"), rid, json.dumps(cfg, ensure_ascii=False)],
                               cwd=ROOT, capture_output=True, text=True, env={**os.environ, "BAKEOFF_OUT": str(base)})
            if p.returncode:
                print(f"FAILED {rid} seed {seed}: {p.stderr[-800:]}")
                continue
            wavs = []
            for s in CORPUS:
                w16 = A.TMP / f"seed{seed}__{rid}__{s['id']}.wav"
                A.TMP.mkdir(exist_ok=True)
                A.sh("ffmpeg", "-y", "-hide_banner", "-i", str(base / "audio" / "raw" / rid / f"{s['id']}.wav"), "-ar", "16000", "-ac", "1", str(w16))
                wavs.append(w16)
            A.sh("whisper-cli", "-m", str(A.WHISPER), "-l", "tr", "-nt", "-np", "-otxt", *map(str, wavs))
            per, tot, n = {}, 0.0, 0
            for s, w in zip(CORPUS, wavs):
                heard = Path(str(w) + ".txt").read_text().strip()
                ref, hyp = A.words(s["text"]), A.words(heard)
                per[s["id"]] = {"wer": round(A.wer(ref, hyp), 3), "heard": heard}
                tot += A.wer(ref, hyp) * len(ref); n += len(ref)
            hyp_c = A.words(per["c"]["heard"])
            res.setdefault(rid, {})[str(seed)] = {"wer": round(tot / n, 3), "numbers_ok": all(x in hyp_c for x in ["seksen", "yirmisi", "otuzu"]),
                                                  "sentences": per}
            print(rid, seed, res[rid][str(seed)]["wer"], res[rid][str(seed)]["numbers_ok"], flush=True)
            OUT.write_text(json.dumps(res, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main(sys.argv[1:])
