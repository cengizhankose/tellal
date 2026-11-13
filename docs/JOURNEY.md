# How Tellal was built

Tellal (it started as "T3 Panel") was built in two days (10–11 November 2026) inside a single T3 Code thread. I dictated every request in Turkish with Wispr Flow; one coding agent (Claude Opus through omp) planned the work and handed slices to sub-agents. This page retells that conversation: what I asked for, what we found, and why each decision went the way it did. Quotes are translated from Turkish.

## 1. "Is HeyClicky the thing I need?"

The starting point was a habit: I talk to my computer, paste the transcript into an agent, then read what the agent did and its final report. I wanted the agent to talk back — narrate its steps ("now I'm doing this, now that") and finish with one sentence I can hear: *"Of the 80 tasks you gave me, 20 are stuck, 30 failed, the rest finished with warnings. Want details?"* Without a paid voice API, and inside the tool I already use, T3 Code.

We looked at the two projects I had tried:

- **HeyClicky / Clicky** — its "proxy" is a Cloudflare Worker that forwards to Anthropic, ElevenLabs and AssemblyAI. The keys move to a server; the bills stay. It also has its own chat brain that knows nothing about my T3 threads.
- **OpenClicky** — offline dictation and offline TTS (Kokoro), but Kokoro has no Turkish, the agent lane talks to the Codex CLI instead of T3, and "spoken task-finished summaries" was still on its roadmap.

Both are screen assistants. What I needed was a voice layer for agent threads.

**Decision:** don't add another model to summarise. The agent already knows what it did, so it writes the spoken line itself (a `🔊` line at the end of its reply), and a small local TTS reads it. A `sesli` ("spoken") skill tells agents how to write for the ear: one short sentence per step, numbers as digits, no paths or code in the first sentence, a `🔊` summary and a `👀` line for things you must look at.

## 2. From a skill to a panel

The first sketch was a script the agent calls to speak. I asked for something else: a small app that stays on top of everything, where I pick which thread I'm listening to, separate from T3's own sidebar. The agent should acknowledge before starting work and keep talking while it works.

T3 already exposes what such a panel needs: `t3 thread list|show|send --json`, a deep link per thread, and the local state database.

**Framework:** I wanted Rust. GPUI (Zed's UI framework) won over Tauri because its `PopUp` window kind is already a non-activating `NSPanel` at pop-up level that joins all Spaces and fullscreen apps — exactly the overlay behaviour, with no WebView.

## 3. v0.1 — measure everything first

My one rule for the build: *"Let's progress in a way we can monitor — performance, what it uses, what goes in, what comes out — so we can improve it as we go."* Every run writes `events.jsonl` (every input and output with its duration), `metrics.jsonl` (RSS/CPU every second) and the audio it spoke; `tellal-report` turns a run into tables and an HTML report.

The telemetry paid off on day one:

- **Polling cost.** The first 30-second idle run showed 54 `t3 thread list` calls eating ~62 % of a CPU core, because each call boots T3's Electron runtime. Fix: one read-only SQLite query per tick (~14 ms) finds which projects changed; only those are listed. −75 % CPU.
- **A breaking dependency.** sherpa-onnx 2.0 dropped espeak-ng, which silently broke Turkish Piper voices (`phoneme_codepoints is empty`). Pinned to 1.13.8.
- **First numbers:** click → audio 377 ms, synthesis RTF 0.04.

## 4. v0.2 — talk back, and channels that behave like video

My list for the second round:

- open a thread by double-clicking its row, inside the T3 app, not the browser;
- a small box to reply, filled by voice (I already use Wispr Flow);
- the agent says "okay, here's my plan" before it starts;
- threads are channels like a radio — unmute to listen — but with video controls: pause one, skip ("okay, next"), jump to live.

What we found:

- T3's desktop bundle registers `t3code://threads/<env>/<thread>`; opening that skips the browser.
- Wispr Flow's official MCP server only exposes Notetaker data, not dictation. Its app does register `wispr-flow://start-hands-free` / `stop-hands-free`, and every dictation lands in its local History database — enough for a reply dock.
- The end-to-end test played a Turkish sentence from the speakers into Wispr: transcript in the dock, identical to Wispr's own record, sent to the thread, focus returned to the previous app. Stop → text 1.19 s; agent reply → spoken 832 ms.

## 5. Finding a better Turkish voice

Piper worked but sounded robotic. I asked for a proper survey: new small models, the company Cactus, Reddit and GitHub opinions, Turkish support first.

- **Survey:** 21 Turkish-capable open models, with licences, runtimes and Rust paths. Cactus Compute ships no TTS model at all.
- **Community:** Turkish users rate a new wave of tiny Turkish-only models (EMA Lightning, Antalia, Ahenk) far above the multilingual giants; everyone agrees text normalisation matters more than the model.
- **Bake-off:** 59 runs on this Mac, each in a fresh process, with the panel's own sentences fed raw (digits, apostrophe suffixes, English tech words). Scored with Whisper read-back, time to first audio, RSS and licence — full tables in [`research/tts/`](../research/tts/README.md).

I picked three by ear; we shipped **EMA Lightning** (8.6 M parameters, 34 MB, Apache-2.0) because it runs in-process.

Porting it to Rust (ONNX Runtime via `ort` + `normalizer-tr`) turned up one more surprise: `ort` defaults to an efficiency policy that registers CoreML on macOS, which held ~550 MB of working memory and took 2.5 s to load. Forcing the CPU provider: ~150 MB, 0.6 s, same speed. A small pronunciation lexicon fixes English words (`build` became `bilt` after Whisper read-back said `bild` blurred into the next word).

## 6. v0.3 — make it look like it belongs

I said the UI looked too plain and asked for a redesign built on three sources:

- my evidence-based UI rules (a knowledge base distilled from 1,773 studies);
- my own design-system notes;
- a close study of T3's sidebar.

Plus small animations, and — "if it's not too hard" — the colours of the theme I picked in T3.

- **The study:** 28 findings from T3's source: row heights, status semantics, project tiles, time format, sections.
- **The panel follows T3 where it should,** and the evidence where it matters:
  - stable order;
  - static status glyphs instead of spinners;
  - an always-visible listen toggle;
  - keyboard paths for everything;
  - Reduce Motion;
  - ≥ 4.5:1 contrast in every theme.
- **Theme sync:** the panel reads T3's selected theme from its localStorage (a tiny read-only LevelDB + Snappy reader) and the built-in palettes from the installed `app.asar`. It follows theme and light/dark changes within 2 seconds. Sampled pixels from T3's window match the resolved colours.
- **Idle CPU:** 1.7 %.

## 7. Open-sourcing it

The last request was to publish the repo with its history. The history was rewritten once before publishing: my private design notes and screenshots of my real threads were removed and local paths replaced; everything else — commits, messages, order — is as it happened. The screenshots and the demo video come from a fixture (`demo/fixture.json`), so you can try the panel without T3 too.
