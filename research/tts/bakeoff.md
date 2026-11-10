# Turkish local TTS bake-off (t3-panel)

Real runs on this Mac (Apple M4 Pro, 48 GB, macOS 27, with the user's other apps open), 2026-11-11T01:13.
Open **[`bakeoff/index.html`](bakeoff/index.html)** (it works from file://) to listen: one player per sentence × engine. The players use loudness-normalized copies (-16 LUFS). Raw numbers are in [`bakeoff/results.json`](bakeoff/results.json). Audio is git-ignored.

## How it was measured

- **Corpus:** 7 panel-style sentences (a–g), fed **raw** (digits, apostrophes and English tech words left as written). See index.html.
- **One fresh process per engine/voice:** `python3 harness/run.py` runs `/usr/bin/time -l <venv python> harness/bench.py <run_id> '<config>'`. The configs are in `harness/runs.json`.
  - **load** = process start → model ready, imports included. Weights were already on disk and in the file cache.
  - A **second fresh process** (`run.py --pass2`) repeats only the timings. Both loads are shown; the first can include one-time pyc compiles or downloads.
- **TTFA:** time to first audio.
  - For streaming engines it is the first chunk: Pocket ~80 ms frames, EMA/Antalia/Seslen per clause, XTTS `inference_stream`, ZONOS HTTP stream.
  - Otherwise it is the full synthesis of the sentence.
  - "+ lead silence" adds the leading silence of the returned audio (ffmpeg silencedetect, -40 dB). That is when you would actually hear speech, unless the panel trims it.
- **RTF** = total synth time / total audio length. **RSS** = peak resident set from `/usr/bin/time -l`. Metal/MPS allocations are not in RSS; index.html also shows the peak memory footprint.
- **Quality proxy:** `whisper-cli` with ggml-large-v3-turbo-q8_0 and `-l tr`.
  - **WER** compares against the written text. On both sides, digits become Turkish words, Turkish lowercase is applied, and suffix apostrophes are dropped ("20'si" = "yirmisi").
  - **Numbers OK?** means sentence c was heard with seksen + yirmisi + otuzu.
  - **EN words** counts how many of pull, request, deploy, build, iOS were heard as such.
  - **Seeds 2/3:** for the shortlisted stochastic engines, `harness/seeds.py` repeated the read-back with 2 more seeds.
  - Whisper can split words ("Dur duruldu" counts as 2 errors), so look at CER and the heard text too (both in index.html).
- **Flags** come from duration and silence checks on the raw wav: leading silence over 0.5 s, trailing over 1 s, inner gap over 0.8 s, syllable rate outside 3–8.5/s, and more than 10 samples at full scale.
- **Commands to reproduce** (from `research/tts/bakeoff/`):
  - `python3 harness/run.py && python3 harness/run.py --pass2`
  - `python3 harness/seeds.py <ids…>`
  - `python3 harness/analyze.py && python3 harness/render.py`
  - Each run's exact command is stored as `command` in results.json.

## Metrics (all runs; baseline first, then by WER)

| Engine / voice (run id) | Device | Load ms (1st / 2nd proc) | TTFA 1st call ms | TTFA p50 ms | + lead silence p50 ms | RTF | Peak RSS MB | WER (seeds 1/2/3) | Numbers OK? | EN words | Flags |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **Piper tr_TR-dfki-medium (panel baseline)** (baseline) `piper-dfki` | CPU | 746 / 790 | 90 | 59 (stream) | 59 | 0.015 | 295 | 12.7% | no | 3/5 | 0 |
| Supertonic 3 M2 · 4 steps `supertonic-M2-s4` | CPU | 364 / 359 | 455 | 446 | 943 | 0.087 | 561 | 0.0% (0.0/3.6/5.5) | yes (3/3) | 5/5 | 4 |
| Supertonic 3 F2 · 8 steps `supertonic-F2-s8` | CPU | 340 / 349 | 808 | 808 | 1389 | 0.168 | 554 | 0.0% (0.0/3.6/0.0) | yes (3/3) | 5/5 | 4 |
| Supertonic 3 M4 · 8 steps `supertonic-M4-s8` | CPU | 353 / 344 | 812 | 812 | 1236 | 0.169 | 550 | 0.0% | yes | 5/5 | 2 |
| Supertonic 3 M2 · 8 steps `supertonic-M2-s8` | CPU | 360 / 354 | 879 | 827 | 1317 | 0.166 | 557 | 0.0% (0.0/0.0/0.0) | yes (3/3) | 5/5 | 2 |
| Supertonic 3 M4 · 4 steps `supertonic-M4-s4` | CPU | 358 / 347 | 426 | 426 | 852 | 0.088 | 548 | 1.8% (1.8/0.0/5.5) | yes (3/3) | 4/5 | 2 |
| Supertonic 3 F2 · 4 steps `supertonic-F2-s4` | CPU | 350 / 342 | 431 | 431 | 998 | 0.090 | 549 | 1.8% (1.8/0.0/9.1) | yes (3/3) | 4/5 | 4 |
| Supertonic 3 M5 · 4 steps `supertonic-M5-s4` | CPU | 346 / 343 | 442 | 442 | 895 | 0.085 | 551 | 1.8% | yes | 4/5 | 1 |
| Supertonic 3 F5 · 4 steps `supertonic-F5-s4` | CPU | 359 / 348 | 513 | 458 | 922 | 0.092 | 561 | 1.8% | yes | 5/5 | 2 |
| Supertonic 3 M3 · 8 steps `supertonic-M3-s8` | CPU | 353 / 352 | 824 | 816 | 1214 | 0.172 | 545 | 1.8% | yes | 4/5 | 3 |
| macOS say -v Yelda (rate 185) `yelda` | macOS speechsynthesisd | 9 / 18 | 1154 | 837 | 837 | 0.202 | 34 | 1.8% | yes | 4/5 | 0 |
| Supertonic 3 F1 · 8 steps `supertonic-F1-s8` | CPU | 343 / 348 | 840 | 840 | 1372 | 0.165 | 578 | 1.8% | yes | 4/5 | 2 |
| Supertonic 3 F3 · 8 steps `supertonic-F3-s8` | CPU | 352 / 340 | 850 | 850 | 1366 | 0.163 | 552 | 1.8% | yes | 4/5 | 2 |
| Supertonic 3 M5 · 8 steps `supertonic-M5-s8` | CPU | 359 / 352 | 864 | 864 | 1356 | 0.165 | 563 | 1.8% | yes | 4/5 | 1 |
| Supertonic 3 int8 · sherpa-onnx 1.13.8 (panel's pinned version) · sid 0 · 8 steps `sherpa113-st3int8-sid0-s8` | CPU | 278 / 303 | 1380 | 927 | 1416 | 0.205 | 436 | 1.8% | yes | 5/5 | 2 |
| Pocket TTS Turkish — female_1 `pocket-female_1` | CPU | 2030 / 1857 | 197 | 25 (stream) | 25 | 0.088 | 1196 | 3.6% (3.6/5.5/5.5) | yes (3/3) | 4/5 | 0 |
| Supertonic 3 F1 · 4 steps `supertonic-F1-s4` | CPU | 340 / 354 | 445 | 445 | 940 | 0.086 | 555 | 3.6% | yes | 5/5 | 3 |
| Supertonic 3 int8 via sherpa-onnx · sid 0 · 4 steps · 4 threads `sherpa-st3int8-sid0-s4-t4` | CPU | 261 / 249 | 486 | 461 | 978 | 0.095 | 455 | 3.6% | yes | 5/5 | 2 |
| Supertonic 3 int8 via sherpa-onnx · sid 5 · 4 steps `sherpa-st3int8-sid5-s4` | CPU | 264 / 258 | 512 | 490 | 969 | 0.099 | 453 | 3.6% | yes | 4/5 | 1 |
| Seslen TTS — ayse (female) `seslen-ayse` | CPU | 712 / 684 | 772 | 505 (stream) | 505 | 0.141 | 633 | 3.6% (3.6/3.6/3.6) | yes (3/3) | 4/5 | 0 |
| Supertonic 3 F5 · 8 steps `supertonic-F5-s8` | CPU | 369 / 355 | 883 | 883 | 1340 | 0.171 | 558 | 3.6% | yes | 5/5 | 1 |
| Supertonic 3 int8 via sherpa-onnx · sid 5 · 8 steps `sherpa-st3int8-sid5-s8` | CPU | 256 / 248 | 952 | 944 | 1414 | 0.189 | 451 | 3.6% | yes | 5/5 | 2 |
| Supertonic 3 int8 via sherpa-onnx · sid 0 · 8 steps `sherpa-st3int8-sid0-s8` | CPU | 793 / 336 | 1316 | 945 | 1422 | 0.200 | 442 | 3.6% | yes | 5/5 | 1 |
| Anka TTS v0.1 — built-in female `anka-female` | MPS | 5450 / 8338 | 8098 | 7982 | 7982 | 1.982 | 3789 | 3.6% | yes | 4/5 | 0 |
| Anka TTS v0.1 — built-in male `anka-male` | MPS | 6612 / 10061 | 9823 | 8625 | 9117 | 2.096 | 3787 | 3.6% | yes | 4/5 | 0 |
| Pocket TTS Turkish — female_2 `pocket-female_2` | CPU | 1861 / 1855 | 214 | 25 (stream) | 25 | 0.086 | 1195 | 5.4% | yes | 3/5 | 0 |
| EMA Lightning — ONNX graphs (onnxruntime CPU) `ema-onnx` | CPU | 2312 / 1680 | 77 | 40 (stream) | 40 | 0.061 | 387 | 5.5% (5.5/9.1/5.5) | yes (3/3) | 3/5 | 0 |
| Ahenk Cep (f022 female) 8 steps `ahenk-cep-s8` | CPU | 558 / 504 | 180 | 180 | 180 | 0.056 | 491 | 5.5% (5.5/5.5/5.5) | yes (3/3) | 3/5 | 0 |
| Ahenk Cep (f022 female) 16 steps `ahenk-cep-s16` | CPU | 6326 / 828 | 258 | 258 | 258 | 0.069 | 520 | 5.5% | yes | 3/5 | 0 |
| Supertonic 3 M1 · 8 steps `supertonic-M1-s8` | CPU | 502 / 500 | 916 | 916 | 1390 | 0.168 | 560 | 1.8% (1.8/0.0/1.8) | no (2/3) | 5/5 | 3 |
| VoxCPM2 2B 4-bit (MLX) — clone `mlx-voxcpm2-clone` | Metal GPU | 2201 / 1817 | 2917 | 2490 | 2490 | 0.654 | 2468 | 1.8% (1.8/7.3/1.8) | no (0/3) | 5/5 | 0 |
| Pocket TTS Turkish — male_1 `pocket-male_1` | CPU | 2991 / 3185 | 228 | 26 (stream) | 26 | 0.090 | 1195 | 7.3% (7.3/7.3/9.1) | yes (2/3) | 4/5 | 0 |
| EMA Lightning 1.1.2 (single voice) `ema-lightning` | CPU | 1232 / 1641 | 90 | 92 (stream) | 92 | 0.049 | 399 | 7.3% (7.3/7.3/3.6) | yes (3/3) | 2/5 | 0 |
| XTTS-v2 — Ana Florence (female) `xtts-ana` | MPS | 17894 / 18030 | 368 | 282 (stream) | 282 | 0.360 | 4102 | 7.3% | yes | 4/5 | 3 |
| Supertonic 3 M3 · 4 steps `supertonic-M3-s4` | CPU | 354 / 334 | 430 | 412 | 849 | 0.089 | 547 | 7.3% | yes | 5/5 | 3 |
| Seslen TTS — ege (male) `seslen-ege` | CPU | 782 / 819 | 621 | 448 (stream) | 448 | 0.143 | 600 | 7.3% (7.3/7.3/7.3) | yes (3/3) | 2/5 | 0 |
| Supertonic 3 int8 via sherpa-onnx · sid 0 · 4 steps `sherpa-st3int8-sid0-s4` | CPU | 256 / 246 | 506 | 489 | 972 | 0.099 | 457 | 7.3% | yes | 5/5 | 1 |
| Piper tr_TR-fahrettin-medium `piper-fahrettin` | CPU | 656 / 704 | 63 | 50 (stream) | 50 | 0.014 | 257 | 9.1% | yes | 4/5 | 5 |
| XTTS-v2 — Gilberto Mathias `xtts-gilberto` | MPS | 23397 / 20493 | 382 | 279 (stream) | 279 | 0.368 | 4106 | 9.1% | yes | 3/5 | 2 |
| Supertonic 3 F4 · 4 steps `supertonic-F4-s4` | CPU | 422 / 349 | 507 | 438 | 916 | 0.095 | 552 | 9.1% | yes | 4/5 | 3 |
| Supertonic 3 F3 · 4 steps `supertonic-F3-s4` | CPU | 358 / 354 | 448 | 448 | 938 | 0.084 | 561 | 9.1% | yes | 4/5 | 2 |
| Supertonic 3 int8 · sherpa-onnx 1.13.8 (panel's pinned version) · sid 0 · 4 steps `sherpa113-st3int8-sid0-s4` | CPU | 252 / 245 | 508 | 488 | 956 | 0.099 | 432 | 9.1% | yes | 5/5 | 2 |
| Supertonic 3 F4 · 8 steps `supertonic-F4-s8` | CPU | 363 / 354 | 806 | 806 | 1320 | 0.194 | 548 | 9.1% | yes | 3/5 | 3 |
| ZONOS3-Mini native C++/ggml server (Metal) — clone `zonos3-native-clone` | Metal GPU | 495 / 492 | 1898 | 22 (stream) | 394 | 0.127 | 1823 | 5.5% | no | 5/5 | 0 |
| Antalia-2 Mini (single male voice) `antalia-mini` | CPU | 643 / 968 | 494 | 121 (stream) | 121 | 0.039 | 417 | 5.5% (5.5/5.5/5.5) | no (0/3) | 4/5 | 0 |
| ZONOS3-Mini (MLX) — clone of user's voice `mlx-zonos3-clone` | Metal GPU | 1546 / 660 | 13406 | 304 (stream) | 756 | 0.630 | 1633 | 5.5% (5.5/7.3/3.6) | no (0/3) | 5/5 | 2 |
| Supertonic 3 M1 · 4 steps `supertonic-M1-s4` | CPU | 368 / 365 | 458 | 458 | 963 | 0.087 | 556 | 5.5% | no | 4/5 | 3 |
| Supertonic 3 int8 · sherpa-onnx 1.13.8 (panel's pinned version) · sid 5 · 4 steps `sherpa113-st3int8-sid5-s4` | CPU | 256 / 261 | 518 | 485 | 987 | 0.099 | 451 | 5.5% | no | 5/5 | 3 |
| Pocket TTS Turkish — male_2 `pocket-male_2` | CPU | 7225 / 3637 | 242 | 31 (stream) | 31 | 0.100 | 1145 | 7.3% | no | 5/5 | 0 |
| OmniVoice bf16 (MLX) — clone `mlx-omnivoice-clone` | Metal GPU | 3561 / 2420 | 3014 | 1840 | 1840 | 0.425 | 1885 | 7.3% | no | 4/5 | 2 |
| Pocket TTS Turkish (alperiox 6-layer) — clone `pocket-alperiox-6l-clone` | CPU | 23489 / 3843 | 29 | 29 (stream) | 29 | 0.096 | 1412 | 9.1% | no | 3/5 | 0 |
| Audio8-TTS-Realistic 0.6B 8-bit (MLX) — clone `mlx-audio8-clone` | Metal GPU | 892 / 903 | 5790 | 4115 | 4115 | 0.917 | 2185 | 9.1% | no | 5/5 | 0 |
| Chatterbox Multilingual — user's clone (benim-sesim-ref) `chatterbox-clone` | MPS | 18014 / 14059 | 11799 | 11564 | 11564 | 2.579 | 5021 | 9.1% | no | 4/5 | 2 |
| Chatterbox Multilingual v3 (MLX) — clone `mlx-chatterbox-clone` | Metal GPU | 29508 / 2207 | 5953 | 3205 | 3205 | 0.792 | 3767 | 16.4% | no | 2/5 | 0 |
| Supertonic 3 F2 · 2 steps `supertonic-F2-s2` | CPU | 373 / 348 | 239 | 232 | 800 | 0.048 | 558 | 30.9% | no | 3/5 | 13 |
| Chatterbox Multilingual — built-in voice `chatterbox-default` | MPS | 15252 / 13996 | 26498 | 16588 | 16588 | 3.135 | 4965 | 38.2% | no | 3/5 | 3 |
| Supertonic 3 F1 · 2 steps `supertonic-F1-s2` | CPU | 359 / 339 | 253 | 253 | 811 | 0.048 | 564 | 45.5% | no | 2/5 | 12 |
| Supertonic 3 M1 · 2 steps `supertonic-M1-s2` | CPU | 358 / 343 | 252 | 252 | 846 | 0.047 | 562 | 49.1% | no | 0/5 | 10 |
| MOSS-TTS-Nano-100M (MLX) — clone `mlx-moss-nano-clone` | Metal GPU | 628 / 426 | 15355 | 1408 | 1408 | 0.214 | 659 | 94.5% | no | 0/5 | 14 |

Device notes:
- Supertonic, sherpa-onnx, Piper, Pocket, EMA, Antalia, Ahenk and Seslen ran on the CPU.
- Chatterbox (torch), XTTS and Anka ran on MPS.
- The `mlx-*` runs and ZONOS native ran on Metal.
- Yelda runs in the system `speechsynthesisd` process, so its RSS (34 MB) does not include the real work.

## Top 3 for the panel (by these numbers)

1. **Supertonic 3 through sherpa-onnx (int8), the crate the panel already pins (`=1.13.8`).**
   - **Path:** in-process Rust with the same crate (`OfflineTtsSupertonicModelConfig`, `extra lang="tr"`). No espeak, no sidecar. The int8 package is 129 MB.
   - **Read-back:** best of all engines. The fp32 SDK voice M2 at 8 steps had **0 % WER on all 3 seeds**, numbers right 3/3, and all 5 English words right. Sherpa 1.13.8 at sid 0 / 8 steps had 1.8 % WER, numbers right, EN 5/5.
   - **Footprint:** load ~0.3 s, RSS ~430 MB (baseline ~290 MB).
   - **Weak spot: latency.** It does not stream.
     - At 8 steps, TTFA p50 is ~0.93 s; at 4 steps it is ~0.49 s (RTF 0.10).
     - Every voice puts **~0.4–0.6 s of silence before speech**, so you hear the first word at ~1.4 s (8 steps) or ~1.0 s (4 steps).
     - 2 steps is too few: WER 31–49 %.
   - To get under 400 ms the panel would have to trim leading silence and synthesize the first clause separately [INFERENCE: synth time scales with text length, RTF ~0.1 at 4 steps].
   - **License:** OpenRAIL-M; commercial use is allowed with use restrictions.
2. **EMA Lightning as ONNX (`ort`).** It is 8.6 M parameters, 35 MB of graphs, Apache-2.0.
   - **Latency:** fastest first audio of the Turkish-trained models. TTFA p50 is **40 ms** with ONNX (36 ms in the second process), RTF 0.02–0.06, RSS ~390 MB, most of which is the torch frontend loaded here.
   - **Read-back:** numbers right on 3/3 seeds, thanks to the built-in normalizer-tr. WER averages 6.7 % over 3 seeds.
   - **Errors:** English tech words: "Build" → "Bu il/Bu ilk", "Pull request" → "Poon Request". EN words 2–3/5.
   - **Path:** working Rust+`ort` reference (Hitabet-Turkce-TTS). normalizer-tr also exists as Rust (wasm wrapper source).
   - **Same-architecture alternative:** Ahenk Cep (female voice f022, 7 styles, ships ONNX, CC-BY-4.0). It is a bit slower (TTFA 173–180 ms) but its WER held steady at 5.5 % on all seeds, with numbers right 3/3.
3. **Pocket TTS Turkish (wite-tech), voice female_1.**
   - **Latency:** true streaming. TTFA p50 is **25 ms**; the first call after load is 171–197 ms. RTF 0.09.
   - **Read-back:** WER 3.6 % (seeds 3.6/5.5/5.5), numbers right 3/3, EN 4/5 ("Build" → "Bu il").
   - **Cost:** RSS ~1.2 GB and load ~1.9 s. There is no ready Rust path for the Turkish fine-tune (candle/sherpa ports cover the base Kyutai model), so today it needs a Python sidecar.
   - Of the other voices, male_1 and male_2 had a seed where the numbers failed.

**Also notable:**
- **ZONOS3-Mini native C++/ggml server (Metal, no Python at runtime):**
  - TTFA p50 is 21–22 ms of stream, but the stream starts with ~0.34 s of silence. RTF 0.06–0.13. The first request after start takes 1.6–1.9 s (Metal pipeline compile). RSS 1.8 GB.
  - Turkish numbers fail: "80" heard as "Hissip/Hissin", no Turkish normalizer.
  - It is a cloned voice.
- **VoxCPM2 4-bit (MLX):** best read-back of the big models, but numbers fail 0/3 ("80" heard as "Sediz"), TTFA ~2.5 s and RSS 2.5 GB.
- **Yelda:** no load cost and 1.8 % WER, but TTFA ~0.84 s via `say -o` (spawn included).

**Baseline Piper dfki:**
- WER 12.7 %, the worst of the usable engines.
- Numbers fail: "30'u" heard as "otuz o".
- "deploy" heard as "Deptoy", and "Durduruldu" heard as "Düldürüldü".
- It is still the cheapest: RSS ~290 MB, TTFA ~60 ms.
- Piper fahrettin reads the numbers right but has mild clipping (12–42 samples at full scale in 5/7 sentences).

## Observations from the numbers only (no listening)

- **Leading silence:** Supertonic (all voices, SDK and sherpa) and ZONOS start most sentences with 0.3–0.6 s of silence. The tiny Turkish models, Pocket, Piper and Yelda start at ~0 s.
- **English tech words** are the weak spot of every Turkish-only model:
  - "Build" is heard as "Bu il / Bu yıl / Bill" in EMA, Antalia, Ahenk, Seslen, Anka, Pocket, Yelda and XTTS.
  - Only Supertonic (all voices), sherpa-Supertonic, ZONOS, VoxCPM2 and Audio8 got all 5 English words.
- **Number reading without a normalizer** fails in the multilingual LLM-style models:
  - ZONOS, VoxCPM2, Audio8, OmniVoice, Chatterbox (torch and MLX) and alperiox Pocket.
  - Antalia-2 Mini, despite its normalizer: "80" heard as "Üsteliksen" on 3/3 seeds.
  - Read the numbers right: EMA, Ahenk and Pocket-wite (built-in normalizers), Seslen, Anka, XTTS, Yelda and Piper fahrettin.
  - Supertonic 3 also read them right with no external normalizer (M2/F2/M4, 3/3 seeds at 8 and 4 steps), which contradicts the NVDA report. Only M1 failed one run ("30'lu"), and sherpa sid 5 at 4 steps failed too.
- **The one-word line "Durduruldu."** trips several engines: Piper dfki, Pocket male_2, Chatterbox, XTTS (inner gap 1.0–1.2 s, 1.3–1.8 syl/s) and sherpa at 4 steps ("Dur-dur-dur").
- **Hallucinations and garbage:**
  - Chatterbox default voice added ~5 s of babble after sentence d and has clipping.
  - MOSS-TTS-Nano produced unusable Turkish (WER 94 %, "Tanrım. Tanrım. …", a 9.5 s silent gap).
- **Resource outliers:**
  - Chatterbox torch/MPS: RSS ~5 GB, footprint ~22 GB, RTF 2.6–3.1.
  - Anka (F5-TTS on MPS): RTF ~2, RSS 3.8 GB.
  - XTTS: load ~18–23 s, RSS 4.1 GB.
  - None of these fits an always-on panel.

## Subjective (not measured)

Nothing in this file is a listening judgement; the agent did not play audio. Naturalness, prosody and voice preference need the user's ear in index.html. Good A/B starting points:
- `supertonic-M2-s8` vs `sherpa113-st3int8-sid0-s8` vs `ema-onnx` vs `ahenk-cep-s8` vs `pocket-female_1` vs `piper-dfki`.
- The cloned-voice rows (`chatterbox-clone`, `zonos3-native-clone`, `mlx-voxcpm2-clone`) for "his own voice".

## Failures (with error text)

- `zonos3-native-clone (first attempts)`: POST /v2/generate → HTTP 400 {"detail":"reference-mode cloning of audio requires zac-encoder.gguf and ssl-teacher.gguf; supply --zac-encoder and --ssl-teacher"}. Resolved: request speaker_mode "embedding" (the downloaded spk-encoder.gguf path). The first harness version also left 2 orphaned zonos3-server processes (SIGTERM hit /usr/bin/time, not the server); they were killed (pkill) and the script now signals the server PID directly.
- `mlx-moss-nano-clone`: Ran without exceptions but output is unusable for Turkish: read-back WER 94.5% (e.g. sentence a heard as 'Tanrım. Tanrım. …', sentence f = 9.5 s of mostly silence heard as 'Altyazı M.K.'). Kept in the tables as measured.
- `chatterbox-default`: Ran, but sentence d hallucinated ~5 s of extra babble after the text (WER 143%), f read as 'Dur duruldu'; clipping on d and g. Matches upstream issue #97 (short/hallucinated segments).
- `pocket (MPS)`: Not retried: reels-lab pocket.py documents an MPS abort ('command encoder is already encoding'); all Pocket runs are CPU.

## Skipped candidates

- **Cactus Compute**: Not a TTS runtime: no TTS model in the Cactus-Compute HF org and no Turkish speech (see research/tts/landscape.md). Nothing to benchmark.
- **Higgs Audio v3 4B (bosonai/higgs-audio-v3-tts-4b)**: 9.3 GB download (> 3 GB budget) and non-commercial license.
- **Trendyol-TTS 2.38B (VoxCPM2 Turkish fine-tune)**: 5.1 GB torch repo / 9.3 GB MLX port (emrekoc/trendyol-tts-mlx) > 3 GB budget; VoxCPM2-4bit (2.2 GB) was measured instead as the closest proxy.
- **VoxCPM2 Turkish LoRA (mramazan/voxcpm2-turkish-tts-lora)**: Needs the full-precision torch VoxCPM2 base; mlx-audio's 4-bit MLX model does not load LoRA adapters [INFERENCE from mlx-audio voxcpm2 code: no LoRA path].
- **MOSS-TTS 8B**: Size far over the 3 GB budget.
- **TheStageAI/pocket-tts-100m-tr (Core ML / ANE build)**: 151 MB, but its engines.zip is QLPA-encrypted and only runs inside the paid TheStage Apple SDK (model card) — no open runtime.
- **FreyaTTS**: HF repo freya-tts/FreyaTTS not accessible (API 401: private or nonexistent).
- **Ahenk (teacher, 2B + LoRA)**: GPU teacher of Ahenk Cep; VoxCPM2-class size (> 3 GB) — Ahenk Cep (the distilled 34 MB student) was measured instead.
- **Antalia-2 Mini / Ahenk Cep ONNX graphs**: Measured through their PyTorch packages only; the ONNX graphs need the JS/Rust bookkeeping ported (only EMA Lightning got an ONNX run here, via the reference numpy pipeline). [INFERENCE] ONNX timing ~ EMA-ONNX class.
- **Supertonic 3 via sherpa-onnx Rust crate (cargo project)**: Not built: the sherpa-onnx 1.13.8 Python wheel (same C++ core as the pinned crate) was measured instead.

## Caveats

- Each number comes from one run, plus a second timing-only process and seeds 2/3 for the shortlist. The user's other apps were running, so expect ±10–30 % timing noise. For example, EMA-ONNX RTF was 0.061 in pass 1 and 0.022 in pass 2.
- Settings:
  - Supertonic used speed 1.05 (supertonic.json "explain").
  - Piper, XTTS and Chatterbox used the reels-lab JSON settings (cta/neutral mood).
  - Clone engines on MLX/ZONOS used the first 8.26 s of `voice-ref.wav`; Chatterbox torch used the full 11.6 s clip.
- The sherpa-onnx sid → voice-name mapping (M1..F5) is undocumented and was not verified.
- "+ lead silence" assumes the panel plays the buffer as returned.
- New venvs created: `~/.venvs/{mlx-audio,ema,sherpa,sherpa113,antalia,seslen,ahenk,pocket-alperiox,anka}`. Models not in the HF cache are in `~/.cache/bakeoff-models/` (ZONOS3 checkout and build, sherpa Supertonic int8, EMA ONNX, Ahenk Cep, ref clip). Whisper is at `~/.cache/whisper/ggml-large-v3-turbo-q8_0.bin`. The existing venvs and reels-lab were not modified.
