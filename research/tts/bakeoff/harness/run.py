#!/usr/bin/env python3
"""Run bake-off entries, each in a fresh process under /usr/bin/time -l (stdlib only).

  python3 harness/run.py [--pass2] [run_id ...]   # default: every entry of harness/runs.json not yet in raw/
  English pass: BAKEOFF_RUNS=runs.en.json BAKEOFF_CORPUS=corpus.en.json BAKEOFF_OUT=<dir> python3 harness/run.py
  (raw/ and audio/ go under BAKEOFF_OUT; read back with harness/readback_en.py).
Adds wall_s, max_rss_bytes and peak_footprint_bytes from /usr/bin/time -l to raw/<run_id>.json; on failure
writes raw/<run_id>.error.txt with the tail of stderr.
"""
import json, os, re, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(os.environ.get("BAKEOFF_OUT") or HERE.parent)
RUNS = json.loads((HERE / os.environ.get("BAKEOFF_RUNS", "runs.json")).read_text())


def run(rid, pass2=False):
    """pass2: repeat in a fresh process for timing only (audio to /tmp), result → raw/pass2/<run_id>.json."""
    e = RUNS[rid]
    out_root = Path("/tmp/bakeoff-pass2") if pass2 else ROOT
    py = os.path.expanduser(e["python"])
    script = os.path.expanduser(e.get("script", str(HERE / "bench.py")))
    cmd = ["/usr/bin/time", "-l", py, script, rid, json.dumps(e["config"], ensure_ascii=False)]
    print("$", " ".join(cmd[:4]), rid, f"'{cmd[-1]}'", flush=True)
    t = time.time()
    p = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=e.get("timeout", 1800),
                       env={**os.environ, **({"BAKEOFF_OUT": str(out_root)} if pass2 else {})})
    wall = time.time() - t
    (ROOT / "raw").mkdir(exist_ok=True)
    out = out_root / "raw" / f"{rid}.json"
    if p.returncode or not out.exists():
        (ROOT / "raw" / f"{rid}.error.txt").write_text(f"exit {p.returncode}\n{p.stdout[-4000:]}\n{p.stderr[-6000:]}")
        print(f"FAILED {rid} (exit {p.returncode})\n{p.stderr[-1500:]}", flush=True)
        return
    rep = json.loads(out.read_text())
    grab = lambda k: int(m.group(1)) if (m := re.search(rf"(\d+)\s+{k}", p.stderr)) else None  # noqa: E731
    rep["wall_s"] = round(wall, 2)
    for k, v in {"max_rss_bytes": grab("maximum resident set size"), "peak_footprint_bytes": grab("peak memory footprint"),
                 "command": " ".join(cmd[2:4]) + f" {rid} '{cmd[-1]}'"}.items():
        rep.setdefault(k, v)  # a sidecar script (zonos_server.py) reports its server's own numbers
    if pass2:
        (ROOT / "raw" / "pass2").mkdir(exist_ok=True)
        out = ROOT / "raw" / "pass2" / f"{rid}.json"
    out.write_text(json.dumps(rep, ensure_ascii=False, indent=1))
    print(p.stdout[-1200:], f"rss {rep['max_rss_bytes']/1e6:.0f} MB, footprint {(rep['peak_footprint_bytes'] or 0)/1e6:.0f} MB, wall {wall:.1f}s", flush=True)


if __name__ == "__main__":
    pass2 = "--pass2" in sys.argv
    args = [a for a in sys.argv[1:] if a != "--pass2"]
    done = ROOT / "raw" / ("pass2" if pass2 else "")
    ids = args or [r for r in RUNS if not (done / f"{r}.json").exists()]
    for rid in ids:
        run(rid, pass2)
