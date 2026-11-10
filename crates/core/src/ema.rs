//! EMA Lightning (Turkish-only, 8.6M params, Apache-2.0) on ONNX Runtime: written Turkish in,
//! 48 kHz mono audio out.
//!
//! A port of `ema_lightning`'s `frontend.py` / `chunker.py` / `engine.py` as mirrored for ONNX by
//! huggingface.co/ozcancelik/ema-lightning-onnx (`web/tts.js`, checked by `export/verify.py`):
//! normalizer-tr reads numbers, dates, units and symbols aloud, the alphabet step maps text to the
//! model's letters, the chunker cuts ≤ 10 s pieces, and each piece runs `text.onnx` (letter
//! features + durations) → frame plan → `sound.onnx` (4 flow steps from seeded noise) →
//! `decoder.onnx` one window at a time.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::Context;
use normalizer_tr::{AmbiguityPolicy, NormalizeOptions, Normalizer};
use ort::environment::Environment;
use ort::session::Session;
use ort::session::builder::AutoDevicePolicy;
use ort::value::Tensor;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Model files inside the model dir.
pub const FILES: [&str; 4] = ["meta.json", "text.onnx", "sound.onnx", "decoder.onnx"];

const MAX_WORD_FRAMES: f64 = 250.0;
const MAX_FRAMES: usize = 3000;
/// A piece's first decoded window is one second, later ones four (as the reference streams).
const FIRST_WINDOW: usize = 25;
const WINDOW: usize = 100;
/// Frames decoded on each side of a window.
const CONTEXT: usize = 8;
const LETTERS_PER_SECOND: f64 = 18.0;
const MAX_SECONDS: f64 = 10.0;
const MAX_LETTERS: usize = 250;
const SENTENCE_PAUSE: f64 = 0.25;
const CLAUSE_PAUSE: f64 = 0.12;
const BLOCK_BYTES: usize = 8 * 1024;
const TURKISH: &str = "çğıöşüÇĞİÖŞÜ";
/// The model's audio ends abruptly; a short fade-out avoids the click.
const FADE_OUT_MS: u32 = 15;
const SPEED: f64 = 1.0;
const SEED: u32 = 0;

#[derive(Deserialize)]
struct Meta {
    vocab: Vec<String>,
    times: Vec<f32>,
    latent_dim: usize,
    hop: usize,
    sample_rate: u32,
}

/// One synthesized text: what the model actually read, and the audio.
pub struct Spoken {
    /// Text after normalizer-tr and the alphabet step (the model's input).
    pub text: String,
    pub pieces: usize,
    pub samples: Vec<f32>,
}

pub struct Ema {
    text: Session,
    sound: Session,
    decoder: Session,
    stoi: HashMap<char, i64>,
    steps: usize,
    latent_dim: usize,
    hop: usize,
    pub sample_rate: u32,
    normalizer: Option<Normalizer>,
    /// Milliseconds spent per load step (`text`, `sound`, `decoder`, `normalizer`).
    pub load_ms: Vec<(&'static str, f64)>,
}

/// ONNX Runtime allows one environment per process.
static ENV: LazyLock<Result<Environment, String>> =
    LazyLock::new(|| ort::init().with_name("t3-panel").build().map_err(|e| e.to_string()));

impl Ema {
    pub fn load(model_dir: &Path, threads: usize) -> anyhow::Result<Ema> {
        let meta: Meta = serde_json::from_slice(
            &std::fs::read(model_dir.join("meta.json"))
                .with_context(|| format!("{}/meta.json (run scripts/fetch-models.sh)", model_dir.display()))?,
        )?;
        let env = ENV.as_ref().map_err(|e| anyhow::anyhow!("onnxruntime init: {e}"))?;
        // `ort` defaults to `AutoDevicePolicy::MaxEfficiency`, which on macOS registers CoreML: it
        // kept ~230 MB (sound) + ~130 MB (decoder) of working memory per session. Plain CPU keeps
        // a few MB and is fast enough (RTF ~0.03).
        let session = |name: &str| -> anyhow::Result<Session> {
            let path = model_dir.join(name);
            Session::builder(env)?
                .with_auto_device(AutoDevicePolicy::PreferCPU)
                .and_then(|b| b.with_intra_threads(threads))
                .and_then(|b| b.with_inter_threads(1))
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .commit_from_file(&path)
                .with_context(|| format!("load {}", path.display()))
        };
        let mut load_ms = Vec::with_capacity(4);
        let mut timed = |step: &'static str, t: Instant| load_ms.push((step, t.elapsed().as_secs_f64() * 1000.0));
        let stoi = meta
            .vocab
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Some((c, i as i64)),
                    _ => None,
                }
            })
            .collect();
        let t = Instant::now();
        let text = session("text.onnx")?;
        timed("text", t);
        let t = Instant::now();
        let sound = session("sound.onnx")?;
        timed("sound", t);
        let t = Instant::now();
        let decoder = session("decoder.onnx")?;
        timed("decoder", t);
        let t = Instant::now();
        let normalizer = Normalizer::new().ok();
        timed("normalizer", t);
        Ok(Ema {
            text,
            sound,
            decoder,
            stoi,
            steps: meta.times.len(),
            latent_dim: meta.latent_dim,
            hop: meta.hop,
            sample_rate: meta.sample_rate,
            normalizer,
            load_ms,
        })
    }

    /// Written Turkish → the model's alphabet (`frontend.py`). Never fails: notation the
    /// normalizer cannot resolve is kept as written and unknown letters become spaces.
    pub fn frontend(&self, text: &str) -> String {
        let text = unsafe_to_space(text);
        if text.trim().is_empty() {
            return String::new();
        }
        let spoken: Vec<String> = blocks(&text).iter().map(|b| self.normalize(b)).collect();
        alphabet(&spoken.join(" "), &self.stoi)
    }

    fn normalize(&self, block: &str) -> String {
        let options = NormalizeOptions { ambiguity_policy: AmbiguityPolicy::Fallback, ..Default::default() };
        match self.normalizer.as_ref().map(|n| n.normalize(block, &options)) {
            Some(Ok(result)) => result.normalized_text().to_owned(),
            _ => block.to_owned(),
        }
    }

    /// Synthesizes `text` (any length) into one buffer, pauses between pieces included.
    pub fn speak(&mut self, text: &str) -> anyhow::Result<Spoken> {
        let spoken = self.frontend(text);
        let pieces = chunk(&spoken, SPEED);
        let mut samples = Vec::new();
        for (k, (piece, pause)) in pieces.iter().enumerate() {
            self.piece(piece, k as u32, &mut samples)?;
            samples.resize(samples.len() + (pause * f64::from(self.sample_rate)).round() as usize, 0.0);
        }
        fade_out(&mut samples, (self.sample_rate * FADE_OUT_MS / 1000) as usize);
        Ok(Spoken { text: spoken, pieces: pieces.len(), samples })
    }

    fn piece(&mut self, piece: &str, k: u32, out: &mut Vec<f32>) -> anyhow::Result<()> {
        let chars: Vec<char> = piece.chars().collect();
        let ids: Vec<i64> = chars.iter().map(|c| self.stoi.get(c).copied().unwrap_or(1)).collect();
        let (cw, wstart) = words(&chars);
        let l = ids.len();

        let text_out = self.text.run(ort::inputs!["ids" => Tensor::from_array(([1, l], ids))?])?;
        let (h_shape, h) = text_out["h"].try_extract_tensor::<f32>()?;
        let h_shape: Vec<i64> = h_shape.iter().copied().collect();
        let h = h.to_vec();
        let (_, dur) = text_out["dur"].try_extract_tensor::<f32>()?;
        let plan = plan(&cw, &wstart, dur, SPEED);
        drop(text_out);

        let t = plan.fw.len();
        let noise = gaussian(SEED.wrapping_mul(1_000_003).wrapping_add(k), self.steps * t * self.latent_dim);
        let sound_out = self.sound.run(ort::inputs![
            "h" => Tensor::from_array((h_shape, h))?,
            "cg" => Tensor::from_array(([1, l], plan.cg))?,
            "fg" => Tensor::from_array(([1, t], plan.fg))?,
            "cw" => Tensor::from_array(([1, l], cw))?,
            "fw" => Tensor::from_array(([1, t], plan.fw))?,
            "noise" => Tensor::from_array(([1, self.steps, t, self.latent_dim], noise))?,
        ])?;
        let (_, lat) = sound_out["latents"].try_extract_tensor::<f32>()?;
        let lat = lat.to_vec(); // [T, D]
        drop(sound_out);

        let d = self.latent_dim;
        for (s, e) in windows(t, if k == 0 { FIRST_WINDOW } else { WINDOW }) {
            let a = s.saturating_sub(CONTEXT);
            let b = (e + CONTEXT).min(t);
            let n = b - a;
            let mut z = vec![0f32; d * n]; // [1, D, n]
            for f in 0..n {
                for j in 0..d {
                    z[j * n + f] = lat[(a + f) * d + j];
                }
            }
            let dec = self.decoder.run(ort::inputs!["z" => Tensor::from_array(([1, d, n], z))?])?;
            let (_, audio) = dec["audio"].try_extract_tensor::<f32>()?;
            let from = ((s - a) * self.hop).min(audio.len());
            let to = ((e - a) * self.hop).min(audio.len());
            out.extend_from_slice(&audio[from..to]);
        }
        Ok(())
    }
}

/// Bidi controls and C0/C1 control characters (except tab/newline) become spaces.
fn unsafe_to_space(text: &str) -> String {
    text.chars()
        .map(|c| {
            let unsafe_char = matches!(c, '\x00'..='\x08' | '\x0b'..='\x1f' | '\x7f'..='\u{9f}')
                || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
            if unsafe_char { ' ' } else { c }
        })
        .collect()
}

/// Whitespace-split blocks the normalizer accepts in one call.
fn blocks(text: &str) -> Vec<String> {
    let (mut out, mut block, mut size) = (Vec::new(), Vec::new(), 0);
    for word in text.split_whitespace() {
        let n = word.len() + 1;
        if !block.is_empty() && size + n > BLOCK_BYTES {
            out.push(block.join(" "));
            block.clear();
            size = 0;
        }
        block.push(word);
        size += n;
    }
    if !block.is_empty() {
        out.push(block.join(" "));
    }
    out
}

fn alphabet(text: &str, vocab: &HashMap<char, i64>) -> String {
    let typo = |c: char| match c {
        '’' | '‘' | 'ʼ' | '´' | '`' => "'".to_owned(),
        '“' | '”' | '„' | '«' | '»' => "\"".to_owned(),
        '–' | '—' | '−' => "-".to_owned(),
        '…' => "...".to_owned(),
        'İ' => "i".to_owned(),
        'I' => "ı".to_owned(),
        c => c.to_lowercase().collect(),
    };
    let mut out = String::with_capacity(text.len());
    for ch in text.chars().flat_map(|c| typo(c).chars().collect::<Vec<_>>()) {
        let mapped: String =
            if TURKISH.contains(ch) { ch.to_string() } else { ch.to_string().nfkd().filter(|c| !is_combining_mark(*c)).collect() };
        if !mapped.is_empty() && mapped.chars().all(|c| vocab.contains_key(&c)) {
            out.push_str(&mapped);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Greedy chunking (`chunker.py`): pieces of ≤ ~10 s, cut at the last sentence end, else clause
/// mark, else space inside the window; each piece ends on terminal punctuation.
fn chunk(text: &str, speed: f64) -> Vec<(String, f64)> {
    let limit = (MAX_LETTERS as f64).min(LETTERS_PER_SECOND * MAX_SECONDS * speed) as usize;
    let mut pieces = Vec::new();
    let mut rest: Vec<char> = text.trim().chars().collect();
    while !rest.is_empty() {
        let (mut cut, mut pause) = (rest.len(), 0.0);
        if rest.len() > limit {
            cut = limit;
            let cuts: [(fn(&[char], usize) -> bool, f64); 3] =
                [(sentence_end, SENTENCE_PAUSE), (clause_end, CLAUSE_PAUSE), (word_end, CLAUSE_PAUSE)];
            for (is_end, gap) in cuts {
                // `finditer(rest, 0, limit + 1)` with `(?= )`: the space must sit at index ≤ limit.
                if let Some(e) = (1..=limit.min(rest.len() - 1)).rev().find(|&e| rest[e] == ' ' && is_end(&rest, e)) {
                    cut = e;
                    pause = gap;
                    break;
                }
            }
        }
        let piece: String = rest[..cut].iter().collect::<String>().trim().to_owned();
        rest = rest[cut..].iter().collect::<String>().trim().chars().collect();
        if piece.chars().any(char::is_alphabetic) {
            pieces.push((finish(&piece), pause));
        }
    }
    if let Some(last) = pieces.last_mut() {
        last.1 = 0.0;
    }
    pieces
}

/// `[.!?]+["')]*` ends right before index `e`.
fn sentence_end(s: &[char], e: usize) -> bool {
    let mut i = e;
    while i > 0 && matches!(s[i - 1], '"' | '\'' | ')') {
        i -= 1;
    }
    i > 0 && matches!(s[i - 1], '.' | '!' | '?')
}

fn clause_end(s: &[char], e: usize) -> bool {
    matches!(s[e - 1], ',' | ';' | ':')
}

fn word_end(s: &[char], e: usize) -> bool {
    !s[e - 1].is_whitespace()
}

fn finish(piece: &str) -> String {
    let core = piece.trim_end_matches(['"', '\'', ')']);
    if core.ends_with(['.', '!', '?']) {
        return piece.to_owned();
    }
    format!("{}.", piece.trim_end_matches([',', ';', ':', '-', ' ']))
}

/// Word index of each letter and the first letter of that word (a space belongs to the next word).
fn words(text: &[char]) -> (Vec<i64>, Vec<usize>) {
    let mut starts: Vec<usize> =
        (0..text.len()).filter(|&i| text[i] != ' ' && (i == 0 || text[i - 1] == ' ')).collect();
    if starts.is_empty() {
        starts.push(0);
    }
    let mut bounds = vec![0];
    bounds.extend_from_slice(&starts[1..]);
    bounds.push(text.len());
    let (mut cw, mut wstart) = (Vec::with_capacity(text.len()), Vec::with_capacity(text.len()));
    for (w, pair) in bounds.windows(2).enumerate() {
        for _ in pair[0]..pair[1] {
            cw.push(w as i64);
            wstart.push(pair[0]);
        }
    }
    (cw, wstart)
}

struct Plan {
    fw: Vec<i64>,
    cg: Vec<f32>,
    fg: Vec<f32>,
}

/// Frame timeline from the letter durations (`engine.plan` + the aligner positions of
/// `sound_stage`, as in `web/tts.js`).
fn plan(cw: &[i64], wstart: &[usize], dur: &[f32], speed: f64) -> Plan {
    let l = cw.len();
    let n_words = cw[l - 1] as usize + 1;
    let d: Vec<f64> = dur.iter().map(|&x| f64::from(x) / speed).collect();
    let mut sums = vec![0f64; n_words];
    for i in 0..l {
        sums[cw[i] as usize] += d[i];
    }
    let counts: Vec<usize> = sums.iter().map(|s| s.round().clamp(1.0, MAX_WORD_FRAMES) as usize).collect();
    let t = counts.iter().sum::<usize>().min(MAX_FRAMES);
    let (mut fw, mut fp) = (Vec::with_capacity(t), Vec::with_capacity(t));
    'words: for (w, &n) in counts.iter().enumerate() {
        for j in 0..n {
            if fw.len() == t {
                break 'words;
            }
            fw.push(w as i64);
            fp.push(j as f32 / n as f32);
        }
    }
    let c: Vec<f32> = d.iter().map(|&x| (x as f32).max(1e-4)).collect();
    let (mut done, mut total) = (vec![0f32; l], vec![0f32; n_words]);
    let mut acc = 0f32;
    for i in 0..l {
        acc += c[i];
        done[i] = acc;
        total[cw[i] as usize] += c[i];
    }
    let mut wlen = vec![0f32; n_words];
    for &w in cw {
        wlen[w as usize] += 1.0;
    }
    let mut woff = vec![0f32; n_words];
    for w in 1..n_words {
        woff[w] = woff[w - 1] + wlen[w - 1].max(1.0);
    }
    let cg = (0..l)
        .map(|i| {
            let w = cw[i] as usize;
            let before = done[wstart[i]] - c[wstart[i]];
            let cp = ((done[i] - before - 0.5 * c[i]) / total[w].max(1e-8)).clamp(0.0, 1.0);
            woff[w] + cp * wlen[w].max(1.0)
        })
        .collect();
    let fg = (0..t).map(|f| woff[fw[f] as usize] + fp[f] * wlen[fw[f] as usize].max(1.0)).collect();
    Plan { fw, cg, fg }
}

fn windows(frames: usize, first: usize) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut s = 0;
    while s < frames {
        let e = frames.min(s + if s == 0 { first } else { WINDOW });
        spans.push((s, e));
        s = e;
    }
    spans
}

/// Seeded standard normal noise (mulberry32 + Box–Muller, as `web/tts.js`).
fn gaussian(seed: u32, n: usize) -> Vec<f32> {
    let mut a = seed;
    let mut uniform = || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    };
    let mut out = vec![0f32; n];
    let mut i = 0;
    while i < n {
        let r = (-2.0 * (1.0 - uniform()).ln()).sqrt();
        let th = 2.0 * std::f64::consts::PI * uniform();
        out[i] = (r * th.cos()) as f32;
        if i + 1 < n {
            out[i + 1] = (r * th.sin()) as f32;
        }
        i += 2;
    }
    out
}

fn fade_out(samples: &mut [f32], len: usize) {
    let len = len.min(samples.len());
    let start = samples.len() - len;
    for (k, s) in samples[start..].iter_mut().enumerate() {
        *s *= 1.0 - (k + 1) as f32 / len as f32;
    }
}
