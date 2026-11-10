# Local Turkish TTS landscape for t3-panel (checked 2026-11-11)

Scope: open-weight TTS models released or updated 2024–2026 that run locally on Apple Silicon and support Turkish, either officially or through a public fine-tune. Every claim links to a page I or a research sub-agent opened. Anything not verified is marked **[INFERENCE]**. No audio was played and no listening judgement was made. Quality evidence comes from model cards and public benchmarks only; the bake-off in `research/tts/bakeoff/` covers the listening side.

## TL;DR

1. **Supertonic 3 already runs inside the Rust crate the panel uses.** The pinned `sherpa-onnx =1.13.8` already ships `OfflineTtsSupertonicModelConfig` ([docs.rs 1.13.8](https://docs.rs/sherpa-onnx/1.13.8/sherpa_onnx/all.html); also in [2.0.1](https://docs.rs/sherpa-onnx/2.0.1/sherpa_onnx/all.html)).
   - Turkish is selected with `extra["lang"]="tr"` ([sherpa Turkish page](https://k2-fsa.github.io/sherpa/onnx/tts/all/Turkish/supertonic-3-tr.html)).
   - Model: int8 archive of 129 MB (measured with `curl -I` on the [release asset](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2)), 10 speakers, a built-in text frontend and no espeak.
   - This is the cheapest upgrade from Piper.
2. **A new generation of tiny Turkish-only models now beats every older open model on intelligibility in Turkish** ([Freya-TR-Eval re-run](https://speech.patientdesk.ai/benchmarks)):
   - EMA Lightning: 8.6M params, Apache-2.0.
   - Antalia-2 Mini: 7.6M params, Apache-2.0.
   - Ahenk Cep: 8.6M params, CC-BY-4.0.
   - All three have ONNX graphs, and [Hitabet-Turkce-TTS](https://github.com/mesutde/Hitabet-Turkce-TTS) already runs them from Rust with the `ort` crate and a Turkish normalizer written in Rust.
3. **Cactus (Cactus Compute, YC S25) is not a TTS runtime.** Its engine runs LLMs, VLMs and ASR models. Its own models are Needle (tool calling), Whistle (ASR in 7 languages, no Turkish) and Mirage (vision). It has no TTS model and no Turkish speech. See the Cactus section below.
4. **Most promising heavier open models with Turkish:**
   - **ZONOS3-Mini** (Zyphra, released 2026-11-09, Apache-2.0): 100M active params, streaming, voice cloning, and a C++/ggml runtime with Metal.
   - **VoxCPM2** and its Turkish fine-tune **Trendyol-TTS**: best naturalness on the Turkish benchmark (UTMOS 3.82), but 2B+ params and slow.

## Turkish benchmark used for comparison

The PatientDesk re-run of Freya-TR-Eval uses one harness for every system: 495 sentences, 3 takes each, faster-whisper large-v3, and RTF measured on an NVIDIA L4 ([table in the Antalia-2 Mini card](https://huggingface.co/cloud0day3/antalia-mini), [dataset](https://huggingface.co/datasets/PatientdeskAI/freya-tr-eval-rerun), [page](https://speech.patientdesk.ai/benchmarks)). WER measures intelligibility. UTMOS is an automatic naturalness predictor, not a MOS study.

| System | Params | WER % | CER % | UTMOS | RTF (L4) |
|---|---|---|---|---|---|
| EMA Lightning | 8.6M | 1.04 | 0.225 | 3.30 | 0.017 |
| Antalia-2 Mini 1.0.1 | 7.62M | 1.09 | 0.220 | 3.18 | 0.029 |
| Trendyol-TTS (VoxCPM2 FT) | 2.38B | 1.16 | 0.23 | **3.82** | 0.962 |
| Anka TTS | 336M | 1.42 | 0.30 | 3.52 | 0.322 |
| Chatterbox Multilingual | 500M | 1.91 | 0.43 | 3.51 | 2.892 |
| **Piper tr_TR dfki (current panel)** | 16M | 3.41 | 0.73 | 3.71 | 0.038 |
| XTTS-v2 | 470M | 3.67 | 1.50 | 2.97 | 0.455 |
| Antalia-1 | 305M | 6.28 | 2.49 | 2.72 | 0.476 |
| MMS-TTS tur | 36M | 6.38 | 1.48 | 3.79 | 0.014 |
| FreyaTTS-small | 183M | 12.02 | 4.95 | 2.81 | 0.286 |

Other results reported on the same test set, each from its own model card:
- Pocket TTS Turkish (wite-tech): 1.9% WER ([card](https://huggingface.co/wite-tech/pocket-tts-turkish)).
- IndexTTS-2.5 Turkish LoRA: 1.32% WER ([card](https://huggingface.co/berrkkaayy/IndexTTS-2.5-Turkish-LoRA)).
- Breeze-TTS-2-Turkish-Base: UTMOS 4.07 ([card](https://huggingface.co/ersanbil/Breeze-TTS-2-Turkish-Base)).

Supertonic 3, ZONOS3-Mini, MOSS-TTS-Nano and OmniVoice have **no Freya-TR-Eval entry**.

## Main table: Turkish-capable models

Abbreviations: **sh** = sherpa-onnx (C/C++/Rust/…), **ort** = the `ort` crate (ONNX Runtime), **MLX** = mlx-audio unless stated, **GGUF** = ggml or llama.cpp, **PT** = PyTorch.

| Model | Turkish? How | Params / download | License | Runtimes | Streaming | Speed evidence | Rust path | Sources |
|---|---|---|---|---|---|---|---|---|
| **Supertonic 3** (Supertone, 2026-04-29) | Yes, official: one of 31 languages, `lang=tr`. 10 preset styles (F1–F5, M1–M5) shared across languages; no Turkish-specific voice | ~99M; ONNX fp32 ~415 MB; sherpa int8 archive 129 MB | OpenRAIL-M weights (use restrictions, commercial use allowed), MIT code | ONNX (official examples in many languages incl. Rust), **sh** (int8), CoreML, GGUF, LiteRT ports | No streaming API; non-autoregressive, chunk by sentence [INFERENCE] | Vendor: avg RTF 0.3 on an Onyx Boox e-reader. Local Reels lab picked it as default (`~/.venvs/supertonic`). No published Mac RTF | **Best: sherpa-onnx crate already pinned** (`OfflineTtsSupertonicModelConfig`, `extra lang=tr`); or `ort` via the official `rust/` example. The upstream repo is archived | [HF](https://huggingface.co/Supertone/supertonic-3), [GitHub (archived)](https://github.com/supertone-inc/supertonic), [sherpa doc](https://k2-fsa.github.io/sherpa/onnx/tts/supertonic.html), [sherpa tr page](https://k2-fsa.github.io/sherpa/onnx/tts/all/Turkish/supertonic-3-tr.html), [Rust example](https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/supertonic_tts.rs) |
| **Antalia-2 Mini** (PatientDesk, 2026-10) | Yes, Turkish-only. One synthetic male voice. Its normalizer spells out numbers, dates and amounts **and respells English and brand words** | 7.62M; 31 MB fp32 / 15 MB fp16; ONNX ~49–51 MB | Apache-2.0 (weights + inference code) | PT (`pip install antalia-mini`), ONNX (official browser Space incl. int8; community `fr0stb1rd/antalia-mini-web-onnx`) | Yes, `stream()` sentence chunks | Card: **Apple M5 CPU, 2 threads: RTF 0.024; time to first audio 72 ms**. L4 RTF 0.029 | `ort` with ONNX graphs; working Rust reference in Hitabet (`antalia_engine.rs`, `antalia_text.rs`) | [card](https://huggingface.co/cloud0day3/antalia-mini), [Space](https://huggingface.co/spaces/cloud0day3/antalia-mini), [Hitabet](https://github.com/mesutde/Hitabet-Turkce-TTS) |
| **EMA Lightning** (2026-10) | Yes, Turkish-only. One synthetic voice. Foreign words are read with Turkish spelling rules (card limitation) | 8.6M (5.6M DiT + 3M vocoder); 34 MB; ONNX 35 MB (+fp16) | Apache-2.0 | PT (`pip install ema-lightning`), ONNX (`ozcancelik/ema-lightning-onnx` with WASM normalizer from Rust source), MLX (`faraday/ema-lightning-mlx`), LiteRT | Yes, first chunk = 1 s of audio | Card: CPU ~6× real time (cloud container); 4090 TTFA 3.86 ms. ONNX card: ~140 ms TTFA in browser on an Apple laptop GPU | `ort`; Hitabet `tts_engine.rs` + `turkish_tokenizer.rs`; the normalizer-tr Rust source ships in the ONNX repo | [card](https://huggingface.co/canberkkkkkk/ema-lightning), [ONNX](https://huggingface.co/ozcancelik/ema-lightning-onnx), [Hitabet](https://github.com/mesutde/Hitabet-Turkce-TTS) |
| **Ahenk Cep** (2026-10) | Yes, Turkish-only fine-tune of EMA. One designed female voice, 7 style tags | 8.6M; 34 MB; ONNX published | CC-BY-4.0 weights, Apache code | PT, ONNX | [INFERENCE] same as EMA | Card: CPU 2 threads RTF 0.20 (slower than EMA 0.13 / Antalia 0.08) | `ort`; Hitabet `ahenk_engine.rs` | [card](https://huggingface.co/ozgurguler/ahenk-cep-tr) |
| **Pocket TTS Turkish** (wite-tech, 2026-09-28) | Yes, via fine-tune of Kyutai Pocket TTS. 6 voices, 5 emotion tags, cloning | ~110M; 442 MB | CC-BY-4.0 | PT (`pocket-tts-turkish`) | Yes (`tts.stream`) | Card: ~5× real time on one CPU thread; first piece <0.1 s after a ~0.4 s first call. 1.9% WER. Local Reels lab: MPS crashes, CPU ~13× real time | Base Pocket TTS has a candle port (crate `pocket-tts`, Metal feature) and a sherpa Pocket family (English int8 model only). Loading the Turkish weights plus its number normalizer there is **[INFERENCE]** | [card](https://huggingface.co/wite-tech/pocket-tts-turkish), [pkg](https://github.com/witetech/pocket-tts-turkish), [Kyutai](https://github.com/kyutai-labs/pocket-tts), [candle port](https://github.com/babybirdprd/pocket-tts), [sherpa pocket](https://k2-fsa.github.io/sherpa/onnx/tts/pocket.html) |
| Pocket TTS Turkish (alperiox, 2026-10-11) | Yes, fine-tune trained on 262 h of synthetic speech | 6-layer 398 MB; 24-layer 1.3 GB | MIT | PT | [INFERENCE] yes (base) | Card: 5.1× CPU real time (6-layer) | Same as above | [card](https://huggingface.co/alperiox/pocket-tts-turkish) |
| **ZONOS3-Mini** (Zyphra, 2026-11-09) | Yes, official: 43 languages incl. Turkish. **No Turkish default voice**; voice cloning needs no transcript. Text normalization does not cover Turkish (raw pass-through) | 800M total / ~100M active MoE; minimal GGUF set ~1.16 GB (ptq 0.66 + keep2 decoder 0.45 + speaker encoder 0.045); MLX 1.41 GB | Apache-2.0 weights, MIT code | **C++/ggml CLI + server (Metal default on Apple Silicon)**, MLX (mlx-audio + own server), WebGPU, SGLang | Yes (prefix streaming, first audio after the first frame) | Card: 80 ms TTFA on H100. Zyphra blog: <150 ms on M3 MacBook Air (per sub-agent) | ggml lib via FFI (**[INFERENCE]**, no documented C API) or local `zonos3-server` HTTP sidecar streaming PCM | [card](https://huggingface.co/Zyphra/ZONOS3-Mini), [ggml README](https://raw.githubusercontent.com/Zyphra/ZONOS3-Mini/main/servers/ggml/README.md), [voices](https://huggingface.co/Zyphra/ZONOS3-Mini/raw/main/gguf/voices/README.md), [MLX](https://huggingface.co/mlx-community/ZONOS3-Mini), [blog](https://www.zyphra.com/our-work/zonos3-mini) |
| **Chatterbox Multilingual v3** (Resemble) | Yes, official: one of 23 languages. Voice cloning; the user's cloned voice is already set up locally | 0.5B; t3 v3 2.14 GB + s3gen 1.06 GB; MLX 2.7 GB; ONNX LM q4 354 MB | MIT, Perth watermark. **Upstream retired Aug 2026** | PT (MPS), MLX, ONNX (`onnx-community`), GGUF (`ggml-org/chatterbox-GGUF`, needs llama.cpp PR) | Not officially | Benchmark RTF 2.892 on L4 (slow); WER 1.91% | No native Rust; GGUF via llama.cpp FFI or multi-graph ONNX via `ort` **[INFERENCE]**; realistically a sidecar | [GitHub](https://github.com/resemble-ai/chatterbox), [MLX](https://huggingface.co/mlx-community/chatterbox-multilingual-v3), [ONNX](https://huggingface.co/onnx-community/chatterbox-multilingual-ONNX), [GGUF](https://huggingface.co/ggml-org/chatterbox-GGUF) |
| **VoxCPM2** (OpenBMB, 2026-04) | Yes, official: one of 30 languages. Community Turkish LoRA `mramazan/voxcpm2-turkish-tts-lora` (single speaker, number/date normalization) | 2B; 4.96 GB; MLX 4-bit 2.3 GB / 8-bit 3.2 GB | Apache-2.0 | PT (MPS), MLX, llama.cpp-omni GGUF (`voxcpm2-cli`, Metal), vLLM | Yes (`generate_streaming`) | README: RTF ~0.30 on RTX 4090. Minimax-MLS Turkish WER 0.817 vs ElevenLabs 0.699. No Mac numbers | GGUF C++ via FFI **[INFERENCE]** or MLX sidecar | [HF](https://huggingface.co/openbmb/VoxCPM2), [GitHub](https://github.com/OpenBMB/VoxCPM), [MLX 4bit](https://huggingface.co/mlx-community/VoxCPM2-4bit), [TR LoRA](https://huggingface.co/mramazan/voxcpm2-turkish-tts-lora) |
| **Trendyol-TTS** (2026-06) | Yes, Turkish fine-tune of VoxCPM2 (merged LoRA, ~20 h private data) | 2.38B; 5.1 GB; MLX port 9.3 GB | MIT (card says to check VoxCPM2 + dataset terms) | PT (`voxcpm`); MLX (`emrekoc/trendyol-tts-mlx`) | [INFERENCE] as VoxCPM2 | Benchmark RTF 0.962 on L4; **best open UTMOS 3.82** | Same as VoxCPM2 | [card](https://huggingface.co/Trendyol/Trendyol-TTS) |
| **MOSS-TTS-Nano-100M** (OpenMOSS, 2026-04) | Yes, official: one of 20 languages. Voice-clone mode is the main workflow | 0.1B; 238 MB PT + 20M audio tokenizer; MLX 285 MB | HF metadata Apache-2.0, but the GitHub README says "not yet licensed for redistribution" until a LICENSE file lands | PT, **official ONNX CPU backend** (`--backend onnx`), MLX, vLLM-Omni | Yes | Card: streaming "can run on a 4-core CPU". No numbers | `ort` with its ONNX export **[INFERENCE]** | [MLX card](https://huggingface.co/mlx-community/MOSS-TTS-Nano-100M), [GitHub](https://github.com/OpenMOSS/MOSS-TTS-Nano) |
| **Audio8-TTS-Realistic-Air** 0.6B (Edge0, 2026-10) | Yes, official: one of 23 languages ("quality varies") | 0.6B + codec; MLX 4-bit 1.7 GB / 8-bit 2.0 GB | Apache-2.0 | PT (custom Transformers code), MLX | No ("one complete waveform per call") | Card: CV3 **Turkish WER 12.57%**, SIM 0.65. No Mac numbers | MLX sidecar only | [card](https://huggingface.co/Edge0/Audio8-TTS-Realistic-Air), [MLX](https://huggingface.co/nativ-community/Audio8-TTS-Realistic-0.6b-bf16) |
| **OmniVoice** (k2-fsa, 2026-03; updated 07) | Yes, official: one of 600+ languages (`tr`). Voice cloning; auto voice | Qwen3-0.6B base; 3.27 GB; MLX bf16 1.64 GB | **CC-BY-NC** weights, Apache code | PT (MPS), MLX; community WebGPU ONNX | Not documented | Card: "RTF as low as 0.025" (GPU). Number normalization for non-zh/en falls back to num2words | None; sherpa-onnx support requested in [#3486](https://github.com/k2-fsa/sherpa-onnx/issues/3486), not shipped | [HF](https://huggingface.co/k2-fsa/OmniVoice), [GitHub](https://github.com/k2-fsa/OmniVoice), [MLX](https://huggingface.co/mlx-community/OmniVoice-bf16) |
| **Anka TTS** v0.1 (2026-09) | Yes, Turkish F5-TTS. Male and female voices + cloning | 336M; 1.36 GB | **CC-BY-NC-4.0** weights | PT (`anka-tts`, mps/cpu) | No | Benchmark RTF 0.322 on L4; WER 1.42%, UTMOS 3.52 | Sidecar | [card](https://huggingface.co/krmkayabasi/Anka-TTS) |
| **XTTS-v2** (Coqui; fork `idiap/coqui-ai-TTS` v0.28) | Yes, official: one of 17 languages. Voice cloning | ~470M; ~2.09 GB | **CPML, non-commercial** | PT; GGUF for audio.cpp / xtts-rs (candle; en/fr tested only) | Yes (Coqui streaming inference) | Benchmark RTF 0.455 on L4; WER 3.67% (worse than Piper) | xtts-rs **[INFERENCE for Turkish]**; else sidecar | [HF](https://huggingface.co/coqui/XTTS-v2), [fork](https://github.com/idiap/coqui-ai-TTS), [xtts-rs](https://github.com/rzafiamy/xtts-rs) |
| **Piper tr_TR** (dfki official; fahrettin/fettah via sherpa) | Yes. Community voices: `99eren99/piper-turkish-tts`, `ilkeraktuna/piper-tts-tr-ilkiz-medium` (MIT), `dcx514ai/piper_tts_turkish_high` (MIT) | 16M; ~60–85 MB fp32, ~21 MB int8 (sherpa) | dfki **data CC-BY-NC-SA-4.0**; piper1 code GPL-3 | sh (VITS), piper | Sentence-level | Benchmark RTF 0.038 on L4; WER 3.41% | sherpa-onnx crate (current); 2.0.0 dropped espeak-ng → stay on 1.13.8 or feed `phoneme_codepoints` | [piper-voices](https://huggingface.co/rhasspy/piper-voices), [dfki card](https://huggingface.co/rhasspy/piper-voices/raw/main/tr/tr_TR/dfki/medium/MODEL_CARD), [sherpa C API note](https://k2-fsa.github.io/sherpa/onnx/c-api/html/tts.html), [99eren99](https://huggingface.co/99eren99/piper-turkish-tts) |
| Higgs Audio v3 (Boson, 2026-06) | Yes, official; "WER/CER under 5" tier | ~4B; 9.32 GB | Research and non-commercial license | SGLang/vLLM-Omni, MLX | Yes (SSE) | Card: RTF 0.147 on GPU | Sidecar | [HF](https://huggingface.co/bosonai/higgs-audio-v3-tts-4b) |
| MOSS-TTS v1.5 (8B / 1.7B Local) | Yes, official: one of 31 languages | 17 GB repo | Apache-2.0 | PT, MLX | MOSS-TTS-Realtime: 180 ms TTFB | – | Sidecar | [HF](https://huggingface.co/OpenMOSS-Team/MOSS-TTS-v1.5) |
| IndexTTS-2.5 Turkish LoRA | Yes, via fine-tune (486.5 h, 3,652 speakers) | ~0.8B GPT + decoders; 6.5 GB | bilibili custom license [not reviewed] | PT on NVIDIA (~6 GB VRAM); MLX ports of the base exist (Turkish untested) | No | WER 1.32%; RTF 0.83 on RTX 5070 Ti | Sidecar | [card](https://huggingface.co/berrkkaayy/IndexTTS-2.5-Turkish-LoRA) |
| KittenTTS 2 (KittenML, 2026-09-30) | Yes, official: Turkish voices `222_fisherman_tr`, `224_news_anchor_tr`, `225_grandma_cooking_tr`; `normalize=False` for Turkish | 1.7B (ternary LM 954 MB + decoder 459 MB or 39 MB student) | Stellon Community License (free below USD 1M) | PT; llama.cpp fork (C++, example builds with `GGML_METAL=OFF`) | – | None published | C++ fork via FFI **[INFERENCE]** | [HF](https://huggingface.co/KittenML/kitten-tts-2), [cpp](https://github.com/KittenML/kitten-tts-2-cpp) |
| Breeze-TTS-2-Turkish-Base (2026-11) | Yes, Turkish continual pretraining with Turkish/English code-switching | ~3.5B; 7.7 GB | **BreezeBlue Research and Non-Commercial** | PT (GPU); mlx-audio has a Breeze TTS loader (Turkish ckpt untested) | Yes (streaming runtime) | Card: UTMOS 4.07, RTF 0.343, TTFA 186.5 ms on GPU | Sidecar | [card](https://huggingface.co/ersanbil/Breeze-TTS-2-Turkish-Base) |
| F5-TTS Turkish fine-tunes | Via fine-tunes: marduk-ra (CC-BY-NC), Karayakar (MIT tag), OpenBible (CC-BY-SA); ONNX `patientxtr/F5_TTS_ONNX_Turkish` | 336M | Mostly NC (F5 base CC-BY-NC) | PT; ONNX via DakeQQ/F5-TTS-ONNX | No | None published (Anka, the stronger F5 Turkish model, is above) | `ort` **[INFERENCE]** | [marduk-ra](https://huggingface.co/marduk-ra/F5-TTS-Turkish), [ONNX](https://huggingface.co/patientxtr/F5_TTS_ONNX_Turkish) |
| Fish Audio S2 Pro (2026-03) | Partial: `tr` under "Other supported languages" (not tier 1/2) | ~4.4B; 11 GB | Fish Audio Research License | PT | Yes | RTF 0.195 on H200 | Sidecar | [HF](https://huggingface.co/fishaudio/s2-pro) |
| MMS-TTS tur (Meta) | Yes, official | 36M; 0.29 GB | CC-BY-NC-4.0 | Transformers; sherpa VITS after conversion (no prebuilt `vits-mms-tur`) | No | RTF 0.014 on L4; WER 6.38% | sherpa VITS (character tokens, no espeak) **[INFERENCE]** | [HF](https://huggingface.co/facebook/mms-tts-tur), [sherpa MMS](https://k2-fsa.github.io/sherpa/onnx/tts/mms.html) |
| Orpheus Turkish fine-tunes | Community only: Karayakar PT-5000 (MIT, GGUF Q5 2.4 GB), AbDhumal v2 (Apache, WER 0.72 on own 10 sentences, RTF 2.2) | 3B | MIT / Apache | GGUF (llama.cpp) + SNAC decoder | Yes (base) | Too slow [INFERENCE: 3B at ~84 tok/s needed] | Sidecar | [Karayakar](https://huggingface.co/Karayakar/Orpheus-TTS-Turkish-PT-5000), [AbDhumal](https://huggingface.co/AbDhumal/orpheus-3b-turkish-tts-v2) |

Other experimental Turkish fine-tunes with no eval and research-only terms or test-grade cards (not shortlisted):
- [Qwen3-TTS Turkish LoRA](https://huggingface.co/hcfk/qwen3-tts-turkish) ("foreign accent… Production-ready: No")
- [Kani-TTS 400M tr](https://huggingface.co/Anilosan15/kani-tts-400m-0.3-tr) (academic only)
- [VibeVoice Turkish narrator LoRA](https://huggingface.co/onurerkan/vibevoice-turkish-narrator-v0.1) ("just for testing")
- [Dia-1.6B-tr](https://huggingface.co/Anilosan15/Dia-1.6B-tr)
- [CSM Turkish](https://huggingface.co/omersaidd/sesame_csm_nisan_kumru_turkish_4000_steps)
- [Llasa Turkish](https://huggingface.co/Cosmobillian/turkish_llasa_tts_v1)

## Cactus (Cactus Compute)

- **Company:** Cactus Compute, Inc., [YC Summer 2025](https://www.ycombinator.com/companies/cactus-compute), founded 2025, team of 8. YC describes it as a fast inference engine optimized for smartphones, with React Native, Flutter and Kotlin bindings.
- **Today it is two products:**
  1. **`cactus-compute/cactus`**, the Python package `cactus-compute` 4.x ([repo](https://github.com/cactus-compute/cactus), Apache-2.0, 14.6k stars). It is now a *model family*:
     - **Needle 3**: a 29–121M tool-calling and extraction model.
     - **Whistle**: 16.9 MB speech-to-text in en/de/fr/es/it/nl/pl only.
     - **Mirage**: vision.
     - The package runs on a sub-1 MB C++ engine with `.cact` weights ([llms.txt](https://raw.githubusercontent.com/cactus-compute/cactus/main/llms.txt), [legacy README](https://github.com/cactus-compute/cactus/tree/legacy)).
  2. **`cactus-compute/engine`**, the earlier general inference SDK, now `pip install cactus-engine` ([repo](https://github.com/cactus-compute/engine), [PyPI](https://pypi.org/project/cactus-engine/)).
     - C/C++ engine with custom "Cactus Quants" kernels, plus CPU and Metal backends.
     - Bindings for Swift, Kotlin, Flutter, React Native, Python and **Rust** (raw `extern "C"` declarations in `bindings/rust/cactus.rs`, linked against a local build) ([Rust bindings](https://github.com/cactus-compute/engine/tree/main/bindings/rust)).
     - macOS is first-class: `brew install cactus-compute/cactus/cactus`, with benchmarks on an M4 Pro.
     - Tested model families: Liquid LFM2/2.5, Gemma 3/3n/4, Qwen3/3.5, Whisper, Parakeet and Moonshine ([HF org listing](https://huggingface.co/Cactus-Compute)).
- **TTS: none today.**
  - None of the 65 models in the Cactus-Compute HF org is a TTS model (listed via the HF API).
  - The engine's `docs/cactus_engine.md` contains no "tts", "synthes" or "vocoder" mentions, and no TTS path exists in the repo tree (checked via the raw doc and the GitHub tree API).
  - In 2025 the Flutter SDK described itself as running "LLMs, VLMs, and TTS models" ([pub.dev 0.2.0 search snippet](https://pub.dev/packages/cactus/versions/0.2.0); [i-programmer, Aug 2025](https://www.i-programmer.info/news/83-mobilephone/18272-cactus-lets-you-build-llm-powered-applications-on-your-mobile-phone-.html)). That TTS path is gone from the current engine **[INFERENCE: it was GGUF/llama.cpp-based]**.
- **Turkish:** no Turkish speech in either direction (Whistle covers 7 languages).
- **License:** `cactus-compute` 4.x is Apache-2.0. The **engine has a custom license**: free for individuals and for organizations under USD 2M in funding and USD 2M in revenue; others need a commercial license ([LICENSE](https://github.com/cactus-compute/engine/blob/main/LICENSE)). It also has cloud hand-off and telemetry features, with telemetry on by default in the Needle binaries ([legacy README](https://github.com/cactus-compute/cactus/tree/legacy)).
- **Maturity:** active and fast-moving. The package was renamed twice (cactus-compute → cactus-engine, with cactus-needle → cactus-compute) and has 116 open issues on the engine.
- **Verdict:** **not a sensible runtime for this panel.** It cannot synthesize speech, and wrapping a TTS model in it would mean porting the model to its graph API yourself. Its Rust bindings are raw and unsafe. Whistle cannot do Turkish ASR either.

## Rust integration paths

| Path | Status | Covers |
|---|---|---|
| **sherpa-onnx crate** (current `=1.13.8`; latest 2.0.1 from 2026-10-27, [crates.io](https://crates.io/crates/sherpa-onnx)) | TTS families: VITS/Piper/MMS, Matcha, Kokoro, Kitten, ZipVoice, **Pocket**, **Supertonic** ([C API doc](https://k2-fsa.github.io/sherpa/onnx/c-api/html/tts.html)). Both 1.13.8 and 2.0.1 expose `OfflineTtsSupertonicModelConfig` and `OfflineTtsPocketModelConfig`. 2.0.0 removed espeak-ng (GPL), so VITS/Piper needs a lexicon or `phoneme_codepoints` there. Supertonic and Pocket have built-in frontends and need no lexicon | **Supertonic 3 Turkish: yes**, drop-in, `GenerationConfig { sid, num_steps, speed, extra: {"lang":"tr"} }` ([Rust example](https://github.com/k2-fsa/sherpa-onnx/blob/master/rust-api-examples/examples/supertonic_tts.rs)). Pocket: only the English int8 package (98 MB, measured via `curl -I`); the Turkish fine-tune would need an ONNX export in sherpa's layout **[INFERENCE]**. Kokoro multi-lang: zh+en only. No Turkish Matcha model exists |
| **`ort` crate** (2.0.0-rc.15, ONNX Runtime 1.31, [crates.io](https://crates.io/crates/ort)) | Mature | Supertonic ONNX (official `rust/` example in the archived repo); EMA Lightning / Antalia-2 Mini / Ahenk Cep ONNX with a **working MIT reference app** ([Hitabet](https://github.com/mesutde/Hitabet-Turkce-TTS): Tauri + `ort` + Turkish normalizer in Rust, `cargo test` parity ~1e-5); MOSS-TTS-Nano ONNX **[INFERENCE]** |
| **candle** | `pocket-tts` crate 0.6.2 (Pocket TTS port: CPU, Metal feature, int8, streaming, voice cloning, [repo](https://github.com/babybirdprd/pocket-tts)); `gradium-ai/xn-ptts` (listed in the [Kyutai README](https://github.com/kyutai-labs/pocket-tts)); xtts-rs | Pocket TTS Turkish weights in the candle port: **[INFERENCE]** (same architecture as the 6-layer base, untested) |
| **MLX via mlx-rs** (0.34.0, [crates.io](https://crates.io/crates/mlx-rs)) | Bindings only; no TTS model ports found | Every MLX model above would need a hand port **[INFERENCE: large effort]**. Practical MLX route = `mlx-audio` sidecar (Python, OpenAI-compatible server, per its [README](https://github.com/Blaizzy/mlx-audio)) |
| **ggml / C++ via FFI** | `zonos3.cpp` (ZONOS3-Mini, Metal), llama.cpp-omni `voxcpm2-cli`, chatterbox GGUF, kitten-tts-2-cpp | No stable C APIs documented. A local HTTP sidecar (`zonos3-server`) is the low-risk route **[INFERENCE]** |
| **Kyutai Rust server** (delayed-streams-modeling) | Rust server for Kyutai TTS 1.6B | en/fr only, no Turkish ([card](https://huggingface.co/kyutai/tts-1.6b-en_fr)) |
| **Cactus C API** | Raw `extern "C"` | No TTS. Not applicable |
| **AVSpeechSynthesizer** (Yelda) | `objc2-avf-audio` exposes `AVSpeechSynthesizer` + `AVSpeechSynthesizerBufferCallback` ([docs.rs](https://docs.rs/objc2-avf-audio/latest/objc2_avf_audio/all.html)); the `tts` crate wraps AVFoundation | macOS Yelda fallback with zero model download |

## Ranked shortlist for t3-panel

1. **Supertonic 3 via the existing sherpa-onnx crate** (int8, `lang=tr`).
   - **Integration:** almost free. Same crate and same version pin, 129 MB, built-in text frontend (no espeak problem), 10 voices. The user already picked it as the Reels default.
   - **Unknowns:** no Turkish WER on a public benchmark; number and English-word handling in Turkish; no streaming API (synthesize per sentence); quality of int8 vs fp32. Bakeoff should time sherpa int8 directly.
2. **Antalia-2 Mini (or EMA Lightning) via `ort`, using Hitabet as the Rust reference.**
   - **Strengths:**
     - Best measured Turkish intelligibility (WER ~1.0%).
     - ~30–35 MB of weights, and an M-series CPU TTFA of 72 ms (Antalia card).
     - Antalia's normalizer respells English and brand words, which fits strings like "PR, deploy, iOS" best; EMA reads foreign words with Turkish spelling.
     - Apache-2.0.
   - **Costs:** a single synthetic voice each. Automatic naturalness is lower than Trendyol, Chatterbox and Piper (UTMOS 3.18–3.30), although UTMOS does not capture "robotic" well.
   - **Effort:** port or reuse Hitabet's Rust normalizer and engines.
3. **Pocket TTS Turkish (wite-tech).**
   - **Strengths:** WER 1.9%, 6 voices plus emotion tags, streaming, real time on one CPU thread. Already installed locally.
   - **Rust paths:** a Python sidecar today. In-process via the candle `pocket-tts` crate or sherpa Pocket would need the Turkish weights and normalizer ported **[INFERENCE]**.
4. **ZONOS3-Mini.**
   - **Strengths:** Apache-2.0, streaming, can clone the user's own voice, ggml with Metal, ~1.2 GB.
   - **Risks:** released 2 days ago; no Turkish voice preset; no Turkish text normalization (numbers must be expanded upstream, which the panel needs anyway); no Turkish WER published. Worth the bake-off; integration would be a sidecar or FFI.
5. **Chatterbox Multilingual v3 with the user's cloned voice: "premium voice" mode only.**
   - Liked quality and WER 1.91%, but RTF 2.9 on an L4 and ~3 GB in size. It needs a Python or MLX sidecar, and upstream is retired.
6. **VoxCPM2 / Trendyol-TTS:** the naturalness ceiling (UTMOS 3.82) but 2B+ params and RTF ~1 on an L4. Too heavy for an always-on panel **[INFERENCE]**; useful only as a reference for "how good can local Turkish sound".

Wildcards for the bake-off only: MOSS-TTS-Nano (license unclear), OmniVoice (non-commercial), Audio8 0.6B (CV3 Turkish WER 12.6%).

## Rejected (one line each)

- **Kokoro-82M / v1.1:** no Turkish and no Turkish fine-tune ([VOICES.md](https://huggingface.co/hexgrad/Kokoro-82M/raw/main/VOICES.md)).
- **Supertonic 2:** 5 languages, no Turkish ([HF](https://huggingface.co/Supertone/supertonic-3); superseded by v3).
- **CosyVoice 2 / Fun-CosyVoice3:** 9 languages, no Turkish ([HF](https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512)).
- **Qwen3-TTS:** 10 languages, no Turkish; the only Turkish LoRA calls itself not production-ready ([HF](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-1.7B-Base)).
- **Mistral Voxtral TTS:** 9 languages, no Turkish; CC-BY-NC ([HF](https://huggingface.co/mistralai/Voxtral-4B-TTS-2603)).
- **Higgs Audio v2:** no Turkish ([mlx-audio list](https://github.com/Blaizzy/mlx-audio)).
- **Higgs Audio v3:** has Turkish but 4B / 9.3 GB and non-commercial; too heavy ([HF](https://huggingface.co/bosonai/higgs-audio-v3-tts-4b)).
- **Fish-Speech 1.5 / OpenAudio S1-mini:** no `tr` in the language metadata; CC-BY-NC-SA ([API](https://huggingface.co/api/models/fishaudio/openaudio-s1-mini)).
- **Fish Audio S2 Pro:** Turkish only in the "other" tier; 11 GB; research license ([HF](https://huggingface.co/fishaudio/s2-pro)).
- **MOSS-TTS v1.5 (8B) / MOSS-TTSD:** too large (17 GB); TTSD v0.5 has no Turkish ([HF](https://huggingface.co/OpenMOSS-Team/MOSS-TTS-v1.5)).
- **OuteTTS 0.3 / 1.0:** no Turkish in any language list ([HF](https://huggingface.co/OuteAI/Llama-OuteTTS-1.0-1B)).
- **NeuTTS Air / Nano / 2E:** en/es/de/fr only ([GitHub](https://github.com/neuphonic/neutts)).
- **Kani-TTS / Kani-TTS-2:** no official Turkish; the community fine-tune is academic-only with no eval ([HF](https://huggingface.co/nineninesix/kani-tts-370m)).
- **IndexTTS2 / 2.5:** no official Turkish; the Turkish LoRA is good (WER 1.32%) but needs an NVIDIA GPU, has no streaming and a custom license ([HF](https://huggingface.co/berrkkaayy/IndexTTS-2.5-Turkish-LoRA)).
- **Microsoft VibeVoice:** TTS code removed from the repo; en/zh plus 9 experimental languages, no Turkish ([GitHub](https://github.com/microsoft/VibeVoice)).
- **Meta MMS-TTS tur:** WER 6.38%, worse than the current Piper; CC-BY-NC ([HF](https://huggingface.co/facebook/mms-tts-tur)).
- **Kyutai TTS 1.6B:** en/fr only ([HF](https://huggingface.co/kyutai/tts-1.6b-en_fr)).
- **Chatterbox Turbo / Nano:** English-only ([GitHub](https://github.com/resemble-ai/chatterbox)).
- **Orpheus (official multilingual):** no Turkish; community 3B fine-tunes are slow with weak or no evals ([GitHub](https://github.com/canopyai/Orpheus-TTS)).
- **Matcha-TTS / sherpa Matcha:** no Turkish checkpoint ([sherpa tts-models](https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models)).
- **KittenTTS 0.1–0.8:** English-only (the [mlx-audio list](https://github.com/Blaizzy/mlx-audio)). KittenTTS 2 has Turkish but is 1.7B with no published speed or quality figures.
- **Spark-TTS, Llasa, Dia, Sesame CSM, MeloTTS, Zonos v0.1 / ZONOS2, StyleTTS2, Parler multilingual, MaskGCT, Soprano, Marvis:** no Turkish ([Spark-TTS](https://huggingface.co/SparkAudio/Spark-TTS-0.5B), [Llasa](https://huggingface.co/HKUSTAudio/Llasa-1B-Multilingual), [Dia2](https://huggingface.co/nari-labs/Dia2-2B), [MeloTTS](https://github.com/myshell-ai/MeloTTS), [ZONOS2](https://huggingface.co/Zyphra/ZONOS2), [StyleTTS2](https://github.com/yl4579/StyleTTS2), [Parler](https://huggingface.co/parler-tts/parler-tts-mini-multilingual-v1.1), [MaskGCT](https://huggingface.co/amphion/MaskGCT), [Soprano](https://github.com/ekwek1/soprano), [Marvis](https://huggingface.co/Marvis-AI/marvis-tts-250m-v0.2)).
- **F5-TTS base:** no Turkish (WER 77.8% in the [EMA table](https://huggingface.co/canberkkkkkk/ema-lightning)); Turkish fine-tunes are NC and Anka supersedes them.
- **Breeze-TTS-2-Turkish:** strong (UTMOS 4.07) but 3.5B, GPU-oriented and non-commercial ([HF](https://huggingface.co/ersanbil/Breeze-TTS-2-Turkish-Base)).
- **Antalia-1:** WER 6.28%, superseded by Antalia-2 Mini ([card table](https://huggingface.co/cloud0day3/antalia-mini)).
- **FreyaTTS-small:** WER 12.02% ([card table](https://huggingface.co/cloud0day3/antalia-mini)).
- **Coqui GlowTTS tr:** WER 10.93% (same table).
- **Cactus (engine + Needle / Whistle):** no TTS model; Whistle has no Turkish (section above).

## Notes for the panel

- **License of the current baseline:** the Piper `tr_TR-dfki-medium` training data is **CC-BY-NC-SA-4.0** ([MODEL_CARD](https://huggingface.co/rhasspy/piper-voices/raw/main/tr/tr_TR/dfki/medium/MODEL_CARD)). Personal use is fine, but every Apache/MIT model above is less restrictive.
- **Numbers and English tech words:** only these models document handling "80 görevin 20'si" and "PR/deploy/iOS" style input out of the box:
  - Antalia-2 Mini: numbers, dates, amounts, plus English and brand respelling ([card](https://huggingface.co/cloud0day3/antalia-mini)).
  - EMA Lightning: numbers through normalizer-tr ([card](https://huggingface.co/canberkkkkkk/ema-lightning)).
  - Pocket TTS Turkish, through its package's normalizer ([card](https://huggingface.co/wite-tech/pocket-tts-turkish)).
  - Ahenk Cep: trnorm ([card](https://huggingface.co/ozgurguler/ahenk-cep-tr)).
  - The VoxCPM2 Turkish LoRA ([card](https://huggingface.co/mramazan/voxcpm2-turkish-tts-lora)).

  Everything else (Supertonic, ZONOS3, OmniVoice, Chatterbox) should receive pre-normalized text. A shared Rust Turkish normalizer (Hitabet's or normalizer-tr's Rust source in the [EMA ONNX repo](https://huggingface.co/ozcancelik/ema-lightning-onnx)) would benefit any engine choice **[INFERENCE]**.
- **mlx-audio** (PyPI 0.6.5, 2026-11-09, MIT) is the one-stop Mac runner for ZONOS3-Mini, MOSS-TTS-Nano, Audio8, OmniVoice, Chatterbox v3, VoxCPM2 and Higgs v3. It also exposes an OpenAI-compatible server for a sidecar setup ([README](https://github.com/Blaizzy/mlx-audio)). The ZONOS3 MLX weights were uploaded after the 0.6.5 release, so a git install may be needed **[INFERENCE]**.
