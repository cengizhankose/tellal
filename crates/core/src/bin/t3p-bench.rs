//! `t3p-bench` — TTS / T3 / end-to-end benchmarks that write a run dir like the app.
//!
//! * default: cold model load + synthesize a fixed Turkish corpus, per-sentence numbers.
//! * `--play`: same, and play each sentence.
//! * `--t3`: 3 rounds of listing every watched project.
//! * `--e2e`: headless `Core::start` → first `ThreadsUpdated` → `speak_thread` on the first
//!   completed thread → `SpeakingFinished`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use t3_panel_core::telemetry::{self, Telemetry, ms};
use t3_panel_core::tts::{TtsEngine, open_output, samples_source};
use t3_panel_core::{Config, Core, CoreEvent, t3};

const CORPUS: [&str; 10] = [
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

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let cfg = Config::from_env();
    if has("--e2e") {
        return e2e(cfg);
    }
    let tel = Telemetry::create(&cfg.log_root)?;
    let mode = if has("--t3") { "t3" } else if has("--play") { "tts+play" } else { "tts" };
    tel.event(
        "app.start",
        None,
        true,
        json!({"config": cfg.to_json(), "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id(),
               "bin": "t3p-bench", "mode": mode}),
        json!({"run_dir": tel.run_dir()}),
        None,
    );
    tel.start_sampler()?;
    if has("--t3") {
        bench_t3(&tel, &cfg)?;
    } else {
        bench_tts(&tel, &cfg, has("--play"))?;
    }
    println!("peak_rss_mb={:.1}", telemetry::peak_rss_mb());
    println!("run_dir={}", tel.run_dir().display());
    Ok(())
}

fn bench_tts(tel: &Arc<Telemetry>, cfg: &Config, play: bool) -> anyhow::Result<()> {
    let rss0 = telemetry::current_rss_mb();
    let t = Instant::now();
    let engine = TtsEngine::load(tel, &cfg.model_dir)?;
    println!(
        "cold_load_ms={:.1} rss_after_load_mb={:.1} (+{:.1}) sample_rate={} speakers={}",
        ms(t.elapsed()),
        telemetry::current_rss_mb(),
        telemetry::current_rss_mb() - rss0,
        engine.sample_rate,
        engine.num_speakers
    );
    let device = if play { Some(open_output()?) } else { None };
    let player = device.as_ref().map(|d| rodio::Player::connect_new(d.mixer()));
    println!("{:>3} {:>5} {:>9} {:>9} {:>7}  text", "#", "chars", "synth_ms", "audio_ms", "rtf");
    let (mut synth_sum, mut audio_sum) = (0.0, 0.0);
    for (i, text) in CORPUS.iter().enumerate() {
        let s = engine.synth(tel, text, None)?;
        synth_sum += s.synth_ms;
        audio_sum += s.audio_ms;
        println!("{:>3} {:>5} {:>9.1} {:>9.1} {:>7.4}  {text}", i + 1, text.chars().count(), s.synth_ms, s.audio_ms, s.rtf);
        if let Some(p) = &player {
            p.append(samples_source(s.samples, engine.sample_rate));
        }
    }
    println!("total synth_ms={synth_sum:.1} audio_ms={audio_sum:.1} rtf={:.4}", synth_sum / audio_sum);
    if let Some(p) = &player {
        p.sleep_until_end();
    }
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
        let shown = t3::select(cfg, results.into_iter().flatten().flatten(), telemetry::now_unix_ms()).len();
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

fn e2e(cfg: Config) -> anyhow::Result<()> {
    let t = Instant::now();
    let (core, rx) = Core::start(cfg)?;
    let run_dir = core.live_stats().run_dir;
    let deadline = Instant::now() + Duration::from_secs(120);
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
    let target = threads
        .iter()
        .find(|t| t.status == "completed")
        .ok_or_else(|| anyhow::anyhow!("no completed thread among {} rows", threads.len()))?;
    println!("speaking: {} [{}] {}", target.id, target.status, target.title);
    core.speak_thread(&target.id);
    loop {
        match next("SpeakingFinished")? {
            CoreEvent::SpeakingStarted { thread_id } => println!("SpeakingStarted {thread_id}"),
            CoreEvent::SpeakingFinished { thread_id } => {
                println!("SpeakingFinished {thread_id}");
                break;
            }
            CoreEvent::Error(e) => eprintln!("core error: {e}"),
            CoreEvent::ThreadsUpdated => {}
        }
    }
    let s = core.live_stats();
    println!(
        "click_to_audio_ms={:?} last_tts_ms={:?} last_rtf={:?} last_t3_poll_ms={:?} rss_mb={:.1} cpu_pct={:.1}",
        s.last_click_to_audio_ms, s.last_tts_ms, s.last_rtf, s.last_t3_poll_ms, s.rss_mb, s.cpu_pct
    );
    println!("peak_rss_mb={:.1}", telemetry::peak_rss_mb());
    println!("run_dir={}", run_dir.display());
    Ok(())
}
