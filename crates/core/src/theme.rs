//! Theme sync: the panel's colors follow the theme selected in the T3 Code desktop app.
//!
//! Inputs (all read-only):
//! * the app's `localStorage` (Chromium leveldb under `<userData>/Local Storage/leveldb`, origin
//!   `t3code://app`): `t3code:theme` (theme id), `t3code:theme-appearance-mode`
//!   (`system|light|dark`), `t3code:theme-follow-system` (legacy), `t3code:theme-halves:v1`
//!   (per-appearance theme ids) and `t3code:themes:v1` (custom themes);
//! * the built-in palettes in the installed app's `app.asar` (see [`bundle`]);
//! * environment themes `~/.t3/userdata/themes/*.json` (file stem = id);
//! * `~/.t3/userdata/client-settings.json`: `fontFamilySans`, `fontSizeInterface`,
//!   `appearanceContrast`;
//! * macOS appearance (`AppleInterfaceStyle` via CFPreferences) for `system` mode.
//!
//! Selection mirrors T3's `useTheme`/`themePalette` code. The panel imitates T3's sidebar, so the
//! view takes the palette's sidebar tokens (`sidebar`, `sidebarForeground`, …). T3's contrast
//! slider is applied like its CSS does, then contrast floors are enforced by nudging OKLCH
//! lightness. A 2 s watcher re-resolves when an input changes and emits `ThemeChanged`.

mod bundle;
pub mod color;
mod leveldb;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime};

use crossbeam_channel::Sender;
use serde_json::{Value, json};

use self::bundle::{Bundle, Mode, Palette, Theme};
use self::color::{Oklch, Rgb};
use crate::telemetry::{Telemetry, ms, round2};
use crate::{Config, CoreEvent, ThemeView};

const ORIGIN: &str = "t3code://app";
const KEY_THEME: &str = "t3code:theme";
const KEY_MODE: &str = "t3code:theme-appearance-mode";
const KEY_FOLLOW: &str = "t3code:theme-follow-system";
const KEY_HALVES: &str = "t3code:theme-halves:v1";
const KEY_CUSTOM: &str = "t3code:themes:v1";
const LS_KEYS: [&str; 5] = [KEY_THEME, KEY_MODE, KEY_FOLLOW, KEY_HALVES, KEY_CUSTOM];
/// Legacy theme ids T3 still accepts (`qi` in its bundle); `t3-chat-dark` also forces dark.
const LEGACY_IDS: [(&str, &str); 5] =
    [("t3-chat-dark", "t3-chat"), ("t3-grove", "grove"), ("t3-ocean", "ocean"), ("t3-ember", "ember"), ("t3-iris", "iris")];
const CHAT_DARK: &str = "t3-chat-dark";
/// The theme T3 fills missing tokens from (its `Ma`).
const BASE_THEME: &str = "t3-chat";
/// Tailwind v4 emerald/blue 400 (dark) and 700 (light): T3's `--success-foreground` /
/// `--info-foreground` (they are not part of theme palettes).
const SUCCESS: [&str; 2] = ["oklch(76.5% 0.177 163.223)", "oklch(50.8% 0.118 165.612)"];
const INFO: [&str; 2] = ["oklch(70.7% 0.165 254.624)", "oklch(48.8% 0.243 264.376)"];
const WATCH_EVERY: Duration = Duration::from_secs(2);

/// Where the inputs live.
#[derive(Clone, Debug)]
pub struct Sources {
    pub local_storage: PathBuf,
    pub app_asar: Option<PathBuf>,
    pub client_settings: PathBuf,
    pub themes_dir: PathBuf,
}

impl Sources {
    pub fn new(cfg: &Config) -> Sources {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
        let userdata = home.join(".t3/userdata");
        Sources {
            local_storage: cfg.t3_app_data.join("Local Storage/leveldb"),
            app_asar: app_asar(cfg, &home),
            client_settings: userdata.join("client-settings.json"),
            themes_dir: userdata.join("themes"),
        }
    }

    fn to_json(&self) -> Value {
        json!({"local_storage": self.local_storage, "app_asar": self.app_asar,
               "client_settings": self.client_settings, "themes_dir": self.themes_dir})
    }
}

/// `Config.t3_app` if set, else the most recently updated installed T3 Code app.
fn app_asar(cfg: &Config, home: &Path) -> Option<PathBuf> {
    let rel = "Contents/Resources/app.asar";
    if let Some(app) = &cfg.t3_app {
        return Some(app.join(rel));
    }
    ["/Applications", &home.join("Applications").to_string_lossy()]
        .iter()
        .flat_map(|dir| ["T3 Code (Nightly).app", "T3 Code.app"].map(|n| Path::new(dir).join(n).join(rel)))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// One resolution: the view plus everything `theme.resolve` logs.
pub struct Resolution {
    pub view: ThemeView,
    /// Why the default palette was used (the view still carries T3's fonts when readable).
    pub error: Option<String>,
    pub report: Value,
    pub dur_ms: f64,
}

/// Snapshot of every input; a change triggers a new resolution.
#[derive(Clone, Debug, PartialEq)]
struct Inputs {
    storage: Result<BTreeMap<String, String>, String>,
    system_dark: bool,
    settings: Settings,
    env_themes: Vec<(String, String)>,
    asar: Option<(PathBuf, Option<SystemTime>, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
struct Settings {
    font_family: String,
    font_size: f32,
    /// T3's Appearance → Contrast slider (50..=200, default 100).
    contrast: f64,
    error: Option<String>,
}

/// Keeps the parsed app bundle between resolutions (keyed by archive path, mtime, size).
#[derive(Default)]
pub struct Resolver {
    bundle: Option<((PathBuf, Option<SystemTime>, u64), Result<Arc<Bundle>, String>)>,
}

impl Resolver {
    /// Reads every input and resolves the view.
    pub fn resolve(&mut self, src: &Sources) -> Resolution {
        let t = Instant::now();
        let inputs = read_inputs(src);
        self.resolve_inputs(src, &inputs, t)
    }

    fn resolve_inputs(&mut self, src: &Sources, inputs: &Inputs, t: Instant) -> Resolution {
        let bundle = self.bundle(inputs.asar.as_ref());
        let mut report = json!({"system_dark": inputs.system_dark, "settings": {
            "font_family": inputs.settings.font_family, "font_size": inputs.settings.font_size,
            "appearance_contrast": inputs.settings.contrast, "error": inputs.settings.error}});
        if let Ok(b) = &bundle {
            report["bundle"] = json!({"app_asar": b.app_asar, "chunk": b.chunk,
                "themes": b.themes.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
                "neutral": [b.neutral_light.is_some(), b.neutral_dark.is_some()]});
        }
        let picked = select(inputs, bundle.as_deref().ok(), &mut report);
        let (view, error) = match (picked, &bundle) {
            (Ok(p), _) => (build_view(&p, inputs, &mut report), None),
            (Err(e), Err(be)) => (fallback(inputs), Some(format!("{e}; bundle: {be}"))),
            (Err(e), Ok(_)) => (fallback(inputs), Some(e)),
        };
        report["sources"] = src.to_json();
        Resolution { view, error, report, dur_ms: ms(t.elapsed()) }
    }

    fn bundle(&mut self, asar: Option<&(PathBuf, Option<SystemTime>, u64)>) -> Result<Arc<Bundle>, String> {
        let Some(key) = asar else { return Err("T3 Code app not found (set TELLAL_T3_APP)".into()) };
        if let Some((k, b)) = &self.bundle
            && k == key
        {
            return b.clone();
        }
        let b = bundle::load(&key.0).map(Arc::new).map_err(|e| format!("{e:#}"));
        self.bundle = Some((key.clone(), b.clone()));
        b
    }
}

fn read_inputs(src: &Sources) -> Inputs {
    let storage = leveldb::local_storage(&src.local_storage, ORIGIN)
        .map(|all| all.into_iter().filter(|(k, _)| LS_KEYS.contains(&k.as_str())).collect())
        .map_err(|e| format!("localStorage: {e:#}"));
    let mut env_themes: Vec<(String, String)> = std::fs::read_dir(&src.themes_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "json").then_some(())?;
            Some((path.file_stem()?.to_string_lossy().into_owned(), std::fs::read_to_string(&path).ok()?))
        })
        .collect();
    env_themes.sort();
    let asar = src.app_asar.as_ref().map(|p| {
        let md = std::fs::metadata(p).ok();
        (p.clone(), md.as_ref().and_then(|m| m.modified().ok()), md.map_or(0, |m| m.len()))
    });
    Inputs { storage, system_dark: system_dark(), settings: read_settings(&src.client_settings), env_themes, asar }
}

fn read_settings(path: &Path) -> Settings {
    let mut s = Settings { font_family: String::new(), font_size: 14.0, contrast: 100.0, error: None };
    let parsed = std::fs::read(path)
        .map_err(|e| format!("{}: {e}", path.display()))
        .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| format!("{}: {e}", path.display())));
    match parsed {
        Ok(v) => {
            if let Some(f) = v.get("fontFamilySans").and_then(Value::as_str) {
                s.font_family = f.trim().to_owned();
            }
            if let Some(n) = v.get("fontSizeInterface").and_then(Value::as_f64).filter(|n| *n > 0.0) {
                s.font_size = n as f32;
            }
            if let Some(c) = v.get("appearanceContrast").and_then(Value::as_f64) {
                s.contrast = c.clamp(50.0, 200.0);
            }
        }
        Err(e) => s.error = Some(e),
    }
    s
}

/// macOS dark mode (`AppleInterfaceStyle == "Dark"` in the global domain).
#[cfg(target_os = "macos")]
fn system_dark() -> bool {
    use std::ffi::{c_char, c_void};
    type CFStringRef = *const c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFPreferencesAnyApplication: CFStringRef;
        fn CFPreferencesAppSynchronize(app: CFStringRef) -> u8;
        fn CFPreferencesCopyAppValue(key: CFStringRef, app: CFStringRef) -> *const c_void;
        fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, encoding: u32) -> CFStringRef;
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFRelease(cf: *const c_void);
    }
    const UTF8: u32 = 0x0800_0100;
    // SAFETY: plain CoreFoundation calls; every created/copied object is released once, and the
    // value is only read as a string after its type was checked.
    unsafe {
        let key = CFStringCreateWithCString(std::ptr::null(), c"AppleInterfaceStyle".as_ptr(), UTF8);
        if key.is_null() {
            return false;
        }
        CFPreferencesAppSynchronize(kCFPreferencesAnyApplication);
        let value = CFPreferencesCopyAppValue(key, kCFPreferencesAnyApplication);
        CFRelease(key);
        if value.is_null() {
            return false;
        }
        let mut buf = [0 as c_char; 16];
        let dark = CFGetTypeID(value) == CFStringGetTypeID()
            && CFStringGetCString(value, buf.as_mut_ptr(), buf.len() as isize, UTF8) != 0
            && std::ffi::CStr::from_ptr(buf.as_ptr()).to_bytes().eq_ignore_ascii_case(b"dark");
        CFRelease(value);
        dark
    }
}

#[cfg(not(target_os = "macos"))]
fn system_dark() -> bool {
    false
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Origin {
    BuiltIn,
    Custom,
    Env,
}

/// T3's theme lookup order (`Q`): built-in, then saved custom themes, then environment themes.
struct Catalog<'a> {
    bundle: Option<&'a Bundle>,
    custom: Vec<Theme>,
    env: Vec<Theme>,
}

impl Catalog<'_> {
    fn find(&self, id: &str) -> Option<(&Theme, Origin)> {
        let id = canonical(id);
        self.bundle
            .and_then(|b| b.theme(id))
            .map(|t| (t, Origin::BuiltIn))
            .or_else(|| self.custom.iter().find(|t| t.id == id).map(|t| (t, Origin::Custom)))
            .or_else(|| self.env.iter().find(|t| t.id == id).map(|t| (t, Origin::Env)))
    }

    fn base(&self, mode: Mode) -> Palette {
        self.bundle
            .and_then(|b| b.theme(BASE_THEME).or(b.themes.first()))
            .and_then(|t| t.palette(mode))
            .cloned()
            .unwrap_or_default()
    }
}

fn canonical(id: &str) -> &str {
    LEGACY_IDS.iter().find(|(old, _)| *old == id).map_or(id, |(_, new)| new)
}

fn reserved(id: &str, bundle: Option<&Bundle>) -> bool {
    matches!(id, "system" | "light" | "dark")
        || LEGACY_IDS.iter().any(|(old, _)| *old == id)
        || bundle.is_some_and(|b| b.theme(id).is_some())
}

/// The selected palette before mapping.
struct Picked {
    source: String,
    name: String,
    mode: Mode,
    palette: Palette,
}

fn select(inputs: &Inputs, bundle: Option<&Bundle>, report: &mut Value) -> Result<Picked, String> {
    let ls = inputs.storage.as_ref().map_err(Clone::clone)?;
    let mut custom_errors = Vec::new();
    let cat = Catalog { bundle, custom: Vec::new(), env: Vec::new() };
    let custom = ls.get(KEY_CUSTOM).map_or_else(Vec::new, |raw| custom_themes(raw, &cat, &mut custom_errors));
    let env: Vec<Theme> = inputs
        .env_themes
        .iter()
        .filter(|(id, _)| !reserved(id, bundle))
        .filter_map(|(id, raw)| match env_theme(id, raw, &cat) {
            Ok(t) => Some(t),
            Err(e) => {
                custom_errors.push(format!("env theme {id}: {e}"));
                None
            }
        })
        .collect();
    let cat = Catalog { custom, env, ..cat };

    // useTheme: theme id (z), appearance mode (L), halves (lo), resolved mode (oo), applied id (uo).
    let stored = ls.get(KEY_THEME).map(String::as_str);
    let theme_id = match stored {
        Some(v) if matches!(v, "system" | "light" | "dark") || cat.find(v).is_some() => {
            if v == CHAT_DARK { v } else { canonical(v) }
        }
        _ => "system",
    };
    let follow = match ls.get(KEY_FOLLOW).map(String::as_str) {
        Some("true") => true,
        Some("false") => false,
        _ => theme_id == "system",
    };
    let preferred = || match theme_id {
        "system" => None,
        "light" => Some(Mode::Light),
        "dark" | CHAT_DARK => Some(Mode::Dark),
        id => cat.find(id).map(|(t, _)| t.appearance),
    };
    let mode_pref = match ls.get(KEY_MODE).map(String::as_str) {
        Some(m @ ("system" | "light" | "dark")) => m.to_owned(),
        _ if follow => "system".to_owned(),
        _ => preferred().unwrap_or(Mode::Light).as_str().to_owned(),
    };
    let halves: HashMap<Mode, String> = ls
        .get(KEY_HALVES)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .map(|v| {
            [Mode::Light, Mode::Dark]
                .into_iter()
                .filter_map(|m| {
                    let id = v.get(m.as_str())?.as_str()?;
                    let (t, _) = cat.find(id)?;
                    t.palette(m).is_some().then(|| (m, t.id.clone()))
                })
                .collect()
        })
        .unwrap_or_default();
    let want = match Mode::parse(&mode_pref) {
        Some(m) => m,
        None if inputs.system_dark => Mode::Dark,
        None => Mode::Light,
    };
    let mode = if halves.contains_key(&want) {
        want
    } else {
        match cat.find(theme_id) {
            Some((t, _)) if t.palette(want).is_none() => t.appearance,
            _ => want,
        }
    };
    let applied = halves.get(&mode).map_or(theme_id, String::as_str);
    report["selection"] = json!({
        "stored_theme": stored, "theme": theme_id, "follow_system": follow, "appearance_mode": mode_pref,
        "halves": halves.iter().map(|(m, id)| (m.as_str(), id.as_str())).collect::<BTreeMap<_, _>>(),
        "resolved": mode.as_str(), "applied": applied,
        "custom_themes": cat.custom.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        "env_themes": cat.env.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        "theme_errors": custom_errors,
    });
    match cat.find(applied) {
        Some((t, origin)) => {
            let prefix = if origin == Origin::Env { "t3-env" } else { "t3" };
            let palette = t.palette(mode).unwrap_or(&t.colors).clone();
            report["selection"]["origin"] = json!(format!("{origin:?}"));
            Ok(Picked { source: format!("{prefix}:{}", t.id), name: t.label.clone(), mode, palette })
        }
        None => {
            let b = bundle.ok_or("built-in palettes unavailable")?;
            let palette = b.neutral(mode).ok_or_else(|| format!("no neutral {} palette in {}", mode.as_str(), b.chunk))?;
            report["selection"]["origin"] = json!("Neutral");
            Ok(Picked { source: format!("t3:{applied}"), name: "T3 Default".into(), mode, palette: palette.clone() })
        }
    }
}

/// `t3code:themes:v1`: `[{id, label, appearance, colors{…}, variants?{…}}]`, each palette merged
/// over T3's base palette (T3's `Ai`). Invalid entries are skipped (reported).
fn custom_themes(raw: &str, cat: &Catalog, errors: &mut Vec<String>) -> Vec<Theme> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw) else {
        errors.push(format!("{KEY_CUSTOM}: not a JSON array"));
        return Vec::new();
    };
    let mut out: Vec<Theme> = Vec::new();
    for item in &items {
        let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
        let label = item.get("label").and_then(Value::as_str).map(str::trim).unwrap_or_default();
        let appearance = item.get("appearance").and_then(Value::as_str).and_then(Mode::parse);
        let (Some(appearance), Some(colors)) = (appearance, item.get("colors").filter(|c| c.is_object())) else {
            errors.push(format!("custom theme {id:?}: missing appearance/colors"));
            continue;
        };
        if id.is_empty() || label.is_empty() || reserved(id, cat.bundle) || out.iter().any(|t| t.id == id) {
            continue;
        }
        out.push(Theme {
            id: id.to_owned(),
            label: label.to_owned(),
            appearance,
            colors: overlay(cat.base(appearance), colors),
            variants: variants(item, appearance, cat),
        });
    }
    out
}

fn variants(item: &Value, own: Mode, cat: &Catalog) -> Vec<(Mode, Palette)> {
    item.get("variants")
        .and_then(Value::as_object)
        .map(|v| {
            v.iter()
                .filter_map(|(m, p)| {
                    let m = Mode::parse(m).filter(|m| *m != own)?;
                    p.is_object().then(|| (m, overlay(cat.base(m), p)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `base` with every parseable color of the JSON object `colors` on top (T3's `Di`).
fn overlay(mut base: Palette, colors: &Value) -> Palette {
    if let Some(obj) = colors.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str().filter(|s| color::parse(s).is_some()) {
                base.insert(k.clone(), s.to_owned());
            }
        }
    }
    base
}

/// An environment theme file: exported (`colors`) or short (`canvas` + `accent`) format; for the
/// short format T3 generates the palette from canvas + accent (its `Da`).
fn env_theme(id: &str, raw: &str, cat: &Catalog) -> Result<Theme, String> {
    let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let appearance = v.get("appearance").and_then(Value::as_str).and_then(Mode::parse).ok_or("missing appearance")?;
    let label = ["name", "label"]
        .iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()))
        .unwrap_or(id)
        .to_owned();
    let (canvas, accent) = (v.get("canvas").and_then(Value::as_str), v.get("accent").and_then(Value::as_str));
    let base = match (canvas, accent) {
        (Some(c), Some(a)) => generate(appearance, c, a, cat),
        _ if v.get("colors").is_some_and(Value::is_object) => cat.base(appearance),
        _ => return Err("needs canvas+accent or colors".into()),
    };
    let colors = match v.get("colors") {
        Some(c) => overlay(base, c),
        None => base,
    };
    Ok(Theme { id: id.to_owned(), label, appearance, colors, variants: variants(&v, appearance, cat) })
}

/// T3's palette generator for the short theme format (`Da` + `Ea`), limited to the tokens the
/// panel maps; everything else comes from the base palette like in T3.
fn generate(mode: Mode, canvas: &str, accent: &str, cat: &Catalog) -> Palette {
    let dark_mode = mode == Mode::Dark;
    let parse = |s: &str, default: Rgb| color::parse(s).map_or(default, |(c, _)| c);
    let i = parse(canvas, if dark_mode { Rgb { r: 24.0, g: 15.0, b: 27.0 } } else { Rgb { r: 250.0, g: 245.0, b: 250.0 } });
    let a = parse(accent, Rgb { r: 168.0, g: 67.0, b: 112.0 });
    let (o, s) = (color::to_oklch(i), color::to_oklch(a));
    let c = color::is_dark(i);
    let hue = if s.c < 0.02 { o.h } else { s.h };
    let u = (s.c * 0.22).clamp(0.008, 0.045);
    let d = if c { 1.0 } else { -1.0 };
    let f = |e: f64, t: f64| color::from_oklch(Oklch { l: (o.l + d * e).clamp(0.05, 0.98), c: t, h: hue });
    let m = Oklch { l: if c { 0.95 } else { 0.2 }, c: (s.c * 0.25).min(0.035), h: hue };
    let muted_target = if c { 5.082 } else { 4.705 };
    let text = color::from_oklch(color::reach(m, &[i], 7.0, c));
    let sidebar = f(0.045, u * 1.4);
    let mut p = cat.base(mode);
    let mut set = |k: &str, v: Rgb| {
        p.insert(k.to_owned(), color::hex(v.to_u32()));
    };
    set("canvas", i);
    set("text", text);
    set("textMuted", color::fade_toward(i, text, muted_target));
    set("surface", f(0.015, u));
    set("border", f(if c { 0.16 } else { 0.12 }, (s.c * 0.35).min(0.07)));
    set("accent", a);
    set("accentForeground", color::on_color(a));
    set("sidebar", sidebar);
    set("sidebarForeground", color::from_oklch(color::reach(m, &[sidebar], 4.6, c)));
    set("sidebarMutedForeground", color::fade_toward(sidebar, text, muted_target));
    set("sidebarRowHover", f(if c { 0.08 } else { 0.06 }, (s.c * 0.45).min(0.08)));
    set("sidebarBorder", f(if c { 0.17 } else { 0.12 }, (s.c * 0.4).min(0.08)));
    // Ea: status foregrounds readable on their tinted surfaces.
    if let Some(neutral) = cat.bundle.and_then(|b| b.neutral(if c { Mode::Dark } else { Mode::Light })) {
        let tint = if c { 0.16 } else { 0.08 };
        for (base, fg) in [("error", "errorForeground"), ("warning", "warningForeground")] {
            let get = |k: &str| neutral.get(k).and_then(|v| color::parse(v)).map(|(c, _)| c);
            if let (Some(b), Some(fgc)) = (get(base), get(fg)) {
                let surface = color::mix(i, b, tint);
                set(base, b);
                set(fg, color::from_oklch(color::reach(color::to_oklch(fgc), &[surface], 4.6, c)));
            }
        }
    }
    p
}

/// Token values (composited over `under` when translucent).
fn token(p: &Palette, keys: &[&str], under: Rgb) -> Option<Rgb> {
    keys.iter().find_map(|k| p.get(*k).and_then(|v| color::parse(v))).map(|(c, a)| color::mix(under, c, a))
}

fn build_view(p: &Picked, inputs: &Inputs, report: &mut Value) -> ThemeView {
    let dark = p.mode == Mode::Dark;
    let def = ThemeView::default();
    let ground = if dark { color::BLACK } else { color::WHITE };
    let canvas = token(&p.palette, &["sidebar", "canvas"], ground).unwrap_or(Rgb::from_u32(def.canvas));
    let get = |keys: &[&str], fallback: u32| token(&p.palette, keys, canvas).unwrap_or(Rgb::from_u32(fallback));
    let surface = get(&["surface"], def.surface);
    let surface_hover = get(&["sidebarRowHover", "secondary"], def.surface_hover);
    let border = get(&["sidebarBorder", "border"], def.border);
    let text = get(&["sidebarForeground", "text"], def.text);
    let text_muted = get(&["sidebarMutedForeground", "textMuted"], def.text_muted);
    let accent = get(&["accent"], def.accent);
    let accent_text = token(&p.palette, &["accentForeground"], accent).unwrap_or_else(|| color::on_color(accent));
    let fixed = |pair: [&str; 2]| color::parse(pair[usize::from(!dark)]).map_or(color::BLACK, |(c, _)| c);
    let palette = [
        ("canvas", canvas),
        ("surface", surface),
        ("surface_hover", surface_hover),
        ("border", border),
        ("text", text),
        ("text_muted", text_muted),
        ("accent", accent),
        ("accent_text", accent_text),
        ("success", fixed(SUCCESS)),
        ("warning", get(&["warning", "warningForeground"], def.warning)),
        ("danger", get(&["error", "errorForeground"], def.danger)),
        ("info", fixed(INFO)),
    ];
    let mut c: BTreeMap<&str, Rgb> = palette.into_iter().collect();
    let original: BTreeMap<&str, String> = c.iter().map(|(k, v)| (*k, color::hex(v.to_u32()))).collect();

    // T3's Appearance → Contrast (its CSS `--contrast-*` vars): text toward white/black by the
    // boost, borders toward the foreground by a quarter of it. Muted text keeps the palette value
    // so the panel's hierarchy survives high settings.
    let level = inputs.settings.contrast;
    let (base, boost) = (level.min(100.0) / 100.0, (level - 100.0).max(0.0) / 100.0);
    let target = if dark { color::WHITE } else { color::BLACK };
    let boosted = |fg: Rgb, bg: Rgb| color::mix_oklab(color::mix_oklab(fg, bg, base), target, 1.0 - boost);
    let bb = boost / 4.0;
    let alpha = base * (1.0 - bb) + bb;
    let border_mix = color::mix(c["border"], c["text"], if alpha > 0.0 { bb / alpha } else { 0.0 });
    c.insert("border", color::mix(canvas, border_mix, alpha));
    c.insert("text", boosted(c["text"], canvas));
    c.insert("accent_text", boosted(c["accent_text"], c["accent"]));
    let after_slider: BTreeMap<&str, String> = c.iter().map(|(k, v)| (*k, color::hex(v.to_u32()))).collect();

    // Contrast floors.
    let backs = [canvas, c["surface"], c["surface_hover"]];
    let mut adjusted = Vec::new();
    let mut floor = |c: &mut BTreeMap<&str, Rgb>, key: &'static str, against: &[Rgb], aim: f64, min: f64| {
        let before = c[key];
        let worst = |x: Rgb| against.iter().map(|&b| color::contrast(x, b)).fold(f64::INFINITY, f64::min);
        if worst(before) >= aim {
            return;
        }
        let lighter = color::is_dark(against[0]);
        let mut after = color::from_oklch(color::reach(color::to_oklch(before), against, aim, lighter));
        if worst(after) < min {
            after = color::on_color(against[0]);
        }
        adjusted.push(json!({"token": key, "from": color::hex(before.to_u32()), "to": color::hex(after.to_u32()),
                             "ratio_from": round2(worst(before)), "ratio_to": round2(worst(after)), "aim": aim}));
        c.insert(key, after);
    };
    floor(&mut c, "text", &backs, 7.0, 4.5);
    floor(&mut c, "text_muted", &backs, 4.5, 4.5);
    // Accent marks progress, focus and selection on the canvas: non-text contrast ≥ 3:1.
    floor(&mut c, "accent", &[canvas], 3.0, 3.0);
    let accent_now = c["accent"];
    floor(&mut c, "accent_text", &[accent_now], 4.5, 4.5);
    for key in ["success", "warning", "danger", "info"] {
        floor(&mut c, key, &[canvas], 4.5, 4.5);
    }

    let v = |k: &str| c[k].to_u32();
    let view = ThemeView {
        source: p.source.clone(),
        name: p.name.clone(),
        dark,
        canvas: v("canvas"),
        surface: v("surface"),
        surface_hover: v("surface_hover"),
        border: v("border"),
        text: v("text"),
        text_muted: v("text_muted"),
        accent: v("accent"),
        accent_text: v("accent_text"),
        success: v("success"),
        warning: v("warning"),
        danger: v("danger"),
        info: v("info"),
        font_family: inputs.settings.font_family.clone(),
        font_size: inputs.settings.font_size,
    };
    report["palette_tokens"] = json!(p.palette.len());
    report["original"] = json!(original);
    report["after_contrast_setting"] = json!(after_slider);
    report["adjusted"] = json!(adjusted);
    view
}

/// Built-in palette with T3's fonts.
fn fallback(inputs: &Inputs) -> ThemeView {
    ThemeView {
        font_family: inputs.settings.font_family.clone(),
        font_size: inputs.settings.font_size,
        ..ThemeView::default()
    }
}

/// `(foreground, background, ratio, required)` for every pairing the panel draws.
pub fn contrast_table(v: &ThemeView) -> Vec<(&'static str, &'static str, f64, f64)> {
    let pairs: [(&str, u32, &str, u32, f64); 12] = [
        ("text", v.text, "canvas", v.canvas, 7.0),
        ("text", v.text, "surface", v.surface, 4.5),
        ("text", v.text, "surface_hover", v.surface_hover, 4.5),
        ("text_muted", v.text_muted, "canvas", v.canvas, 4.5),
        ("text_muted", v.text_muted, "surface", v.surface, 4.5),
        ("text_muted", v.text_muted, "surface_hover", v.surface_hover, 4.5),
        ("accent_text", v.accent_text, "accent", v.accent, 4.5),
        ("accent", v.accent, "canvas", v.canvas, 3.0),
        ("success", v.success, "canvas", v.canvas, 4.5),
        ("warning", v.warning, "canvas", v.canvas, 4.5),
        ("danger", v.danger, "canvas", v.canvas, 4.5),
        ("info", v.info, "canvas", v.canvas, 4.5),
    ];
    pairs
        .into_iter()
        .map(|(f, fc, b, bc, req)| (f, b, round2(color::contrast(Rgb::from_u32(fc), Rgb::from_u32(bc))), req))
        .collect()
}

/// The view's colors as `#rrggbb` (for logs and the bench).
pub fn colors_json(v: &ThemeView) -> Value {
    let h = color::hex;
    json!({"canvas": h(v.canvas), "surface": h(v.surface), "surface_hover": h(v.surface_hover),
           "border": h(v.border), "text": h(v.text), "text_muted": h(v.text_muted), "accent": h(v.accent),
           "accent_text": h(v.accent_text), "success": h(v.success), "warning": h(v.warning),
           "danger": h(v.danger), "info": h(v.info)})
}

/// Writes one `theme.resolve` event.
fn log(tel: &Telemetry, r: &Resolution, method: &str, changed: &[&str]) {
    let v = &r.view;
    let mut out = r.report.clone();
    out["source"] = json!(v.source);
    out["name"] = json!(v.name);
    out["dark"] = json!(v.dark);
    out["colours"] = colors_json(v);
    out["font"] = json!({"family": v.font_family, "size": v.font_size});
    out["contrast"] = contrast_table(v)
        .into_iter()
        .map(|(f, b, ratio, req)| json!({"fg": f, "bg": b, "ratio": ratio, "required": req}))
        .collect();
    if let Some(e) = &r.error {
        out["fallback_reason"] = json!(e);
    }
    let input = json!({"method": method, "changed": changed});
    tel.event("theme.resolve", Some(r.dur_ms), r.error.is_none(), input, out, r.error.as_deref());
}

/// Resolves once (logged as `theme.resolve` with `method`), then spawns the watcher thread.
pub(crate) fn start(tel: Arc<Telemetry>, cfg: &Config, tx: Sender<CoreEvent>) -> std::io::Result<Arc<RwLock<ThemeView>>> {
    let src = Sources::new(cfg);
    let mut resolver = Resolver::default();
    let t = Instant::now();
    let mut inputs = read_inputs(&src);
    let first = resolver.resolve_inputs(&src, &inputs, t);
    log(&tel, &first, "startup", &[]);
    let shared = Arc::new(RwLock::new(first.view));
    let view = shared.clone();
    std::thread::Builder::new().name("tellal-theme".into()).spawn(move || {
        let mut stamp = watch_stamp(&src);
        loop {
            std::thread::sleep(WATCH_EVERY);
            let now = watch_stamp(&src);
            if now == stamp {
                continue;
            }
            stamp = now;
            let t = Instant::now();
            let next = read_inputs(&src);
            let changed = diff(&inputs, &next);
            if changed.is_empty() {
                continue;
            }
            inputs = next;
            let r = resolver.resolve_inputs(&src, &inputs, t);
            log(&tel, &r, "refresh", &changed);
            let mut cur = view.write().unwrap_or_else(|p| p.into_inner());
            if *cur != r.view {
                *cur = r.view;
                drop(cur);
                let _ = tx.send(CoreEvent::ThemeChanged);
            }
        }
    })?;
    Ok(shared)
}

/// Cheap change detector: sizes + mtimes of the inputs' files and the system appearance.
fn watch_stamp(src: &Sources) -> Vec<(u64, Option<SystemTime>)> {
    let stat = |p: &Path| std::fs::metadata(p).map_or((0, None), |m| (m.len(), m.modified().ok()));
    let mut out: Vec<(u64, Option<SystemTime>)> = Vec::with_capacity(16);
    for dir in [&src.local_storage, &src.themes_dir] {
        let mut entries: Vec<(PathBuf, (u64, Option<SystemTime>))> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .map(|p| {
                let s = stat(&p);
                (p, s)
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        out.push((entries.len() as u64, None));
        out.extend(entries.into_iter().map(|(_, s)| s));
    }
    out.push(stat(&src.client_settings));
    if let Some(a) = &src.app_asar {
        out.push(stat(a));
    }
    out.push((u64::from(system_dark()), None));
    out
}

fn diff(a: &Inputs, b: &Inputs) -> Vec<&'static str> {
    let mut out = Vec::new();
    if a.storage != b.storage {
        out.push("local_storage");
    }
    if a.system_dark != b.system_dark {
        out.push("system_appearance");
    }
    if a.settings != b.settings {
        out.push("client_settings");
    }
    if a.env_themes != b.env_themes {
        out.push("env_themes");
    }
    if a.asar != b.asar {
        out.push("app_bundle");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(ls: &[(&str, &str)], system_dark: bool) -> Inputs {
        Inputs {
            storage: Ok(ls.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()),
            system_dark,
            settings: Settings { font_family: "SF Pro Display".into(), font_size: 14.0, contrast: 100.0, error: None },
            env_themes: Vec::new(),
            asar: None,
        }
    }

    fn bundle() -> Bundle {
        let pal = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        Bundle {
            app_asar: PathBuf::new(),
            chunk: "themePalette-test.js".into(),
            themes: vec![
                Theme {
                    id: "t3-chat".into(),
                    label: "T3 Chat".into(),
                    appearance: Mode::Light,
                    colors: pal(&[("sidebar", "#fafafa"), ("sidebarForeground", "#202020")]),
                    variants: vec![(Mode::Dark, pal(&[("sidebar", "#171018"), ("sidebarForeground", "#f4f4f5")]))],
                },
                Theme {
                    id: "grove".into(),
                    label: "Grove".into(),
                    appearance: Mode::Light,
                    colors: pal(&[("sidebar", "#eef5ee")]),
                    variants: Vec::new(),
                },
            ],
            neutral_light: Some(pal(&[("canvas", "#fcfcfc"), ("sidebar", "#fafafa")])),
            neutral_dark: Some(pal(&[("canvas", "#0a0a0a"), ("sidebar", "#000000")])),
        }
    }

    fn pick(ls: &[(&str, &str)], system_dark: bool) -> Picked {
        select(&inputs(ls, system_dark), Some(&bundle()), &mut json!({})).unwrap()
    }

    #[test]
    fn selection_follows_t3_rules() {
        // System mode follows macOS; a theme with a dark variant switches with it.
        let p = pick(&[(KEY_THEME, "t3-chat"), (KEY_MODE, "system")], true);
        assert_eq!((p.source.as_str(), p.mode, p.palette["sidebar"].as_str()), ("t3:t3-chat", Mode::Dark, "#171018"));
        assert_eq!(pick(&[(KEY_THEME, "t3-chat"), (KEY_MODE, "system")], false).mode, Mode::Light);
        // A light-only theme stays light even when dark is wanted.
        assert_eq!(pick(&[(KEY_THEME, "grove"), (KEY_MODE, "dark")], true).mode, Mode::Light);
        // Legacy ids map; unknown ids fall back to the neutral system palette.
        assert_eq!(pick(&[(KEY_THEME, "t3-grove")], false).source, "t3:grove");
        let p = pick(&[(KEY_THEME, "nope")], true);
        assert_eq!((p.source.as_str(), p.palette["sidebar"].as_str()), ("t3:system", "#000000"));
        // Per-appearance halves win for their mode.
        let halves = r#"{"dark":"t3-chat","light":"grove"}"#;
        let p = pick(&[(KEY_THEME, "system"), (KEY_MODE, "system"), (KEY_HALVES, halves)], false);
        assert_eq!(p.source, "t3:grove");
        // Custom themes come from localStorage and fill gaps from the base palette.
        let custom = r##"[{"id":"mine","label":"Mine","appearance":"dark","colors":{"sidebar":"#102030"}}]"##;
        let p = pick(&[(KEY_THEME, "mine"), (KEY_CUSTOM, custom)], false);
        assert_eq!((p.mode, p.palette["sidebar"].as_str(), p.palette["sidebarForeground"].as_str()), (Mode::Dark, "#102030", "#f4f4f5"));
    }

    #[test]
    fn view_meets_contrast_floors() {
        let mut inp = inputs(&[], true);
        inp.settings.contrast = 200.0;
        let p = Picked {
            source: "t3:x".into(),
            name: "X".into(),
            mode: Mode::Dark,
            palette: [("sidebar", "#202020"), ("sidebarForeground", "#555555"), ("sidebarMutedForeground", "#444444"),
                      ("accent", "#ffee00"), ("accentForeground", "#ffffff")]
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        };
        let mut report = json!({});
        let v = build_view(&p, &inp, &mut report);
        for (f, b, ratio, req) in contrast_table(&v) {
            if f != "accent" {
                assert!(ratio >= req.min(4.5), "{f} on {b}: {ratio} < {req}");
            }
        }
        assert_eq!(v.text, 0xff_ffff, "contrast 200 drives text to white");
        assert!(!report["adjusted"].as_array().unwrap().is_empty());
    }

    #[test]
    fn generated_palette_is_readable() {
        let b = bundle();
        let cat = Catalog { bundle: Some(&b), custom: Vec::new(), env: Vec::new() };
        let p = generate(Mode::Dark, "#1a1b26", "#7aa2f7", &cat);
        let c = |k: &str| color::parse(&p[k]).unwrap().0;
        assert_eq!(p["canvas"], "#1a1b26");
        assert!(color::contrast(c("text"), c("canvas")) >= 7.0);
        assert!(color::contrast(c("sidebarForeground"), c("sidebar")) >= 4.6);
        assert!(color::contrast(c("accentForeground"), c("accent")) >= 4.5);
    }
}
