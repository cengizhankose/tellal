//! Local TTS (sherpa-onnx VITS/Piper) and the speech queue.
//!
//! Pipeline: `speak_thread` logs `speak.request` and enqueues → prep worker (`t3 thread show`,
//! extract, sentence split) → TTS worker (synthesize sentence 1, start playback, synthesize the
//! next while the previous plays) → `audio.play`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use serde_json::{Value, json};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig};

use crate::extract::{extract, split_sentences};
use crate::telemetry::{Telemetry, current_rss_mb, ms, round2};
use crate::{Config, CoreEvent, t3};

/// Messages fetched per `speak_thread`.
const SHOW_MESSAGES: u32 = 12;
/// The audio device is released after this much idle time (an open CoreAudio stream costs CPU).
const DEVICE_IDLE: Duration = Duration::from_secs(30);
const ONNX_THREADS: i32 = 2;

/// A loaded sherpa-onnx VITS/Piper voice.
pub struct TtsEngine {
    tts: OfflineTts,
    gen_cfg: GenerationConfig,
    pub sample_rate: u32,
    pub num_speakers: i32,
}

/// One synthesized sentence.
pub struct Synth {
    pub samples: Vec<f32>,
    pub synth_ms: f64,
    pub audio_ms: f64,
    pub rtf: f64,
}

impl TtsEngine {
    /// Loads `<model_dir>/*.onnx` + `tokens.txt` + `espeak-ng-data`; logs `tts.load`.
    pub fn load(tel: &Telemetry, model_dir: &Path) -> anyhow::Result<TtsEngine> {
        let start = Instant::now();
        let rss_before = current_rss_mb();
        let input = json!({"model_dir": model_dir, "num_threads": ONNX_THREADS});
        let res = (|| {
            let model = std::fs::read_dir(model_dir)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|e| e == "onnx"))
                .ok_or_else(|| anyhow::anyhow!("no .onnx model in {} (run scripts/fetch-models.sh)", model_dir.display()))?;
            let path = |p: &Path| Some(p.to_string_lossy().into_owned());
            let config = OfflineTtsConfig {
                model: OfflineTtsModelConfig {
                    vits: OfflineTtsVitsModelConfig {
                        model: path(&model),
                        tokens: path(&model_dir.join("tokens.txt")),
                        data_dir: path(&model_dir.join("espeak-ng-data")),
                        ..Default::default()
                    },
                    num_threads: ONNX_THREADS,
                    provider: Some("cpu".into()),
                    ..Default::default()
                },
                max_num_sentences: 1,
                ..Default::default()
            };
            let tts = OfflineTts::create(&config)
                .ok_or_else(|| anyhow::anyhow!("sherpa-onnx rejected model {}", model.display()))?;
            anyhow::Ok((tts, model))
        })();
        let load_ms = ms(start.elapsed());
        match res {
            Ok((tts, model)) => {
                let sample_rate = u32::try_from(tts.sample_rate()).unwrap_or(22_050);
                let num_speakers = tts.num_speakers();
                let rss_delta = round2(current_rss_mb() - rss_before);
                tel.event(
                    "tts.load",
                    Some(load_ms),
                    true,
                    input,
                    json!({"model": model, "sample_rate": sample_rate, "num_speakers": num_speakers,
                           "load_ms": load_ms, "rss_delta_mb": rss_delta}),
                    None,
                );
                Ok(TtsEngine { tts, gen_cfg: GenerationConfig::default(), sample_rate, num_speakers })
            }
            Err(e) => {
                tel.event("tts.load", Some(load_ms), false, input, Value::Null, Some(&format!("{e:#}")));
                Err(e)
            }
        }
    }

    /// Synthesizes one sentence; logs `tts.synth` (with `wav` = the utterance file, if any).
    pub fn synth(&self, tel: &Telemetry, text: &str, wav: Option<&Path>) -> anyhow::Result<Synth> {
        let start = Instant::now();
        let chars = text.chars().count();
        let audio = self.tts.generate_with_config(text, &self.gen_cfg, None::<fn(&[f32], f32) -> bool>);
        let synth_ms = ms(start.elapsed());
        let input = json!({"text": text, "chars": chars});
        let Some(audio) = audio else {
            tel.event("tts.synth", Some(synth_ms), false, input, Value::Null, Some("sherpa-onnx generate failed"));
            anyhow::bail!("sherpa-onnx generate failed");
        };
        let samples = audio.samples().to_vec();
        let audio_ms = round2(samples.len() as f64 * 1000.0 / f64::from(self.sample_rate));
        let rtf = if audio_ms > 0.0 { (synth_ms / audio_ms * 10_000.0).round() / 10_000.0 } else { 0.0 };
        tel.event(
            "tts.synth",
            Some(synth_ms),
            true,
            input,
            json!({"samples": samples.len(), "audio_ms": audio_ms, "rtf": rtf, "wav": wav}),
            None,
        );
        tel.update_stats(|s| {
            s.last_tts_ms = Some(synth_ms);
            s.last_rtf = Some(rtf);
        });
        Ok(Synth { samples, synth_ms, audio_ms, rtf })
    }
}

/// Opens the default output device (kept by the caller; dropping it closes the stream).
///
/// rodio's default asks for a fixed ~50 ms buffer (2048 frames at 48 kHz), which some CoreAudio
/// devices reject (supported range 14..=1024); let the OS choose first, then fall back to
/// rodio's device/config probing.
pub fn open_output() -> anyhow::Result<rodio::MixerDeviceSink> {
    let mut sink = rodio::DeviceSinkBuilder::from_default_device()
        .and_then(|b| b.with_buffer_size(rodio::cpal::BufferSize::Default).open_stream())
        .or_else(|_| rodio::DeviceSinkBuilder::open_default_sink())?;
    sink.log_on_drop(false);
    Ok(sink)
}

/// Mono f32 samples → playable rodio source.
pub fn samples_source(samples: Vec<f32>, sample_rate: u32) -> rodio::buffer::SamplesBuffer {
    rodio::buffer::SamplesBuffer::new(
        std::num::NonZero::new(1).expect("1 != 0"),
        std::num::NonZero::new(sample_rate.max(1)).expect("max(1)"),
        samples,
    )
}

/// 16-bit PCM mono WAV.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> std::io::Result<()> {
    use std::io::Write;
    let data_len = (samples.len() * 2) as u32;
    let mut buf = Vec::with_capacity(44 + samples.len() * 2);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    std::fs::File::create(path)?.write_all(&buf)
}

struct Request {
    seq: u64,
    thread: String,
    click: Instant,
    generation: u64,
}

struct Utterance {
    seq: u64,
    thread: String,
    sentences: Vec<String>,
    click: Instant,
    enqueued: Instant,
    generation: u64,
}

/// Speech queue handle owned by `Core`.
pub(crate) struct Speech {
    tel: Arc<Telemetry>,
    req_tx: Sender<Request>,
    req_rx: Receiver<Request>,
    /// Asks the TTS worker to open the audio device while `t3 thread show` runs.
    warm_tx: Sender<()>,
    utt_rx: Receiver<Utterance>,
    /// Bumped by `stop`; work tagged with an older generation is discarded.
    generation: Arc<AtomicU64>,
}

impl Speech {
    pub(crate) fn start(tel: Arc<Telemetry>, cfg: &Config, events: Sender<CoreEvent>) -> std::io::Result<Speech> {
        let (req_tx, req_rx) = crossbeam_channel::unbounded::<Request>();
        let (utt_tx, utt_rx) = crossbeam_channel::unbounded::<Utterance>();
        let (warm_tx, warm_rx) = crossbeam_channel::unbounded::<()>();
        let generation = Arc::new(AtomicU64::new(0));
        {
            let (tel, rx, gen_, t3_bin, ev) =
                (tel.clone(), req_rx.clone(), generation.clone(), cfg.t3_bin.clone(), events.clone());
            let utt_rx = utt_rx.clone();
            std::thread::Builder::new().name("t3p-speech-prep".into()).spawn(move || {
                for req in rx.iter() {
                    prepare(&tel, &t3_bin, &gen_, &utt_tx, &ev, req);
                    tel.set_queue_len(rx.len() + utt_rx.len());
                }
            })?;
        }
        {
            let (tel, rx, gen_) = (tel.clone(), utt_rx.clone(), generation.clone());
            let model_dir = cfg.model_dir.clone();
            let save_audio = cfg.save_audio.then(|| tel.run_dir().join("audio"));
            std::thread::Builder::new()
                .name("t3p-tts".into())
                .spawn(move || tts_worker(&tel, &model_dir, save_audio, &rx, &warm_rx, &gen_, &events))?;
        }
        Ok(Speech { tel, req_tx, req_rx, warm_tx, utt_rx, generation })
    }

    pub(crate) fn speak(&self, thread: &str) {
        let click = Instant::now();
        let seq = self.tel.event("speak.request", None, true, json!({"thread": thread}), Value::Null, None);
        let req = Request { seq, thread: thread.to_owned(), click, generation: self.generation.load(Ordering::SeqCst) };
        let _ = self.req_tx.send(req);
        let _ = self.warm_tx.send(());
        self.tel.set_queue_len(self.queue_len());
    }

    pub(crate) fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let dropped = self.req_rx.try_iter().count() + self.utt_rx.try_iter().count();
        self.tel.set_queue_len(0);
        self.tel.event("speak.stop", None, true, Value::Null, json!({"dropped": dropped}), None);
    }

    fn queue_len(&self) -> usize {
        self.req_rx.len() + self.utt_rx.len()
    }
}

fn prepare(
    tel: &Telemetry,
    t3_bin: &Path,
    generation: &AtomicU64,
    utt_tx: &Sender<Utterance>,
    events: &Sender<CoreEvent>,
    req: Request,
) {
    if req.generation != generation.load(Ordering::SeqCst) {
        return;
    }
    let show = match t3::show_thread(tel, t3_bin, &req.thread, SHOW_MESSAGES) {
        Ok(s) => s,
        Err(e) => {
            let _ = events.send(CoreEvent::Error(format!("t3 thread show {}: {e:#}", req.thread)));
            return;
        }
    };
    let start = Instant::now();
    let source_chars: usize = show.messages.iter().map(|m| m.text.chars().count()).sum();
    let ex = extract(&show.thread.status, &show.thread.title, &show.messages);
    let sentences = split_sentences(&ex.text);
    tel.event(
        "speak.extract",
        Some(ms(start.elapsed())),
        true,
        json!({"thread": req.thread, "request_seq": req.seq, "status": show.thread.status,
               "title": show.thread.title, "messages": show.messages.len(), "source_chars": source_chars}),
        json!({"text": ex.text, "rule": ex.rule, "chars": ex.text.chars().count(), "sentences": sentences.len()}),
        None,
    );
    if req.generation != generation.load(Ordering::SeqCst) {
        return;
    }
    let _ = utt_tx.send(Utterance {
        seq: req.seq,
        thread: req.thread,
        sentences,
        click: req.click,
        enqueued: Instant::now(),
        generation: req.generation,
    });
}

fn tts_worker(
    tel: &Telemetry,
    model_dir: &Path,
    audio_dir: Option<PathBuf>,
    rx: &Receiver<Utterance>,
    warm_rx: &Receiver<()>,
    generation: &AtomicU64,
    events: &Sender<CoreEvent>,
) {
    let engine = TtsEngine::load(tel, model_dir)
        .inspect_err(|e| {
            let _ = events.send(CoreEvent::Error(format!("TTS model load failed: {e:#}")));
        })
        .ok();
    let mut device: Option<rodio::MixerDeviceSink> = None;
    loop {
        let utt = crossbeam_channel::select! {
            recv(rx) -> u => match u {
                Ok(u) => u,
                Err(_) => return,
            },
            recv(warm_rx) -> _ => {
                if device.is_none() {
                    let start = Instant::now();
                    let res = open_output();
                    let (ok, err) = match res {
                        Ok(d) => {
                            device = Some(d);
                            (true, None)
                        }
                        Err(e) => (false, Some(format!("{e:#}"))),
                    };
                    tel.event("audio.open", Some(ms(start.elapsed())), ok, json!({"reason": "warm"}), Value::Null, err.as_deref());
                }
                continue;
            },
            default(DEVICE_IDLE) => {
                if device.take().is_some() {
                    tel.event("audio.close", None, true, json!({"idle_s": DEVICE_IDLE.as_secs()}), Value::Null, None);
                }
                continue;
            },
        };
        tel.set_queue_len(rx.len());
        if utt.generation != generation.load(Ordering::SeqCst) {
            continue;
        }
        let Some(engine) = &engine else {
            tel.event("audio.play", None, false, json!({"thread": utt.thread, "request_seq": utt.seq}),
                Value::Null, Some("TTS model not loaded"));
            continue;
        };
        tel.update_stats(|s| s.speaking = Some(utt.thread.clone()));
        let _ = events.send(CoreEvent::SpeakingStarted { thread_id: utt.thread.clone() });
        let (ok, err, out) = speak_utterance(tel, engine, &mut device, audio_dir.as_deref(), &utt, generation);
        tel.event("audio.play", out["play_ms"].as_f64(), ok,
            json!({"thread": utt.thread, "request_seq": utt.seq}), out, err.as_deref());
        tel.update_stats(|s| s.speaking = None);
        let _ = events.send(CoreEvent::SpeakingFinished { thread_id: utt.thread });
    }
}

/// Synthesizes sentence by sentence into one `Player` so playback of sentence N overlaps
/// synthesis of N+1. Returns `(ok, err, audio.play out)`.
fn speak_utterance(
    tel: &Telemetry,
    engine: &TtsEngine,
    device: &mut Option<rodio::MixerDeviceSink>,
    audio_dir: Option<&Path>,
    utt: &Utterance,
    generation: &AtomicU64,
) -> (bool, Option<String>, Value) {
    let queue_wait_ms = ms(utt.enqueued.elapsed());
    let open_start = Instant::now();
    let device_open_ms = if device.is_none() {
        match open_output() {
            Ok(d) => *device = Some(d),
            Err(e) => return (false, Some(format!("audio device: {e:#}")), json!({"queue_wait_ms": queue_wait_ms})),
        }
        Some(ms(open_start.elapsed()))
    } else {
        None
    };
    let dev = device.as_ref().expect("opened above");
    let player = rodio::Player::connect_new(dev.mixer());
    let wav = audio_dir.map(|d| d.join(format!("{}.wav", utt.seq)));
    let cancelled = || utt.generation != generation.load(Ordering::SeqCst);
    let mut all = Vec::new();
    let mut first_audio: Option<Instant> = None;
    let mut click_to_audio_ms = None;
    let mut synth_total = 0.0;
    let mut audio_total = 0.0;
    let mut err = None;
    for sentence in &utt.sentences {
        if cancelled() {
            break;
        }
        match engine.synth(tel, sentence, wav.as_deref()) {
            Ok(s) => {
                synth_total += s.synth_ms;
                audio_total += s.audio_ms;
                if wav.is_some() {
                    all.extend_from_slice(&s.samples);
                }
                player.append(samples_source(s.samples, engine.sample_rate));
                if first_audio.is_none() {
                    first_audio = Some(Instant::now());
                    let c2a = ms(utt.click.elapsed());
                    click_to_audio_ms = Some(c2a);
                    tel.update_stats(|st| st.last_click_to_audio_ms = Some(c2a));
                }
            }
            Err(e) => {
                err = Some(format!("{e:#}"));
                break;
            }
        }
    }
    while !player.empty() {
        if cancelled() {
            player.stop();
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let stopped = cancelled();
    let play_ms = first_audio.map(|t| ms(t.elapsed()));
    let mut wav_out = None;
    if let Some(path) = &wav
        && !all.is_empty()
    {
        let saved = std::fs::create_dir_all(path.parent().expect("audio dir")).and_then(|()| write_wav(path, &all, engine.sample_rate));
        match saved {
            Ok(()) => wav_out = Some(path.clone()),
            Err(e) => err = err.or(Some(format!("wav write: {e}"))),
        }
    }
    if stopped {
        err = err.or(Some("stopped".to_owned()));
    }
    let out = json!({
        "queue_wait_ms": queue_wait_ms, "play_ms": play_ms, "click_to_audio_ms": click_to_audio_ms,
        "device_open_ms": device_open_ms, "sentences": utt.sentences.len(), "synth_ms_total": round2(synth_total),
        "audio_ms_total": round2(audio_total), "stopped": stopped, "wav": wav_out,
    });
    (err.is_none(), err, out)
}
