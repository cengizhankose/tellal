# Research

The notes behind t3-panel's two big choices: which local Turkish voice to use, and how the
panel should look next to T3 Code. Dated November 2026. Claims link to their sources;
anything not measured or read directly is marked `[INFERENCE]`.

## Voice (TTS)

Question: which open, local text-to-speech model reads Turkish agent messages best on an
Apple Silicon Mac, at zero API cost? Answer: EMA Lightning on ONNX Runtime, inside the Rust
process.

| File | What it is |
|---|---|
| [`tts/README.md`](tts/README.md) | Summary and decision (in Turkish): shortlist, why EMA Lightning, the Rust engine's numbers. |
| [`tts/landscape.md`](tts/landscape.md) | 21 open models with Turkish support: licence, size, runtime, Rust integration path. |
| [`tts/community.md`](tts/community.md) | What Turkish users and agent-narration projects report (Reddit, Hugging Face, GitHub, forums). |
| [`tts/bakeoff.md`](tts/bakeoff.md) | The bake-off: 59 runs on an M4 Pro, 7 panel-style sentences, load time, time to first audio, RTF, memory, Whisper read-back WER. |
| [`tts/bakeoff/index.html`](tts/bakeoff/index.html) | Sortable results table of the bake-off (open locally; the audio players need the git-ignored audio files). |
| [`tts/bakeoff/results.json`](tts/bakeoff/results.json), [`tts/bakeoff/harness/`](tts/bakeoff/harness/), [`tts/bakeoff/raw/`](tts/bakeoff/raw/) | Raw numbers, the benchmark harness and per-run measurements. |
| [`tts/compare.html`](tts/compare.html) | Chart of the leading candidates. |

Audio files (`tts/listen/`, `tts/bakeoff/audio/`) are not in the repository. To hear the
panel's voice, watch the [demo video](../docs/media/demo.mp4).

## UI

| File | What it is |
|---|---|
| [`ui/t3-sidebar.md`](ui/t3-sidebar.md) | Measured study of T3 Code's thread sidebar (sizes, tokens, status glyphs, order, provider and project marks), with source line references, and how the panel's design maps onto it. |
| [`ui/assets-preview.png`](ui/assets-preview.png) | Rendered sheet of the bundled icons and provider marks. |

The design spec built on this study is [`docs/DESIGN.md`](../docs/DESIGN.md).
