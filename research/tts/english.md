# English voice for the panel (19 Nov 2026)

Question: which local English voice ships next to EMA Lightning (Turkish) in a downloadable MIT
macOS app? Hard constraints: CPU on Apple Silicon, inference through `ort` (or another permissive
pure-Rust path), weights and every runtime dependency shippable with an MIT binary (no GPL: no
espeak-ng), download ≤ 150 MB, RTF ≲ 0.1 per sentence and fast first audio (EMA: RTF ~0.04,
~250 ms click-to-audio).

**Pick: Supertonic 2, voice M1, with sherpa-onnx's int8 vector estimator and vocoder** —
`en-supertonic-2`, 96 MB. Character input (no G2P at all), the fastest and most accurate
candidate in the bake-off, and the reference pipeline is four ONNX graphs that port directly to
`ort` (`crates/core/src/supertonic.rs`).

## Candidates and licenses

| Model | Weights license | Text front end | Download | Verdict |
|---|---|---|---|---|
| **Supertonic 2** (Supertone) | [BigScience OpenRAIL-M](https://huggingface.co/Supertone/supertonic-2/blob/main/LICENSE): royalty-free, redistribution and commercial use allowed; the use restrictions (Attachment A) must be passed on. Reference code [MIT](https://github.com/supertone-oss-archive/supertonic/blob/main/LICENSE) | Unicode characters → ids (`unicode_indexer.json`), no phonemizer | fp32 263 MB; **96 MB** with [sherpa-onnx's int8](https://huggingface.co/csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06) `vector_estimator` + `vocoder` (its `duration_predictor`/`text_encoder` are byte-identical to Supertone's) | **Picked** |
| Supertonic 3 (Supertone) | [OpenRAIL-M](https://huggingface.co/Supertone/supertonic-3/blob/main/LICENSE) | characters, 31 languages | fp32 398 MB; [sherpa int8](https://huggingface.co/csukuangfj2/sherpa-onnx-supertonic-3-tts-int8-2026-05-11) 145 MB | 4× slower than v2 in English (RTF 0.126), 0.5 s leading silence |
| Kokoro-82M (hexgrad) | [Apache-2.0](https://huggingface.co/hexgrad/Kokoro-82M) | needs phonemes: misaki (Apache) lexicon + **espeak-ng (GPL-3.0)** fallback | [int8 ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX) 92 MB + voice | Out: GPL G2P (a permissive OOV G2P would have to be built); RTF 0.71 in this test |
| KittenTTS nano (KittenML) | [Apache-2.0](https://huggingface.co/KittenML/kitten-tts-nano-0.2) | `phonemizer` → **espeak-ng (GPL)** | 24 MB | Out: GPL G2P; RTF 0.12 |
| Pocket TTS (Kyutai) | [CC-BY-4.0](https://huggingface.co/kyutai/pocket-tts) (gated on HF); [ONNX export](https://huggingface.co/KevinAHM/pocket-tts-onnx) CC-BY-4.0 | SentencePiece tokens (no G2P), voice from a reference clip | int8 ~125 MB | Out: RTF 0.13, 700 MB RSS, autoregressive, needs a SentencePiece port + voice encoding |
| Piper en (rhasspy / OHF) | voices mostly MIT/CC; engine [piper1-gpl](https://github.com/OHF-Voice/piper1-gpl) GPL-3.0 | `piper-phonemize` → **espeak-ng (GPL)** | 60–75 MB | Out without running: GPL phonemizer |

Licensing notes: OpenRAIL-M is not an OSI license, but it grants a free, irrevocable copyright
license to reproduce and distribute the weights; Tellal downloads them from the publisher at run
time (not bundled) and installs Supertone's `LICENSE` next to them. sherpa-onnx 2.0.2 itself
refused Kokoro/Kitten without a lexicon file (it no longer phonemizes with espeak-ng); the bake-off
ran those two on sherpa-onnx 1.12.40 only to measure them. Supertone archived the Supertonic
repository in 2026 (weights stay published; no further development).

## Bake-off (existing harness, English pass)

Same harness as the Turkish bake-off: `bench.py` (fresh process per entry under
`/usr/bin/time -l`), `run.py` with `BAKEOFF_RUNS=runs.en.json BAKEOFF_CORPUS=corpus.en.json`,
read-back with `readback_en.py` (whisper-cli `ggml-large-v3-turbo-q8_0`, `-l en`; WER on both
sides after the same canonical form: digits/times spelled out, `ms`/`milliseconds` and similar
merged). Corpus: 10 agent lines (`harness/corpus.en.json`: "80 tasks: 20 stuck, 30 failed…",
"npm test passed in 42 seconds…", "3:45 PM", "14:30", "120 ms", PR, build, deploy, iOS), fed raw.
All entries through sherpa-onnx, CPU, 2 threads, int8. Apple M4 Pro, 48 GB. Results:
`bakeoff/en-results.json`.

| Run | Load ms | Synth p50 ms | RTF | Max RSS MB | WER |
|---|---|---|---|---|---|
| Supertonic 2 int8, M1, 5 steps | 280 | 123 | **0.030** | 387 | **0.0%** |
| Supertonic 2 int8, F1, 5 steps | 238 | 133 | 0.030 | 385 | 3.8% |
| Supertonic 2 int8, M1, 3 steps | 227 | 94 | 0.022 | 405 | 7.5% |
| Supertonic 3 int8, M1, 5 steps | 285 | 595 | 0.126 | 447 | 1.9% |
| Pocket TTS int8, `bria` reference | 368 | 490 | 0.134 | 700 | 4.7% |
| KittenTTS nano v0.2 fp16 | 409 | 682 | 0.120 | 300 | 3.8% |
| Kokoro-82M v0.19 int8 (sherpa 1.12) | 411 | 2784 | 0.709 | 641 | 0.0% |

Per-sentence misses are in `en-results.json`. "npm" was the most common ("NPN", "Magnium",
"End PMM"); raw "120 ms" came back as "120 miles" (Supertonic 3) and "20 minutes" (Pocket), and
"Pull request" as "Poll request" or "call request".

## In the panel (Rust, `tellal-bench --voice en|tr`)

`tellal-bench --voice en` (12 lines: the corpus above plus a path/hash line and a
percent/MB/version line) vs `--voice tr` (EMA's 10-line Turkish corpus), 2 ONNX threads, fresh
process, M4 Pro:

| Voice | Cold load | Per sentence (min–median–max) | RTF | Peak RSS | Whisper WER |
|---|---|---|---|---|---|
| `en-supertonic-2` (8 steps) | 252 ms | 139–173–231 ms | 0.038 | 245 MB | **0.8%** (1 word in 12 lines: "2.4 MB" heard as "2.5") |
| `tr-ema-lightning` | 526 ms | 20–136–234 ms | 0.041 | 156 MB | (Turkish bake-off) |

Read-back: `bakeoff/en-tellal-readback.json` (`readback_en.py --tellal <run dir>`).
What changed after the read-back:

- **8 flow steps, not 5.** With 5, short lines lost words ("Running the tests now." → "Running
  now.", across three noise seeds); 8 read every line (RTF 0.028 → 0.038).
- **Normalization before the voice** (`crates/core/src/english.rs`): times ("3:45 PM" → "three
  forty five PM", "14:30" → "fourteen thirty"), dates, money, percentages, ordinals, ranges,
  `#42`, versions ("v2.1.0" → "version two point one point zero"), numbers with units ("120 ms" →
  "one hundred twenty milliseconds", "2.4 MB" → "two point four megabytes"); URLs → "link",
  commit hashes dropped, paths reduced to the file name ("src/panel/voice.rs" → "voice dot R S").
- **Respellings** (`english::BUILTIN`): "npm" → "N P M", "pull request" → "pul request" (heard
  "Poll request" otherwise), plus CLI, UI, JSON, YAML, macOS, README, TODO, LGTM, Wispr.

First audio: the first sentence takes ~140–230 ms to synthesize, about EMA's, so click-to-audio
stays in EMA's range once the voice is loaded; the first English line of a session also pays the
one-time ~250 ms load when Turkish is the preferred voice [INFERENCE: from the load and synth
times above, not measured end to end with an English catch-up line].

## Cleanup

The trial venvs, models (~880 MB) and audio were deleted after the runs; only the JSON results
stay in `bakeoff/`.
