//! `tellal-bench` — TTS / T3 / end-to-end benchmarks that write a run dir like the app.
//!
//! * default: cold model load + synthesize a fixed corpus (`--voice tr`, default: Turkish;
//!   `--voice en`: English agent lines), per-sentence numbers; each sentence is saved as
//!   `<run_dir>/audio/bench-NN.wav` (unless `TELLAL_SAVE_AUDIO=0`) and the numbers as
//!   `<run_dir>/bench.json` (read back by `research/tts/bakeoff/harness/readback_en.py --tellal`).
//!   `--corpus FILE` reads the sentences from FILE instead (one per line).
//! * `--play`: same, and play each sentence.
//! * `--download [ID ...]`: download voices (default: every voice) into `TELLAL_MODELS_DIR`
//!   through the app's downloader (pinned URLs, SHA-256, atomic install); fails unless all end
//!   Ready.
//! * `--t3`: 3 rounds of listing every watched project.
//! * `--theme`: resolve the T3 theme once; print the `ThemeView` as JSON, the resolution report
//!   and the contrast table.
//! * `--e2e`: headless `Core::start` → first `ThreadsUpdated` (rows dumped, then compared with
//!   the next poll's order) → voices listed, any download awaited → `set_listening(id, true)` on
//!   the first completed thread → its catch-up line, sampling `ChannelView` progress every
//!   250 ms, pausing ~1.2 s in for 1.5 s → `SpeakingFinished` → (demo: every thread listens until
//!   the script is spoken, deadline 300 s instead of 120 s) → `mute_all` → events.jsonl check:
//!   each `audio.play` in a language whose voice is Ready used that voice (the languages heard
//!   are printed). With `TELLAL_DEMO` the listed threads must be exactly the fixture's.
//! * `--computer [--secs N] [--warmup N]`: headless `Core::start`, prints `Core::computer()` on
//!   every `ComputerUpdated` and `TaskStarted`, then this process's CPU% over the `N` seconds
//!   after the warmup (default 60 / 15). Real mode with `--stop-on-session`: the first live
//!   session triggers `computer_stop_all` 3 s later. Demo mode (`TELLAL_DEMO`): listens to the
//!   first fixture thread whose computer session is scripted (`t-notes`), grants permissions at
//!   3 s, dictates `NEW_TASK` at 38 s and starts the task with the transcript, stops all
//!   computer sessions at 58 s.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tellal_core::telemetry::{self, Telemetry, ms};
use tellal_core::tts::{TtsEngine, open_output, samples_source, write_wav};
use tellal_core::{Config, Core, CoreEvent, Lang, NEW_TASK, ThreadView, VoiceState, t3, theme};

const CORPUS_TR: [&str; 10] = [
    "Tamam.",
    "Görev tamamlandı.",
    "80 görevin 20'si takıldı, kalanlar başarıyla bitti.",
    "Testlerin hepsi geçti ve değişiklikler ana dala birleştirildi.",
    "Hâlâ çalışıyor, şu an veritabanı göçünü uyguluyor.",
    "Durduruldu.",
    "Derleme 3 dakika 42 saniye sürdü; uyarı sayısı sıfır.",
    "Kullanıcı arayüzünde iki küçük hata buldum, ikisini de düzelttim ve ekran görüntülerini klasöre kaydettim.",
    "Saat 14:30'da başlayan iş, yüzde 75 oranında ilerledi.",
    "Model yüklemesi bittiğinde panel, seçtiğin konuşmanın son durumunu kısa bir Türkçe cümleyle okuyacak.",
];

/// Agent-style English lines (the English bake-off corpus plus code, paths and units).
const CORPUS_EN: [&str; 12] = [
    "Okay, I'm collecting the errors first, then fixing the most common one.",
    "Running the tests now.",
    "80 tasks: 20 stuck, 30 failed, the rest finished with warnings. Want to start with the errors?",
    "Pull request opened; waiting for your approval to deploy.",
    "Build finished; I took a screenshot in the iOS simulator and added it to the report.",
    "Stopped.",
    "npm test passed in 42 seconds, 3 tests skipped.",
    "The PR is ready for review, CI finished at 3:45 PM.",
    "Deployed at 14:30; the API now answers in 120 ms.",
    "Still running. 5 files changed so far, about 2 minutes left.",
    "Fixed the crash in src/panel/voice.rs and pushed 3f9a2c1d to main.",
    "Bundle size dropped 18% to 2.4 MB after the v2.1.0 upgrade.",
];

fn download(cfg: &Config, ids: &[String]) -> anyhow::Result<()> {
    let ids: Vec<String> = if ids.is_empty() {
        ["tr-ema-lightning", "en-supertonic-2"].map(str::to_owned).to_vec()
    } else {
        ids.to_vec()
    };
    let t = Instant::now();
    let mut last_print = Instant::now();
    let views = tellal_core::download_voices(cfg, &ids, |views| {
        if last_print.elapsed() >= Duration::from_secs(1) {
            last_print = Instant::now();
            for v in views {
                if let VoiceState::Downloading { done_bytes, total_bytes } = v.state {
                    println!("  {:>5.1}s {} {:.1}/{:.1} MB", ms(t.elapsed()) / 1000.0, v.id, done_bytes as f64 / 1e6, total_bytes as f64 / 1e6);
                }
            }
        }
    })?;
    println!("models_dir={} after {:.1} s", cfg.models_dir.display(), ms(t.elapsed()) / 1000.0);
    for v in &views {
        println!("voice {} ({} MB): {:?}", v.id, v.download_mb, v.state);
    }
    if let Some(v) = views.iter().find(|v| ids.contains(&v.id) && v.state != VoiceState::Ready) {
        anyhow::bail!("{} is not Ready: {:?}", v.id, v.state);
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let cfg = Config::from_env();
    if has("--e2e") {
        return e2e(cfg);
    }
    if let Some(i) = args.iter().position(|a| a == "--download") {
        return download(&cfg, &args[i + 1..]);
    }
    if has("--computer") {
        let num = |f: &str, d: u64| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(d);
        return computer(cfg, num("--secs", 60), num("--warmup", 15), has("--stop-on-session"));
    }
    if has("--theme") {
        return print_theme(&cfg);
    }
    let tel = Telemetry::create(&cfg.log_root)?;
    let mode = if has("--t3") { "t3" } else if has("--play") { "tts+play" } else { "tts" };
    tel.event(
        "app.start",
        None,
        true,
        json!({"config": cfg.to_json(), "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id(),
               "bin": "tellal-bench", "mode": mode}),
        json!({"run_dir": tel.run_dir()}),
        None,
    );
    tel.start_sampler()?;
    if has("--t3") {
        bench_t3(&tel, &cfg)?;
    } else {
        let lang = match args.iter().position(|a| a == "--voice").and_then(|i| args.get(i + 1)) {
            Some(v) => Lang::parse(v).ok_or_else(|| anyhow::anyhow!("--voice {v}: expected tr | en"))?,
            None => Lang::Tr,
        };
        let corpus: Vec<String> = match args.iter().position(|a| a == "--corpus").and_then(|i| args.get(i + 1)) {
            Some(path) => std::fs::read_to_string(path)?.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect(),
            None if lang == Lang::En => CORPUS_EN.iter().map(|s| (*s).to_owned()).collect(),
            None => CORPUS_TR.iter().map(|s| (*s).to_owned()).collect(),
        };
        bench_tts(&tel, &cfg, lang, &corpus, has("--play"))?;
    }
    println!("peak_rss_mb={:.1}", telemetry::peak_rss_mb());
    println!("run_dir={}", tel.run_dir().display());
    Ok(())
}

fn bench_tts(tel: &Arc<Telemetry>, cfg: &Config, lang: Lang, corpus: &[String], play: bool) -> anyhow::Result<()> {
    let rss0 = telemetry::current_rss_mb();
    let t = Instant::now();
    let engine = TtsEngine::load(tel, cfg, lang)?;
    let load_ms = ms(t.elapsed());
    println!(
        "voice={} cold_load_ms={load_ms:.1} rss_after_load_mb={:.1} (+{:.1}) sample_rate={} threads={}",
        engine.voice,
        telemetry::current_rss_mb(),
        telemetry::current_rss_mb() - rss0,
        engine.sample_rate,
        cfg.tts_threads
    );
    let audio_dir = tel.run_dir().join("audio");
    if cfg.save_audio {
        std::fs::create_dir_all(&audio_dir)?;
    }
    let device = if play { Some(open_output()?) } else { None };
    let player = device.as_ref().map(|d| {
        let player = rodio::Player::connect_new(d.mixer());
        player.set_volume(cfg.output_volume);
        player
    });
    println!("{:>3} {:>5} {:>9} {:>9} {:>7}  text", "#", "chars", "synth_ms", "audio_ms", "rtf");
    let (mut synth_sum, mut audio_sum) = (0.0, 0.0);
    let mut rows = Vec::with_capacity(corpus.len());
    for (i, text) in corpus.iter().enumerate() {
        let wav = cfg.save_audio.then(|| audio_dir.join(format!("bench-{:02}.wav", i + 1)));
        let s = engine.synth(tel, text, wav.as_deref())?;
        if let Some(path) = &wav {
            write_wav(path, &s.samples, engine.sample_rate)?;
        }
        synth_sum += s.synth_ms;
        audio_sum += s.audio_ms;
        println!("{:>3} {:>5} {:>9.1} {:>9.1} {:>7.4}  {text}", i + 1, text.chars().count(), s.synth_ms, s.audio_ms, s.rtf);
        rows.push(json!({"id": format!("{:02}", i + 1), "text": text, "synth_ms": s.synth_ms, "audio_ms": s.audio_ms,
                         "rtf": s.rtf, "wav": wav}));
        if let Some(p) = &player {
            p.append(samples_source(s.samples, engine.sample_rate));
        }
    }
    println!("total synth_ms={synth_sum:.1} audio_ms={audio_sum:.1} rtf={:.4}", synth_sum / audio_sum);
    if let Some(p) = &player {
        p.sleep_until_end();
    }
    let report = json!({"voice": engine.voice, "load_ms": load_ms, "sample_rate": engine.sample_rate,
                        "threads": cfg.tts_threads, "peak_rss_mb": telemetry::peak_rss_mb(), "sentences": rows});
    std::fs::write(tel.run_dir().join("bench.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

fn bench_t3(tel: &Telemetry, cfg: &Config) -> anyhow::Result<()> {
    let projects = t3::discover(tel, cfg)?;
    println!("projects={}", projects.len());
    for round in 1..=3 {
        let cpu0 = telemetry::children_cpu_ms();
        let t = Instant::now();
        let results = t3::list_all(tel, cfg, &projects);
        let wall = ms(t.elapsed());
        let mut threads = 0;
        let mut errors = 0;
        for (p, r) in projects.iter().zip(&results) {
            match r {
                Ok(l) => threads += l.len(),
                Err(e) => {
                    errors += 1;
                    eprintln!("  {}: {e:#}", p.display_name());
                }
            }
        }
        let meta = t3::Meta::default();
        let shown = t3::select(cfg, results.into_iter().flatten().flatten(), telemetry::now_unix_ms(), &meta).len();
        println!(
            "round {round}: wall_ms={wall:.1} child_cpu_ms={:.1} threads={threads} shown={shown} errors={errors}",
            telemetry::children_cpu_ms() - cpu0
        );
        tel.event(
            "bench.t3_round",
            Some(wall),
            errors == 0,
            json!({"round": round, "projects": projects.len()}),
            json!({"threads": threads, "shown": shown, "child_cpu_ms": telemetry::children_cpu_ms() - cpu0}),
            None,
        );
    }
    Ok(())
}

/// The demo fixture's scripted live messages arrive by ~24 s after `Core::start`.
const DEMO_SCRIPT_S: u64 = 26;
/// Demo runs hear every thread's script to the end (English lines run longer than Turkish ones).
const DEMO_DEADLINE: Duration = Duration::from_secs(300);
const DEADLINE: Duration = Duration::from_secs(120);

fn e2e(cfg: Config) -> anyhow::Result<()> {
    let t = Instant::now();
    let demo = cfg.demo.clone();
    let (core, rx) = Core::start(cfg)?;
    let run_dir = core.live_stats().run_dir;
    let deadline = Instant::now() + if demo.is_some() { DEMO_DEADLINE } else { DEADLINE };
    let next = |what: &str| -> anyhow::Result<CoreEvent> {
        rx.recv_deadline(deadline).map_err(|_| anyhow::anyhow!("timed out waiting for {what}"))
    };
    loop {
        match next("ThreadsUpdated")? {
            CoreEvent::ThreadsUpdated => break,
            CoreEvent::Error(e) => eprintln!("core error: {e}"),
            _ => {}
        }
    }
    let threads = core.threads();
    println!("first ThreadsUpdated after {:.1} ms: {} threads", ms(t.elapsed()), threads.len());
    dump_threads(&threads);
    if let Some(path) = &demo {
        check_demo_threads(path, &threads)?;
    }
    // Voices first: a scripted (demo) or automatic download finishes before anything is spoken.
    print_voices(&core);
    while core.voices().iter().any(|v| matches!(v.state, VoiceState::Downloading { .. })) {
        match next("voice download")? {
            CoreEvent::VoicesUpdated => {}
            CoreEvent::Error(e) => eprintln!("core error: {e}"),
            _ => {}
        }
    }
    print_voices(&core);
    let completed: Vec<&ThreadView> = threads.iter().filter(|t| t.status == "completed").collect();
    let target = *completed.first().ok_or_else(|| anyhow::anyhow!("no completed thread among {} rows", threads.len()))?;
    println!("listening: {} [{}] {}", target.id, target.status, target.title);
    core.set_listening(&target.id, true);
    let mut started: Option<Instant> = None;
    let mut paused_at: Option<Instant> = None;
    let mut resumed = false;
    let mut polls = 1;
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(CoreEvent::SpeakingStarted { thread_id }) => {
                println!("SpeakingStarted {thread_id}");
                started.get_or_insert_with(Instant::now);
            }
            Ok(CoreEvent::SpeakingFinished { thread_id }) if thread_id == target.id && (resumed || paused_at.is_none()) => {
                println!("SpeakingFinished {thread_id}");
                break;
            }
            Ok(CoreEvent::ThreadsUpdated) => {
                polls += 1;
                let order = |v: &[ThreadView]| v.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
                let now = core.threads();
                println!("poll {polls}: {} rows, order unchanged vs first poll: {}", now.len(), order(&now) == order(&threads));
            }
            Ok(CoreEvent::Error(e)) => eprintln!("core error: {e}"),
            Ok(_) | Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(e) => anyhow::bail!("core channel: {e}"),
        }
        if Instant::now() > deadline {
            anyhow::bail!("timed out waiting for SpeakingFinished");
        }
        let ch = core.channels().get(&target.id).cloned().unwrap_or_default();
        if let Some(s) = started {
            println!(
                "  t={:>6.0}ms paused={} speaking={} elapsed_ms={:?} total_ms={:?}",
                ms(s.elapsed()),
                ch.paused,
                ch.speaking,
                ch.now_elapsed_ms,
                ch.now_total_ms
            );
        }
        if paused_at.is_none() && ch.now_elapsed_ms.is_some_and(|e| e >= 1200) {
            println!("  -> toggle_pause (pause)");
            core.toggle_pause(&target.id);
            paused_at = Some(Instant::now());
        } else if !resumed && paused_at.is_some_and(|p| p.elapsed() >= Duration::from_millis(1500)) {
            println!("  -> toggle_pause (resume)");
            core.toggle_pause(&target.id);
            resumed = true;
        }
    }
    // Demo: listen to every thread through the fixture's scripted live messages until the queue
    // is drained; events.jsonl is checked for the voice of each line below.
    if demo.is_some() {
        for th in threads.iter().filter(|th| th.id != target.id) {
            core.set_listening(&th.id, true);
        }
        println!("listening to all {} threads until the script (t >= {DEMO_SCRIPT_S} s) is spoken", threads.len());
        loop {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(CoreEvent::SpeakingStarted { thread_id }) => println!("SpeakingStarted {thread_id}"),
                Ok(CoreEvent::ThreadsUpdated) => polls += 1,
                Ok(CoreEvent::Error(e)) => eprintln!("core error: {e}"),
                Ok(_) | Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(e) => anyhow::bail!("core channel: {e}"),
            }
            let busy = core.channels().values().any(|c| c.speaking || c.queued > 0);
            if t.elapsed() >= Duration::from_secs(DEMO_SCRIPT_S) && !busy {
                break;
            }
            if Instant::now() > deadline {
                anyhow::bail!("timed out waiting for the demo script's lines");
            }
        }
    }
    core.mute_all();
    while polls < 2 {
        if let CoreEvent::ThreadsUpdated = next("second ThreadsUpdated")? {
            polls += 1;
            let now = core.threads();
            let same = now.iter().map(|t| &t.id).eq(threads.iter().map(|t| &t.id));
            println!("poll {polls}: {} rows, order unchanged vs first poll: {same}", now.len());
        }
    }
    let s = core.live_stats();
    println!(
        "click_to_audio_ms={:?} last_tts_ms={:?} last_rtf={:?} last_t3_poll_ms={:?} rss_mb={:.1} cpu_pct={:.1} \
         voice_ready={} t3_connected={}",
        s.last_click_to_audio_ms, s.last_tts_ms, s.last_rtf, s.last_t3_poll_ms, s.rss_mb, s.cpu_pct, s.voice_ready,
        s.t3_connected
    );
    println!("peak_rss_mb={:.1}", telemetry::peak_rss_mb());
    println!("run_dir={}", run_dir.display());
    check_voices_used(&run_dir, &core.voices())
}

fn print_voices(core: &Core) {
    for v in core.voices() {
        println!("voice {} ({}, {}, {} MB): {:?}", v.id, v.name, v.lang.code(), v.download_mb, v.state);
    }
}

/// Every `audio.play` in this run's events.jsonl: the line, its language and the voice that
/// spoke it. A line whose language's voice is Ready must be spoken by it; at least one line must
/// have been spoken. Which languages occur depends on the content (and `Config.lang` for the
/// core's own lines), so it is reported, not required.
fn check_voices_used(run_dir: &std::path::Path, voices: &[tellal_core::VoiceView]) -> anyhow::Result<()> {
    // `audio.play` is logged right after `SpeakingFinished`: wait until every queued line has one.
    let read = || -> anyhow::Result<Vec<serde_json::Value>> {
        Ok(std::fs::read_to_string(run_dir.join("events.jsonl"))?.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
    };
    let count = |events: &[serde_json::Value], kind: &str| events.iter().filter(|e| e["kind"] == kind && e["ok"] == true).count();
    let mut events = read()?;
    let settle = Instant::now() + Duration::from_secs(2);
    while count(&events, "audio.play") < count(&events, "speak.enqueue") && Instant::now() < settle {
        std::thread::sleep(Duration::from_millis(50));
        events = read()?;
    }
    let mut langs = std::collections::BTreeMap::<String, usize>::new();
    for e in events.iter().filter(|e| e["kind"] == "audio.play" && e["ok"] == true) {
        let (lang, voice) = (e["in"]["lang"].as_str().unwrap_or("?"), e["in"]["voice"].as_str().unwrap_or("?"));
        println!("audio.play lang={lang} voice={voice} text={}", e["in"]["text"]);
        let own = voices.iter().find(|v| v.lang.code() == lang && v.state == VoiceState::Ready).map(|v| v.id.as_str());
        if own.is_some_and(|o| o != voice) {
            anyhow::bail!("a {lang} line was spoken by {voice}, expected {own:?} (Ready)");
        }
        *langs.entry(lang.to_owned()).or_default() += 1;
    }
    if langs.is_empty() {
        anyhow::bail!("no line was spoken");
    }
    println!("lines per language {langs:?}, each by its own Ready voice: OK");
    Ok(())
}

fn computer(cfg: Config, secs: u64, warmup: u64, stop_on_session: bool) -> anyhow::Result<()> {
    let demo = cfg.demo.is_some();
    let (core, rx) = Core::start(cfg)?;
    let run_dir = core.live_stats().run_dir;
    let t0 = Instant::now();
    let at = |s: u64| t0 + Duration::from_secs(s);
    let end = at(warmup + secs);
    println!("initial computer(): {:#?}", core.computer());
    // Demo walkthrough beats and the real-mode stop (one shot each).
    let (mut granted, mut dictated, mut stopped) = (!demo, !demo, !demo && !stop_on_session);
    let mut stop_at: Option<Instant> = None;
    let mut window: Option<(Instant, f64)> = None;
    if demo {
        core.set_listening("t-notes", true);
    }
    while Instant::now() < end {
        if window.is_none() && Instant::now() >= at(warmup) {
            window = Some((Instant::now(), self_cpu_ms()));
            println!("[{:>5.1}s] cpu window starts", t0.elapsed().as_secs_f64());
        }
        if !granted && Instant::now() >= at(3) {
            granted = true;
            println!("[{:>5.1}s] -> computer_grant_permissions", t0.elapsed().as_secs_f64());
            core.computer_grant_permissions();
        }
        if !dictated && Instant::now() >= at(38) {
            dictated = true;
            println!("[{:>5.1}s] -> dictation_start(NEW_TASK)", t0.elapsed().as_secs_f64());
            core.dictation_start(NEW_TASK);
        }
        if (!stopped && demo && Instant::now() >= at(58)) || stop_at.is_some_and(|s| Instant::now() >= s) {
            stopped = true;
            stop_at = None;
            println!("[{:>5.1}s] -> computer_stop_all ({} live)", t0.elapsed().as_secs_f64(), core.computer().sessions.len());
            core.computer_stop_all();
        }
        let ts = t0.elapsed().as_secs_f64();
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(CoreEvent::ComputerUpdated) => {
                let v = core.computer();
                println!("[{ts:>5.1}s] ComputerUpdated: {v:#?}");
                if stop_on_session && !stopped && stop_at.is_none() && !v.sessions.is_empty() {
                    stop_at = Some(Instant::now() + Duration::from_secs(3));
                }
            }
            Ok(CoreEvent::DictationTranscript { thread_id, text }) => {
                println!("[{ts:>5.1}s] DictationTranscript {thread_id}: {text}");
                if thread_id == NEW_TASK {
                    println!("[{ts:>5.1}s] -> start_task");
                    core.start_task(&text);
                }
            }
            Ok(CoreEvent::TaskStarted { thread_id, ok, detail }) => {
                println!("[{ts:>5.1}s] TaskStarted ok={ok} thread={thread_id:?} detail={detail}");
                if let Some(t) = core.threads().iter().find(|t| Some(&t.id) == thread_id.as_ref()) {
                    let ch = core.channels().get(&t.id).cloned().unwrap_or_default();
                    println!("         listed: [{}] {} · {} ({}/{}) listening={}", t.status, t.title, t.project, t.provider, t.model, ch.listening);
                }
            }
            Ok(CoreEvent::SpeakingStarted { thread_id }) => {
                let text = core.channels().get(&thread_id).and_then(|c| c.now_text.clone()).unwrap_or_default();
                println!("[{ts:>5.1}s] SpeakingStarted {thread_id}: {text}");
            }
            Ok(CoreEvent::DictationFailed { thread_id, error }) => println!("[{ts:>5.1}s] DictationFailed {thread_id}: {error}"),
            Ok(CoreEvent::Error(e)) => eprintln!("[{ts:>5.1}s] core error: {e}"),
            Ok(_) | Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(e) => anyhow::bail!("core channel: {e}"),
        }
    }
    if let Some((start, cpu0)) = window {
        let wall = start.elapsed().as_secs_f64() * 1000.0;
        let cpu = self_cpu_ms() - cpu0;
        println!(
            "cpu window {:.1}s: self_cpu_ms={cpu:.1} self_cpu_pct={:.2} children_cpu_ms_total={:.1}",
            wall / 1000.0,
            100.0 * cpu / wall,
            telemetry::children_cpu_ms()
        );
    }
    println!("final computer(): {:#?}", core.computer());
    println!("run_dir={}", run_dir.display());
    Ok(())
}

/// User + system CPU of this process so far (ms).
fn self_cpu_ms() -> f64 {
    // SAFETY: getrusage only writes into the provided, properly sized struct.
    let ru = unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        ru
    };
    let tv = |t: libc::timeval| t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

/// Demo mode: the snapshot lists exactly the fixture's thread ids.
fn check_demo_threads(path: &std::path::Path, threads: &[ThreadView]) -> anyhow::Result<()> {
    let fixture: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let mut want: Vec<&str> = fixture["threads"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t["id"].as_str()).collect())
        .unwrap_or_default();
    let mut got: Vec<&str> = threads.iter().map(|t| t.id.as_str()).collect();
    want.sort_unstable();
    got.sort_unstable();
    anyhow::ensure!(got == want, "demo threads {got:?} != fixture threads {want:?} ({})", path.display());
    println!("demo fixture {}: all {} threads listed", path.display(), got.len());
    Ok(())
}

fn dump_threads(threads: &[ThreadView]) {
    for (i, t) in threads.iter().enumerate() {
        let icon = t.project_icon.as_ref().map(|i| format!("{}:{}:{}", i.kind, i.name, i.color.as_deref().unwrap_or("-")));
        println!(
            "{:>2} pin={} snz={} settled={} {:<11} created={} since={} proj={} ({}{}) icon={} | {}",
            i + 1,
            u8::from(t.pinned),
            u8::from(t.snoozed),
            u8::from(t.settled),
            t.status,
            t.created_at,
            t.running_since.as_deref().unwrap_or("-"),
            t.project,
            t.project_id,
            if t.project_pinned { ", pinned" } else { "" },
            icon.as_deref().unwrap_or("-"),
            t.title
        );
    }
}

fn print_theme(cfg: &Config) -> anyhow::Result<()> {
    let r = theme::Resolver::default().resolve(&theme::Sources::new(cfg));
    let v = &r.view;
    let view = serde_json::json!({
        "source": v.source, "name": v.name, "dark": v.dark, "colors": theme::colors_json(v),
        "font_family": v.font_family, "font_size": v.font_size,
    });
    println!("{}", serde_json::to_string_pretty(&view)?);
    if let Some(e) = &r.error {
        println!("fallback_reason: {e}");
    }
    println!("resolve_ms={:.1}", r.dur_ms);
    println!("report: {}", serde_json::to_string_pretty(&r.report)?);
    println!("{:<12} {:<14} {:>6} {:>5}  ok", "fg", "bg", "ratio", "min");
    for (f, b, ratio, min) in theme::contrast_table(v) {
        println!("{f:<12} {b:<14} {ratio:>6.2} {min:>5.1}  {}", if ratio >= min { "yes" } else { "NO" });
    }
    Ok(())
}
