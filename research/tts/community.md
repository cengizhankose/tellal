# Community findings: local Turkish TTS, small TTS models, agent narrators, Cactus

Agent: `Community` · Collected 2026-11-10/11 · Web research only. No audio was played or generated for this file.

**Evidence tags.** Every quote links to the page it came from, and each was opened while writing this file.
- **[measured]**: a benchmark or test with a stated method, run by a third party or reproduced in public.
- **[vendor]**: the model's own card or README, or a company that sells a competing model.
- **[anecdotal]**: one user's listening impression or report.
- **[INFERENCE]**: my own reasoning; nobody tested it.

**Coverage limits.**
- Reddit blocks anonymous fetches, so threads were read through BrowserOS (old.reddit JSON).
- Reddit search only partly worked. Turkish subreddits (r/CodingTR, r/TurkDev) have almost no local-TTS discussion; they mostly ask about APIs.
- Ekşi Sözlük's "text to speech" topic is about cloud voices (Edge, Google), not open models.
- I found almost no YouTube comparisons, and I could not read YouTube comments. One video exists: ["Açık Kaynak Yapay Zeka ile Türkçe Ses Klonla | Chatterbox"](https://www.youtube.com/watch?v=9380RWclErA). I could not see its content.
- In practice, the most useful Turkish community feedback lives in Hugging Face discussions and GitHub issues of the new Turkish-only models, which appeared in October 2026.

---

## 0. TL;DR for Tellal

1. **There is a new class of Turkish-only, very small, open models, released Sep–Oct 2026.** They were trained on synthetic Turkish speech and include a Turkish text normalizer:
   - **EMA Lightning**: 8.6M parameters, Apache-2.0.
   - **Antalia-2 Mini**: 7.6M, Apache-2.0.
   - **Ahenk Cep**: 8.6M, CC-BY-4.0, ships ONNX.
   - **Pocket TTS Turkish**: 110M, CC-BY-4.0, 6 voices plus emotion tags.
   - **seslen-tts**: ONNX StyleTTS2, 16 voices, CC BY-SA.

   Turkish users react with real enthusiasm, which never happened for Piper, Chatterbox or Supertonic. They also report concrete bugs: clicks on "ç", a rushed "bir" in numbers, and weak foreign and brand words.
2. **A Rust/`ort` reference implementation of three of them already exists:** [Hitabet-Turkce-TTS](https://github.com/mesutde/Hitabet-Turkce-TTS) (Tauri + `ort` + cpal + rubato, with ONNX export scripts). This is the shortest path into GPUI. **[INFERENCE]**
3. **Text normalization matters more than the model for Tellal's text.** Numbers, apostrophe suffixes and "PR/deploy/iOS" break almost every model unless the text is normalized first. The community uses three Turkish normalizers:
   - [`normalizer-tr`](https://github.com/erdemtuna/normalizer-tr) (behind EMA)
   - `trnorm` (Ahenk)
   - [`turkce-normalize`](https://github.com/seslen-tr/turkce-normalize) (seslen)

   Pre-normalizing helps every engine, Piper included. **[INFERENCE, consistent with every maintainer's advice below]**
4. **Multilingual models get a consistent Turkish verdict.**
   - Chatterbox MTL, OmniVoice and Qwen3-TTS sound "foreign / English-optimized" to Turkish listeners.
   - Supertonic 3 is intelligible but bad at numbers, and its repo was archived in 2026.
   - Chatterbox was retired (no longer maintained) in Aug 2026.
   - XTTS-v2 has a non-commercial licence and its company is gone.
5. **Agent-narration projects converge on the same design:** a persistent daemon with the model kept warm, sentence-level chunking with prefetch, a FIFO queue across sessions, short spoken summaries instead of full replies, markdown/code stripping, and a pronunciation dictionary for tech words. None of them ships Turkish.
6. **Cactus Compute ships no TTS today and does not support Turkish.** Its Whistle speech-to-text model covers 7 European languages, and Turkish is not one of them. Cactus is not a candidate for this panel.

---

## 1. Turkish benchmarks the community cites (context for the per-model notes)

| System (open weights unless noted) | Params | WER % ↓ | UTMOS ↑ | ×RT (L4 GPU) | Source |
|---|---|---|---|---|---|
| EMA Lightning | 8.6M | 1.04 | 3.30 | 58× (plain) | PatientDesk re-run |
| Antalia-2 Mini | 7.62M | 1.09 | 3.18 | 35× | PatientDesk re-run |
| Trendyol-TTS (VoxCPM2 LoRA) | 2.38B | 1.16 | 3.82 | 1.0× | PatientDesk re-run |
| Gemini 3.8 Flash TTS (cloud) | – | 1.23 | 3.53 | – | PatientDesk re-run |
| Anka TTS (F5) | 336M | 1.42 | 3.52 | 3.1× | PatientDesk re-run |
| Chatterbox Multilingual | 500M | 1.91 | 3.51 | 0.3× | PatientDesk re-run |
| **Piper tr_TR-dfki-medium (current panel voice)** | 16M | 3.41 | **3.71** | 26× (CPU) | PatientDesk re-run |
| XTTS-v2 | 470M | 3.67 | 2.97 | 2.2× | PatientDesk re-run |
| MMS-TTS tur | 36M | 6.38 | 3.79 | 72× | PatientDesk re-run |
| FreyaTTS-small | 183M | 12.02 | 2.81 | 3.5× | PatientDesk re-run |

Source: [speech.patientdesk.ai/en/benchmarks](https://speech.patientdesk.ai/en/benchmarks), raw data at [PatientdeskAI/freya-tr-eval-rerun](https://huggingface.co/datasets/PatientdeskAI/freya-tr-eval-rerun). **[measured, but by a company that also sells Alania/Antalia]**

Caveats the benchmark authors state themselves:
- "The sentences are short and contain no numbers, dates or abbreviations, which is where Turkish text normalisation is hardest."
- "Word error rate measures whether Whisper understands the audio, not how warm or pleasant the voice is."

Also note that Piper's UTMOS (3.71) is *higher* than EMA's (3.30). UTMOS is trained on English and rewards clean vocoders, a failure mode Reddit benchmarkers call out (see §2.11). So the numbers do not settle "sounds robotic". **[INFERENCE]**

Two other relevant measurements:
- **Length matters most for the panel.** A Sestek paper benchmarking Turkish TTS with 18 objective metrics found: "All systems degrade on short utterances, but the degree varies enormously: the CosyVoice/OmniVoice/VoxCPM2 families stay relatively flat, while **Chatterbox collapses across nearly every metric on short input**." ([arXiv 2610.06057](https://arxiv.org/html/2610.06057v1)) **[measured]** The panel's lines are 5–15 words long, so this applies directly.
- **Pocket TTS Turkish's own card** reports 1.9% WER at RTF 0.048 on GPU, against Trendyol 1.1%, Qwen3-TTS-0.6B-Turkish 1.7%, Piper 3.2% and MMS 6.1%. All systems were fed numbers already written as words ([card](https://huggingface.co/wite-tech/pocket-tts-turkish)). **[vendor]**

---

## 2. Per-model sentiment

### 2.1 EMA Lightning (`canberkkkkkk/ema-lightning`): Turkish-only, 8.6M, Apache-2.0
Links: [HF card](https://huggingface.co/canberkkkkkk/ema-lightning) (461 likes, 88K downloads), [GitHub](https://github.com/canberk7/ema-lightning). Released Oct 2026.

**Good**
- "aksine burada tamamen türkçe odaklı ciddi bir çalışma var, rtx 3090 ile ses kalitesi gayet tatmin edici" ([HF #6](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/6)) [anecdotal]
- An integrator who added it to the Pakize narration tool: "İşlemcide (torch CPU) bir dakikalık sesi ~7–17 saniyede üretiyor, Türkçesi gerçekten çok iyi." ([HF #7](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/7)) [anecdotal, with measurements]
- A team that tested it as an ElevenLabs Flash replacement: "EMA read every price, date, percentage and TL amount in our test set correctly from raw text … On an M1 Pro CPU it runs about 9× real time" ([issue #1](https://github.com/canberk7/ema-lightning/issues/1)) [measured, small test set]
- "Elinize sağlık, cok az kaynak ile cok kaliteli calısıyor." ([HF #9](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/9)) [anecdotal]
- Ecosystem built within weeks:
  - an ONNX/WebGPU port ([ema-lightning-web](https://github.com/ozcancelik/ema-lightning-web))
  - a LiteRT build ([litert-community](https://huggingface.co/litert-community/EMA-Lightning-LiteRT))
  - an NVDA screen-reader add-on ([issue #13](https://github.com/canberk7/ema-lightning/issues/13))
  - the Hitabet Tauri/Rust app ([repo](https://github.com/mesutde/Hitabet-Turkce-TTS))
  - Meram, fine-tunes with extra voices ([repo](https://github.com/Chapulcu/meram-tts))

**Bad**
- Seed variance and swallowed letters: "Harfleri yutuyor ve çok kötü okuyor" ([HF #5](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/5)). The author's answer: the default seed is random, so try several seeds.
- Clicks and peaks: "özellikle 'ç' ve ardından 'ş/s/z/t/k/p' geçen kelimelerde … dakikada ~26–34 sert tepe". Independent of seed; the author attributes it to the dataset. Output also ends abruptly, so the integrator adds a 15 ms fade and silence at the end ([HF #7](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/7)). [measured by integrator]
- Only one female voice: "tek bir kadın sesi var farklı sesleri bekleriz" ([HF #8](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/8)). There is no cloning and no emotion control (card). Meram adds the voices Elif, Kerem and Defne.
- Naturalness, in the author's own words: "Naturalness is good, not the best. UTMOS is about 3.30" ([card](https://huggingface.co/canberkkkkkk/ema-lightning)) [vendor]

**Turkish-specific**
- **Foreign words inside Turkish text are its weak spot.** A user: "yabancı dil ve terimler işin içine girince biraz afalladığını düşünüyorum". The author: "cümle içindeki yabancı kelimeler şu an modelin en zayıf yanı. 'Feedback', 'meeting' gibi sık kullanılanları olabildiğince kapsamaya çalıştım" ([HF #15](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/15)). This matters a lot for "PR / deploy / build / iOS". **[INFERENCE]**
- Brands are read letter by letter, or garbled when suffixed. With normalizer-tr 0.5, `iPhone'umdan` came out as "i pe he o ne e kesme u me de a ne". 1.1.0 fixed plain brands; suffixed forms remain a known limit, and the workaround is to write the word as it is said ([issue #8](https://github.com/canberk7/ema-lightning/issues/8)).
- **Numbers:** "bir" at the end of a number is rushed ("seksen bir lira" → "bir" gets about 120 ms), and â/î/û are dropped (kâr/kar) ([issue #1](https://github.com/canberk7/ema-lightning/issues/1)). Normalizer 0.7.0 fixed dates, `%20'si`, `14.30'da`, `13:00` and TL ([HF #9](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/9)). Normalizer 0.8.0 reads single capital letters by name ("S dalgası" → "se dalgası") ([issue #10](https://github.com/canberk7/ema-lightning/issues/10)).
- Isolated letters (license plates) are inconsistent even when spelled out as "a, be, ce" ([issue #14](https://github.com/canberk7/ema-lightning/issues/14)).

**Latency tips**
- Warm up once. The first call made the RTF look like 0.85 on a 3090; on the second call it was 0.0074 ([HF #6](https://huggingface.co/canberkkkkkk/ema-lightning/discussions/6)).
- From the author: "say() yerine stream() kullanabilirsin. stream() ilk 1 saniyeyi hemen veriyor". Text is processed in roughly 10 s chunks, so split long text ([issue #13](https://github.com/canberk7/ema-lightning/issues/13)).

**Siblings built on the same architecture (Turkish-only, tiny)**
- **Ahenk Cep**: "8,6M, 34 MB, RTF ~0.2 on CPU", built on EMA, CC-BY-4.0, 7 style presets, ships ONNX. Its README also says: "EMA Lightning's TR-Zor error is 14.0 % with its own front-end and 1.9 % with trnorm" ([ahenk-turkish-tts](https://github.com/ozguradmin/ahenk-turkish-tts)) [vendor]. That says more about the normalizer than about the model.
- **Antalia-2 Mini**: 7.62M, Apache-2.0, essentially tied with EMA on WER ([card](https://huggingface.co/cloud0day3/antalia-mini), PatientDesk table). Hitabet describes it as "doğal tonlama, ayarlanabilir adımlar" (steps 8, cfg 2.0, temperature 0.6). [vendor/anecdotal]

### 2.2 Pocket TTS Turkish (`wite-tech/pocket-tts-turkish`): 110M, CC-BY-4.0; plus Kyutai Pocket TTS in general
Links: [HF card](https://huggingface.co/wite-tech/pocket-tts-turkish), [GitHub](https://github.com/witetech/pocket-tts-turkish). Already wrapped in the reels lab as `pocket`.

**Turkish community evidence: thin.** The HF discussions are only about licensing; the authors confirmed "Yes you can use them commercially" ([HF #2](https://huggingface.co/wite-tech/pocket-tts-turkish/discussions/2)). Before this fine-tune existed, a Turkish user tried base Pocket: "Since Turkish isn't officially supported, I got poor results for Turkish" ([kyutai #215](https://github.com/kyutai-labs/pocket-tts/issues/215)). Other community Turkish ports exist:
- [alperiox/pocket-tts-turkish](https://huggingface.co/alperiox/pocket-tts-turkish), MIT
- [OrionCAF/pocket-tts-turkish-8l](https://huggingface.co/OrionCAF/pocket-tts-turkish-8l)
- a Core ML/ANE repack: [TheStageAI/pocket-tts-100m-tr](https://huggingface.co/TheStageAI/pocket-tts-100m-tr) (0 downloads; unverified quality)

**Card rules that matter.** "Without the package, write numbers as words, generate one sentence at a time, start each sentence with a lowercase letter, and use a voice reference that stops between two words." It streams: "the first piece is ready in under 0.1 seconds on one CPU thread, after a first call of about 0.4 seconds that prepares the voice". Voices: female_1–3, male_1–3 (male_1 is "calm and deep, narrator style"). Emotion tags: `mutlu`, `üzgün`, `kızgın`, `şaşkın`, `sakin`. [vendor]

**General Pocket TTS sentiment (English, r/LocalLLaMA [thread](https://www.reddit.com/r/LocalLLaMA/comments/1up07mk/))**
- Good:
  - "for its size, I haven't found anything else that comes close." ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1up07mk/comment/ovw7y36/))
  - The latency is flat (RTF 0.69–0.76 regardless of text length) and it streams (post; CPU Xeon). [measured]
- Bad:
  - "it isn't AS accurate as kokoro cause it doesn't have G2P. So it hallucinations and mispronunces. And it's pretty emotionless" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1up07mk/comment/ovwh0bw/))
  - "Pocket tts sounds terrible. Went with Kokoro." ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1up07mk/comment/ow0srvw/))
  - On long text: "It's just noise after 2 minutes" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1up07mk/comment/pab8kw8/))
  - All [anecdotal].
- The 2026 local-TTS list: "The only lever for expressive control is sampling temperature. It gets toxic with uppercase and unusual punctuation." ([r/LocalTextToSpeech](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/)) [anecdotal, long-term user]

**Apple Silicon**
- pocket-tts 2.1.0 on macOS showed "intermittent but severe semantic corruptions … repeated words and inserted phrases". A follow-up comment found that a foreign-accented cloning reference "massively amplifies the problem" ([kyutai #221](https://github.com/kyutai-labs/pocket-tts/issues/221)). [measured by a production user]
- Ports: a native Core ML/MLX macOS app with "Sub-second first-audio latency, ~3x real-time" ([kyutai #192](https://github.com/kyutai-labs/pocket-tts/issues/192)) and a **Rust/Candle port** at RTF ~0.17 on CPU ([kyutai #94](https://github.com/kyutai-labs/pocket-tts/issues/94)). One user found the claimed MLX speedup disappears once eval time is counted: "On my M1 mini, I don't see any speed up at all" (same issue). [anecdotal]
- The local reels lab already notes that MPS crashes and CPU runs at about 13× real time (context, not new evidence).

### 2.3 Supertonic 3 (Supertone): 99M, 31 languages incl. Turkish, ONNX, OpenRAIL-M
Links: [HF](https://huggingface.co/Supertone/supertonic-3), [archived repo](https://github.com/supertone-oss-archive/supertonic). This is the reels default.

**Project status.** The README now says: "This repository will be archived, and there will be no further development or official support for the open-source Supertonic models." (notice of July 23, 2026). The weights and PyPI 1.3.1 remain available.

**Good**
- Speed and footprint. A Supertonic 3 C# port compared against Kokoro on sherpa: "Supertonic was noticably faster on CPU with better quality voices" ([comment](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/comment/osx46zm/)) [anecdotal]
- CPU benchmark: 5 steps gives "the practical sweet spot at MOS 4.37 and 3.2x real-time" ([r/LocalLLaMA](https://www.reddit.com/r/LocalLLaMA/comments/1udg3rf/)) [measured, English]
- A Turkish developer recommends it as a free local option: "Pinokio kurup içinden Supertonic3 kur hocam Türkçe desteği de var." ([r/TurkDev](https://www.reddit.com/r/TurkDev/comments/1uiabg1/comment/ouggk3d/)) [anecdotal]

**Bad**
- "2-step … sounds robotic (MOS 1.53). Don't ship it." (same benchmark post) [measured]
- "there are numerous errors in the long, paragraph and extended outputs" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1udg3rf/comment/otdtdxx/)) [anecdotal]
- 2026 list: "Output is often flawed. It tends to spell uppercase text OR emphasize it, but not controllably." ([r/LocalTextToSpeech](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/)) [anecdotal]
- Emotion tags: training data is "mostly for Korean, English, and Japanese", so tags are ignored elsewhere (maintainer, [HF #6](https://huggingface.co/Supertone/supertonic-3/discussions/6)).

**Turkish-specific**
- A blind Turkish developer built an NVDA driver: "the pronunciation of numbers is really terrible … When it sees a number, it makes up nonsense words and reads something completely different." On a low-end Ryzen 3 CPU: "In screen reader UX, anything above 100-150ms feels broken; here, it takes seconds." ([issue #144](https://github.com/supertone-oss-archive/supertonic/issues/144)) [anecdotal, detailed]
- Maintainer: "it is not currently a causal or streaming TTS model … we do not have a near-term plan to convert Supertonic into a fully causal / streami[ng]" (same issue).
- Numbers depend on training data, with no TR normalizer: "we tried to rely less on language-specific text normalization rules" ([HF #13](https://huggingface.co/Supertone/supertonic-3/discussions/13)). Long numbers mumble or skip ([issue #31](https://github.com/supertone-oss-archive/supertonic/issues/31)).
- Diacritics bug (German umlauts) in the **Python package** preprocessing: combining marks were stripped after NFKD. It was fixed around 1.2.1 ([HF #8](https://huggingface.co/Supertone/supertonic-3/discussions/8)). Any Rust port must keep combining marks, because ş/ç/ğ/ö/ü decompose under NFKD. **[INFERENCE]**

### 2.4 Chatterbox Multilingual (Resemble AI): 500M, MIT (+PerTh watermark)
Links: [GitHub](https://github.com/resemble-ai/chatterbox). README: "**As of August 2026, Chatterbox is retired research and is no longer maintained.**" This is the user's cloned-voice engine in the reels lab.

**Good**
- 2026 list: "Probably the best free open-source TTS starting point … The issue is not really the model only. It is everything around it. Segmentation, retries, weird generations, timing, silence handling, pronunciation" ([r/LocalTextToSpeech](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/)) [anecdotal]
- WER 1.91 on Turkish when given a Turkish male reference (PatientDesk) [measured]. Voice cloning is good, and the user already likes his cloned voice.

**Bad**
- "for actual real-time voice conversation, chatterbox makes sense but the sentence-by-sentence latency ceiling is real" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1r7bsfd/comment/o7wp0b4/)) [anecdotal]
- Speed: 0.3× real time on an L4. It also "slowed down from pass to pass (RTF 1.69, 2.89, 4.21)" ([PatientDesk](https://speech.patientdesk.ai/en/benchmarks)) [measured]
- Short inputs hallucinate: "short segments like 'Hi!', 'Why?', 'Yes', 'No', or single letters or numbers tend to produce gibberish" ([issue #97](https://github.com/resemble-ai/chatterbox/issues/97)). The Sestek paper confirms it collapses on short Turkish input (see §1).
- Numbers are misread or skipped, and noise/breathing appears at the end. Users' workarounds: "chunking the input text by 300 chars max", tail trimming, repetition penalty ≤1.2. Uppercase acronyms ("HTML or AI aren't pronounced well") ([issue #287](https://github.com/resemble-ai/chatterbox/issues/287)).

**Turkish-specific**
- "Turkish specific char like : 'ç,ş,ü,ğ,ö' … are skipped or doesn't sound right." ([issue #261](https://github.com/resemble-ai/chatterbox/issues/261))
- HF: "turkish characters like şıüğçö are not get pronounced correctly" (Sep 2025). Later reply: "Looks like problem solved is it?" (Dec 2025, after v2 weights) ([HF #33](https://huggingface.co/ResembleAI/chatterbox/discussions/33)). [mixed]
- Voicebox users: "Speech sounds English-optimized rather than Turkish-native", and "especially 'r' sounds feel unnatural/English-like". The same user: "I have tested XTTS-v2 for Turkish, and in my case it provides noticeably better results compared to Chatterbox." ([voicebox #846](https://github.com/jamiepine/voicebox/issues/846)) [anecdotal ×2]
- r/TextToSpeech: "only one supports Turkish, but it struggles quite a bit with understanding sentences and often gets confused … it sounds very flat" ([post](https://www.reddit.com/r/TextToSpeech/comments/1tqx8d1/)) [anecdotal]
- Accent bleeds from the reference language into the output ([HF #55](https://huggingface.co/ResembleAI/chatterbox/discussions/55)). No Turkish Turbo model exists ([issue #541](https://github.com/resemble-ai/chatterbox/issues/541)).

**Apple Silicon / Rust**
- "Critical Memory Leak (Apple Silicon)". An M5 user traced it to MPS allocator fragmentation in the T3 kv-cache loop ([issue #218](https://github.com/resemble-ai/chatterbox/issues/218)). This matters for an always-on panel. **[INFERENCE]**
- Fanless Mac: "after 1 day of testing i find it's better to stick to cpu on fanless macbooks" ([issue #147](https://github.com/resemble-ai/chatterbox/issues/147)).
- A Tauri (Rust) app author: "I ended up just picking mlx-audio and running Chatterbox through a Python sidecar." ([issue #384](https://github.com/resemble-ai/chatterbox/issues/384))
- ONNX exports exist ([onnx-community/chatterbox-multilingual-ONNX](https://huggingface.co/onnx-community/chatterbox-multilingual-ONNX)). An iOS/macOS app saw "Peak RAM sits around 3.2 GB" ([HF #42](https://huggingface.co/ResembleAI/chatterbox/discussions/42)).

### 2.5 XTTS-v2 (Coqui): 470M, CPML non-commercial
- Turkish: one user preferred it over Chatterbox ("ı, ö, ü … feel more natural", [voicebox #846](https://github.com/jamiepine/voicebox/issues/846)) [anecdotal]. Measured runs disagree: WER 3.67 and UTMOS 2.97, the lowest naturalness of the major open systems ([PatientDesk](https://speech.patientdesk.ai/en/benchmarks)) [measured].
- 2026 list: "It's not very good at cloning … The company does not exist anymore." ([r/LocalTextToSpeech](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/))
- CPU benchmark post: "Don't bother on CPU: … XTTSv2 (GPU-only, 8GB+ VRAM)" ([r/TextToSpeech](https://www.reddit.com/r/TextToSpeech/comments/1sqkiuc/)) [measured on x86, not Apple]
- MPS fails with "Output channels > 65536 not supported at the MPS device" ([idiap #248](https://github.com/idiap/coqui-ai-TTS/issues/248)).

### 2.6 Piper Turkish (dfki / fahrettin / fettah): VITS, current panel engine
**Good**
- "Piper is old but latency is very low. Some voices are ok quality too, and it sips cpu" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1udg3rf/comment/otbh1wn/)) [anecdotal]
- A Turkish call-center builder measured Piper at about "300–500 ms inference time" on CPU ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1qqmmn0/comment/o2kfbfp/)).
- Benchmark post: "Piper Medium is still the default" for notifications and short utterances ([r/TextToSpeech](https://www.reddit.com/r/TextToSpeech/comments/1sqkiuc/)).

**Bad (the user's complaint, confirmed by other Turkish users)**
- The same Turkish customer-service builder: "customers are not happy at all, my friend. It sounds very robotic and completely disconnected from the emotional context of the sentence. Even if you write the most depressing sentence in the world, Piper reads it without adding any emotion." ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1qqmmn0/comment/o2kfrf3/)) [anecdotal, Turkish production use]
- The same author's post: "I have previously trained Piper, but none of the customers liked the quality" ([post](https://www.reddit.com/r/LocalLLaMA/comments/1qqmmn0/)).
- 2026 list: "Expect strange noises, pauses." ([r/LocalTextToSpeech](https://www.reddit.com/r/LocalTextToSpeech/comments/1u3kxxx/))

**Turkish-specific**
- On the Windows CLI, Turkish characters were dropped ("türkçe deki özel karakterlerin hiçbirini söylemiyor"). It was an encoding issue, fixed with `chcp 65001` ([piper #630](https://github.com/rhasspy/piper/issues/630)).
- A user noticed that ğ/ü/ş/ı are missing from `phoneme_id_map` ([piper #686](https://github.com/rhasspy/piper/issues/686)). [INFERENCE: expected, because ids are espeak phonemes, not letters.]
- Short phrases ("Hello", "Thank you") come out poorly when training data lacks them; the maintainer confirms ([piper1-gpl #79](https://github.com/OHF-Voice/piper1-gpl/issues/79)).
- dfki is fine-tuned from US-English lessac, and its dataset is CC BY-NC-SA ([MODEL_CARD](https://huggingface.co/datasets/rhasspy/piper-checkpoints/blob/4348dcaff20e321f785c5e6978be6d734332e6eb/tr/tr_TR/dfki/medium/MODEL_CARD)).
- Quick TTS ships only `tr_TR-fettah-medium` for Turkish ([quick-tts](https://quick-tts.com/blog/piper-voices-ranked.html)). [weak signal]
- sherpa-onnx 2.x removed espeak-ng and piper-phonemize in favour of a `lexicon.txt` per model ([sherpa-onnx #3731](https://github.com/k2-fsa/sherpa-onnx/issues/3731)). A contributor there notes that lexicons work for "english, turkish, and other languages with static pronounciation" but still need number normalization first. This is the breaking change that pinned the panel to `=1.13.8`.

### 2.7 seslen-tts: Turkish StyleTTS2 student, ONNX-only, 330 MB, CC BY-SA 4.0 weights
- README claims, which matter for Tellal:
  - "16 voices × 9 emotions … correct Turkish lexical stress and question intonation … ONNX Runtime only … RTF ≈ 0.2–0.4 on a laptop CPU"
  - "Teknik yazımlar Türkçe okunur: 'LLM' → elelem … 'v0.2.1', 'Qwen2.5' … 'fine-tuning'"
  - streams sentence by sentence (`s.seslendir`)
  - per-voice UTMOS: ayse 4.01, ege 3.82, serdar 3.46

  Source: [GitHub](https://github.com/seslen-tr/seslen-tts). [vendor]
- **No community feedback found.** The repo has 1 star and the HF model 0 downloads. Treat as unproven, but cheap to test.

### 2.8 Large Turkish models: Trendyol-TTS, VoxCPM2, Ahenk, Fish Audio Drama 3, Breeze
- **Trendyol-TTS** (VoxCPM2 LoRA, 2.38B, MIT). It has the best naturalness score on the Turkish leaderboard (UTMOS 3.82) but runs at 1.0× real time on an L4 ([PatientDesk](https://speech.patientdesk.ai/en/benchmarks)). Recommended settings from a Trendyol engineer: "cfg_value=1.8 inference_timesteps=16 max_len=600 normalize=False seed=42" ([HF #1](https://huggingface.co/Trendyol/Trendyol-TTS/discussions/1)). An MLX port exists ([emrekoc/trendyol-tts-mlx](https://huggingface.co/emrekoc/trendyol-tts-mlx)).
- **VoxCPM2 Turkish voice-design tool** (405★): "On a MacBook Air M4 16 GB: ~2 s of compute per 1 s of speech … peak ~5.3 GB RAM". The tool refuses digits ("write numbers in words") and needs a foreign-word pronunciation dictionary ([alexturkey/voxcpm2-turkce-ses-tasarimi](https://github.com/alexturkey/voxcpm2-turkce-ses-tasarimi)). [measured by author] That is too slow and too heavy for an always-on panel. **[INFERENCE]**
- A Kokoro user who wanted Turkish gave up: "anyway no need for me, i move https://huggingface.co/openbmb/VoxCPM2" ([kokoro #204](https://github.com/hexgrad/kokoro/issues/204)).
- **Fish Audio Drama 3**: "its Turkish is good", but "Its weights take 41.5 GB" ([r/LocalLLM](https://www.reddit.com/r/LocalLLM/comments/1xb4fm6/)). Out of scope.
- **Breeze TTS 2**: an English-centric thread praises it ("BreezeTTS 2 occasionally omits a word or phrase", [comment](https://www.reddit.com/r/LocalLLaMA/comments/1wxd404/comment/pdtneax/)). On audio.cpp: "just checked on M4 pro it can do realtime streaming with Q8" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1wxd404/comment/pdvjquo/)). A Turkish base exists ([ersanbil/Breeze-TTS-2-Turkish-Base](https://huggingface.co/ersanbil/Breeze-TTS-2-Turkish-Base)), but I found no user feedback on it.

### 2.9 OmniVoice (k2-fsa): 646 languages, voice cloning
- Turkish: "Words are intelligible but sound like a non-native speaker is reading Turkish text." The model has only 125 h of Turkish training data ([OmniVoice #49](https://github.com/k2-fsa/OmniVoice/issues/49)).
- Numbers: "In Turkish, it can only pronounce the number 6, others are glitchy." Maintainer: "I recommend using a text normalization tool to convert digits into words." ([OmniVoice #51](https://github.com/k2-fsa/OmniVoice/issues/51))
- Recommended twice to the Voicebox Turkish user ("Try Omnivoice", [comment](https://www.reddit.com/r/speechtech/comments/1tt5nhe/comment/op3b3j6/)).
- General opinion: "the best for its size/speed … But it lacks in consistency … not so much for real-time applications" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1xs46nh/comment/19sea0gk62o0y/)) [anecdotal]

### 2.10 Qwen3-TTS
- Second-hand Turkish report: "they said the resulting voice sounded like a foreigner speaking Turkish" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1qqmmn0/comment/o2kftrk/)) [anecdotal, hearsay]
- A Turkish 0.6B fine-tune appears in the Pocket TTS Turkish comparison at 1.7% WER ([card](https://huggingface.co/wite-tech/pocket-tts-turkish)). [vendor of a competitor]

### 2.11 General small-TTS sentiment (English-only models, for calibration)
- **Kokoro 82M** is the community default for "fast and good enough", but has no Turkish. The request [kokoro #204](https://github.com/hexgrad/kokoro/issues/204) is still open after 11 comments of Turkish "+1"s. Benchmark: "Kokoro is the slowest … but it's the only thing that actually sounds human" ([post](https://www.reddit.com/r/LocalLLaMA/comments/1udg3rf/)). Others now call it dated: "Kokoro is outdated now. There are a few super fast alternatives that are far better, like Pocket TTS." ([comment](https://www.reddit.com/r/TextToSpeech/comments/1xmwbkt/comment/19s4da3807gkh/))
- **UTMOS bias.** "UTMOS rewards clean vocoders over natural prosody, classic failure mode" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1udg3rf/comment/otbiqfw/)). Agents are not reliable judges either: "Then I listened to the actual output and disagreed with every single thing the agent found." ([comment](https://www.reddit.com/r/TextToSpeech/comments/1sqkiuc/comment/ohciwlj/)) So the Bakeoff needs human listening, not only WER/UTMOS.
- **Kitten TTS** (English only): "Under 25MB and cpu only for this quality is amazing" versus "The voices sound artificial and a bit grating" ([HN](https://news.ycombinator.com/item?id=44807868)).
- **Drifting-TTS (Turkish, 68M, from scratch).** Turkish redditors: "as a turkish speaking person it is extremely obvious and disturbingly robotic" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1x0oaum/comment/penho3x/)) and "the 'melody' of the language is off" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1x0oaum/comment/pes3na3/)). This is a reminder that **low WER does not mean natural Turkish prosody**.

---

## 3. Open-source spoken narration for coding agents

Overview comparison (checked Oct 2026): [heard.dev/compare](https://heard.dev/compare/open-source-voice-tools-for-coding-agents). That page is written by Heard, a competitor; I re-checked each README myself.

| Project | Agents | Engine(s) shipped / default | Latency & UX techniques (from README) | Turkish |
|---|---|---|---|---|
| [heardlabs/heard](https://github.com/heardlabs/heard) (218★, Apache-2.0 engine, macOS) | Claude Code, Codex hooks (Stop/Pre/PostToolUse/UserPromptSubmit) | Kokoro local (~350 MB); ElevenLabs/Speechify optional | Daemon auto-starts on first tool call. Speaks a **short summary** on finish, fail or needs-approval instead of reading everything. Template summaries when there is no LLM key. "Call-aware: stays quiet while you're on a Zoom…". Multi-agent queue. Notch in the closed 2.0 app | no |
| [mbailey/voicemode](https://github.com/mbailey/voicemode) (1449★, MIT) | Claude Code + MCP | Local Kokoro + whisper.cpp, or OpenAI | Local OpenAI-compatible services kept running | no |
| [rwunsch/voicemode-local](https://github.com/rwunsch/voicemode-local) | VoiceMode fork | Kokoro (FastAPI or ONNX) + **Piper for non-English** | `KOKORO_CPUS` cap "keeps TTS from starving the audio pipeline". Strict FIFO session queue. Always-on-top "voice pill". "fails loud" instead of silently swapping to a cloud voice | Piper route could carry tr [INFERENCE] |
| [paulpreibisch/AgentVibes](https://github.com/paulpreibisch/AgentVibes) (155★) | Claude Code, Claude Desktop (MCP), Warp | Piper (900+ voices), Kokoro, macOS say, ElevenLabs | The hook injects instructions so the agent speaks at task start and end. `/agent-vibes:set-language` | Piper tr voices available [INFERENCE] |
| [shreyas-s-rao/claude-code-narrator](https://github.com/shreyas-s-rao/claude-code-narrator) | Claude Code plugin | Kokoro in its own venv | "All sessions share a single daemon and FIFO". Final reply capped at ~1000 chars ending at a sentence boundary. `/narrator:hush`. UserPromptSubmit silences speech | no |
| [LLRHook/claude-speak](https://github.com/LLRHook/claude-speak) | Claude Code | Kokoro (default), Piper, ElevenLabs | "Parallel chunk generation, SIGUSR1 instant queue notification, persistent audio stream". `max_chunk_chars` 400 (lower to 100–200 "for faster first audio"). 24-step normalizer (units, dates, code). Model loaded once in the daemon | no |
| [aeriondyseti/claude-speaks](https://github.com/aeriondyseti/claude-speaks) | Claude Code | Pocket TTS (CPU) or any OpenAI `/v1/audio/speech` | "Speech starts about 0.1 s after a reply finishes". `http_split_sentences`: "first audio comes after one sentence's synthesis (≈0.3 s with Kokoro-FastAPI on a GPU, against ≈3.7 s for a whole paragraph)". User-editable `replacements` list ("RTX" → "ar tee ex") | no |
| [matiomacedo/pocket-voice](https://github.com/matiomacedo/pocket-voice) | Claude Code | Pocket TTS | "background daemon that keeps the model in memory … exits after 30 minutes idle". Skips code blocks, URLs, file:line refs. Responses spoken in finish order | no |
| [lav45/claude-code-tts](https://github.com/lav45/claude-code-tts) | Claude Code | Piper (default), Kokoro / mlx-audio on Apple Silicon | Opt-in sentence streaming. `queue.prefetch_next`. Workers unloaded after `worker_idle_unload_s` (1800). **Mic-aware pause**: pauses while you dictate, resumes and rewinds to the sentence start. "built by a human with ADHD" | no |
| [Abdalrhman-Attar/claude-voice](https://github.com/Abdalrhman-Attar/claude-voice) | Claude Code (+ Codex/Gemini/Cursor via MCP) | Kokoro ONNX; Arabic via NVIDIA MagpieTTS/NeMo-Speech.cpp | "Speaks while it types. Each sentence is read as soon as it is written." Sentence panel. Pauses during dictation. ~100-entry `pronounce.json` for tech words (RAM, JSON, CLI) and rules for "PRs", "200ms", "v2.1.0" | non-English via a separate engine, the same pattern Turkish would need [INFERENCE] |
| [ElMastho/Agent-Voice](https://github.com/ElMastho/Agent-Voice) (GPL-3, Windows-first, macOS beta) | Claude Code, Codex, Gemini CLI, Cursor | Piper (light, default) / Kokoro (natural) | Agents asked for a one-line `Voice summary:`. **Per-sentence language detection** routes FR or EN sentences to matching voices. Read-along text under the notch | bilingual FR/EN; the routing idea applies to TR + EN [INFERENCE] |
| [omarelkhal/aftertone](https://github.com/omarelkhal/aftertone) | Cursor, Claude Code, Codex | **Supertonic 3 ONNX** daemon | Speaks only `<spoken_summary>…</spoken_summary>` ("no tag, no speech"). Per-session on/off. `/aftertone_lang` | Supertonic covers tr, so possible but untested [INFERENCE] |
| [jochiang/voice-mcp](https://github.com/jochiang/voice-mcp) | Claude Code MCP | Whisper + Supertonic (voice M1) | Combined speak+listen tool call "reduce[s] latency" | no |
| Reddit "Breeze + Claude sessions" ([post](https://www.reddit.com/r/LocalLLaMA/comments/1wxd404/)) | Claude (cloud LLM) | Breeze TTS (4 GB VRAM) | "She watches for any claude session that finishes, and sends me the results in a very short, spoken style summary". "As soon as I receive the first few words, I pass it into breeze" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1wxd404/comment/pdutbfs/)). Push-to-talk avoids VAD wait | no |

Notch and island apps (visual only; no built-in speech except Heard 2.0 and Agent-Voice):
- [CodeIsland](https://github.com/wxtsky/CodeIsland)
- [notchi](https://github.com/sk-ruban/notchi)
- [agent-notch](https://github.com/enderdev01/agent-notch)
- [hyderay/notch](https://github.com/hyderay/notch)

They all work the same way: a Claude Code hook in `~/.claude/settings.json` or a Codex hook in `~/.codex/hooks.json` sends events to a Unix socket.

**Recurring techniques worth copying into Tellal** (they were adopted independently by several projects):
1. **Warm model in one long-lived process, plus idle unload.** Seen in pocket-voice (30 min), lav45 (1800 s), Heard and claude-speak. Tellal is already in-process, so this mostly means do not reload, and pre-compute voice and conditioning state once (Pocket's 0.4 s voice preparation; Chatterbox's precomputed conditionals per [HF #42](https://huggingface.co/ResembleAI/chatterbox/discussions/42)).
2. **Sentence chunking with prefetch of the next sentence.** Claude-speaks measured ≈0.3 s for the first sentence against ≈3.7 s for a whole paragraph. claude-speak runs parallel chunk generation on a persistent audio stream.
3. **Summaries instead of verbatim reading.** Heard, Agent-Voice, Aftertone and the Breeze setup all do this. Some inject an instruction so the agent writes a spoken-summary line or tag.
4. **Cleanup before TTS.** Strip markdown, code, URLs and file:line; then apply a pronunciation dictionary for tech jargon (claude-voice, claude-speaks). For Turkish the equivalent is normalizer-tr, trnorm or turkce-normalize, plus a custom map such as "PR" → "pi ar" and "deploy" → "diploy". **[INFERENCE]**
5. **Interrupt and queue hygiene.** A new prompt or dictation pauses or drops stale speech; a FIFO queue runs across sessions; the panel stays quiet during calls (Heard); it never silently falls back to cloud (voicemode-local).
6. **CPU capping** so TTS doesn't starve audio or the UI (voicemode-local `KOKORO_CPUS`). This matters for "runs all day next to other apps". **[INFERENCE]**

---

## 4. Cactus (Cactus Compute, YC S25)

**What it is today.** The [cactus-compute/cactus](https://github.com/cactus-compute/cactus) repo (14.6k★, Apache-2.0 per its LICENSE) offers:
- Needle 3, a tool-calling model of 8–29 MB
- **Whistle**, a speech-to-text model of 16.9 MB
- Mirage, a vision model

The README's only voice feature is transcription: "Whistle transcribes speech in seven languages". The [Whistle announcement](https://www.reddit.com/r/LocalLLaMA/comments/1wyemcb/) lists "English, German, French, Spanish, Italian, Dutch and Polish". **There is no TTS and no Turkish.** The older general SDK (now `cactus-engine`) once had a `cactus_tts` binary using OuteTTS GGUF; I found this only via secondary sources and did not verify it. The current README does not mention TTS. An issue some sources cite about Qwen-TTS support returns 404.

**User experience**
- Positive:
  - "I've been using Cactus for a few months, great product! Makes it really easy to plug and play different models on my phone." ([HN](https://news.ycombinator.com/item?id=45291334))
  - "Y'all over at Cactus are kinda my heroes, between this and Needle." ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1wyemcb/comment/pe2xw5o/))
  - A r/LocalLLaMA moderator kept a Whistle post up, noting that "their Apache-2.0 open source offering appears to be capable of fully-local operation" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1x3bpfl/comment/pfawzic/))
- Whistle quality: "skips some words on my crappy mic and messed up others … 85-90% success rate" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1x3bpfl/comment/pf9ksug/)). There is an open bug report: "Whistle writes a short phrase (mostly 'Thank you.') on 1,945 of 2,000 ESC-50 non-speech clips" ([issue #179](https://github.com/cactus-compute/cactus/issues/179)).
- Trust and licensing: in 2025 the project switched from Apache to non-commercial ("Open source for the PR, then switching to non-open licensing is a cowardly, bullshit move", [HN](https://news.ycombinator.com/item?id=45291893)). The repo has since gone back to Apache-2.0 (README). Pricing was "per-monthly-active-device" ([HN Launch](https://news.ycombinator.com/item?id=45291024)).
- Needle reliability: the web demo maps unrelated prompts to `lock_door` ("Query: HN … lock_door", [HN](https://news.ycombinator.com/item?id=49249736)). [anecdotal]
- Integration: there are C headers and native binaries. A C# user got it working after "passing it a path instead of the raw bytes of the model", using "cactus_rs' documentation" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1wyemcb/comment/peegdnq/)). So Rust bindings exist [unverified].
- Community pairing suggestion: "such a model paired with cactus' own needle model … then something like kokoro 82M for TTS" ([comment](https://www.reddit.com/r/LocalLLaMA/comments/1x3bpfl/comment/pf9ut1b/)). In other words, even fans expect TTS to come from somewhere else.

**Verdict for Tellal:** not applicable to Turkish TTS. The only plausible future use is on-device STT or intent parsing, and even that needs Turkish, which Whistle lacks.

---

## 5. Consensus

The community view in late 2026 is consistent. **For Turkish, the multilingual "big-name" open models are the wrong tool:**
- Chatterbox MTL, OmniVoice and Qwen3-TTS sound English-accented or "non-native" to Turkish listeners, and they misread numbers.
- Chatterbox is also slow, collapses on short lines, leaks memory on MPS, and was retired in Aug 2026.
- Supertonic 3 is fast and ONNX-friendly, but it invents words for Turkish numbers, does not stream, and its repo is archived.
- XTTS-v2 is the least natural on Turkish leaderboards and non-commercial.
- Piper is universally described as fast, robotic and emotionless. That matches the user's complaint, even though WER and UTMOS do not penalize it much.

What changed in Sep–Oct 2026 is a wave of **Turkish-only, tiny models trained on synthetic Turkish speech, each with a dedicated Turkish normalizer**: EMA Lightning, Antalia-2 Mini, Ahenk Cep, Pocket TTS Turkish and seslen-tts. Turkish users respond with unusual enthusiasm ("Türkçesi gerçekten çok iyi", "İnanılmaz") and with precise, fixable bug reports rather than "it sounds foreign". Their shared weak spots are exactly Tellal's hard cases:
- English and brand words inside Turkish text (PR, deploy, iOS, iPhone'um)
- isolated letters
- some number edge cases

So budget for a pronunciation map on top of the normalizer.

Evidence quality:
- EMA and Pocket TTS Turkish have the most real-world reports, including M1/M4 CPU usage.
- Antalia-2 Mini and seslen-tts have almost none.
- Every WER/UTMOS table comes from parties with a stake. PatientDesk sells Alania, and each model card benchmarks itself.

For latency and integration, agent-narration projects agree on a warm long-lived model, sentence chunking with prefetch, spoken summaries, and markdown/jargon cleanup. Hitabet shows that EMA, Antalia and Ahenk Cep run in-process from Rust through `ort` with ONNX exports. **[INFERENCE: that makes them the closest match to Tellal's architecture; Bakeoff should confirm by ear and by time-to-first-audio on the M4 Pro.]**

---

## 6. Settings tips sent to `agent://Bakeoff` (summary)
1. **New candidate EMA Lightning:** `pip install ema-lightning`, `EMA().say(text, seed=…)` / `.stream()`. Use ≥1.1.0 so normalizer-tr ≥0.5 is pulled in. Pin the seed and try 3–5 seeds. Warm up once. Add a ~15 ms tail fade. Expect "ç"-cluster clicks and weak English words. ONNX port: ema-lightning-web, Hitabet export script.
2. **Normalize all engines' input** with `normalizer-tr` ≥0.8 (`ambiguity_policy="fallback"`). This is a known Supertonic number failure; suffixed brands are a known gap. A Supertonic port must keep NFKD combining marks.
3. **Rust path:** Hitabet (`ort` + cpal + rubato) for EMA, Antalia-2 Mini (steps 8, cfg 2.0, temperature 0.6) and Ahenk Cep (16 Euler steps, cfg 2.0). seslen-tts is ONNX-only (ayse/ege/serdar voices). PatientDesk numbers attached.
4. **Chatterbox:** short-line hallucination (merge lines under 20 chars); chunk ≤300 chars; trim the tail; repetition penalty ≤1.2; MPS memory growth on Apple Silicon; keep a Turkish reference. **Pocket:** 2.1.0 inserted-word bug on Apple Silicon, amplified by accented references; Core ML and Rust/Candle ports exist.
5. **Pocket TTS Turkish:** use the `pocket_tts_turkish` package. With raw pocket-tts you must write numbers as words, send one sentence per call, start with a lowercase letter, and use a clean reference. Pre-warm the voice state (0.4 s). The male_1 voice is the narrator style. Emotion tag `sakin`.
