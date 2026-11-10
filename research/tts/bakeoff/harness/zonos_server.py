#!/usr/bin/env python3
"""ZONOS3-Mini native C++/ggml server (Metal, no Python at runtime) as a local sidecar: the bench for the
"FFI/sidecar without Python" path. stdlib only.

  python3 harness/zonos_server.py <run_id>     (cfg from harness/runs.json; needs the built zonos3-server + gguf/)
Starts `/usr/bin/time -l zonos3-server gguf/zonos3-ptq.gguf --port P` (cwd servers/ggml), load_ms = spawn → first
200 from /v2/capabilities; caches the clone voice once (POST /v2/speakers, not counted per sentence); per sentence
POST /v2/generate stream=true (float32 PCM 44.1 kHz): first_chunk_ms = first body bytes, synth_ms = last byte.
Writes the same raw/<run_id>.json + audio/raw/<run_id>/*.wav as bench.py; RSS from /usr/bin/time of the server.
"""
import array, base64, json, os, re, signal, subprocess, sys, time, urllib.error, urllib.request, wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(os.environ.get("BAKEOFF_OUT") or HERE.parent)
CORPUS = json.loads((HERE / "corpus.json").read_text())
SRV = Path.home() / ".cache/bakeoff-models/ZONOS3-Mini/servers/ggml"
LOG = Path("/tmp/zonos3-server.log")


def call(port, path, body=None, headers=None, stream=False):
    req = urllib.request.Request(f"http://127.0.0.1:{port}{path}", data=None if body is None else json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json", **(headers or {})})
    try:
        return urllib.request.urlopen(req, timeout=300)
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{path}: HTTP {e.code} {e.read().decode(errors='replace')[:1500]}") from None


def main():
    rid = sys.argv[1]
    cfg = json.loads((HERE / "runs.json").read_text())[rid]["config"]
    port = cfg.get("port", 19191)
    t0 = time.perf_counter()
    proc = subprocess.Popen(["/usr/bin/time", "-l", str(SRV / "build/zonos3-server"), "gguf/zonos3-ptq.gguf", "--port", str(port)],
                            cwd=SRV, stdout=subprocess.DEVNULL, stderr=open(LOG, "w"))  # file, so a chatty server never blocks
    try:
        while True:
            try:
                caps = json.load(call(port, "/v2/capabilities"))
                break
            except Exception:  # noqa: BLE001 — not up yet
                if proc.poll() is not None:
                    raise RuntimeError(LOG.read_text()[-3000:])
                time.sleep(0.05)
        load_ms = (time.perf_counter() - t0) * 1000
        sess = {"X-TTS-Session-ID": "bakeoff"}
        ref = Path(os.path.expanduser(cfg["ref"])).read_bytes()
        t = time.perf_counter()
        spk = json.load(call(port, "/v2/speakers", {"speaker_audio_base64": base64.b64encode(ref).decode()}, sess))
        voice_ms = (time.perf_counter() - t) * 1000
        sr = 44100
        out = ROOT / "audio" / "raw" / rid
        out.mkdir(parents=True, exist_ok=True)
        rows = []
        for s in CORPUS:
            body = {"text": s["text"], "stream": True, "seed": cfg.get("seed", 1), "speaker_embedding_id": spk["id"], "speaker_mode": "embedding",
                    "text_normalization": False}
            t = time.perf_counter()
            r = call(port, "/v2/generate", body, sess)
            first, buf = None, bytearray()
            while chunk := r.read1(65536) if hasattr(r, "read1") else r.read(65536):
                first = first or time.perf_counter() - t
                buf += chunk
            el = time.perf_counter() - t
            pcm = array.array("f", bytes(buf[: len(buf) // 4 * 4]))
            with wave.open(str(out / f"{s['id']}.wav"), "wb") as w:
                w.setnchannels(1); w.setsampwidth(2); w.setframerate(sr)
                w.writeframes(array.array("h", (int(max(-1.0, min(1.0, x)) * 32767) for x in pcm)).tobytes())
            audio_ms = len(pcm) / sr * 1000
            rows.append({"id": s["id"], "text": s["text"], "synth_ms": round(el * 1000, 1), "first_chunk_ms": round(first * 1000, 1),
                         "audio_ms": round(audio_ms, 1), "rtf": round(el * 1000 / audio_ms, 4) if audio_ms else None})
            print(rid, s["id"], rows[-1], flush=True)
    finally:
        # signal the server itself (child of /usr/bin/time) so time survives to print its rusage
        kids = subprocess.run(["pgrep", "-P", str(proc.pid)], capture_output=True, text=True).stdout.split()
        for k in kids or [str(proc.pid)]:
            os.kill(int(k), signal.SIGTERM)
        try:
            proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill(); proc.wait()
    err = LOG.read_text()
    grab = lambda k: int(m.group(1)) if (m := re.search(rf"(\d+)\s+{k}", err)) else None  # noqa: E731
    rep = {"run": rid, "config": cfg, "import_ms": 0, "load_ms": round(load_ms, 1), "voice_cache_ms": round(voice_ms, 1),
           "device": "Metal GPU (ggml, native C++ server)", "sample_rate": sr, "sentences": rows,
           "max_rss_bytes": grab("maximum resident set size"), "peak_footprint_bytes": grab("peak memory footprint"),
           "command": f"python3 harness/zonos_server.py {rid}  (server: build/zonos3-server gguf/zonos3-ptq.gguf --port {port})",
           "decoder": caps.get("decoder")}
    (ROOT / "raw").mkdir(exist_ok=True)
    (ROOT / "raw" / f"{rid}.json").write_text(json.dumps(rep, ensure_ascii=False, indent=1))
    print(f"{rid}: load {rep['load_ms']} ms, voice cache {voice_ms:.0f} ms, rss {(rep['max_rss_bytes'] or 0)/2**20:.0f} MB")


if __name__ == "__main__":
    main()
