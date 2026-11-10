#!/usr/bin/env python3
"""One engine/voice, one fresh process: cold load + per-sentence synth timings for the bake-off corpus.

Run (from research/tts/bakeoff):  /usr/bin/time -l <venv python> harness/bench.py <run_id> '<config json>'
Config: {"engine": "piper|supertonic|pocket|chatterbox|xtts|yelda|...", ...engine settings}.
Writes audio/raw/<run_id>/<sentence>.wav (engine-native sample rate, 16-bit mono) and raw/<run_id>.json:
  {"run", "config", "import_ms", "load_ms", "device", "sample_rate", "sentences": [{"id", "text",
   "synth_ms", "first_chunk_ms"?, "audio_ms", "rtf"}]}
load_ms counts from process start (imports included) to a ready model, i.e. what a fresh sidecar pays.
Synthesis is timed with perf_counter around the engine call; the corpus is fed raw (no number normalization).
"""
import time

T0 = time.perf_counter()
import os  # noqa: E402
import json, subprocess, sys, tempfile, wave  # noqa: E402
from pathlib import Path  # noqa: E402

HERE = Path(__file__).resolve().parent
ROOT = Path(os.environ.get("BAKEOFF_OUT") or HERE.parent)  # BAKEOFF_OUT: timing-only repeat pass (run.py --pass2)
sys.path[:] = [p for p in sys.path if Path(p or ".").resolve() != HERE]  # don't shadow `piper`/`supertonic`
CORPUS = json.loads((HERE / "corpus.json").read_text())
ms = lambda a, b: round((b - a) * 1000, 1)  # noqa: E731


def write_wav(path, samples, sr):
    import numpy as np
    a = np.clip(np.asarray(samples, dtype="float32").reshape(-1), -1, 1)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(int(sr)); w.writeframes((a * 32767).astype("<i2").tobytes())


# Each engine: load(cfg) -> (state, device, sample_rate); synth(state, cfg, text) -> (samples, first_chunk_s or None)


def piper_load(cfg):
    from piper import PiperVoice
    v = PiperVoice.load(Path(os.path.expanduser("~/.local/share/piper-voices")) / f"{cfg['voice']}.onnx")
    return v, "CPU (onnxruntime " + ",".join(v.session.get_providers()) + ")", v.config.sample_rate


def piper_synth(v, cfg, text):
    import numpy as np
    from piper import SynthesisConfig
    sc = SynthesisConfig(length_scale=cfg.get("length_scale", 1.0), noise_scale=cfg.get("noise_scale", 0.667),
                         noise_w_scale=cfg.get("noise_w", 0.8))
    t, first, parts = time.perf_counter(), None, []
    for c in v.synthesize(text, sc):  # one chunk per sentence -> sentence-level streaming
        first = first or time.perf_counter() - t
        parts.append(c.audio_float_array)
    return np.concatenate(parts), first


def supertonic_load(cfg):
    from supertonic import TTS
    tts = TTS(model="supertonic-3")
    style = tts.get_voice_style(cfg["voice"])
    return (tts, style), "CPU (onnxruntime CPUExecutionProvider)", tts.sample_rate


def supertonic_synth(st, cfg, text):
    import numpy as np
    tts, style = st
    np.random.seed(cfg.get("seed", 1))
    wav, _ = tts.synthesize(text, voice_style=style, lang="tr", speed=cfg.get("speed", 1.05), total_steps=cfg["steps"])
    return np.asarray(wav, dtype="float32").reshape(-1), None


def pocket_load(cfg):
    from pocket_tts_turkish import TurkishTTS
    m = TurkishTTS.from_pretrained(device="cpu")
    return m, "CPU (torch; MPS aborts in Metal per reels-lab)", m.sample_rate


def pocket_synth(m, cfg, text):
    import numpy as np
    t, first, parts = time.perf_counter(), None, []
    for chunk in m.stream(text, voice=cfg["voice"], emotion=cfg.get("emotion"), seed=cfg.get("seed", 1)):  # ~80 ms pieces
        first = first or time.perf_counter() - t
        parts.append(np.asarray(chunk, dtype="float32").reshape(-1))
    return np.concatenate(parts), first


def chatterbox_load(cfg):
    import torch
    from chatterbox.mtl_tts import ChatterboxMultilingualTTS
    dev = "mps" if torch.backends.mps.is_available() else "cpu"
    m = ChatterboxMultilingualTTS.from_pretrained(device=dev)
    if cfg.get("ref"):  # clone conditioning computed once, as a panel would cache it
        m.prepare_conditionals(os.path.expanduser(cfg["ref"]), exaggeration=cfg["exaggeration"])
    return m, dev.upper() + " (torch)", m.sr


def chatterbox_synth(m, cfg, text):
    import torch
    torch.manual_seed(cfg.get("seed", 1))
    wav = m.generate(text, language_id="tr", exaggeration=cfg["exaggeration"], cfg_weight=cfg["cfg_weight"])
    return wav.squeeze(0).cpu().numpy(), None


XTTS_P = {"temperature": 0.65, "speed": 1.0, "repetition_penalty": 5.0, "top_k": 50, "top_p": 0.85}  # xtts.json base+cta


def xtts_load(cfg):
    os.environ.setdefault("COQUI_TOS_AGREED", "1")
    import torch
    from TTS.api import TTS
    tts = TTS("tts_models/multilingual/multi-dataset/xtts_v2")
    model = tts.synthesizer.tts_model
    spk = model.speaker_manager.speakers[cfg["speaker"]]
    st = {"model": model, "spk": spk, "device": "cpu"}
    if torch.backends.mps.is_available():
        try:
            tts.to("mps"); st["device"] = "mps"
            xtts_full(st, "Merhaba.")  # same MPS probe/warm-up as reels-lab xtts.py; counted in load
        except Exception as err:  # noqa: BLE001
            print(f"xtts: MPS failed ({err!r}); using CPU", flush=True)
            tts.to("cpu"); st["device"] = "cpu"
    return st, st["device"].upper() + " (torch)", model.config.audio.output_sample_rate


def xtts_full(st, text):
    import torch
    torch.manual_seed(1)
    d = st["device"]
    out = st["model"].inference(text, "tr", st["spk"]["gpt_cond_latent"].to(d), st["spk"]["speaker_embedding"].to(d),
                                enable_text_splitting=False, **XTTS_P)
    return out["wav"]


def xtts_synth(st, cfg, text):
    import numpy as np, torch
    t = time.perf_counter()
    wav = xtts_full(st, text)
    full = time.perf_counter() - t
    # second pass with inference_stream: time to first streamed chunk (audio kept from the full pass)
    torch.manual_seed(1)
    d = st["device"]
    t = time.perf_counter()
    gen = st["model"].inference_stream(text, "tr", st["spk"]["gpt_cond_latent"].to(d), st["spk"]["speaker_embedding"].to(d),
                                       enable_text_splitting=False, **XTTS_P)
    next(gen)
    first = time.perf_counter() - t
    for _ in gen:
        pass
    return np.asarray(wav, dtype="float32"), first, full


def anka_load(cfg):
    from anka import AnkaTTS
    m = AnkaTTS.from_pretrained("anka-tts/v0.1", device=cfg.get("device"), progress=False)
    dev = str(getattr(m, "device", cfg.get("device") or "auto"))
    return m, f"{dev.upper()} (torch, F5-TTS)", int(getattr(m, "sample_rate", 24000))


def anka_synth(m, cfg, text):
    kw = {"nfe_step": cfg["nfe_step"]} if cfg.get("nfe_step") else {}
    wav = m.synthesize(text, voice=cfg.get("voice"), seed=cfg.get("seed", 1), progress=False, **kw)
    return wav, None


def pocket4_load(cfg):
    """Upstream pocket-tts 4.x with a config yaml (alperiox/pocket-tts-turkish 6-layer student), cloned voice."""
    from pocket_tts import TTSModel
    m = TTSModel.load_model(config=cfg["config"])
    state = m.get_state_for_audio_prompt(os.path.expanduser(cfg["ref"]))  # voice prompt cached once
    return (m, state), "CPU (torch)", m.sample_rate


def pocket4_synth(st, cfg, text):
    import numpy as np
    m, state = st
    text = text.replace("I", "ı").replace("İ", "i").lower()  # model card: no İ/I in training text → Turkish lowercase
    import torch
    torch.manual_seed(cfg.get("seed", 1))
    t, first, parts = time.perf_counter(), None, []
    for chunk in m.generate_audio_stream(state, text):
        first = first or time.perf_counter() - t
        parts.append(chunk.detach().cpu().numpy().reshape(-1).astype("float32"))
    return np.concatenate(parts), first


def ema_onnx_load(cfg):
    """EMA Lightning's own Python frontend (normalizer-tr + chunker) feeding the ozcancelik ONNX graphs, mirroring
    export/verify.py of huggingface.co/ozcancelik/ema-lightning-onnx: the networks run in onnxruntime CPU only."""
    import onnxruntime as ort
    from ema_lightning import EMA
    d = Path(os.path.expanduser("~/.cache/bakeoff-models/ema-onnx"))
    so = ort.SessionOptions()
    so.intra_op_num_threads = cfg.get("threads", 0)
    sess = {k: ort.InferenceSession(str(d / f"{k}.onnx"), so, providers=["CPUExecutionProvider"]) for k in ("text", "sound", "decoder")}
    return (EMA(device="cpu"), sess), f"CPU (onnxruntime {ort.__version__}; torch only for text frontend)", 48000


def _ema_plan(cw, wstart, dur, speed=1.0):  # numpy twin of plan() in ema-lightning-onnx export/verify.py
    import numpy as np
    dur = dur / speed
    n_words = cw[-1] + 1
    counts = np.zeros(n_words)
    np.add.at(counts, cw, dur)
    counts = np.clip(np.round(counts), 1, 250).astype(np.int64)
    frames = min(int(counts.sum()), 3000)
    fw = np.repeat(np.arange(n_words), counts)[:frames]
    starts = np.cumsum(counts) - counts
    fp = ((np.arange(frames) - starts[fw]) / counts[fw]).astype(np.float32)
    c = np.maximum(dur, 1e-4).astype(np.float32)
    done = np.cumsum(c)
    before = done - c
    total = np.zeros(n_words, np.float32)
    np.add.at(total, cw, c)
    cp = np.clip((done - before[wstart] - 0.5 * c) / np.maximum(total[cw], 1e-8), 0, 1)
    wlen = np.maximum(np.bincount(cw, minlength=n_words).astype(np.float32), 1)
    woff = np.cumsum(wlen) - wlen
    return fw, (woff[cw] + cp * wlen[cw]).astype(np.float32), (woff[fw] + fp * wlen[fw]).astype(np.float32)


def ema_onnx_synth(st, cfg, text):
    import numpy as np
    from ema_lightning.engine import CONTEXT, windows
    tts, s = st
    rng = np.random.default_rng(cfg.get("seed", 1))
    t, first, parts = time.perf_counter(), None, []
    for piece in tts._pieces(text, 1.0, cfg.get("seed", 1)):
        ids = piece.ids.numpy()[None].astype(np.int64)
        cw, wstart = piece.cw.numpy(), piece.wstart.numpy()
        h, dur = s["text"].run(None, {"ids": ids})
        fw, cg, fg = _ema_plan(cw, wstart, dur[0])
        noise = rng.standard_normal((1, 4, len(fw), 64), dtype=np.float32)
        (lat,) = s["sound"].run(None, {"h": h, "cg": cg[None], "fg": fg[None], "cw": cw[None].astype(np.int64),
                                       "fw": fw[None].astype(np.int64), "noise": noise})
        for a0, e in windows(len(fw), 25):  # 1 s first window, then 4 s (as the web engine streams)
            a, b = max(0, a0 - CONTEXT), min(len(fw), e + CONTEXT)
            (audio,) = s["decoder"].run(None, {"z": np.ascontiguousarray(lat[:, a:b].transpose(0, 2, 1))})
            parts.append(audio[0, (a0 - a) * 1920:(e - a) * 1920])
            first = first or time.perf_counter() - t
        parts.append(np.zeros(int(48000 * getattr(piece, "pause", 0.0)), np.float32))
    return np.concatenate(parts), first


def antalia_load(cfg):
    from antalia_mini import Antalia
    m = Antalia(device=cfg.get("device"))
    import torch
    return m, f"{m.device.type.upper()} (torch, {torch.get_num_threads()} threads)", m.sample_rate


def antalia_synth(m, cfg, text):
    import numpy as np
    t, first, parts = time.perf_counter(), None, []
    for chunk in m.stream(text, seed=cfg.get("seed", 1)):  # first sentence first, then sentence-sized chunks
        first = first or time.perf_counter() - t
        parts.append(np.asarray(chunk, dtype="float32").reshape(-1))
    return np.concatenate(parts), first


def seslen_load(cfg):
    from seslen_tts import Seslen
    return Seslen(denetim=cfg.get("denetim", False)), "CPU (onnxruntime)", 24000


def seslen_synth(s, cfg, text):
    import numpy as np
    np.random.seed(cfg.get("seed", 1))
    t, first, parts = time.perf_counter(), None, []
    for p in s.seslendir(text, ses=cfg["voice"], ton=cfg.get("ton", "notr")):  # one piece per sentence/clause
        first = first or time.perf_counter() - t
        parts += [p.ses, np.zeros(int(24000 * p.sessizlik_ms / 1000), np.float32)]
    return np.concatenate(parts[:-1]), first


def ahenk_load(cfg):
    d = os.path.expanduser("~/.cache/bakeoff-models/ahenk-cep-tr")
    sys.path.insert(0, d)
    from ahenk_cep import AhenkCep
    return AhenkCep(f"{d}/student.pt", f"{d}/decoder.pt"), "CPU (torch)", 48000


def ahenk_synth(m, cfg, text):
    audio, _ = m.say(text, steps=cfg.get("steps", 16), seed=cfg.get("seed", 1))
    return audio, None


def ema_load(cfg):
    from ema_lightning import EMA
    m = EMA(device=cfg.get("device", "auto"))
    return m, f"{m.device.type.upper()} (torch)", cfg.get("sample_rate", 48000)


def ema_synth(m, cfg, text):
    import numpy as np
    t, first, parts = time.perf_counter(), None, []
    for chunk in m.stream(text, seed=cfg.get("seed", 1), sample_rate=cfg.get("sample_rate", 48000)):
        first = first or time.perf_counter() - t
        parts.append(np.asarray(chunk, dtype="float32").reshape(-1))
    return np.concatenate(parts), first


def sherpa_load(cfg):
    import sherpa_onnx as so
    d = os.path.expanduser(cfg["dir"]) + "/"
    st = so.OfflineTtsSupertonicModelConfig(
        duration_predictor=d + "duration_predictor.int8.onnx", text_encoder=d + "text_encoder.int8.onnx",
        vector_estimator=d + "vector_estimator.int8.onnx", vocoder=d + "vocoder.int8.onnx", tts_json=d + "tts.json",
        unicode_indexer=d + "unicode_indexer.bin", voice_style=d + "voice.bin")
    tts = so.OfflineTts(so.OfflineTtsConfig(model=so.OfflineTtsModelConfig(
        supertonic=st, num_threads=cfg.get("threads", 2), provider="cpu")))
    return tts, f"CPU (sherpa-onnx {so.__version__}, {cfg.get('threads', 2)} threads, int8)", tts.sample_rate


def sherpa_synth(tts, cfg, text):
    import numpy as np, sherpa_onnx as so
    g = so.GenerationConfig()
    g.sid, g.num_steps, g.speed = cfg["sid"], cfg["steps"], cfg.get("speed", 1.05)
    g.extra["lang"] = "tr"
    a = tts.generate(text, g)
    return np.asarray(a.samples, dtype="float32"), None


def mlx_load(cfg):
    import mlx.core as mx
    from mlx_audio.tts.utils import load_model
    m = load_model(cfg["repo"])
    return m, f"Metal GPU (MLX {mx.default_device()})", m.sample_rate


def mlx_synth(m, cfg, text):
    import numpy as np
    kw = {k: (os.path.expanduser(v) if isinstance(v, str) and v.startswith("~") else v) for k, v in cfg.get("kwargs", {}).items()}
    t, first, parts = time.perf_counter(), None, []
    for r in m.generate(text=text, **kw):  # stream=True in kwargs → ~interval-sized chunks; else one result per segment
        a = np.asarray(r.audio, dtype="float32").reshape(-1)
        first = first or time.perf_counter() - t
        parts.append(a)
    return np.concatenate(parts), (first if cfg.get("kwargs", {}).get("stream") or len(parts) > 1 else None)


def yelda_load(cfg):
    return None, "macOS speechsynthesisd (system process)", 22050


def yelda_synth(_, cfg, text):
    import numpy as np
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "y.wav"
        subprocess.run(["say", "-v", cfg["voice"], "-r", str(cfg.get("rate", 185)), "-o", str(out),
                        "--file-format=WAVE", "--data-format=LEI16@22050", text], check=True)
        with wave.open(str(out)) as w:
            a = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype("float32") / 32768
    return a, None


def main():
    run_id, cfg = sys.argv[1], json.loads(sys.argv[2])
    eng = cfg["engine"]
    load, synth = globals()[f"{eng}_load"], globals()[f"{eng}_synth"]
    t_imp = time.perf_counter()
    state, device, sr = load(cfg)
    t_ready = time.perf_counter()
    out = ROOT / "audio" / "raw" / run_id
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for s in CORPUS:
        t = time.perf_counter()
        res = synth(state, cfg, s["text"])
        el = time.perf_counter() - t
        samples, first = res[0], res[1]
        if len(res) > 2:  # xtts: synth time is the full (non-stream) pass only
            el = res[2]
        write_wav(out / f"{s['id']}.wav", samples, sr)
        audio_ms = len(samples) / sr * 1000
        row = {"id": s["id"], "text": s["text"], "synth_ms": round(el * 1000, 1), "audio_ms": round(audio_ms, 1),
               "rtf": round(el * 1000 / audio_ms, 4) if audio_ms else None}
        if first is not None:
            row["first_chunk_ms"] = round(first * 1000, 1)
        rows.append(row)
        print(f"{run_id} {s['id']}: synth {row['synth_ms']} ms, audio {row['audio_ms']} ms, first {row.get('first_chunk_ms')}", flush=True)
    rep = {"run": run_id, "config": cfg, "import_ms": ms(T0, t_imp), "load_ms": ms(T0, t_ready), "device": device,
           "sample_rate": sr, "sentences": rows}
    (ROOT / "raw").mkdir(exist_ok=True)
    (ROOT / "raw" / f"{run_id}.json").write_text(json.dumps(rep, ensure_ascii=False, indent=1))
    print(f"{run_id}: load {rep['load_ms']} ms on {device}", flush=True)


if __name__ == "__main__":
    main()
