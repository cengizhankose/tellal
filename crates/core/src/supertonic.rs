//! Supertonic 2 (Supertone; English voice `M1`; OpenRAIL-M weights) on ONNX Runtime: written
//! English in (after `english::normalize`), 44.1 kHz mono audio out.
//!
//! A port of the reference `rust/src/helper.rs` (github.com/supertone-oss-archive/supertonic, MIT):
//! characters → `unicode_indexer.json` ids inside `<en>…</en>`, `duration_predictor` (seconds),
//! `text_encoder`, [`STEPS`] flow-matching steps of `vector_estimator` from seeded noise, then
//! `vocoder`. The vector estimator and vocoder are sherpa-onnx's int8 quantization
//! (huggingface.co/csukuangfj2/sherpa-onnx-supertonic-tts-int8-2026-03-06); every other file is
//! Supertone's own (huggingface.co/Supertone/supertonic-2).

use std::path::Path;
use std::time::Instant;

use anyhow::Context;
use ort::session::Session;
use ort::value::Tensor;
use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

use crate::ema::{Spoken, cpu_session, fade_out, gaussian};

/// Flow-matching steps. Whisper read-back of the bench corpus: 5 steps swallowed words of short
/// lines ("Running the tests now." → "Running now."), 8 read them all (RTF 0.028 → 0.039).
const STEPS: usize = 8;
const SPEED: f32 = 1.05;
/// The reference splits longer text into pieces of at most this many characters.
const MAX_PIECE_CHARS: usize = 300;
const PIECE_PAUSE_S: f32 = 0.25;
const FADE_OUT_MS: u32 = 15;
const SEED: u32 = 0;
const LANG: &str = "en";

#[derive(Deserialize)]
struct Cfg {
    ae: Ae,
    ttl: Ttl,
}

#[derive(Deserialize)]
struct Ae {
    sample_rate: u32,
    base_chunk_size: usize,
}

#[derive(Deserialize)]
struct Ttl {
    chunk_compress_factor: usize,
    latent_dim: usize,
}

#[derive(Deserialize)]
struct StyleFile {
    style_ttl: Component,
    style_dp: Component,
}

#[derive(Deserialize)]
struct Component {
    data: Vec<Vec<Vec<f32>>>,
    dims: Vec<usize>,
}

impl Component {
    fn tensor(&self) -> anyhow::Result<Tensor<f32>> {
        let flat: Vec<f32> = self.data.iter().flatten().flatten().copied().collect();
        Ok(Tensor::from_array((self.dims.clone(), flat))?)
    }
}

pub struct Supertonic {
    duration: Session,
    text_encoder: Session,
    estimator: Session,
    vocoder: Session,
    /// Unicode code point → model id (`-1` = not in the model's alphabet).
    indexer: Vec<i64>,
    style_ttl: Tensor<f32>,
    style_dp: Tensor<f32>,
    pub sample_rate: u32,
    /// Audio samples per latent frame.
    frame: usize,
    latent_channels: usize,
    /// Milliseconds spent per load step.
    pub load_ms: Vec<(&'static str, f64)>,
}

impl Supertonic {
    /// `dir` holds the files of the `en-supertonic-2` voice (see `voices.rs`).
    pub fn load(dir: &Path, threads: usize) -> anyhow::Result<Supertonic> {
        let read = |name: &str| std::fs::read(dir.join(name)).with_context(|| format!("{}/{name}", dir.display()));
        let mut load_ms = Vec::with_capacity(5);
        let mut timed = |step: &'static str, t: Instant| load_ms.push((step, t.elapsed().as_secs_f64() * 1000.0));
        let t = Instant::now();
        let cfg: Cfg = serde_json::from_slice(&read("tts.json")?).context("tts.json")?;
        let indexer: Vec<i64> = serde_json::from_slice(&read("unicode_indexer.json")?).context("unicode_indexer.json")?;
        let style: StyleFile = serde_json::from_slice(&read("M1.json")?).context("M1.json")?;
        timed("json", t);
        let mut session = |step: &'static str, name: &str| -> anyhow::Result<Session> {
            let t = Instant::now();
            let s = cpu_session(&dir.join(name), threads)?;
            timed(step, t);
            Ok(s)
        };
        Ok(Supertonic {
            duration: session("duration", "duration_predictor.onnx")?,
            text_encoder: session("text_encoder", "text_encoder.onnx")?,
            estimator: session("vector_estimator", "vector_estimator.int8.onnx")?,
            vocoder: session("vocoder", "vocoder.int8.onnx")?,
            indexer,
            style_ttl: style.style_ttl.tensor()?,
            style_dp: style.style_dp.tensor()?,
            sample_rate: cfg.ae.sample_rate,
            frame: cfg.ae.base_chunk_size * cfg.ttl.chunk_compress_factor,
            latent_channels: cfg.ttl.latent_dim * cfg.ttl.chunk_compress_factor,
            load_ms,
        })
    }

    /// Synthesizes `text` (any length) into one buffer, pauses between pieces included.
    pub fn speak(&mut self, text: &str) -> anyhow::Result<Spoken> {
        let prepared = preprocess(text);
        let pieces = pieces(&prepared);
        let mut samples = Vec::new();
        for (k, piece) in pieces.iter().enumerate() {
            if k > 0 {
                samples.resize(samples.len() + (PIECE_PAUSE_S * self.sample_rate as f32) as usize, 0.0);
            }
            self.piece(piece, k as u32, &mut samples)?;
        }
        fade_out(&mut samples, (self.sample_rate * FADE_OUT_MS / 1000) as usize);
        Ok(Spoken { text: prepared, pieces: pieces.len(), samples })
    }

    fn piece(&mut self, piece: &str, k: u32, out: &mut Vec<f32>) -> anyhow::Result<()> {
        let tagged = format!("<{LANG}>{piece}</{LANG}>");
        let ids: Vec<i64> =
            tagged.chars().filter_map(|c| self.indexer.get(c as usize).copied().filter(|&id| id >= 0)).collect();
        let n = ids.len();
        let text_ids = Tensor::from_array(([1, n], ids))?;
        let text_mask = Tensor::from_array(([1, 1, n], vec![1f32; n]))?;

        let dp = self.duration.run(ort::inputs![
            "text_ids" => &text_ids, "style_dp" => &self.style_dp, "text_mask" => &text_mask,
        ])?;
        let seconds = dp["duration"].try_extract_tensor::<f32>()?.1.first().copied().unwrap_or(0.0) / SPEED;
        drop(dp);
        let wav_len = (seconds.max(0.0) * self.sample_rate as f32) as usize;
        let frames = wav_len.div_ceil(self.frame).max(1);

        let enc = self.text_encoder.run(ort::inputs![
            "text_ids" => &text_ids, "style_ttl" => &self.style_ttl, "text_mask" => &text_mask,
        ])?;
        let (emb_shape, emb) = enc["text_emb"].try_extract_tensor::<f32>()?;
        let text_emb = Tensor::from_array((emb_shape.to_vec(), emb.to_vec()))?;
        drop(enc);

        let shape = [1, self.latent_channels, frames];
        let mut latent = gaussian(SEED.wrapping_mul(1_000_003).wrapping_add(k), self.latent_channels * frames);
        let latent_mask = Tensor::from_array(([1, 1, frames], vec![1f32; frames]))?;
        let total = Tensor::from_array(([1], vec![STEPS as f32]))?;
        for step in 0..STEPS {
            let est = self.estimator.run(ort::inputs![
                "noisy_latent" => Tensor::from_array((shape, latent))?,
                "text_emb" => &text_emb,
                "style_ttl" => &self.style_ttl,
                "latent_mask" => &latent_mask,
                "text_mask" => &text_mask,
                "current_step" => Tensor::from_array(([1], vec![step as f32]))?,
                "total_step" => &total,
            ])?;
            latent = est["denoised_latent"].try_extract_tensor::<f32>()?.1.to_vec();
        }

        let voc = self.vocoder.run(ort::inputs!["latent" => Tensor::from_array((shape, latent))?])?;
        let (_, wav) = voc["wav_tts"].try_extract_tensor::<f32>()?;
        out.extend_from_slice(&wav[..wav_len.min(wav.len())]);
        Ok(())
    }
}

/// The reference `preprocess_text` (without the language tags): NFKD, typographic quotes and
/// dashes to ASCII, brackets/slashes/hashes to spaces, spacing before punctuation removed, and a
/// final period when the text ends without punctuation.
fn preprocess(text: &str) -> String {
    let mut s = String::with_capacity(text.len() + 1);
    for c in text.nfkd() {
        match c {
            '–' | '‑' | '—' => s.push('-'),
            '“' | '”' => s.push('"'),
            '‘' | '’' | '´' | '`' => s.push('\''),
            '_' | '[' | ']' | '|' | '/' | '#' | '→' | '←' => s.push(' '),
            '♥' | '☆' | '♡' | '©' | '\\' => {}
            '@' => s.push_str(" at "),
            c if c.is_whitespace() => s.push(' '),
            c => s.push(c),
        }
    }
    let mut out = String::with_capacity(s.len() + 1);
    for word in s.split(' ').filter(|w| !w.is_empty()) {
        let attaches = word.starts_with([',', '.', '!', '?', ';', ':', '\'']);
        if !out.is_empty() && !attaches {
            out.push(' ');
        }
        out.push_str(word);
    }
    if !out.is_empty() && !out.ends_with(['.', '!', '?', ';', ':', ',', '\'', '"', ')', '…']) {
        out.push('.');
    }
    out
}

/// Pieces of at most [`MAX_PIECE_CHARS`] characters, cut after the last sentence end, else
/// comma, else space inside the window.
fn pieces(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while rest.chars().count() > MAX_PIECE_CHARS {
        let window_end = rest.char_indices().nth(MAX_PIECE_CHARS).map_or(rest.len(), |(i, _)| i);
        let window = &rest[..window_end];
        let cut = [". ", "! ", "? ", ", ", "; ", " "]
            .iter()
            .find_map(|sep| window.rfind(sep).map(|i| i + sep.len() - 1))
            .filter(|&i| i > 0)
            .unwrap_or(window_end);
        out.push(rest[..cut].trim().to_owned());
        rest = rest[cut..].trim_start();
    }
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocess_mirrors_the_reference() {
        assert_eq!(preprocess("Build finished — see “notes” , ok"), "Build finished - see \"notes\", ok.");
        assert_eq!(preprocess("a/b  c_d [x]?"), "a b c d x?");
    }

    #[test]
    fn long_text_is_cut_at_sentence_ends() {
        let s = "This sentence has exactly forty-two chars. ".repeat(10);
        let p = pieces(&s);
        assert!(p.len() > 1);
        assert!(p.iter().all(|x| x.chars().count() <= MAX_PIECE_CHARS && x.ends_with('.')));
        assert_eq!(p.join(" "), s.trim());
    }
}
