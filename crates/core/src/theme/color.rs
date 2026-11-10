//! Color math as T3 Code's theme code does it (`themePalette` chunk): sRGB ⇄ OKLab/OKLCH with
//! chroma-reduction gamut mapping, WCAG 2 contrast, T3's lightness search and foreground pick,
//! CSS `color-mix(in oklab, …)`, and parsing of the color strings T3 palettes use.

/// sRGB, channels 0..=255 (unrounded while mixing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

pub const WHITE: Rgb = Rgb { r: 255.0, g: 255.0, b: 255.0 };
pub const BLACK: Rgb = Rgb { r: 0.0, g: 0.0, b: 0.0 };
/// T3's tinted light / dark foregrounds (`ra` / `ia` in the bundle).
const TINT_LIGHT: Rgb = Rgb { r: 255.0, g: 250.0, b: 255.0 };
const TINT_DARK: Rgb = Rgb { r: 36.0, g: 21.0, b: 35.0 };

impl Rgb {
    pub fn from_u32(v: u32) -> Rgb {
        Rgb { r: f64::from((v >> 16) & 0xff), g: f64::from((v >> 8) & 0xff), b: f64::from(v & 0xff) }
    }

    pub fn to_u32(self) -> u32 {
        let c = |x: f64| x.round().clamp(0.0, 255.0) as u32;
        (c(self.r) << 16) | (c(self.g) << 8) | c(self.b)
    }
}

/// `#rrggbb`.
pub fn hex(v: u32) -> String {
    format!("#{v:06x}")
}

fn to_linear(v: f64) -> f64 {
    let t = v / 255.0;
    if t <= 0.04045 { t / 12.92 } else { ((t + 0.055) / 1.055).powf(2.4) }
}

fn from_linear(x: f64) -> f64 {
    let n = if x <= 0.003_130_8 { x * 12.92 } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    n.clamp(0.0, 1.0) * 255.0
}

pub fn to_oklch(c: Rgb) -> Oklch {
    let (r, g, b) = (to_linear(c.r), to_linear(c.g), to_linear(c.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let ll = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let bb = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    Oklch { l: ll, c: a.hypot(bb), h: bb.atan2(a).to_degrees() }
}

fn oklch_linear(o: Oklch) -> [f64; 3] {
    let (a, b) = (o.c * o.h.to_radians().cos(), o.c * o.h.to_radians().sin());
    let l = (o.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (o.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (o.l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}

/// Reduces chroma until the color fits sRGB (T3's gamut mapping), keeping L and h.
fn gamut_map(o: Oklch) -> Oklch {
    let fits = |c: f64| oklch_linear(Oklch { c, ..o }).iter().all(|v| (-1e-4..=1.0001).contains(v));
    if fits(o.c) {
        return o;
    }
    let (mut lo, mut hi, eps) = (0.0, o.c, 1e-6);
    let steps = (o.c.max(eps).log2() - f64::log2(eps)).ceil().max(1.0) as u32;
    for _ in 0..steps {
        let mid = (lo + hi) / 2.0;
        if fits(mid) { lo = mid } else { hi = mid }
    }
    Oklch { c: lo, ..o }
}

/// OKLCH → sRGB rounded to integer channels (T3's `J`).
pub fn from_oklch(o: Oklch) -> Rgb {
    let [r, g, b] = oklch_linear(gamut_map(o));
    Rgb::from_u32(Rgb { r: from_linear(r), g: from_linear(g), b: from_linear(b) }.to_u32())
}

/// WCAG relative luminance (T3 uses the 0.03928 threshold).
pub fn luminance(c: Rgb) -> f64 {
    let ch = |v: f64| {
        let t = v / 255.0;
        if t <= 0.03928 { t / 12.92 } else { ((t + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
}

/// WCAG 2 contrast ratio.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub fn is_dark(c: Rgb) -> bool {
    luminance(c) < 0.179
}

/// Linear sRGB-channel mix `a → b` by `t` (T3's `K`).
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    Rgb { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t }
}

/// CSS `color-mix(in oklab, a <wa>, b)`.
pub fn mix_oklab(a: Rgb, b: Rgb, wa: f64) -> Rgb {
    let lab = |c: Rgb| {
        let o = to_oklch(c);
        (o.l, o.c * o.h.to_radians().cos(), o.c * o.h.to_radians().sin())
    };
    let ((l1, a1, b1), (l2, a2, b2)) = (lab(a), lab(b));
    let (l, aa, bb) = (l1 * wa + l2 * (1.0 - wa), a1 * wa + a2 * (1.0 - wa), b1 * wa + b2 * (1.0 - wa));
    from_oklch(Oklch { l, c: aa.hypot(bb), h: bb.atan2(aa).to_degrees() })
}

/// Moves `o`'s lightness (toward white when `lighter`) until its contrast against every color in
/// `against` reaches `target` (T3's `Y`: 18-step bisection).
pub fn reach(o: Oklch, against: &[Rgb], target: f64, lighter: bool) -> Oklch {
    let worst = |x: Oklch| against.iter().map(|&b| contrast(from_oklch(x), b)).fold(f64::INFINITY, f64::min);
    if worst(o) >= target {
        return o;
    }
    let (mut lo, mut hi) = if lighter { (o.l, 1.0) } else { (0.0, o.l) };
    for _ in 0..18 {
        let mid = (lo + hi) / 2.0;
        let ok = worst(Oklch { l: mid, ..o }) >= target;
        if ok == lighter { hi = mid } else { lo = mid }
    }
    Oklch { l: if lighter { hi } else { lo }, ..o }
}

/// The color between `fg` and `bg` closest to `bg` that keeps `target` contrast against `bg`
/// (T3's `Oa`, used for muted text).
pub fn fade_toward(bg: Rgb, fg: Rgb, target: f64) -> Rgb {
    let (mut best, mut lo, mut hi) = (fg, 0.0, 1.0);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        let c = mix(fg, bg, mid);
        if contrast(c, bg) >= target {
            best = c;
            lo = mid;
        } else {
            hi = mid;
        }
    }
    best
}

/// Readable foreground on `bg`: T3's tinted white/black when ≥ 4.5:1, else pure white/black
/// (T3's `Z`).
pub fn on_color(bg: Rgb) -> Rgb {
    let (l, d) = (contrast(bg, TINT_LIGHT), contrast(bg, TINT_DARK));
    if l.max(d) >= 4.5 {
        if l >= d { TINT_LIGHT } else { TINT_DARK }
    } else if contrast(bg, WHITE) >= contrast(bg, BLACK) {
        WHITE
    } else {
        BLACK
    }
}

/// Parses the color strings found in T3 palettes: `#rgb[a]`, `#rrggbb[aa]`, `oklch(L C h [/ a])`
/// (L as number or %), `rgb()/rgba()` (comma or space syntax), `transparent`. Returns the color
/// and its alpha (0..=1).
pub fn parse(s: &str) -> Option<(Rgb, f64)> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("transparent") {
        return Some((BLACK, 0.0));
    }
    if let Some(h) = s.strip_prefix('#') {
        let n = |i: usize, w: usize| u8::from_str_radix(h.get(i..i + w)?, 16).ok().map(f64::from);
        let (r, g, b, a) = match h.len() {
            3 | 4 => {
                let d = |i| n(i, 1).map(|v| v * 17.0);
                (d(0)?, d(1)?, d(2)?, if h.len() == 4 { d(3)? } else { 255.0 })
            }
            6 | 8 => (n(0, 2)?, n(2, 2)?, n(4, 2)?, if h.len() == 8 { n(6, 2)? } else { 255.0 }),
            _ => return None,
        };
        return Some((Rgb { r, g, b }, a / 255.0));
    }
    let (name, args) = s.strip_suffix(')')?.split_once('(')?;
    let (main, alpha) = match args.split_once('/') {
        Some((m, a)) => (m, Some(a.trim())),
        None => (args, None),
    };
    let parts: Vec<&str> = main.split([' ', ',']).map(str::trim).filter(|p| !p.is_empty()).collect();
    let num = |p: &str, pct_scale: f64| -> Option<f64> {
        if p == "none" {
            return Some(0.0);
        }
        match p.strip_suffix('%') {
            Some(v) => v.parse::<f64>().ok().map(|v| v / 100.0 * pct_scale),
            None => p.trim_end_matches("deg").parse().ok(),
        }
    };
    let alpha_of = |a: Option<&str>| a.map_or(Some(1.0), |a| num(a, 1.0)).map(|a| a.clamp(0.0, 1.0));
    match name.trim().to_ascii_lowercase().as_str() {
        "oklch" if parts.len() == 3 => {
            let o = Oklch { l: num(parts[0], 1.0)?.clamp(0.0, 1.0), c: num(parts[1], 0.4)?.max(0.0), h: num(parts[2], 360.0)? };
            Some((from_oklch(o), alpha_of(alpha)?))
        }
        "rgb" | "rgba" if parts.len() >= 3 => {
            let alpha = alpha.or(parts.get(3).copied());
            let rgb = Rgb { r: num(parts[0], 255.0)?, g: num(parts[1], 255.0)?, b: num(parts[2], 255.0)? };
            Some((rgb, alpha_of(alpha)?))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklch_matches_rendered_t3_pixels() {
        // T3 Chat dark: sidebar row hover and canvas, sampled from the live T3 window (sRGB).
        let (c, a) = parse("oklch(0.23366 0.026081 338.196)").unwrap();
        assert_eq!((c.to_u32(), a), (0x26_1922, 1.0));
        let (c, _) = parse("oklch(0.22813 0.020366 307.469)").unwrap();
        assert_eq!(c.to_u32(), 0x1f_1a24);
        let back = to_oklch(c);
        assert!((back.l - 0.22813).abs() < 0.003, "{back:?}");
    }

    #[test]
    fn parses_css_forms() {
        assert_eq!(parse("#fff").map(|(c, _)| c.to_u32()), Some(0xff_ffff));
        assert_eq!(parse("#11223380").map(|(c, a)| (c.to_u32(), (a * 100.0).round())), Some((0x11_2233, 50.0)));
        assert_eq!(parse("rgb(1, 2, 3)").map(|(c, _)| c.to_u32()), Some(0x01_0203));
        assert_eq!(parse("oklch(100% 0 0 / 0.5)").map(|(c, a)| (c.to_u32(), a)), Some((0xff_ffff, 0.5)));
        assert_eq!(parse("var(--x)"), None);
    }

    #[test]
    fn contrast_and_reach() {
        assert!((contrast(WHITE, BLACK) - 21.0).abs() < 1e-9);
        let bg = Rgb::from_u32(0x17_1018);
        let o = reach(to_oklch(Rgb::from_u32(0x55_4455)), &[bg], 7.0, true);
        assert!(contrast(from_oklch(o), bg) >= 7.0);
        let muted = fade_toward(bg, WHITE, 5.082);
        assert!((5.0..5.3).contains(&contrast(muted, bg)), "{}", contrast(muted, bg));
    }
}
