//! T3 Code's built-in theme palettes, read at runtime from the installed desktop app.
//!
//! The Electron archive (`Contents/Resources/app.asar`) is read in place: its JSON header is
//! parsed, and only the web client's `themePalette-*.js` chunk is read from it. The chunk is
//! minified, but T3 declares its palettes as literal objects. Every `{` is tried as the start of
//! a JS object literal (strings, numbers, `!0`/`!1`, nested objects/arrays only — anything else
//! rejects that candidate), which yields:
//! * the built-in themes: `{id, label, appearance, colors{…}, variants{dark|light{…}}}`;
//! * the neutral palettes T3 uses for the `system` / `light` / `dark` theme choice: flat objects
//!   without `id` whose `canvas` and `sidebar` are colors (dark or light by `canvas` luminance).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde_json::Value;

use super::color;

/// Token name → CSS color string, as written in the palette.
pub type Palette = BTreeMap<String, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "light" => Some(Mode::Light),
            "dark" => Some(Mode::Dark),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
        }
    }
}

/// A T3 theme with full palettes for its own appearance and optional variants.
#[derive(Clone, Debug)]
pub struct Theme {
    pub id: String,
    pub label: String,
    pub appearance: Mode,
    pub colors: Palette,
    pub variants: Vec<(Mode, Palette)>,
}

impl Theme {
    /// The palette for `mode` (T3's `$`): own colors or a variant, else `None`.
    pub fn palette(&self, mode: Mode) -> Option<&Palette> {
        if mode == self.appearance {
            Some(&self.colors)
        } else {
            self.variants.iter().find(|(m, _)| *m == mode).map(|(_, p)| p)
        }
    }
}

/// What was read from the installed app.
#[derive(Clone, Debug)]
pub struct Bundle {
    pub app_asar: PathBuf,
    /// Path of the chunk inside the archive.
    pub chunk: String,
    pub themes: Vec<Theme>,
    pub neutral_light: Option<Palette>,
    pub neutral_dark: Option<Palette>,
}

impl Bundle {
    pub fn theme(&self, id: &str) -> Option<&Theme> {
        self.themes.iter().find(|t| t.id == id)
    }

    pub fn neutral(&self, mode: Mode) -> Option<&Palette> {
        match mode {
            Mode::Light => self.neutral_light.as_ref(),
            Mode::Dark => self.neutral_dark.as_ref(),
        }
    }
}

/// Reads and parses the theme chunk of `app_asar`.
pub fn load(app_asar: &Path) -> anyhow::Result<Bundle> {
    let mut f = File::open(app_asar).with_context(|| format!("open {}", app_asar.display()))?;
    let mut head = [0u8; 16];
    f.read_exact(&mut head)?;
    let word = |i: usize| u32::from_le_bytes(head[i..i + 4].try_into().expect("4 bytes")) as u64;
    let (header_size, json_len) = (word(4), word(12));
    if word(0) != 4 || json_len + 8 > header_size {
        bail!("not an asar archive (bad pickle header)");
    }
    let mut json = vec![0u8; usize::try_from(json_len)?];
    f.read_exact(&mut json)?;
    let header: Value = serde_json::from_slice(&json).context("asar header json")?;
    let mut found = Vec::new();
    find_chunks(&header, String::new(), &mut found);
    // Prefer the web client's copy.
    found.sort_by_key(|(path, _, _)| !path.contains("/client/"));
    let (chunk, offset, size) = found.into_iter().next().context("no themePalette-*.js in app.asar")?;
    f.seek(SeekFrom::Start(8 + header_size + offset))?;
    let mut src = vec![0u8; usize::try_from(size)?];
    f.read_exact(&mut src)?;
    let mut bundle = parse_chunk(&src);
    if bundle.themes.is_empty() {
        bail!("{chunk}: no theme literals found (T3 changed its bundle format?)");
    }
    bundle.app_asar = app_asar.to_owned();
    bundle.chunk = chunk;
    Ok(bundle)
}

fn find_chunks(node: &Value, path: String, out: &mut Vec<(String, u64, u64)>) {
    let Some(files) = node.get("files").and_then(Value::as_object) else { return };
    for (name, entry) in files {
        let p = format!("{path}/{name}");
        if entry.get("files").is_some() {
            find_chunks(entry, p, out);
        } else if name.starts_with("themePalette-") && name.ends_with(".js") && entry.get("unpacked").is_none() {
            let offset = entry.get("offset").and_then(Value::as_str).and_then(|o| o.parse().ok());
            if let (Some(offset), Some(size)) = (offset, entry.get("size").and_then(Value::as_u64)) {
                out.push((p, offset, size));
            }
        }
    }
}

fn parse_chunk(src: &[u8]) -> Bundle {
    let mut bundle =
        Bundle { app_asar: PathBuf::new(), chunk: String::new(), themes: Vec::new(), neutral_light: None, neutral_dark: None };
    let mut i = 0;
    while i < src.len() {
        if src[i] != b'{' {
            i += 1;
            continue;
        }
        let mut j = i;
        match Lit::object(src, &mut j) {
            Some(obj) => {
                classify(&obj, &mut bundle);
                // Nested objects of a recognized literal are part of it; skip past.
                i = j;
            }
            None => i += 1,
        }
    }
    bundle
}

fn classify(obj: &[(String, Lit)], bundle: &mut Bundle) {
    let get = |k: &str| obj.iter().find(|(key, _)| key == k).map(|(_, v)| v);
    if let (Some(Lit::Str(id)), Some(Lit::Str(label)), Some(Lit::Str(app)), Some(Lit::Obj(colors))) =
        (get("id"), get("label"), get("appearance"), get("colors"))
        && let Some(appearance) = Mode::parse(app)
    {
        let variants = match get("variants") {
            Some(Lit::Obj(v)) => v
                .iter()
                .filter_map(|(m, p)| match p {
                    Lit::Obj(p) => Some((Mode::parse(m)?, palette(p))),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        bundle.themes.push(Theme { id: id.clone(), label: label.clone(), appearance, colors: palette(colors), variants });
        return;
    }
    if get("id").is_none()
        && let (Some(Lit::Str(canvas)), Some(Lit::Str(sidebar))) = (get("canvas"), get("sidebar"))
        && color::parse(sidebar).is_some()
        && let Some((c, _)) = color::parse(canvas)
    {
        let slot = if color::is_dark(c) { &mut bundle.neutral_dark } else { &mut bundle.neutral_light };
        slot.get_or_insert_with(|| palette(obj));
    }
}

fn palette(obj: &[(String, Lit)]) -> Palette {
    obj.iter()
        .filter_map(|(k, v)| match v {
            Lit::Str(s) => Some((k.clone(), s.clone())),
            _ => None,
        })
        .collect()
}

/// A JS literal value (only what palettes use).
enum Lit {
    Str(String),
    Other,
    Obj(Vec<(String, Lit)>),
}

impl Lit {
    fn object(b: &[u8], i: &mut usize) -> Option<Vec<(String, Lit)>> {
        expect(b, i, b'{')?;
        let mut out = Vec::new();
        loop {
            skip_ws(b, i);
            if *b.get(*i)? == b'}' {
                *i += 1;
                return Some(out);
            }
            let key = match b[*i] {
                b'"' | b'\'' | b'`' => string(b, i)?,
                c if is_ident(c) => {
                    let s = *i;
                    while b.get(*i).is_some_and(|&c| is_ident(c)) {
                        *i += 1;
                    }
                    String::from_utf8(b[s..*i].to_vec()).ok()?
                }
                _ => return None,
            };
            skip_ws(b, i);
            expect(b, i, b':')?;
            skip_ws(b, i);
            out.push((key, Lit::value(b, i)?));
            skip_ws(b, i);
            match *b.get(*i)? {
                b',' => *i += 1,
                b'}' => {}
                _ => return None,
            }
        }
    }

    fn value(b: &[u8], i: &mut usize) -> Option<Lit> {
        match *b.get(*i)? {
            b'{' => Lit::object(b, i).map(Lit::Obj),
            b'"' | b'\'' | b'`' => string(b, i).map(Lit::Str),
            b'[' => {
                *i += 1;
                loop {
                    skip_ws(b, i);
                    if *b.get(*i)? == b']' {
                        *i += 1;
                        return Some(Lit::Other);
                    }
                    Lit::value(b, i)?;
                    skip_ws(b, i);
                    if *b.get(*i)? == b',' {
                        *i += 1;
                    }
                }
            }
            b'!' if matches!(b.get(*i + 1), Some(b'0' | b'1')) => {
                *i += 2;
                Some(Lit::Other)
            }
            c if c == b'-' || c == b'.' || c.is_ascii_digit() => {
                let s = *i;
                *i += 1;
                while b.get(*i).is_some_and(|&c| c.is_ascii_alphanumeric() || c == b'.') {
                    *i += 1;
                }
                std::str::from_utf8(&b[s..*i]).ok()?.parse::<f64>().ok().map(|_| Lit::Other)
            }
            _ => {
                for word in [&b"null"[..], b"true", b"false", b"void 0"] {
                    if b[*i..].starts_with(word) && !b.get(*i + word.len()).is_some_and(|&c| is_ident(c)) {
                        *i += word.len();
                        return Some(Lit::Other);
                    }
                }
                None
            }
        }
    }
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while b.get(*i).is_some_and(u8::is_ascii_whitespace) {
        *i += 1;
    }
}

fn expect(b: &[u8], i: &mut usize, c: u8) -> Option<()> {
    (*b.get(*i)? == c).then(|| *i += 1)
}

/// A quoted string or a template literal without substitutions.
fn string(b: &[u8], i: &mut usize) -> Option<String> {
    let q = b[*i];
    *i += 1;
    let mut out = Vec::new();
    loop {
        let c = *b.get(*i)?;
        *i += 1;
        match c {
            _ if c == q => return String::from_utf8(out).ok(),
            b'\\' => {
                let e = *b.get(*i)?;
                *i += 1;
                match e {
                    b'n' => out.push(b'\n'),
                    b't' => out.push(b'\t'),
                    b'u' => {
                        let code = u32::from_str_radix(std::str::from_utf8(b.get(*i..*i + 4)?).ok()?, 16).ok()?;
                        *i += 4;
                        out.extend_from_slice(char::from_u32(code)?.to_string().as_bytes());
                    }
                    _ => out.push(e),
                }
            }
            b'$' if q == b'`' && b.get(*i) == Some(&b'{') => return None,
            b'\n' if q != b'`' => return None,
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_themes_and_neutrals_from_minified_js() {
        let src = br##"var a=1;function f(e){return{...e,x:1}}var Ft={canvas:`#0a0a0a`,sidebar:`#000000`,text:`#f5f5f5`},
            Pt={canvas:`#fcfcfc`,sidebar:'#fafafa',text:"#27272a"},
            to={canvas:`--app-theme-canvas`,sidebar:`--app-theme-sidebar`},
            It={id:`t3-chat`,label:`T3 Chat`,appearance:`light`,sidebarArtwork:!0,n:.5,colors:{canvas:`oklch(0.98 0.01 325)`},
               variants:{dark:{canvas:`oklch(0.22813 0.020366 307.469)`}}},
            bad={id:`x`,label:`X`,appearance:`dark`,colors:{canvas:`${e}`}};"##;
        let b = parse_chunk(src);
        assert_eq!(b.themes.len(), 1);
        let t = &b.themes[0];
        assert_eq!((t.id.as_str(), t.label.as_str(), t.appearance), ("t3-chat", "T3 Chat", Mode::Light));
        assert_eq!(t.palette(Mode::Dark).unwrap()["canvas"], "oklch(0.22813 0.020366 307.469)");
        assert_eq!(b.neutral(Mode::Dark).unwrap()["sidebar"], "#000000");
        assert_eq!(b.neutral(Mode::Light).unwrap()["text"], "#27272a");
    }
}
