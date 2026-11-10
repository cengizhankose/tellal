//! Local TTS (EMA Lightning on ONNX Runtime, see `ema.rs`) and the channel scheduler.
//!
//! Every thread is a channel ([`ChannelView`]). Utterances come from the catch-up worker
//! (unmute), the live tail (new assistant messages, waiting requests) and the poller (stopped
//! runs). The `t3p-tts` thread keeps at most one utterance audible: a resumed paused utterance
//! first, else the earliest arrival across listening, unpaused channels. It synthesizes just in
//! time, sentence by sentence into one rodio `Player` (playback of sentence N overlaps synthesis
//! of N+1), and prefetches the next utterance while the current one plays. Pausing a channel
//! pauses its `Player` and parks it; resuming continues mid-sentence.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant};

use crossbeam_channel::Sender;
use serde_json::{Value, json};

use crate::ema::{self, Ema};
use crate::extract::{extract, normalize, split_sentences};
use crate::lexicon::Lexicon;
use crate::telemetry::{self, Telemetry, current_rss_mb, ms, now_unix_ms, rfc3339_from_ms, round2};
use crate::{ChannelView, Config, CoreEvent, ThreadView, t3};

/// Messages read for an unmute catch-up line.
const CATCHUP_MESSAGES: u32 = 12;
/// The audio device is released after this much idle time (an open CoreAudio stream costs CPU).
const DEVICE_IDLE: Duration = Duration::from_secs(30);
/// Finish detection tick while an utterance is audible.
const PLAY_TICK: Duration = Duration::from_millis(15);
/// A speaker label is spoken when the channel changes or after this much silence.
const LABEL_AFTER_SILENCE: Duration = Duration::from_secs(20);
/// Titles up to this many chars are the label; longer ones are cut to [`LABEL_WORDS`] words.
const LABEL_MAX_CHARS: usize = 30;
const LABEL_WORDS: usize = 4;

/// The loaded voice: EMA Lightning plus the pronunciation lexicon.
pub struct TtsEngine {
    ema: Mutex<Ema>,
    lexicon: Lexicon,
    pub sample_rate: u32,
}

/// One synthesized sentence.
pub struct Synth {
    pub samples: Vec<f32>,
    pub synth_ms: f64,
    pub audio_ms: f64,
    pub rtf: f64,
}

impl TtsEngine {
    /// Loads the EMA ONNX graphs from `cfg.model_dir` and the lexicon; logs `tts.load`.
    pub fn load(tel: &Telemetry, cfg: &Config) -> anyhow::Result<TtsEngine> {
        let start = Instant::now();
        let rss_before = current_rss_mb();
        let input = json!({"engine": "ema-lightning-onnx", "model_dir": cfg.model_dir, "threads": cfg.tts_threads,
                           "lexicon": cfg.lexicon_path});
        let res = Ema::load(&cfg.model_dir, cfg.tts_threads)
            .and_then(|ema| Lexicon::load(&cfg.lexicon_path).map(|lexicon| (ema, lexicon)));
        let load_ms = ms(start.elapsed());
        match res {
            Ok((ema, lexicon)) => {
                let sample_rate = ema.sample_rate;
                let model_bytes: u64 =
                    ema::FILES.iter().filter_map(|f| std::fs::metadata(cfg.model_dir.join(f)).ok()).map(|m| m.len()).sum();
                tel.event(
                    "tts.load",
                    Some(load_ms),
                    true,
                    input,
                    json!({"sample_rate": sample_rate, "model_mb": round2(model_bytes as f64 / 1_048_576.0),
                           "steps_ms": ema.load_ms.iter().map(|(k, v)| (*k, round2(*v))).collect::<HashMap<_, _>>(),
                           "lexicon_entries": lexicon.entries, "lexicon_user_entries": lexicon.user_entries,
                           "load_ms": load_ms, "rss_delta_mb": round2(current_rss_mb() - rss_before)}),
                    None,
                );
                Ok(TtsEngine { ema: Mutex::new(ema), lexicon, sample_rate })
            }
            Err(e) => {
                tel.event("tts.load", Some(load_ms), false, input, Value::Null, Some(&format!("{e:#}")));
                Err(e)
            }
        }
    }

    /// Synthesizes one sentence; logs `tts.synth` with the text as written, as respelled by the
    /// lexicon, and as the model read it (after normalizer-tr).
    pub fn synth(&self, tel: &Telemetry, text: &str, wav: Option<&Path>) -> anyhow::Result<Synth> {
        let start = Instant::now();
        let respelled = self.lexicon.apply(text);
        let res = self.ema.lock().unwrap_or_else(|p| p.into_inner()).speak(&respelled);
        let synth_ms = ms(start.elapsed());
        let input = json!({"text": text, "chars": text.chars().count(),
                           "respelled": (respelled != text).then_some(&respelled)});
        let spoken = match res {
            Ok(s) => s,
            Err(e) => {
                tel.event("tts.synth", Some(synth_ms), false, input, Value::Null, Some(&format!("{e:#}")));
                return Err(e);
            }
        };
        let samples = spoken.samples;
        let audio_ms = round2(samples.len() as f64 * 1000.0 / f64::from(self.sample_rate));
        let rtf = if audio_ms > 0.0 { (synth_ms / audio_ms * 10_000.0).round() / 10_000.0 } else { 0.0 };
        tel.event(
            "tts.synth",
            Some(synth_ms),
            true,
            input,
            json!({"spoken": spoken.text, "pieces": spoken.pieces, "samples": samples.len(), "audio_ms": audio_ms,
                   "rtf": rtf, "wav": wav}),
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

// ---------- Channels & scheduler ----------

/// One queued utterance.
struct Item {
    /// `speak.enqueue` seq; also names the WAV.
    id: u64,
    thread: String,
    /// `catchup` | `live` | `wait` | `stopped`.
    source: &'static str,
    text: String,
    arrived: Instant,
    /// T3 `created_at` (Unix ms) of the spoken assistant message → `msg_to_audio_ms`.
    created_ms: Option<i64>,
    /// Unmute click → `click_to_audio_ms`.
    click: Option<Instant>,
    prefetch: Prefetch,
}

/// Sentence audio synthesized while the previous utterance plays.
#[derive(Default)]
enum Prefetch {
    #[default]
    None,
    Failed,
    Ready(Vec<Synth>),
}

/// The audible utterance, or a paused one held by its channel.
struct Playing {
    item: Item,
    player: rodio::Player,
    sample_rate: u32,
    /// Speaker label spoken before the text.
    label: Option<String>,
    queue_wait_ms: f64,
    /// Every sentence was synthesized and appended (or synthesis failed).
    synth_done: bool,
    sentences: usize,
    synth_ms: f64,
    audio_ms: f64,
    prefetched: bool,
    device_open_ms: Option<f64>,
    first_audio: Option<Instant>,
    msg_to_audio_ms: Option<f64>,
    click_to_audio_ms: Option<f64>,
    paused_since: Option<Instant>,
    paused_ms: f64,
    /// Everything appended, kept only when audio is saved.
    samples: Vec<f32>,
    err: Option<String>,
}

impl Playing {
    fn new(item: Item, player: rodio::Player, sample_rate: u32, label: Option<String>, prefetched: bool) -> Playing {
        Playing {
            queue_wait_ms: ms(item.arrived.elapsed()),
            prefetched,
            item,
            player,
            sample_rate,
            label,
            synth_done: false,
            sentences: 0,
            synth_ms: 0.0,
            audio_ms: 0.0,
            device_open_ms: None,
            first_audio: None,
            msg_to_audio_ms: None,
            click_to_audio_ms: None,
            paused_since: None,
            paused_ms: 0.0,
            samples: Vec::new(),
            err: None,
        }
    }

    fn mark_first_audio(&mut self, tel: &Telemetry) {
        self.first_audio = Some(Instant::now());
        self.msg_to_audio_ms = self.item.created_ms.map(|c| (now_unix_ms() - c) as f64);
        self.click_to_audio_ms = self.item.click.map(|c| ms(c.elapsed()));
        let (m2a, c2a) = (self.msg_to_audio_ms, self.click_to_audio_ms);
        tel.update_stats(|s| {
            if m2a.is_some() {
                s.last_msg_to_audio_ms = m2a;
            }
            if c2a.is_some() {
                s.last_click_to_audio_ms = c2a;
            }
        });
    }
}

#[derive(Default)]
struct Chan {
    listening: bool,
    paused: bool,
    unheard: usize,
    waiting_for_user: bool,
    /// Live messages created before this (Unix ms) are covered by the catch-up line or predate
    /// the reply, so they are not spoken.
    listen_since_ms: i64,
    /// Bumped on every listen/mute so a late catch-up line is discarded.
    epoch: u64,
    queue: VecDeque<Item>,
}

#[derive(Default)]
struct State {
    chans: HashMap<String, Chan>,
    current: Option<Playing>,
    /// Paused channels' utterances (rodio-paused players).
    held: HashMap<String, Playing>,
    /// Channels whose held utterance was resumed; they get the audible slot first.
    resumed: VecDeque<String>,
    /// Channel of the last audible utterance and when audio last stopped (speaker labels).
    last_thread: Option<String>,
    last_audible_end: Option<Instant>,
    /// Last published views (non-default channels only).
    views: HashMap<String, ChannelView>,
    /// Open the audio device ahead of an expected utterance.
    warm: bool,
}

impl State {
    fn chan(&mut self, thread: &str) -> &mut Chan {
        if !self.chans.contains_key(thread) {
            self.chans.insert(thread.to_owned(), Chan::default());
        }
        self.chans.get_mut(thread).expect("inserted above")
    }

    fn is_current(&self, thread: &str) -> bool {
        self.current.as_ref().is_some_and(|p| p.item.thread == thread)
    }

    /// Stops and takes the channel's current or held utterance. `bool` = it was audible.
    fn take_playing(&mut self, thread: &str) -> Option<(Playing, bool)> {
        self.resumed.retain(|t| t != thread);
        let (p, audible) = if self.is_current(thread) {
            self.last_audible_end = Some(Instant::now());
            (self.current.take()?, true)
        } else {
            (self.held.remove(thread)?, false)
        };
        p.player.stop();
        Some((p, audible))
    }

    /// Pauses the channel; its audible utterance is held mid-sentence. Returns whether one was audible.
    fn pause(&mut self, thread: &str) -> bool {
        self.chan(thread).paused = true;
        self.resumed.retain(|t| t != thread);
        if !self.is_current(thread) {
            return false;
        }
        let mut p = self.current.take().expect("is_current");
        p.player.pause();
        p.paused_since = Some(Instant::now());
        self.last_audible_end = Some(Instant::now());
        self.held.insert(thread.to_owned(), p);
        true
    }

    fn unpause(&mut self, thread: &str) {
        self.chan(thread).paused = false;
        if self.held.contains_key(thread) && !self.resumed.iter().any(|t| t == thread) {
            self.resumed.push_back(thread.to_owned());
        }
    }

    fn audible(&self, thread: &str) -> bool {
        self.chans.get(thread).is_some_and(|c| c.listening && !c.paused)
    }

    /// The first resumed utterance whose channel can be heard (its resume time is accounted).
    fn pop_resumed(&mut self) -> Option<Playing> {
        while let Some(thread) = self.resumed.pop_front() {
            if self.audible(&thread)
                && let Some(mut p) = self.held.remove(&thread)
            {
                if let Some(t) = p.paused_since.take() {
                    p.paused_ms += ms(t.elapsed());
                }
                return Some(p);
            }
        }
        None
    }

    /// Channel with the earliest-arrived queued utterance among listening, unpaused channels
    /// that hold nothing.
    fn next_thread(&self) -> Option<&str> {
        self.chans
            .iter()
            .filter(|(id, c)| c.listening && !c.paused && !self.held.contains_key(*id))
            .filter_map(|(id, c)| Some((c.queue.front()?.arrived, id)))
            .min()
            .map(|(_, id)| id.as_str())
    }

    fn has_next(&self) -> bool {
        self.next_thread().is_some() || self.resumed.iter().any(|t| self.held.contains_key(t) && self.audible(t))
    }

    fn needs_label(&self, thread: &str) -> bool {
        self.last_thread.as_deref() != Some(thread)
            || self.last_audible_end.is_none_or(|t| t.elapsed() > LABEL_AFTER_SILENCE)
    }

    fn playing_mut(&mut self, id: u64) -> Option<&mut Playing> {
        match &mut self.current {
            Some(p) if p.item.id == id => Some(p),
            _ => self.held.values_mut().find(|p| p.item.id == id),
        }
    }

    fn views(&self) -> HashMap<String, ChannelView> {
        self.chans
            .iter()
            .filter_map(|(id, c)| {
                let speaking = self.is_current(id);
                let playing = if speaking { self.current.as_ref() } else { self.held.get(id) };
                let v = ChannelView {
                    listening: c.listening,
                    paused: c.paused,
                    speaking,
                    queued: c.queue.len(),
                    unheard: c.unheard,
                    now_text: playing.map(|p| p.item.text.clone()),
                    waiting_for_user: c.waiting_for_user,
                };
                (v != ChannelView::default()).then(|| (id.clone(), v))
            })
            .collect()
    }
}

struct Shared {
    tel: Arc<Telemetry>,
    events: Sender<CoreEvent>,
    threads: Arc<RwLock<Vec<ThreadView>>>,
    audio_dir: Option<PathBuf>,
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        telemetry::lock(&self.state)
    }

    /// Publishes view changes (→ `ChannelsUpdated`) and HUD numbers, and wakes the player.
    fn commit(&self, st: &mut State) {
        let views = st.views();
        if views != st.views {
            st.views = views;
            let _ = self.events.send(CoreEvent::ChannelsUpdated);
        }
        let speaking = st.current.as_ref().map(|p| p.item.thread.clone());
        let listening = st.chans.values().filter(|c| c.listening).count();
        self.tel.update_stats(|s| {
            s.speaking = speaking;
            s.listening = listening;
        });
        self.tel.set_queue_len(st.chans.values().map(|c| c.queue.len()).sum());
        self.wake.notify_all();
    }

    /// Appends an utterance to the channel queue; logs `speak.enqueue`.
    #[allow(clippy::too_many_arguments)]
    fn enqueue(
        &self,
        st: &mut State,
        thread: &str,
        source: &'static str,
        text: String,
        created_ms: Option<i64>,
        click: Option<Instant>,
        extra: Value,
    ) {
        let chan = st.chan(thread);
        let mut input = json!({"thread": thread, "source": source, "text": text, "chars": text.chars().count()});
        if let (Value::Object(dst), Value::Object(src)) = (&mut input, extra) {
            dst.extend(src);
        }
        let id = self.tel.event("speak.enqueue", None, true, input, json!({"queue_len": chan.queue.len() + 1}), None);
        let item = Item {
            id,
            thread: thread.to_owned(),
            source,
            text,
            arrived: Instant::now(),
            created_ms,
            click,
            prefetch: Prefetch::None,
        };
        chan.queue.push_back(item);
    }

    /// Stops the channel's current/held utterance; `SpeakingFinished` if it was audible.
    fn drop_now(&self, st: &mut State, thread: &str) -> Option<Playing> {
        let (p, audible) = st.take_playing(thread)?;
        if audible {
            let _ = self.events.send(CoreEvent::SpeakingFinished { thread_id: thread.to_owned() });
        }
        Some(p)
    }

    /// `(status, title)` from the latest thread snapshot.
    fn thread_info(&self, thread: &str) -> (String, String) {
        let threads = self.threads.read().unwrap_or_else(|p| p.into_inner());
        threads.iter().find(|t| t.id == thread).map_or_else(Default::default, |t| (t.status.clone(), t.title.clone()))
    }

    /// Appends synthesized audio to utterance `id` wherever it is (audible or held). `false` =
    /// it was dropped meanwhile.
    fn append(&self, id: u64, samples: Vec<f32>, sample_rate: u32, synth_ms: Option<f64>) -> bool {
        let save = self.audio_dir.is_some();
        let mut st = self.lock();
        let audible = st.current.as_ref().is_some_and(|p| p.item.id == id);
        let Some(p) = st.playing_mut(id) else { return false };
        p.audio_ms += samples.len() as f64 * 1000.0 / f64::from(sample_rate);
        if let Some(s) = synth_ms {
            p.synth_ms += s;
            p.sentences += 1;
        }
        if save {
            p.samples.extend_from_slice(&samples);
        }
        p.player.append(samples_source(samples, sample_rate));
        if audible && p.first_audio.is_none() {
            p.mark_first_audio(&self.tel);
        }
        true
    }

    /// Logs `audio.play` for an utterance that ended or was dropped (`dropped` = reason) and
    /// saves its WAV.
    fn finish(&self, p: Playing, dropped: Option<&str>) {
        let paused_ms = p.paused_ms + p.paused_since.map_or(0.0, |t| ms(t.elapsed()));
        let play_ms = p.first_audio.map(|t| ms(t.elapsed()));
        let mut err = p.err;
        let mut wav = None;
        if let Some(dir) = &self.audio_dir
            && !p.samples.is_empty()
        {
            let path = dir.join(format!("{}.wav", p.item.id));
            match std::fs::create_dir_all(dir).and_then(|()| write_wav(&path, &p.samples, p.sample_rate)) {
                Ok(()) => wav = Some(path),
                Err(e) => err = err.or(Some(format!("wav write: {e}"))),
            }
        }
        self.tel.event(
            "audio.play",
            play_ms,
            err.is_none(),
            json!({"thread": p.item.thread, "item": p.item.id, "source": p.item.source, "text": p.item.text}),
            json!({
                "msg_to_audio_ms": p.msg_to_audio_ms, "click_to_audio_ms": p.click_to_audio_ms,
                "paused_ms": round2(paused_ms), "skipped": dropped.is_some(), "dropped": dropped,
                "label_spoken": p.label.is_some() && p.first_audio.is_some(), "label": p.label,
                "queue_wait_ms": p.queue_wait_ms, "play_ms": play_ms, "sentences": p.sentences,
                "synth_ms_total": round2(p.synth_ms), "audio_ms_total": round2(p.audio_ms),
                "prefetched": p.prefetched, "device_open_ms": p.device_open_ms, "wav": wav,
            }),
            err.as_deref(),
        );
    }

    /// Logs a failed `audio.play` for an utterance that could not be played at all.
    fn fail(&self, item: &Item, err: &str) {
        self.tel.event(
            "audio.play",
            None,
            false,
            json!({"thread": item.thread, "item": item.id, "source": item.source, "text": item.text}),
            Value::Null,
            Some(err),
        );
    }
}

struct CatchUp {
    thread: String,
    click: Instant,
    epoch: u64,
}

/// Channel/speech handle owned by `Core` (cheap to clone).
#[derive(Clone)]
pub(crate) struct Speech {
    shared: Arc<Shared>,
    catchup_tx: Sender<CatchUp>,
}

impl Speech {
    pub(crate) fn start(
        tel: Arc<Telemetry>,
        cfg: &Config,
        events: Sender<CoreEvent>,
        threads: Arc<RwLock<Vec<ThreadView>>>,
    ) -> std::io::Result<Speech> {
        let shared = Arc::new(Shared {
            audio_dir: cfg.save_audio.then(|| tel.run_dir().join("audio")),
            tel,
            events,
            threads,
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
        });
        let (catchup_tx, catchup_rx) = crossbeam_channel::unbounded::<CatchUp>();
        {
            let shared = shared.clone();
            std::thread::Builder::new().name("t3p-catchup".into()).spawn(move || {
                for req in catchup_rx.iter() {
                    catch_up(&shared, req);
                }
            })?;
        }
        {
            let shared = shared.clone();
            let cfg = cfg.clone();
            std::thread::Builder::new().name("t3p-tts".into()).spawn(move || player_loop(&shared, &cfg))?;
        }
        Ok(Speech { shared, catchup_tx })
    }

    /// Radio switch (see `Core::set_listening`).
    pub(crate) fn set_listening(&self, thread: &str, on: bool) {
        let click = Instant::now();
        let sh = &*self.shared;
        let mut st = sh.lock();
        let chan = st.chan(thread);
        if on {
            if chan.listening {
                sh.tel.event("channel.listen", None, true, json!({"thread": thread, "on": true}), json!({"already": true}), None);
                return;
            }
            let unheard = std::mem::take(&mut chan.unheard);
            chan.listening = true;
            chan.paused = false;
            chan.listen_since_ms = now_unix_ms();
            chan.epoch += 1;
            let epoch = chan.epoch;
            st.warm = true;
            sh.tel.event(
                "channel.listen",
                None,
                true,
                json!({"thread": thread, "on": true, "catch_up": true}),
                json!({"unheard": unheard}),
                None,
            );
            sh.commit(&mut st);
            drop(st);
            let _ = self.catchup_tx.send(CatchUp { thread: thread.to_owned(), click, epoch });
        } else {
            chan.listening = false;
            chan.paused = false;
            chan.epoch += 1;
            let dropped_queue = std::mem::take(&mut chan.queue).len();
            let dropped = sh.drop_now(&mut st, thread);
            sh.tel.event(
                "channel.listen",
                None,
                true,
                json!({"thread": thread, "on": false}),
                json!({"dropped_queue": dropped_queue, "dropped_now": dropped.as_ref().map(|p| &p.item.text)}),
                None,
            );
            sh.commit(&mut st);
            drop(st);
            if let Some(p) = dropped {
                sh.finish(p, Some("mute"));
            }
        }
    }

    /// Listening without a catch-up line; only messages created at/after `since_ms` are spoken
    /// (a reply's acknowledgement). Leaves an already-listening channel unchanged.
    pub(crate) fn listen_from(&self, thread: &str, since_ms: i64) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let chan = st.chan(thread);
        let already = chan.listening;
        if !already {
            chan.listening = true;
            chan.paused = false;
            chan.unheard = 0;
            chan.listen_since_ms = since_ms;
            chan.epoch += 1;
        }
        st.warm = true;
        sh.tel.event(
            "channel.listen",
            None,
            true,
            json!({"thread": thread, "on": true, "catch_up": false, "source": "reply", "since": rfc3339_from_ms(since_ms)}),
            json!({"already": already}),
            None,
        );
        sh.commit(&mut st);
    }

    /// Video pause/resume (see `Core::toggle_pause`).
    pub(crate) fn toggle_pause(&self, thread: &str) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let chan = st.chan(thread);
        if !chan.listening {
            sh.tel.event("channel.pause", None, false, json!({"thread": thread}), Value::Null, Some("channel is muted"));
            return;
        }
        let paused = !chan.paused;
        let held_audible = if paused {
            let audible = st.pause(thread);
            if audible {
                let _ = sh.events.send(CoreEvent::SpeakingFinished { thread_id: thread.to_owned() });
            }
            audible
        } else {
            st.unpause(thread);
            false
        };
        let holding = st.held.contains_key(thread);
        let queued = st.chan(thread).queue.len();
        sh.tel.event(
            "channel.pause",
            None,
            true,
            json!({"thread": thread, "paused": paused}),
            json!({"held_audible": held_audible, "holding": holding, "queued": queued}),
            None,
        );
        sh.commit(&mut st);
    }

    /// "Okey, geç": drop the channel's current utterance and unpause it.
    pub(crate) fn skip(&self, thread: &str) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let dropped = sh.drop_now(&mut st, thread);
        let chan = st.chan(thread);
        chan.paused = false;
        let queued = chan.queue.len();
        sh.tel.event(
            "channel.skip",
            None,
            true,
            json!({"thread": thread}),
            json!({"dropped_now": dropped.as_ref().map(|p| &p.item.text), "queued": queued}),
            None,
        );
        sh.commit(&mut st);
        drop(st);
        if let Some(p) = dropped {
            sh.finish(p, Some("skip"));
        }
    }

    /// Drop the channel's current utterance and backlog, unpause, keep listening.
    pub(crate) fn jump_to_live(&self, thread: &str) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let dropped = sh.drop_now(&mut st, thread);
        let chan = st.chan(thread);
        chan.paused = false;
        let dropped_queue = std::mem::take(&mut chan.queue).len();
        sh.tel.event(
            "channel.live",
            None,
            true,
            json!({"thread": thread}),
            json!({"dropped_now": dropped.as_ref().map(|p| &p.item.text), "dropped_queue": dropped_queue}),
            None,
        );
        sh.commit(&mut st);
        drop(st);
        if let Some(p) = dropped {
            sh.finish(p, Some("live"));
        }
    }

    /// Mute every channel and silence playback.
    pub(crate) fn mute_all(&self) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let threads: Vec<String> = st.chans.keys().cloned().collect();
        let (mut listening, mut dropped_queue, mut dropped) = (0, 0, Vec::new());
        for thread in &threads {
            let chan = st.chan(thread);
            listening += usize::from(chan.listening);
            chan.listening = false;
            chan.paused = false;
            chan.epoch += 1;
            dropped_queue += std::mem::take(&mut chan.queue).len();
            dropped.extend(sh.drop_now(&mut st, thread));
        }
        sh.tel.event(
            "channel.mute_all",
            None,
            true,
            json!({"listening": listening}),
            json!({"dropped_now": dropped.len(), "dropped_queue": dropped_queue}),
            None,
        );
        sh.commit(&mut st);
        drop(st);
        for p in dropped {
            sh.finish(p, Some("mute_all"));
        }
    }

    pub(crate) fn channels(&self) -> HashMap<String, ChannelView> {
        self.shared.lock().views.clone()
    }

    /// A new complete assistant message from the live tail. Returns the `live.message` action:
    /// `enqueued` | `unheard` (muted) | `before_listen` (covered by catch-up/reply cursor) |
    /// `empty` (nothing speakable).
    pub(crate) fn live_message(&self, thread: &str, created_ms: i64, speakable: &str) -> &'static str {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let chan = st.chan(thread);
        let action = if !chan.listening {
            chan.unheard += 1;
            "unheard"
        } else if created_ms < chan.listen_since_ms {
            "before_listen"
        } else if speakable.is_empty() {
            "empty"
        } else {
            sh.enqueue(&mut st, thread, "live", speakable.to_owned(), Some(created_ms), None, Value::Null);
            "enqueued"
        };
        sh.commit(&mut st);
        action
    }

    /// Speaks a fixed notice (`wait`, `stopped`) on a listening channel. Returns whether it was
    /// enqueued.
    pub(crate) fn notice(&self, thread: &str, source: &'static str, text: &str) -> bool {
        let sh = &*self.shared;
        let mut st = sh.lock();
        let listening = st.chans.get(thread).is_some_and(|c| c.listening);
        if listening {
            sh.enqueue(&mut st, thread, source, text.to_owned(), None, None, Value::Null);
            sh.commit(&mut st);
        }
        listening
    }

    /// `waiting_for_user` for every listed thread.
    pub(crate) fn set_waiting(&self, listed: &[String], waiting: &HashSet<&str>) {
        let sh = &*self.shared;
        let mut st = sh.lock();
        for thread in listed {
            let w = waiting.contains(thread.as_str());
            match st.chans.get_mut(thread) {
                Some(c) => c.waiting_for_user = w,
                None if w => st.chan(thread).waiting_for_user = true,
                None => {}
            }
        }
        sh.commit(&mut st);
    }
}

/// Short spoken label from a thread title: the title if ≤ 30 chars, else its first 4 words.
/// Brackets, double quotes and inner punctuation become spaces and apostrophes are trimmed at
/// word edges, so the label reads as one phrase ("t3-panel test: sadece 'Tamam, …" →
/// "t3-panel test sadece Tamam.") while Turkish suffixes keep theirs ("Eternals’a").
fn label_for(title: &str) -> Option<String> {
    const NOISE: &[char] = &['.', '!', '?', '…', ':', ';', ',', '"', '“', '”', '(', ')', '[', ']', '{', '}'];
    const APOSTROPHES: &[char] = &['\'', '‘', '’'];
    let cleaned: String = normalize(title).chars().map(|c| if NOISE.contains(&c) { ' ' } else { c }).collect();
    let words: Vec<&str> =
        cleaned.split_whitespace().map(|w| w.trim_matches(APOSTROPHES)).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return None;
    }
    let full = words.join(" ");
    let short = if full.chars().count() <= LABEL_MAX_CHARS { full } else { words[..words.len().min(LABEL_WORDS)].join(" ") };
    Some(format!("{short}."))
}

/// Unmute catch-up line: latest messages from the state DB → `extract` → enqueue.
fn catch_up(sh: &Shared, req: CatchUp) {
    let valid = |st: &State| st.chans.get(&req.thread).is_some_and(|c| c.listening && c.epoch == req.epoch);
    if !valid(&sh.lock()) {
        return;
    }
    let (status, title) = sh.thread_info(&req.thread);
    let messages = match t3::latest_messages(&sh.tel, &req.thread, CATCHUP_MESSAGES) {
        Ok(m) => m,
        Err(e) => {
            let _ = sh.events.send(CoreEvent::Error(format!("catch-up {}: {e:#}", req.thread)));
            return;
        }
    };
    let ex = extract(&status, &title, &messages);
    let mut st = sh.lock();
    if !valid(&st) {
        return;
    }
    let extra = json!({"rule": ex.rule, "status": status, "messages": messages.len(), "prep_ms": ms(req.click.elapsed())});
    sh.enqueue(&mut st, &req.thread, "catchup", ex.text, None, Some(req.click), extra);
    sh.commit(&mut st);
}

fn open_device(tel: &Telemetry, reason: &str) -> (Option<rodio::MixerDeviceSink>, f64) {
    let start = Instant::now();
    let res = open_output();
    let open_ms = ms(start.elapsed());
    let err = res.as_ref().err().map(|e| format!("{e:#}"));
    tel.event("audio.open", Some(open_ms), res.is_ok(), json!({"reason": reason}), Value::Null, err.as_deref());
    (res.ok(), open_ms)
}

type Opened = (Option<rodio::MixerDeviceSink>, f64);

/// Opens the output device on a short-lived thread (synthesis continues meanwhile); opens
/// inline if that thread cannot be spawned.
fn open_device_async(tel: &Arc<Telemetry>, reason: &'static str) -> crossbeam_channel::Receiver<Opened> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let (bg_tel, bg_tx) = (tel.clone(), tx.clone());
    let spawned = std::thread::Builder::new()
        .name("t3p-audio-open".into())
        .spawn(move || drop(bg_tx.send(open_device(&bg_tel, reason))));
    if spawned.is_err() {
        let _ = tx.send(open_device(tel, reason));
    }
    rx
}

/// Synthesizes and appends utterance `id` (label, then sentences or its prefetched audio).
fn synthesize(
    sh: &Shared,
    engine: &TtsEngine,
    labels: &mut HashMap<String, Vec<f32>>,
    id: u64,
    label: Option<&str>,
    text: &str,
    prefetch: Prefetch,
) {
    let tel = &*sh.tel;
    let sr = engine.sample_rate;
    if let Some(label) = label {
        let samples = match labels.get(label) {
            Some(s) => Some(s.clone()),
            None => engine.synth(tel, label, None).ok().map(|s| {
                labels.insert(label.to_owned(), s.samples.clone());
                s.samples
            }),
        };
        if let Some(s) = samples
            && !sh.append(id, s, sr, None)
        {
            return;
        }
    }
    match prefetch {
        Prefetch::Ready(parts) if !parts.is_empty() => {
            for s in parts {
                if !sh.append(id, s.samples, sr, Some(s.synth_ms)) {
                    return;
                }
            }
        }
        _ => {
            for sentence in split_sentences(text) {
                match engine.synth(tel, &sentence, None) {
                    Ok(s) => {
                        if !sh.append(id, s.samples, sr, Some(s.synth_ms)) {
                            return;
                        }
                    }
                    Err(e) => {
                        if let Some(p) = sh.lock().playing_mut(id) {
                            p.err = Some(format!("{e:#}"));
                        }
                        break;
                    }
                }
            }
        }
    }
    if let Some(p) = sh.lock().playing_mut(id) {
        p.synth_done = true;
    }
    sh.wake.notify_all();
}

fn prefetch(tel: &Telemetry, engine: &TtsEngine, text: &str) -> Prefetch {
    let parts: anyhow::Result<Vec<Synth>> = split_sentences(text).iter().map(|s| engine.synth(tel, s, None)).collect();
    parts.map_or(Prefetch::Failed, Prefetch::Ready)
}

/// The `t3p-tts` thread: owns the voice, the audio device and every `Player`.
fn player_loop(sh: &Shared, cfg: &Config) {
    let tel = &*sh.tel;
    let engine = TtsEngine::load(tel, cfg)
        .inspect_err(|e| {
            let _ = sh.events.send(CoreEvent::Error(format!("TTS model load failed: {e:#}")));
        })
        .ok();
    let mut device: Option<rodio::MixerDeviceSink> = None;
    let mut device_open_ms: Option<f64> = None;
    let mut opening: Option<crossbeam_channel::Receiver<Opened>> = None;
    let mut idle_since = Instant::now();
    let mut labels: HashMap<String, Vec<f32>> = HashMap::new();
    let mut st = sh.lock();
    loop {
        if st.current.as_ref().is_some_and(|p| p.synth_done && p.player.empty()) {
            let p = st.current.take().expect("checked above");
            st.last_audible_end = Some(Instant::now());
            let _ = sh.events.send(CoreEvent::SpeakingFinished { thread_id: p.item.thread.clone() });
            sh.commit(&mut st);
            drop(st);
            sh.finish(p, None);
            st = sh.lock();
            continue;
        }
        if let Some(rx) = &opening
            && let Ok((d, open_ms)) = rx.try_recv()
        {
            opening = None;
            device = d;
            device_open_ms = Some(open_ms);
            idle_since = Instant::now();
            if device.is_none() {
                // Nothing can play without a device: fail what is queued instead of spinning.
                let failed: Vec<Item> =
                    st.chans.values_mut().filter(|c| c.listening).flat_map(|c| c.queue.drain(..)).collect();
                sh.commit(&mut st);
                drop(st);
                for item in &failed {
                    sh.fail(item, "audio device unavailable");
                }
                if !failed.is_empty() {
                    let _ = sh.events.send(CoreEvent::Error("audio device unavailable".to_owned()));
                }
                st = sh.lock();
            }
            continue;
        }
        let pending = st.current.is_none() && st.has_next();
        if pending || st.current.is_some() || !st.held.is_empty() {
            idle_since = Instant::now();
        }
        if device.is_none() && (pending || st.warm || opening.is_some()) {
            st.warm = false;
            if opening.is_none() {
                opening = Some(open_device_async(&sh.tel, if pending { "play" } else { "warm" }));
                continue;
            }
            // CoreAudio takes ~0.3 s to open: synthesize the first utterance (and its label)
            // meanwhile so it starts the moment the device is up.
            if let Some(engine) = &engine
                && let Some(thread) = st.next_thread().map(str::to_owned)
                && let Some(head) = st.chans.get(&thread).and_then(|c| c.queue.front())
                && matches!(head.prefetch, Prefetch::None)
            {
                let (id, text) = (head.id, head.text.clone());
                let label = if st.needs_label(&thread) { label_for(&sh.thread_info(&thread).1) } else { None };
                drop(st);
                if let Some(label) = label
                    && !labels.contains_key(&label)
                    && let Ok(s) = engine.synth(tel, &label, None)
                {
                    labels.insert(label, s.samples);
                }
                let audio = prefetch(tel, engine, &text);
                st = sh.lock();
                if let Some(item) = st.chans.get_mut(&thread).and_then(|c| c.queue.iter_mut().find(|i| i.id == id)) {
                    item.prefetch = audio;
                }
                continue;
            }
            st = sh.wake.wait_timeout(st, PLAY_TICK).unwrap_or_else(|p| p.into_inner()).0;
            continue;
        }
        st.warm = false;
        if st.current.is_none() {
            if let Some(mut p) = st.pop_resumed() {
                p.player.play();
                if p.first_audio.is_none() && !p.player.empty() {
                    p.mark_first_audio(tel);
                }
                st.last_thread = Some(p.item.thread.clone());
                let _ = sh.events.send(CoreEvent::SpeakingStarted { thread_id: p.item.thread.clone() });
                st.current = Some(p);
                sh.commit(&mut st);
                continue;
            }
            if let Some(thread) = st.next_thread().map(str::to_owned) {
                let mut item = st.chan(&thread).queue.pop_front().expect("next_thread has a head");
                let (Some(engine), Some(dev)) = (&engine, &device) else {
                    sh.commit(&mut st);
                    sh.fail(&item, "TTS model not loaded");
                    continue;
                };
                let label = if st.needs_label(&thread) { label_for(&sh.thread_info(&thread).1) } else { None };
                let id = item.id;
                let text = item.text.clone();
                let prefetched = std::mem::take(&mut item.prefetch);
                let player = rodio::Player::connect_new(dev.mixer());
                let ready = matches!(prefetched, Prefetch::Ready(_));
                let mut p = Playing::new(item, player, engine.sample_rate, label.clone(), ready);
                p.device_open_ms = device_open_ms.take();
                st.last_thread = Some(thread.clone());
                let _ = sh.events.send(CoreEvent::SpeakingStarted { thread_id: thread });
                st.current = Some(p);
                sh.commit(&mut st);
                drop(st);
                synthesize(sh, engine, &mut labels, id, label.as_deref(), &text, prefetched);
                st = sh.lock();
                continue;
            }
        } else if let Some(engine) = &engine
            && st.current.as_ref().is_some_and(|p| p.synth_done)
            && let Some(thread) = st.next_thread().map(str::to_owned)
            && let Some(head) = st.chans.get(&thread).and_then(|c| c.queue.front())
            && matches!(head.prefetch, Prefetch::None)
        {
            let (id, text) = (head.id, head.text.clone());
            drop(st);
            let audio = prefetch(tel, engine, &text);
            st = sh.lock();
            if let Some(item) = st.chans.get_mut(&thread).and_then(|c| c.queue.iter_mut().find(|i| i.id == id)) {
                item.prefetch = audio;
            }
            continue;
        }
        if device.is_some() && st.current.is_none() && st.held.is_empty() && idle_since.elapsed() >= DEVICE_IDLE {
            device = None;
            tel.event("audio.close", None, true, json!({"idle_s": DEVICE_IDLE.as_secs()}), Value::Null, None);
        }
        let timeout = if st.current.is_some() {
            PLAY_TICK
        } else if device.is_some() {
            DEVICE_IDLE.saturating_sub(idle_since.elapsed()).max(PLAY_TICK)
        } else {
            Duration::from_secs(3600)
        };
        st = sh.wake.wait_timeout(st, timeout).unwrap_or_else(|p| p.into_inner()).0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(st: &mut State, thread: &str, id: u64) {
        let chan = st.chan(thread);
        chan.listening = true;
        chan.queue.push_back(Item {
            id,
            thread: thread.to_owned(),
            source: "live",
            text: format!("t{id}"),
            arrived: Instant::now(),
            created_ms: None,
            click: None,
            prefetch: Prefetch::None,
        });
        std::thread::sleep(Duration::from_millis(1));
    }

    /// Pops the next queued item the way the player loop does and makes it audible.
    fn start_next(st: &mut State) -> Option<u64> {
        if let Some(p) = st.pop_resumed() {
            let id = p.item.id;
            st.current = Some(p);
            return Some(id);
        }
        let thread = st.next_thread()?.to_owned();
        let item = st.chan(&thread).queue.pop_front()?;
        let id = item.id;
        st.current = Some(Playing::new(item, rodio::Player::new().0, 22_050, None, false));
        Some(id)
    }

    fn finish_current(st: &mut State) {
        st.current = None;
    }

    #[test]
    fn fifo_across_listening_channels() {
        let mut st = State::default();
        item(&mut st, "a", 1);
        item(&mut st, "b", 2);
        item(&mut st, "a", 3);
        st.chan("c").queue.push_back(Item {
            id: 9,
            thread: "c".into(),
            source: "live",
            text: String::new(),
            arrived: Instant::now() - Duration::from_secs(5),
            created_ms: None,
            click: None,
            prefetch: Prefetch::None,
        });
        let mut order = Vec::new();
        while let Some(id) = start_next(&mut st) {
            order.push(id);
            finish_current(&mut st);
        }
        // `c` is muted, so its (older) item never plays.
        assert_eq!(order, [1, 2, 3]);
    }

    #[test]
    fn pause_holds_and_resumed_goes_first() {
        let mut st = State::default();
        item(&mut st, "a", 1);
        item(&mut st, "a", 2);
        item(&mut st, "b", 3);
        item(&mut st, "b", 4);
        assert_eq!(start_next(&mut st), Some(1));
        assert!(st.pause("a"));
        assert!(st.current.is_none() && st.held.contains_key("a"));
        // Paused channel keeps queueing; others speak meanwhile.
        item(&mut st, "a", 5);
        assert_eq!(start_next(&mut st), Some(3));
        st.unpause("a");
        finish_current(&mut st);
        // The resumed utterance gets the slot before b's older queued item.
        assert_eq!(start_next(&mut st), Some(1));
        finish_current(&mut st);
        assert_eq!(start_next(&mut st), Some(2));
        finish_current(&mut st);
        assert_eq!(start_next(&mut st), Some(4));
        finish_current(&mut st);
        assert_eq!(start_next(&mut st), Some(5));
    }

    #[test]
    fn paused_channel_without_utterance_waits() {
        let mut st = State::default();
        item(&mut st, "a", 1);
        item(&mut st, "b", 2);
        assert!(!st.pause("a"));
        assert_eq!(start_next(&mut st), Some(2));
        finish_current(&mut st);
        assert_eq!(start_next(&mut st), None);
        st.unpause("a");
        assert_eq!(start_next(&mut st), Some(1));
    }

    #[test]
    fn take_playing_drops_current_or_held() {
        let mut st = State::default();
        item(&mut st, "a", 1);
        item(&mut st, "b", 2);
        assert_eq!(start_next(&mut st), Some(1));
        st.pause("a");
        st.unpause("a");
        assert!(st.resumed.iter().any(|t| t == "a"));
        let (p, audible) = st.take_playing("a").expect("held");
        assert_eq!((p.item.id, audible), (1, false));
        assert!(st.resumed.is_empty());
        assert_eq!(start_next(&mut st), Some(2));
        let (p, audible) = st.take_playing("b").expect("current");
        assert_eq!((p.item.id, audible), (2, true));
        assert!(st.current.is_none());
    }

    #[test]
    fn views_report_only_non_default_channels() {
        let mut st = State::default();
        st.chan("idle");
        item(&mut st, "a", 1);
        item(&mut st, "a", 2);
        start_next(&mut st);
        let v = st.views();
        assert_eq!(v.len(), 1);
        let a = &v["a"];
        assert!(a.listening && a.speaking && !a.paused);
        assert_eq!((a.queued, a.now_text.as_deref()), (1, Some("t1")));
        st.pause("a");
        let a = &st.views()["a"];
        assert!(a.paused && !a.speaking);
        assert_eq!(a.now_text.as_deref(), Some("t1"));
    }

    #[test]
    fn labels() {
        assert_eq!(label_for("Kısa başlık").as_deref(), Some("Kısa başlık."));
        assert_eq!(
            label_for("t3-panel test: sadece 'Tamam, hazirım.' yaz ve bitir.").as_deref(),
            Some("t3-panel test sadece Tamam.")
        );
        assert_eq!(label_for("Eternals’a Sürpriz Ara Sahne Ekleme").as_deref(), Some("Eternals’a Sürpriz Ara Sahne."));
        assert_eq!(label_for("  "), None);
    }
}
