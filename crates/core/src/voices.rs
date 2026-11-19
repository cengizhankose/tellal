//! Voice registry and model management: which voices exist ([`VOICES`]), whether each is
//! installed under `Config.models_dir/<dir>`, and downloads.
//!
//! A download fetches every file over HTTPS from its pinned Hugging Face revision into
//! `<models_dir>/.<dir>.part`, checks each file's size and SHA-256 while streaming, then renames
//! the directory into place (an older `<dir>` is swapped out first), so a voice is either complete
//! or absent. Progress → `CoreEvent::VoicesUpdated` at most every [`PROGRESS_EVERY`]; each
//! attempt is logged as `voice.download`. "Installed" = every file present with its expected size
//! (checked at start and after each download, never per call).
//!
//! Demo mode (`Config.demo`) never touches the network: the fixture's optional `voices` block
//! scripts the states, and a download is simulated over [`DEMO_DOWNLOAD`].

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use crossbeam_channel::Sender;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::telemetry::{Telemetry, lock, ms};
use crate::{CoreEvent, Lang, VoiceState, VoiceView};

/// One file of a voice: local name, pinned source URL, size and SHA-256 (hex).
pub(crate) struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub(crate) struct VoiceDef {
    pub id: &'static str,
    pub lang: Lang,
    pub name: &'static str,
    /// Directory under `Config.models_dir`.
    pub dir: &'static str,
    pub files: &'static [ModelFile],
}

impl VoiceDef {
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
}

/// Pinned Hugging Face revisions.
macro_rules! ema {
    ($f:literal) => {
        concat!("https://huggingface.co/ozcancelik/ema-lightning-onnx/resolve/13c431db0356b2f7fafb1247cd823ec0d777c820/", $f)
    };
}
macro_rules! supertone {
    ($f:literal) => {
        concat!("https://huggingface.co/Supertone/supertonic-2/resolve/75e6727618a02f323c720cba9478152d4bc16ca4/", $f)
    };
}
macro_rules! sherpa {
    ($f:literal) => {
        concat!(
            "https://huggingface.co/csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06/resolve/",
            "5dec5897b86e2b293790eedb5e1f561a3295651f/",
            $f
        )
    };
}

/// Every voice Tellal knows, in `Core::voices` order.
pub(crate) static VOICES: [VoiceDef; 2] = [
    VoiceDef {
        id: "tr-ema-lightning",
        lang: Lang::Tr,
        name: "EMA Lightning",
        // The directory of the single-voice releases: existing installs stay valid.
        dir: "ema-lightning-onnx",
        files: &[
            ModelFile {
                name: "meta.json",
                url: ema!("meta.json"),
                size: 357,
                sha256: "76627201405723633f5319a208ed7dea63c42acbf3ca997671bccdca9d9186b2",
            },
            ModelFile {
                name: "text.onnx",
                url: ema!("text.onnx"),
                size: 4_806_400,
                sha256: "ad5d4885b5d218f63d976a6e562334ea0194aec54847a6c6482d328e784089f0",
            },
            ModelFile {
                name: "sound.onnx",
                url: ema!("sound.onnx"),
                size: 18_120_655,
                sha256: "958364217360cecaa92dc5ff2170d19f3c9f6995fd5e61241ea4068971a46a59",
            },
            ModelFile {
                name: "decoder.onnx",
                url: ema!("decoder.onnx"),
                size: 12_031_945,
                sha256: "396c06a8e5711fd40bfd2c3f4f33851feb2ad07a86485833844e0ae8ff637b17",
            },
            ModelFile {
                name: "LICENSE",
                url: ema!("LICENSE"),
                size: 11_356,
                sha256: "5c9817c129b98e7bb966bca028c43c19107102ef8e03fe799bffb4354f4ef015",
            },
            ModelFile {
                name: "NOTICE",
                url: ema!("NOTICE"),
                size: 1_170,
                sha256: "498684f051de6523c86c08b6f182f2d69b65df873ed260b96dbe802be4df94d3",
            },
        ],
    },
    VoiceDef {
        id: "en-supertonic-2",
        lang: Lang::En,
        name: "Supertonic 2",
        dir: "supertonic-2-en",
        files: &[
            ModelFile {
                name: "tts.json",
                url: supertone!("onnx/tts.json"),
                size: 8_699,
                sha256: "ee531d9af9b80438a2ed703e22155ee6c83b12595ab22fd3bb6de94c7502fe96",
            },
            ModelFile {
                name: "unicode_indexer.json",
                url: supertone!("onnx/unicode_indexer.json"),
                size: 262_196,
                sha256: "b7662a73a0703f43b97c0f2e089f8e8325e26f5d841aca393b5a54c509c92df1",
            },
            ModelFile {
                name: "M1.json",
                url: supertone!("voice_styles/M1.json"),
                size: 420_510,
                sha256: "a04c823cbda6dd1c7de131ec68fea83bbb70d7f29d61623304eb871e3b83b5a1",
            },
            ModelFile {
                name: "duration_predictor.onnx",
                url: supertone!("onnx/duration_predictor.onnx"),
                size: 1_521_526,
                sha256: "6d556b3691165c364be91dc0bd894656b5949f5acd2750d8ec2f954010845011",
            },
            ModelFile {
                name: "text_encoder.onnx",
                url: supertone!("onnx/text_encoder.onnx"),
                size: 27_431_318,
                sha256: "dd5f535ed629f7df86071043e15f541ce1b2ab7f1bdbce4c7892b307bca79fa3",
            },
            ModelFile {
                name: "vector_estimator.int8.onnx",
                url: sherpa!("vector_estimator.int8.onnx"),
                size: 40_686_657,
                sha256: "9cce89801068dc998719fbc91777f99f3f4d16ee36592281f8703f32e305daa2",
            },
            ModelFile {
                name: "vocoder.int8.onnx",
                url: sherpa!("vocoder.int8.onnx"),
                size: 25_977_132,
                sha256: "db99c36fb871fbfb128b903662995f2e22213e875f2434f319f8d10ee0f10e90",
            },
            ModelFile {
                name: "LICENSE",
                url: supertone!("LICENSE"),
                size: 15_007,
                sha256: "0d944a9110fed9a9602d60e0423a272903e7bd21ab060490774efc77c2275e9f",
            },
        ],
    },
];

/// Download progress reaches the UI at most this often (~4/s).
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// A simulated (demo) download from 0 % to Ready.
const DEMO_DOWNLOAD: Duration = Duration::from_secs(5);
const READ_BUF: usize = 64 * 1024;

/// Fixture `voices` entry (demo mode).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DemoVoice {
    pub id: String,
    /// `ready` | `missing` | `failed:<message>` | `downloading:<percent>` (progresses at once).
    pub state: String,
    /// The first download attempt fails halfway (a retry succeeds).
    #[serde(default)]
    pub download_fails: bool,
}

enum DemoStart {
    State(VoiceState),
    Downloading(u64),
}

fn parse_demo_state(s: &str) -> anyhow::Result<DemoStart> {
    Ok(match s.split_once(':') {
        None if s == "ready" => DemoStart::State(VoiceState::Ready),
        None if s == "missing" => DemoStart::State(VoiceState::Missing),
        Some(("failed", msg)) => DemoStart::State(VoiceState::Failed(msg.to_owned())),
        Some(("downloading", pct)) => match pct.trim().trim_end_matches('%').parse::<u64>() {
            Ok(p) if p <= 100 => DemoStart::Downloading(p),
            _ => bail!("voice state {s:?}: percent must be 0–100"),
        },
        _ => bail!("voice state {s:?}: expected ready | missing | failed:<message> | downloading:<percent>"),
    })
}

/// Voice states and downloads (cheap to clone).
#[derive(Clone)]
pub(crate) struct Voices {
    inner: Arc<Inner>,
}

struct Inner {
    tel: Arc<Telemetry>,
    events: Sender<CoreEvent>,
    root: PathBuf,
    defs: &'static [VoiceDef],
    /// Demo mode: simulated downloads.
    demo: bool,
    st: Mutex<St>,
}

type Waker = Arc<dyn Fn() + Send + Sync>;

struct St {
    /// Aligned with `defs`.
    states: Vec<VoiceState>,
    /// Demo: the next download of this voice fails.
    fail_next: Vec<bool>,
    /// Demo: `downloading:<percent>` start states, begun by [`Voices::start`].
    demo_starts: Vec<(usize, u64)>,
    last_progress_event: Option<Instant>,
    on_ready: Option<Waker>,
}

impl Voices {
    /// Scans `root` (logged as `voice.scan`); in demo mode applies the fixture's states.
    pub(crate) fn new(
        tel: Arc<Telemetry>,
        events: Sender<CoreEvent>,
        root: PathBuf,
        defs: &'static [VoiceDef],
        demo: Option<&[DemoVoice]>,
    ) -> anyhow::Result<Voices> {
        let mut states: Vec<VoiceState> =
            defs.iter().map(|d| if installed(&root, d) { VoiceState::Ready } else { VoiceState::Missing }).collect();
        let mut fail_next = vec![false; defs.len()];
        let mut demo_starts = Vec::new();
        for v in demo.unwrap_or_default() {
            let Some(i) = defs.iter().position(|d| d.id == v.id) else {
                let known: Vec<&str> = defs.iter().map(|d| d.id).collect();
                bail!("demo fixture voices: unknown id {:?} (known: {known:?})", v.id);
            };
            match parse_demo_state(&v.state).with_context(|| format!("demo fixture voice {}", v.id))? {
                DemoStart::State(s) => states[i] = s,
                DemoStart::Downloading(p) => {
                    states[i] = VoiceState::Downloading { done_bytes: defs[i].total_bytes() * p / 100, total_bytes: defs[i].total_bytes() };
                    demo_starts.push((i, p));
                }
            }
            fail_next[i] = v.download_fails;
        }
        let scan: Vec<Value> = defs.iter().zip(&states).map(|(d, s)| json!({"id": d.id, "state": state_label(s)})).collect();
        tel.event("voice.scan", None, true, json!({"models_dir": root, "demo": demo.is_some()}), json!({"voices": scan}), None);
        let st = St { states, fail_next, demo_starts, last_progress_event: None, on_ready: None };
        Ok(Voices { inner: Arc::new(Inner { tel, events, root, defs, demo: demo.is_some(), st: Mutex::new(st) }) })
    }

    /// Starts the fixture's scripted downloads (demo) or downloads the voice for `lang` when it
    /// is missing (real mode).
    pub(crate) fn start(&self, lang: Lang) {
        if self.inner.demo {
            let starts = std::mem::take(&mut lock(&self.inner.st).demo_starts);
            for (i, pct) in starts {
                self.spawn(i, pct);
            }
        } else if let Some(def) = self.inner.defs.iter().find(|d| d.lang == lang)
            && self.state(def) == VoiceState::Missing
        {
            self.download(def.id, "auto");
        }
    }

    /// `f` runs whenever a voice turns Ready.
    pub(crate) fn on_ready(&self, f: impl Fn() + Send + Sync + 'static) {
        lock(&self.inner.st).on_ready = Some(Arc::new(f));
    }

    pub(crate) fn views(&self) -> Vec<VoiceView> {
        let st = lock(&self.inner.st);
        self.inner
            .defs
            .iter()
            .zip(&st.states)
            .map(|(d, s)| VoiceView {
                id: d.id.to_owned(),
                lang: d.lang,
                name: d.name.to_owned(),
                download_mb: (d.total_bytes() as f64 / 1e6).round() as u32,
                state: s.clone(),
            })
            .collect()
    }

    fn state(&self, def: &VoiceDef) -> VoiceState {
        let i = self.index(def.id).expect("def from this registry");
        lock(&self.inner.st).states[i].clone()
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.inner.defs.iter().position(|d| d.id == id)
    }

    /// The Ready voice for `lang`, else any other Ready voice.
    pub(crate) fn pick(&self, lang: Lang) -> Option<&'static VoiceDef> {
        self.pick_except(lang, &HashSet::new())
    }

    /// [`Voices::pick`] ignoring the voices in `skip`.
    pub(crate) fn pick_except(&self, lang: Lang, skip: &HashSet<&'static str>) -> Option<&'static VoiceDef> {
        let st = lock(&self.inner.st);
        let defs = self.inner.defs;
        let usable = |d: &&VoiceDef| {
            !skip.contains(d.id) && defs.iter().position(|x| x.id == d.id).is_some_and(|i| st.states[i] == VoiceState::Ready)
        };
        defs.iter().filter(usable).find(|d| d.lang == lang).or_else(|| defs.iter().find(usable))
    }

    pub(crate) fn dir(&self, def: &VoiceDef) -> PathBuf {
        self.inner.root.join(def.dir)
    }

    /// Why nothing can speak, e.g. `no voice ready (tr-ema-lightning: missing, en-supertonic-2:
    /// downloading 40%)`.
    pub(crate) fn none_ready(&self) -> String {
        let st = lock(&self.inner.st);
        let parts: Vec<String> =
            self.inner.defs.iter().zip(&st.states).map(|(d, s)| format!("{}: {}", d.id, state_label(s))).collect();
        format!("no voice ready ({})", parts.join(", "))
    }

    /// Downloads voice `id` in the background (`source` = `user` | `auto`, logged). Ignored while
    /// it is Ready or downloading.
    pub(crate) fn download(&self, id: &str, source: &'static str) {
        let Some(i) = self.index(id) else {
            let err = format!("unknown voice {id:?}");
            self.inner.tel.event("voice.download", None, false, json!({"id": id, "source": source}), Value::Null, Some(&err));
            let _ = self.inner.events.send(CoreEvent::Error(err));
            return;
        };
        {
            let mut st = lock(&self.inner.st);
            if matches!(st.states[i], VoiceState::Ready | VoiceState::Downloading { .. }) {
                let skipped = state_label(&st.states[i]);
                drop(st);
                let input = json!({"id": id, "source": source});
                self.inner.tel.event("voice.download", None, true, input, json!({"skipped": skipped}), None);
                return;
            }
            st.states[i] = VoiceState::Downloading { done_bytes: 0, total_bytes: self.inner.defs[i].total_bytes() };
        }
        let _ = self.inner.events.send(CoreEvent::VoicesUpdated);
        self.spawn(i, 0);
    }

    fn spawn(&self, i: usize, from_pct: u64) {
        let v = self.clone();
        let spawned = std::thread::Builder::new().name("tellal-voice-download".into()).spawn(move || {
            if v.inner.demo { v.simulate(i, from_pct) } else { v.fetch_logged(i) }
        });
        if let Err(e) = spawned {
            self.set(i, VoiceState::Failed(format!("download thread: {e}")));
        }
    }

    fn fetch_logged(&self, i: usize) {
        let def = &self.inner.defs[i];
        let total = def.total_bytes();
        let start = Instant::now();
        let res = fetch(def, &self.inner.root, &mut |done| {
            self.set(i, VoiceState::Downloading { done_bytes: done, total_bytes: total });
        });
        let wall = ms(start.elapsed());
        let input = json!({"id": def.id, "dir": self.dir(def), "total_bytes": total,
                           "files": def.files.iter().map(|f| json!({"name": f.name, "url": f.url, "size": f.size})).collect::<Vec<_>>()});
        match res {
            Ok(()) if installed(&self.inner.root, def) => {
                let out = json!({"mb_per_s": (total as f64 / 1e6 / (wall / 1000.0).max(0.001) * 100.0).round() / 100.0});
                self.inner.tel.event("voice.download", Some(wall), true, input, out, None);
                self.set(i, VoiceState::Ready);
            }
            res => {
                let err = res.err().map_or_else(|| "files incomplete after download".to_owned(), |e| format!("{e:#}"));
                self.inner.tel.event("voice.download", Some(wall), false, input, Value::Null, Some(&err));
                self.set(i, VoiceState::Failed(err));
            }
        }
    }

    /// Demo: progress from `from_pct` to Ready over the rest of [`DEMO_DOWNLOAD`], or a failure
    /// halfway when the fixture says `download_fails` (once).
    fn simulate(&self, i: usize, from_pct: u64) {
        let def = &self.inner.defs[i];
        let total = def.total_bytes();
        let start = Instant::now();
        let fails = std::mem::take(&mut lock(&self.inner.st).fail_next[i]);
        let steps = (DEMO_DOWNLOAD.as_millis() / PROGRESS_EVERY.as_millis()) as u64;
        let input = json!({"id": def.id, "demo": true, "from_pct": from_pct, "total_bytes": total});
        for k in (from_pct * steps / 100)..=steps {
            if fails && k * 2 >= steps {
                let err = "download failed (demo)";
                self.inner.tel.event("voice.download", Some(ms(start.elapsed())), false, input, Value::Null, Some(err));
                self.set(i, VoiceState::Failed(err.to_owned()));
                return;
            }
            self.set(i, VoiceState::Downloading { done_bytes: total * k / steps, total_bytes: total });
            std::thread::sleep(PROGRESS_EVERY);
        }
        self.inner.tel.event("voice.download", Some(ms(start.elapsed())), true, input, json!({"skipped_network": true}), None);
        self.set(i, VoiceState::Ready);
    }

    /// Stores a state; `VoicesUpdated` for every change except progress faster than
    /// [`PROGRESS_EVERY`]; Ready also runs the `on_ready` waker.
    fn set(&self, i: usize, state: VoiceState) {
        let mut st = lock(&self.inner.st);
        let progress = matches!(state, VoiceState::Downloading { .. });
        let now = Instant::now();
        let emit = !progress || st.last_progress_event.is_none_or(|t| now.duration_since(t) >= PROGRESS_EVERY);
        if progress && emit {
            st.last_progress_event = Some(now);
        }
        let ready = state == VoiceState::Ready;
        st.states[i] = state;
        let waker = ready.then(|| st.on_ready.clone()).flatten();
        drop(st);
        if emit {
            let _ = self.inner.events.send(CoreEvent::VoicesUpdated);
        }
        if let Some(w) = waker {
            w();
        }
    }
}

fn state_label(s: &VoiceState) -> String {
    match s {
        VoiceState::Ready => "ready".to_owned(),
        VoiceState::Missing => "missing".to_owned(),
        VoiceState::Downloading { done_bytes, total_bytes } => {
            format!("downloading {}%", done_bytes * 100 / (*total_bytes).max(1))
        }
        VoiceState::Failed(e) => format!("failed: {e}"),
    }
}

/// Every file present with its expected size.
fn installed(root: &Path, def: &VoiceDef) -> bool {
    let dir = root.join(def.dir);
    def.files.iter().all(|f| std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.is_file() && m.len() == f.size))
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .user_agent(concat!("tellal/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Downloads every file of `def` into `<root>/.<dir>.part` (size and SHA-256 checked while
/// streaming), then swaps it in as `<root>/<dir>`. `progress` gets the bytes done so far.
fn fetch(def: &VoiceDef, root: &Path, progress: &mut dyn FnMut(u64)) -> anyhow::Result<()> {
    std::fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let part = root.join(format!(".{}.part", def.dir));
    if part.exists() {
        std::fs::remove_dir_all(&part).with_context(|| format!("remove stale {}", part.display()))?;
    }
    std::fs::create_dir_all(&part).with_context(|| format!("create {}", part.display()))?;
    if let Err(e) = fetch_files(def, &part, progress) {
        let _ = std::fs::remove_dir_all(&part);
        return Err(e);
    }
    let dest = root.join(def.dir);
    if dest.exists() {
        let old = root.join(format!(".{}.old", def.dir));
        let _ = std::fs::remove_dir_all(&old);
        std::fs::rename(&dest, &old).with_context(|| format!("move aside {}", dest.display()))?;
        std::fs::rename(&part, &dest).with_context(|| format!("install {}", dest.display()))?;
        let _ = std::fs::remove_dir_all(&old);
    } else {
        std::fs::rename(&part, &dest).with_context(|| format!("install {}", dest.display()))?;
    }
    Ok(())
}

fn fetch_files(def: &VoiceDef, dir: &Path, progress: &mut dyn FnMut(u64)) -> anyhow::Result<()> {
    let agent = agent();
    let mut buf = vec![0u8; READ_BUF];
    let mut done = 0u64;
    for f in def.files {
        let resp = agent.get(f.url).call().with_context(|| format!("{}: GET {}", f.name, f.url))?;
        let mut body = resp.into_body().into_reader();
        let path = dir.join(f.name);
        let mut out = std::fs::File::create(&path).with_context(|| format!("create {}", path.display()))?;
        let mut hash = hmac_sha256::Hash::new();
        let mut got = 0u64;
        loop {
            let n = body.read(&mut buf).with_context(|| format!("{}: read", f.name))?;
            if n == 0 {
                break;
            }
            got += n as u64;
            if got > f.size {
                bail!("{}: more than the expected {} bytes", f.name, f.size);
            }
            out.write_all(&buf[..n]).with_context(|| format!("write {}", path.display()))?;
            hash.update(&buf[..n]);
            done += n as u64;
            progress(done);
        }
        out.sync_all().with_context(|| format!("sync {}", path.display()))?;
        if got != f.size {
            bail!("{}: {got} bytes, expected {}", f.name, f.size);
        }
        let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if hex != f.sha256 {
            bail!("{}: sha256 mismatch (got {hex}, expected {})", f.name, f.sha256);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::BufRead;
    use std::net::TcpListener;

    use super::*;

    /// Serves `body` for every request on 127.0.0.1 (HTTP/1.1, one response per connection).
    fn serve(body: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = std::io::BufReader::new(&stream);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let mut w = &stream;
                let _ = w.write_all(head.as_bytes()).and_then(|()| w.write_all(body));
            }
        });
        format!("http://{addr}/file")
    }

    /// One local server for every test; its URL lives in a static, so the registries below can
    /// borrow it for `'static` like the real one.
    static URL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| serve(b"hello"));
    const BAD_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    /// sha256("hello")
    const GOOD_SHA: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    static BAD_FILES: std::sync::LazyLock<[ModelFile; 1]> =
        std::sync::LazyLock::new(|| [ModelFile { name: "model.onnx", url: URL.as_str(), size: 5, sha256: BAD_SHA }]);
    static GOOD_FILES: std::sync::LazyLock<[ModelFile; 1]> =
        std::sync::LazyLock::new(|| [ModelFile { name: "model.onnx", url: URL.as_str(), size: 5, sha256: GOOD_SHA }]);
    static BAD: std::sync::LazyLock<[VoiceDef; 1]> = std::sync::LazyLock::new(|| [test_def(&*BAD_FILES)]);
    static GOOD: std::sync::LazyLock<[VoiceDef; 1]> = std::sync::LazyLock::new(|| [test_def(&*GOOD_FILES)]);

    fn test_def(files: &'static [ModelFile]) -> VoiceDef {
        VoiceDef { id: "en-test", lang: Lang::En, name: "Test", dir: "test-voice", files }
    }

    fn wait_settled(v: &Voices) -> VoiceState {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let s = v.views()[0].state.clone();
            if !matches!(s, VoiceState::Downloading { .. }) || Instant::now() > deadline {
                return s;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn setup(defs: &'static [VoiceDef], name: &str) -> (Voices, PathBuf, crossbeam_channel::Receiver<CoreEvent>) {
        let base = std::env::temp_dir().join(format!("tellal-voices-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let tel = Telemetry::create(&base.join("logs")).expect("telemetry");
        let (tx, rx) = crossbeam_channel::unbounded();
        let root = base.join("models");
        (Voices::new(tel, tx, root.clone(), defs, None).expect("voices"), base, rx)
    }

    #[test]
    fn sha256_mismatch_fails_and_installs_nothing() {
        let (v, base, rx) = setup(&*BAD, "bad");
        assert_eq!(v.views()[0].state, VoiceState::Missing);
        v.download("en-test", "user");
        match wait_settled(&v) {
            VoiceState::Failed(e) => assert!(e.contains("sha256 mismatch"), "{e}"),
            s => panic!("expected Failed, got {s:?}"),
        }
        assert!(!base.join("models/test-voice").exists());
        assert!(!base.join("models/.test-voice.part").exists());
        assert!(rx.try_iter().any(|e| matches!(e, CoreEvent::VoicesUpdated)));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn verified_download_turns_ready() {
        let (v, base, _rx) = setup(&*GOOD, "good");
        let woke = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let w = woke.clone();
        v.on_ready(move || w.store(true, std::sync::atomic::Ordering::SeqCst));
        v.download("en-test", "user");
        assert_eq!(wait_settled(&v), VoiceState::Ready);
        assert_eq!(std::fs::read(base.join("models/test-voice/model.onnx")).expect("installed"), b"hello");
        assert!(woke.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(v.pick(Lang::Tr).map(|d| d.id), Some("en-test"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn registry_is_consistent() {
        for d in &VOICES {
            assert!(d.files.iter().all(|f| f.url.starts_with("https://huggingface.co/") && f.sha256.len() == 64));
        }
        assert_eq!(VOICES.iter().map(|d| (d.total_bytes() as f64 / 1e6).round() as u32).collect::<Vec<_>>(), [35, 96]);
        assert!(parse_demo_state("downloading:40").is_ok() && parse_demo_state("downloading:140").is_err());
        assert!(matches!(parse_demo_state("failed:disk full"), Ok(DemoStart::State(VoiceState::Failed(m))) if m == "disk full"));
    }
}
