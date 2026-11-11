//! Demo source (`Config.demo`, env `T3P_DEMO`): threads, messages and a timed script from a
//! fixture JSON (`demo/fixture.json`) instead of T3 and Wispr Flow. It feeds the same seams as
//! the real sources: the thread snapshot through [`t3::select`], new assistant messages and
//! waiting requests through [`live::deliver_message`] / [`live::detect_wait`], status changes as
//! `state.change` (+ the poller's stop notice), catch-up messages through the speech
//! `MessageSource`, and dictation / reply results as the usual `CoreEvent`s. It never starts a
//! subprocess. Driver-side events are logged as `demo.*`; shared ones carry `"demo": true`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::live;
use crate::speech::{MessageSource, Speech};
use crate::t3::{self, Message, Meta, ProjectMeta, RawThread};
use crate::telemetry::{Telemetry, lock, ms, now_unix_ms, rfc3339_from_ms};
use crate::{Config, CoreEvent, ProjectIcon, ThreadView};

/// `dictation_start` → the scripted transcript.
const DICTATION_DELAY: Duration = Duration::from_millis(2500);
/// `dictation_stop` → the scripted transcript.
const DICTATION_STOP_DELAY: Duration = Duration::from_millis(300);
/// `send_reply` → the scripted acknowledgement message.
const ACK_DELAY: Duration = Duration::from_millis(1500);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// `t3` (default): the T3 desktop app's theme is read as usual; `default`: built-in palette.
    #[serde(default)]
    theme: Option<String>,
    projects: Vec<FxProject>,
    threads: Vec<FxThread>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxProject {
    id: String,
    title: String,
    #[serde(default)]
    icon: Option<FxIcon>,
    #[serde(default)]
    pinned: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxIcon {
    kind: String,
    name: String,
    #[serde(default)]
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxThread {
    id: String,
    project: String,
    title: String,
    /// Spoken speaker label (else derived from the title).
    #[serde(default)]
    label: Option<String>,
    status: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    settled: bool,
    #[serde(default)]
    snoozed: bool,
    created_ago_s: u64,
    updated_ago_s: u64,
    /// Start of the active run, seconds before `Core::start`.
    #[serde(default)]
    running_for_s: Option<u64>,
    #[serde(default)]
    messages: Vec<FxMessage>,
    #[serde(default)]
    script: Vec<FxStep>,
    #[serde(default)]
    reply: Option<FxReply>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxMessage {
    role: String,
    text: String,
    /// Seconds before `Core::start`; orders the history (oldest first).
    #[serde(default)]
    ago_s: u64,
    /// English caption (logged, never spoken).
    #[serde(default)]
    en: Option<String>,
}

/// One timed script entry; any of `text`, `status`, `waiting` (applied in that order).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxStep {
    /// Seconds after `Core::start`.
    at_s: f64,
    /// A new complete assistant message.
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    en: Option<String>,
    #[serde(default)]
    status: Option<String>,
    /// `true`: an approval request starts waiting for the user; `false`: it is gone.
    #[serde(default)]
    waiting: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FxReply {
    /// What `dictation_start` "hears".
    dictation: String,
    #[serde(default)]
    dictation_en: Option<String>,
    /// The agent's answer to any reply, as a new assistant message.
    ack: String,
    #[serde(default)]
    ack_en: Option<String>,
}

fn validate(fx: &Fixture) -> anyhow::Result<()> {
    if let Some(theme) = fx.theme.as_deref()
        && !matches!(theme, "t3" | "default")
    {
        bail!("theme must be \"t3\" or \"default\", got {theme:?}");
    }
    let projects: HashSet<&str> = fx.projects.iter().map(|p| p.id.as_str()).collect();
    let mut ids = HashSet::new();
    for t in &fx.threads {
        if !ids.insert(t.id.as_str()) {
            bail!("duplicate thread id {:?}", t.id);
        }
        if !projects.contains(t.project.as_str()) {
            bail!("thread {:?}: unknown project {:?}", t.id, t.project);
        }
        if t.status.trim().is_empty() {
            bail!("thread {:?}: empty status", t.id);
        }
        for s in &t.script {
            if !s.at_s.is_finite() || s.at_s < 0.0 {
                bail!("thread {:?}: at_s must be ≥ 0, got {}", t.id, s.at_s);
            }
            if s.text.is_none() && s.status.is_none() && s.waiting.is_none() {
                bail!("thread {:?}: script entry at {} s has no text, status or waiting", t.id, s.at_s);
            }
            if s.status.as_deref().is_some_and(|st| st.trim().is_empty()) {
                bail!("thread {:?}: empty status at {} s", t.id, s.at_s);
            }
        }
    }
    Ok(())
}

struct DemoMessage {
    msg: Message,
    en: Option<String>,
}

/// The one scripted dictation in progress.
struct Dictation {
    id: u64,
    thread: usize,
    stopped: bool,
}

enum Job {
    Step { thread: usize, step: usize },
    Reply { thread: usize, queued: bool },
    Ack { thread: usize, complete: bool },
    Transcript { session: u64 },
}

/// Timed jobs for the driver thread (handed to [`Demo::run`]).
pub(crate) struct Jobs(Receiver<(Instant, Job)>);

/// Mutable demo state; thread indexes are fixture order.
struct State {
    raw: Vec<RawThread>,
    meta: Meta,
    messages: Vec<Vec<DemoMessage>>,
    waiting: HashSet<usize>,
    dictation: Option<Dictation>,
    /// Message, request and dictation ids.
    seq: u64,
}

impl State {
    fn next_id(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

struct Shared {
    tel: Arc<Telemetry>,
    cfg: Config,
    events: Sender<CoreEvent>,
    snapshot: Arc<RwLock<Vec<ThreadView>>>,
    jobs: Sender<(Instant, Job)>,
    default_theme: bool,
    /// Fixture threads (script, reply, label); `messages` moved into [`State`].
    threads: Vec<FxThread>,
    state: Mutex<State>,
}

/// Handle owned by `Core` in demo mode.
pub(crate) struct Demo {
    sh: Arc<Shared>,
}

impl Demo {
    /// Reads and validates the fixture (logged as `demo.load`) and schedules its script relative
    /// to now (= `Core::start`).
    pub(crate) fn load(
        tel: Arc<Telemetry>,
        cfg: &Config,
        path: &Path,
        snapshot: Arc<RwLock<Vec<ThreadView>>>,
        events: Sender<CoreEvent>,
    ) -> anyhow::Result<(Demo, Jobs)> {
        let start = Instant::now();
        let start_ms = now_unix_ms();
        let parsed = std::fs::read(path)
            .with_context(|| format!("read demo fixture {}", path.display()))
            .and_then(|b| serde_json::from_slice::<Fixture>(&b).with_context(|| format!("parse demo fixture {}", path.display())))
            .and_then(|fx| validate(&fx).map(|()| fx));
        let mut fx = match parsed {
            Ok(fx) => fx,
            Err(e) => {
                tel.event("demo.load", None, false, json!({"path": path}), Value::Null, Some(&format!("{e:#}")));
                return Err(e);
            }
        };
        let at = |ago_s: u64| rfc3339_from_ms(start_ms - ago_s as i64 * 1000);
        let mut meta = Meta::default();
        for p in &fx.projects {
            let icon = p.icon.as_ref().map(|i| ProjectIcon {
                kind: i.kind.clone(),
                name: i.name.clone(),
                color: i.color.clone().filter(|c| !c.is_empty()),
            });
            meta.projects.insert(p.id.clone(), ProjectMeta { name: p.title.clone(), icon, pinned: p.pinned });
        }
        let (tx, rx) = crossbeam_channel::unbounded();
        let (mut raw, mut messages, mut steps) = (Vec::new(), Vec::new(), 0);
        let mut span_s: f64 = 0.0;
        for (i, t) in fx.threads.iter_mut().enumerate() {
            if t.pinned {
                meta.pins.insert(t.id.clone(), Some(format!("demo-{i:03}")));
            }
            if let Some(r) = t.running_for_s {
                meta.running_since.insert(t.id.clone(), at(r));
            }
            raw.push(RawThread {
                thread_id: t.id.clone(),
                project_id: t.project.clone(),
                title: t.title.clone(),
                status: t.status.clone(),
                settled: t.settled,
                updated_at: at(t.updated_ago_s),
                created_at: at(t.created_ago_s),
                snoozed: t.snoozed,
                snoozed_until: None,
                link: None,
                provider_instance_id: Some(t.provider.clone()).filter(|p| !p.is_empty()),
                model: Some(t.model.clone()).filter(|m| !m.is_empty()),
                latest_run_id: None,
            });
            let mut history = std::mem::take(&mut t.messages);
            history.sort_by_key(|m| std::cmp::Reverse(m.ago_s));
            messages.push(
                history
                    .into_iter()
                    .map(|m| DemoMessage { msg: Message { role: m.role, run_id: None, text: m.text }, en: m.en })
                    .collect(),
            );
            for (j, s) in t.script.iter().enumerate() {
                steps += 1;
                span_s = span_s.max(s.at_s);
                let _ = tx.send((start + Duration::from_secs_f64(s.at_s), Job::Step { thread: i, step: j }));
            }
        }
        let default_theme = fx.theme.as_deref() == Some("default");
        tel.event(
            "demo.load",
            Some(ms(start.elapsed())),
            true,
            json!({"path": path}),
            json!({"projects": fx.projects.len(), "threads": fx.threads.len(), "script_steps": steps,
                   "script_span_s": span_s, "theme": if default_theme { "default" } else { "t3" }}),
            None,
        );
        let state = State { raw, meta, messages, waiting: HashSet::new(), dictation: None, seq: 0 };
        let sh = Shared {
            tel,
            cfg: cfg.clone(),
            events,
            snapshot,
            jobs: tx,
            default_theme,
            threads: fx.threads,
            state: Mutex::new(state),
        };
        Ok((Demo { sh: Arc::new(sh) }, Jobs(rx)))
    }

    /// The fixture sets `"theme": "default"`: T3's theme is not read.
    pub(crate) fn default_theme(&self) -> bool {
        self.sh.default_theme
    }

    /// Thread id → fixture `label`.
    pub(crate) fn labels(&self) -> HashMap<String, String> {
        self.sh.threads.iter().filter_map(|t| Some((t.id.clone(), t.label.clone()?))).collect()
    }

    /// Catch-up messages: the fixture history plus every message the script added.
    pub(crate) fn message_source(&self) -> MessageSource {
        let sh = self.sh.clone();
        Arc::new(move |tel: &Telemetry, thread: &str, n: u32| Ok(sh.latest_messages(tel, thread, n)))
    }

    /// Publishes the initial snapshot and spawns the `t3p-demo` driver thread that runs the
    /// script, replies and dictations.
    pub(crate) fn run(&self, jobs: Jobs, speech: Speech) -> std::io::Result<()> {
        let sh = self.sh.clone();
        std::thread::Builder::new().name("t3p-demo".into()).spawn(move || {
            sh.publish(&speech);
            // (due, arrival order, job); a handful at most, so a linear scan picks the next.
            let mut pending: Vec<(Instant, u64, Job)> = Vec::new();
            let mut seq = 0u64;
            loop {
                let next = pending.iter().enumerate().min_by_key(|(_, (due, s, _))| (*due, *s)).map(|(i, (due, ..))| (i, *due));
                let got = match next {
                    Some((_, due)) => jobs.0.recv_deadline(due),
                    None => jobs.0.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match (got, next) {
                    (Ok((due, job)), _) => {
                        pending.push((due, seq, job));
                        seq += 1;
                    }
                    (Err(RecvTimeoutError::Timeout), Some((i, _))) => {
                        let (due, _, job) = pending.swap_remove(i);
                        sh.exec(&speech, due, job);
                    }
                    (Err(_), _) => break,
                }
            }
        })?;
        Ok(())
    }

    /// Scripted dictation: `DictationTranscript` with the thread's `reply.dictation` after
    /// [`DICTATION_DELAY`] (or [`DICTATION_STOP_DELAY`] after `dictation_stop`). One at a time.
    pub(crate) fn dictation_start(&self, thread_id: &str) {
        let sh = &*self.sh;
        let input = json!({"thread": thread_id, "demo": true});
        let Some(idx) = sh.index(thread_id) else {
            sh.fail_dictation(thread_id, input, "unknown demo thread".to_owned());
            return;
        };
        let started = {
            let mut st = lock(&sh.state);
            match st.dictation.as_ref().map(|d| d.thread) {
                Some(t) => Err(t),
                None => {
                    let id = st.next_id();
                    st.dictation = Some(Dictation { id, thread: idx, stopped: false });
                    Ok(id)
                }
            }
        };
        match started {
            Ok(id) => {
                let _ = sh.jobs.send((Instant::now() + DICTATION_DELAY, Job::Transcript { session: id }));
                let out = json!({"transcript_in_ms": DICTATION_DELAY.as_millis() as u64});
                sh.tel.event("reply.dictation_start", None, true, input, out, None);
            }
            Err(t) if t == idx => {
                let out = json!({"ignored": "dictation already active for this thread"});
                sh.tel.event("reply.dictation_start", None, true, input, out, None);
            }
            Err(t) => sh.fail_dictation(thread_id, input, format!("another dictation is active ({})", sh.threads[t].id)),
        }
    }

    /// Ends the scripted dictation; its transcript follows after [`DICTATION_STOP_DELAY`].
    pub(crate) fn dictation_stop(&self) {
        let sh = &*self.sh;
        let active = lock(&sh.state).dictation.as_mut().map(|d| {
            d.stopped = true;
            (d.id, d.thread)
        });
        let Some((id, thread)) = active else {
            sh.tel.event("reply.dictation_stop", None, false, json!({"demo": true}), Value::Null, Some("no active dictation"));
            return;
        };
        let _ = sh.jobs.send((Instant::now() + DICTATION_STOP_DELAY, Job::Transcript { session: id }));
        let out = json!({"transcript_in_ms": DICTATION_STOP_DELAY.as_millis() as u64});
        sh.tel.event("reply.dictation_stop", None, true, json!({"thread": sh.threads[thread].id, "demo": true}), out, None);
    }

    /// Ends the scripted dictation without a transcript.
    pub(crate) fn dictation_cancel(&self) {
        let sh = &*self.sh;
        let Some(d) = lock(&sh.state).dictation.take() else {
            sh.tel.event("reply.dictation_cancel", None, false, json!({"demo": true}), Value::Null, Some("no active dictation"));
            return;
        };
        let input = json!({"thread": sh.threads[d.thread].id, "demo": true});
        sh.tel.event("reply.dictation_cancel", None, true, input, Value::Null, None);
    }

    /// Logs `reply.send` (nothing is sent) and schedules the reply's effects: a waiting request
    /// is answered, a thread that is not running starts running, and the fixture `reply.ack`
    /// arrives after [`ACK_DELAY`] (then the thread completes unless the reply was queued
    /// behind a running turn). Returns `(ok, queued, detail)` for `ReplySent`.
    pub(crate) fn send_reply(&self, thread_id: &str, payload: &str) -> (bool, bool, String) {
        let sh = &*self.sh;
        let Some(idx) = sh.index(thread_id) else {
            let err = "unknown demo thread".to_owned();
            sh.tel.event("reply.send", None, false, json!({"thread": thread_id, "demo": true}), Value::Null, Some(&err));
            return (false, false, err);
        };
        let (status, waiting) = {
            let st = lock(&sh.state);
            (st.raw[idx].status.clone(), st.waiting.contains(&idx))
        };
        // A reply to a waiting request answers it; otherwise a running thread queues it.
        let queued = status == "running" && !waiting;
        let ack = sh.threads[idx].reply.as_ref().map(|r| r.ack.as_str());
        sh.tel.event(
            "reply.send",
            Some(0.0),
            true,
            json!({"thread": thread_id, "text_with_prefix": payload, "queued": queued, "status": status,
                   "waiting": waiting, "demo": true}),
            json!({"ack": ack, "ack_in_ms": ACK_DELAY.as_millis() as u64}),
            None,
        );
        let _ = sh.jobs.send((Instant::now(), Job::Reply { thread: idx, queued }));
        (true, queued, "demo: reply not sent anywhere".to_owned())
    }
}

impl Shared {
    fn index(&self, thread_id: &str) -> Option<usize> {
        self.threads.iter().position(|t| t.id == thread_id)
    }

    fn fail_dictation(&self, thread_id: &str, input: Value, error: String) {
        self.tel.event("reply.dictation_start", None, false, input, Value::Null, Some(&error));
        let _ = self.events.send(CoreEvent::DictationFailed { thread_id: thread_id.to_owned(), error });
    }

    fn latest_messages(&self, tel: &Telemetry, thread: &str, n: u32) -> Vec<Message> {
        let (msgs, en) = match self.index(thread) {
            Some(idx) => {
                let st = lock(&self.state);
                let all = &st.messages[idx];
                let tail = &all[all.len().saturating_sub(n as usize)..];
                let en = tail.iter().rev().find(|m| m.msg.role == "assistant").and_then(|m| m.en.clone());
                (tail.iter().map(|m| m.msg.clone()).collect::<Vec<_>>(), en)
            }
            None => (Vec::new(), None),
        };
        let out = json!({"messages": msgs.len(), "latest_assistant_en": en});
        tel.event("demo.messages", Some(0.0), true, json!({"thread": thread, "n": n}), out, None);
        msgs
    }

    /// Thread snapshot in T3's sidebar order (`t3::select`), `waiting_for_user`, then
    /// `ThreadsUpdated`.
    fn publish(&self, speech: &Speech) {
        let (views, waiting) = {
            let st = lock(&self.state);
            let views = t3::select(&self.cfg, st.raw.iter().cloned(), now_unix_ms(), &st.meta);
            let waiting: Vec<String> = st.waiting.iter().map(|&i| st.raw[i].thread_id.clone()).collect();
            (views, waiting)
        };
        let ids: Vec<String> = views.iter().map(|v| v.id.clone()).collect();
        *self.snapshot.write().unwrap_or_else(|p| p.into_inner()) = views;
        speech.set_waiting(&ids, &waiting.iter().map(String::as_str).collect());
        self.tel.update_stats(|s| s.t3_connected = true);
        let _ = self.events.send(CoreEvent::ThreadsUpdated);
    }

    fn exec(&self, speech: &Speech, due: Instant, job: Job) {
        match job {
            Job::Step { thread, step } => {
                let t = &self.threads[thread];
                let s = &t.script[step];
                self.tel.event(
                    "demo.step",
                    None,
                    true,
                    json!({"thread": t.id, "at_s": s.at_s, "text": s.text, "en": s.en, "status": s.status,
                           "waiting": s.waiting}),
                    json!({"late_ms": ms(due.elapsed())}),
                    None,
                );
                if let Some(text) = &s.text {
                    self.add_message(speech, thread, text, s.en.as_deref());
                }
                if let Some(status) = &s.status {
                    self.set_status(speech, thread, status);
                }
                if let Some(on) = s.waiting {
                    self.set_waiting(speech, thread, on);
                }
            }
            Job::Reply { thread, queued } => {
                self.set_waiting(speech, thread, false);
                if !queued {
                    self.set_status(speech, thread, "running");
                }
                let _ = self.jobs.send((Instant::now() + ACK_DELAY, Job::Ack { thread, complete: !queued }));
            }
            Job::Ack { thread, complete } => {
                let t = &self.threads[thread];
                let ack = t.reply.as_ref().map(|r| (r.ack.as_str(), r.ack_en.as_deref()));
                self.tel.event(
                    "demo.reply_ack",
                    None,
                    true,
                    json!({"thread": t.id}),
                    json!({"text": ack.map(|a| a.0), "en": ack.and_then(|a| a.1), "complete": complete}),
                    None,
                );
                if let Some((text, en)) = ack {
                    self.add_message(speech, thread, text, en);
                }
                if complete {
                    self.set_status(speech, thread, "completed");
                }
            }
            Job::Transcript { session } => {
                self.transcript(session);
                return;
            }
        }
        self.publish(speech);
    }

    /// Delivers the scripted dictation of session `session` unless it was cancelled or already
    /// delivered (a stop schedules a second, earlier delivery).
    fn transcript(&self, session: u64) {
        let d = {
            let mut st = lock(&self.state);
            if st.dictation.as_ref().is_some_and(|d| d.id == session) { st.dictation.take() } else { None }
        };
        let Some(d) = d else { return };
        let t = &self.threads[d.thread];
        let thread_id = t.id.clone();
        let input = json!({"thread": t.id, "stopped": d.stopped});
        match &t.reply {
            Some(r) => {
                self.tel.event("demo.dictation", None, true, input, json!({"text": r.dictation, "en": r.dictation_en}), None);
                let _ = self.events.send(CoreEvent::DictationTranscript { thread_id, text: r.dictation.clone() });
            }
            None => {
                let error = "no scripted dictation for this demo thread".to_owned();
                self.tel.event("demo.dictation", None, false, input, Value::Null, Some(&error));
                let _ = self.events.send(CoreEvent::DictationFailed { thread_id, error });
            }
        }
    }

    /// A new complete assistant message: kept for catch-up and spoken through the live rules.
    fn add_message(&self, speech: &Speech, idx: usize, text: &str, en: Option<&str>) {
        let now = now_unix_ms();
        let created_at = rfc3339_from_ms(now);
        let (thread, message_id) = {
            let mut st = lock(&self.state);
            let id = st.next_id();
            let msg = Message { role: "assistant".to_owned(), run_id: None, text: text.to_owned() };
            st.messages[idx].push(DemoMessage { msg, en: en.map(str::to_owned) });
            st.raw[idx].updated_at.clone_from(&created_at);
            (st.raw[idx].thread_id.clone(), format!("demo-msg-{id}"))
        };
        let input = json!({"thread": thread, "message_id": message_id, "created_at": created_at, "en": en, "demo": true});
        live::deliver_message(&self.tel, speech, input, &thread, now, text);
    }

    /// Status change with the poller's semantics (`state.change` + stop notice); any waiting
    /// request ends with it.
    fn set_status(&self, speech: &Speech, idx: usize, to: &str) {
        let now = rfc3339_from_ms(now_unix_ms());
        let (thread, title, from) = {
            let mut guard = lock(&self.state);
            let st = &mut *guard;
            let t = &mut st.raw[idx];
            if t.status == to {
                return;
            }
            let from = std::mem::replace(&mut t.status, to.to_owned());
            t.updated_at.clone_from(&now);
            if to == "running" {
                st.meta.running_since.entry(t.thread_id.clone()).or_insert(now);
            } else {
                st.meta.running_since.remove(&t.thread_id);
            }
            st.waiting.remove(&idx);
            (t.thread_id.clone(), t.title.clone(), from)
        };
        self.tel.event(
            "state.change",
            None,
            true,
            json!({"thread": thread, "title": title, "from": from, "to": to, "demo": true}),
            Value::Null,
            None,
        );
        crate::status_changed(speech, &thread, to);
    }

    /// An approval request starts (`wait.detect`, spoken on a listening channel) or ends.
    fn set_waiting(&self, speech: &Speech, idx: usize, on: bool) {
        let started = {
            let mut st = lock(&self.state);
            if !on {
                st.waiting.remove(&idx);
                None
            } else if st.waiting.insert(idx) {
                let id = st.next_id();
                Some((st.raw[idx].thread_id.clone(), format!("demo-req-{id}")))
            } else {
                None
            }
        };
        if let Some((thread, request)) = started {
            let input = json!({"thread": thread, "request": request, "type": "approval_request",
                               "updated_at": rfc3339_from_ms(now_unix_ms()), "demo": true});
            live::detect_wait(&self.tel, speech, &thread, input, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `demo/fixture.json` parses, validates, labels every thread and captions every line that
    /// can be spoken (assistant messages, script texts, waiting steps, replies).
    #[test]
    fn repo_fixture_is_valid() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../demo/fixture.json");
        let fx: Fixture = serde_json::from_slice(&std::fs::read(path).expect("fixture readable")).expect("fixture parses");
        validate(&fx).expect("fixture valid");
        for t in &fx.threads {
            assert!(t.label.is_some(), "{}: label", t.id);
            assert!(t.messages.iter().filter(|m| m.role == "assistant").all(|m| m.en.is_some()), "{}: message en", t.id);
            assert!(t.script.iter().filter(|s| s.text.is_some() || s.waiting == Some(true)).all(|s| s.en.is_some()), "{}: step en", t.id);
            if let Some(r) = &t.reply {
                assert!(r.dictation_en.is_some() && r.ack_en.is_some(), "{}: reply en", t.id);
            }
        }
        let bad = r#"{"projects": [], "threads": [{"id": "a", "project": "nope", "title": "x", "status": "idle",
                      "created_ago_s": 1, "updated_ago_s": 1}]}"#;
        assert!(validate(&serde_json::from_str(bad).expect("parses")).is_err());
        assert!(serde_json::from_str::<Fixture>(r#"{"projects": [], "threads": [], "typo": 1}"#).is_err());
    }
}
