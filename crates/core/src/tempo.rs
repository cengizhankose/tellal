//! Speech speed by time-stretching (WSOLA): the voice speaks at its normal pace and the samples
//! are shortened by `speed` without changing pitch. Each output frame (Hann window, 50 %
//! overlap) is taken from around its nominal input position, shifted by up to [`SEEK_MS`] to
//! where it best continues the previous frame (cross-correlation), so periods line up and no
//! phasing or clicks appear. Whisper read-back of the bench corpora: asking the models for
//! shorter durations instead dropped words (WER 0.52 English / 0.55 Turkish at 2×).

use std::f32::consts::TAU;

const FRAME_MS: usize = 25;
const SEEK_MS: usize = 10;
/// The search first runs on sums of this many samples, then refines around the best spot
/// sample by sample.
const DECIMATE: usize = 4;

/// `input` played `speed` times faster (`len / speed` samples, same pitch). Every output sample
/// is a convex mix of input samples, so the peak never grows. Returned unchanged at 1.0 or when
/// shorter than two frames.
pub(crate) fn stretch(input: Vec<f32>, sample_rate: u32, speed: f32) -> Vec<f32> {
    let n = (sample_rate as usize * FRAME_MS / 1000) & !1;
    let hop = n / 2;
    if speed == 1.0 || n == 0 || input.len() < 2 * n {
        return input;
    }
    let seek = sample_rate as usize * SEEK_MS / 1000;
    let last = input.len() - n;
    let window: Vec<f32> = (0..n).map(|i| 0.5 - 0.5 * (TAU * i as f32 / n as f32).cos()).collect();
    let coarse: Vec<f32> = input.chunks(DECIMATE).map(|c| c.iter().sum()).collect();
    let out_len = (input.len() as f64 / f64::from(speed)).round() as usize;
    let mut out = vec![0f32; out_len + n];
    let mut weight = vec![0f32; out_len + n];
    let mut prev = 0usize;
    let mut k = 0usize;
    while k * hop < out_len {
        let nominal = ((k * hop) as f64 * f64::from(speed)).round() as usize;
        let pos = if k == 0 {
            0
        } else {
            best_match(&input, &coarse, (prev + hop).min(last), nominal.min(last), seek, n, last)
        };
        let at = k * hop;
        for i in 0..n {
            out[at + i] += input[pos + i] * window[i];
            weight[at + i] += window[i];
        }
        prev = pos;
        k += 1;
    }
    out.truncate(out_len);
    // Inside, the windows sum to 1; at both ends this undoes the half window.
    for (s, w) in out.iter_mut().zip(&weight) {
        if *w > 1e-3 {
            *s /= w;
        }
    }
    out
}

/// The start in `[nominal - seek, nominal + seek]` (within `0..=last`) whose frame best matches
/// the frame at `natural` (cross-correlation): coarse on `coarse` (sums of [`DECIMATE`]
/// samples), then exact within ±[`DECIMATE`] samples of the coarse winner.
fn best_match(
    input: &[f32],
    coarse: &[f32],
    natural: usize,
    nominal: usize,
    seek: usize,
    n: usize,
    last: usize,
) -> usize {
    let (lo, hi) = (nominal.saturating_sub(seek), (nominal + seek).min(last));
    let m = n / DECIMATE;
    let target = &coarse[natural / DECIMATE..][..m];
    let mut best = (f32::MIN, lo / DECIMATE);
    for c in lo / DECIMATE..=hi / DECIMATE {
        let corr = dot(&coarse[c..c + m], target);
        if corr > best.0 {
            best = (corr, c);
        }
    }
    let center = best.1 * DECIMATE;
    let target = &input[natural..natural + n];
    let mut best = (f32::MIN, center.clamp(lo, hi));
    for c in center.saturating_sub(DECIMATE).max(lo)..=(center + DECIMATE).min(hi) {
        let corr = dot(&input[c..c + n], target);
        if corr > best.0 {
            best = (corr, c);
        }
    }
    best.1
}

/// Dot product with eight independent sums (lets the compiler vectorize it).
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let (ca, cb) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = ca.remainder().iter().zip(cb.remainder()).map(|(x, y)| x * y).sum();
    let mut acc = [0f32; 8];
    for (x, y) in ca.zip(cb) {
        for ((s, x), y) in acc.iter_mut().zip(x).zip(y) {
            *s += x * y;
        }
    }
    acc.iter().sum::<f32>() + tail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_scales_and_a_tone_keeps_its_pitch() {
        let sr = 16_000;
        let frame = sr * FRAME_MS / 1000;
        let tone: Vec<f32> = (0..sr).map(|i| (TAU * 200.0 * i as f32 / sr as f32).sin() * 0.5).collect();
        assert_eq!(stretch(tone.clone(), sr as u32, 1.0), tone);
        for speed in [1.25f32, 1.5, 2.0] {
            let out = stretch(tone.clone(), sr as u32, speed);
            assert!(out.len().abs_diff((sr as f32 / speed) as usize) <= frame, "speed {speed}: {} samples", out.len());
            // Same 200 Hz: ~400 zero crossings per second, and the level stays put.
            let crossings = out.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f32;
            let per_s = crossings * sr as f32 / out.len() as f32;
            assert!((per_s - 400.0).abs() < 12.0, "speed {speed}: {per_s} crossings/s");
            let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
            assert!((peak - 0.5).abs() < 0.05, "speed {speed}: peak {peak}");
        }
    }

    #[test]
    fn speech_like_input_stays_finite_and_under_its_peak() {
        let sr = 44_100u32;
        // Gliding pitch, a loud burst and silence, like a sentence with a pause.
        let input: Vec<f32> = (0..sr as usize * 2)
            .map(|i| {
                let t = i as f32 / sr as f32;
                let env = if (0.8..1.1).contains(&t) { 0.0 } else if t < 0.4 { 0.95 } else { 0.4 };
                env * (TAU * (120.0 + 80.0 * t) * t).sin() * (0.6 + 0.4 * (TAU * 3.0 * t).sin().abs())
            })
            .collect();
        let peak_in = input.iter().fold(0f32, |m, s| m.max(s.abs()));
        for speed in [0.75f32, 1.5, 2.0] {
            let out = stretch(input.clone(), sr, speed);
            assert!(out.iter().all(|s| s.is_finite()), "speed {speed}: NaN/inf");
            let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
            assert!(peak <= peak_in + 1e-6, "speed {speed}: peak {peak} > input {peak_in}");
        }
    }
}
