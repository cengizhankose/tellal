//! Lucide UI icons, provider glyphs and T3 project icons from the embedded assets. SVGs render
//! as single-colour masks tinted by `text_color`.

use std::collections::HashMap;
use std::sync::LazyLock;

use gpui::{Hsla, Svg, prelude::*, px, svg};

use crate::assets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Logo,
    ChevronRight,
    ChevronDown,
    Search,
    X,
    Minimize,
    Maximize,
    Bell,
    CircleCheck,
    CircleAlert,
    CirclePause,
    CircleDot,
    /// Running: T3's static dashed ring (never spun).
    CircleDashed,
    /// T3's "No project" group icon.
    MessageSquareDashed,
    /// T3's fallback project icon.
    FolderCode,
    TriangleAlert,
    Pause,
    Play,
    SkipForward,
    ChevronsRight,
    Volume1,
    Volume2,
    VolumeX,
    MessageSquare,
    ArrowUpRight,
    Pin,
    Archive,
    Gauge,
    Mic,
    Send,
    Folder,
    Check,
    /// Computer use: an agent drives the pointer.
    MousePointer2,
    /// Emergency stop (`Durdur`).
    OctagonX,
    /// Computer-use permissions missing.
    ShieldAlert,
    /// Voice download.
    Download,
    /// New task.
    Plus,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Icon::Logo => "logo.svg",
            Icon::ChevronRight => "icons/chevron-right.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::Search => "icons/search.svg",
            Icon::X => "icons/x.svg",
            Icon::Minimize => "icons/minimize-2.svg",
            Icon::Maximize => "icons/maximize-2.svg",
            Icon::Bell => "icons/bell.svg",
            Icon::CircleCheck => "icons/circle-check.svg",
            Icon::CircleAlert => "icons/circle-alert.svg",
            Icon::CirclePause => "icons/circle-pause.svg",
            Icon::CircleDot => "icons/circle-dot.svg",
            Icon::CircleDashed => "icons/circle-dashed.svg",
            Icon::MessageSquareDashed => "icons/message-square-dashed.svg",
            Icon::FolderCode => "icons/folder-code.svg",
            Icon::TriangleAlert => "icons/triangle-alert.svg",
            Icon::Pause => "icons/pause.svg",
            Icon::Play => "icons/play.svg",
            Icon::SkipForward => "icons/skip-forward.svg",
            Icon::ChevronsRight => "icons/chevrons-right.svg",
            Icon::Volume1 => "icons/volume-1.svg",
            Icon::Volume2 => "icons/volume-2.svg",
            Icon::VolumeX => "icons/volume-x.svg",
            Icon::MessageSquare => "icons/message-square.svg",
            Icon::ArrowUpRight => "icons/arrow-up-right.svg",
            Icon::Pin => "icons/pin.svg",
            Icon::Archive => "icons/archive.svg",
            Icon::Gauge => "icons/gauge.svg",
            Icon::Mic => "icons/mic.svg",
            Icon::Send => "icons/send.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::Check => "icons/check.svg",
            Icon::MousePointer2 => "icons/mouse-pointer-2.svg",
            Icon::OctagonX => "icons/octagon-x.svg",
            Icon::ShieldAlert => "icons/shield-alert.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::Download => "icons/download.svg",
        }
    }

    pub const ALL: [Icon; 37] = [
        Icon::Logo,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::Search,
        Icon::X,
        Icon::Minimize,
        Icon::Maximize,
        Icon::Bell,
        Icon::CircleCheck,
        Icon::CircleAlert,
        Icon::CirclePause,
        Icon::CircleDot,
        Icon::CircleDashed,
        Icon::MessageSquareDashed,
        Icon::FolderCode,
        Icon::TriangleAlert,
        Icon::Pause,
        Icon::Play,
        Icon::SkipForward,
        Icon::ChevronsRight,
        Icon::Volume1,
        Icon::Volume2,
        Icon::VolumeX,
        Icon::MessageSquare,
        Icon::ArrowUpRight,
        Icon::Pin,
        Icon::Archive,
        Icon::Gauge,
        Icon::Mic,
        Icon::Send,
        Icon::Folder,
        Icon::Check,
        Icon::MousePointer2,
        Icon::OctagonX,
        Icon::ShieldAlert,
        Icon::Plus,
        Icon::Download,
    ];
}

pub fn icon(i: Icon, size: f32, color: impl Into<Hsla>) -> Svg {
    svg().path(i.path()).flex_none().size(px(size)).text_color(color.into())
}

/// An embedded SVG by asset path.
pub fn asset_svg(path: &'static str, size: f32, color: impl Into<Hsla>) -> Svg {
    svg().path(path).flex_none().size(px(size)).text_color(color.into())
}

/// UI icons the code references but the asset tree lacks (logged once at startup as `ui.assets`).
pub fn missing() -> Vec<&'static str> {
    Icon::ALL.iter().map(|i| i.path()).filter(|p| assets::get(p).is_none()).collect()
}

/// A provider's mark as T3 draws it (`providers/providers.json`).
pub struct ProviderMark {
    /// Glyph asset; `None` = monogram tile only.
    pub glyph: Option<&'static str>,
    /// Brand colour (dark, light); `None` = muted foreground.
    pub color: Option<(u32, u32)>,
    /// Display name for the tooltip (`omp (firstmate)`, `Claude`, …).
    pub label: String,
    /// Two-letter tile T3 adds to generic marks (ACP registry instances show `OM`, `OF`).
    pub monogram: Option<String>,
}

struct Driver {
    file: Option<&'static str>,
    label: String,
    color: Option<(u32, u32)>,
}

struct Catalog {
    drivers: HashMap<String, Driver>,
    /// Instance id → (driver kind, display name).
    instances: HashMap<String, (String, Option<String>)>,
    /// T3 project colour name → (dark, light) `0xRRGGBB`.
    colors: HashMap<String, (u32, u32)>,
    /// Stored project icon name → file stem, where they differ.
    aliases: HashMap<String, String>,
}

fn hex(v: &serde_json::Value) -> Option<u32> {
    u32::from_str_radix(v.as_str()?.trim_start_matches('#'), 16).ok()
}

fn dark_light(v: &serde_json::Value) -> Option<(u32, u32)> {
    Some((hex(v.get("dark")?)?, hex(v.get("light")?)?))
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let json = |path: &str| -> serde_json::Value {
        assets::get(path).and_then(|b| serde_json::from_slice(b).ok()).unwrap_or_default()
    };
    let obj = |v: &serde_json::Value, key: &str| v.get(key).and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let providers = json("providers/providers.json");
    let drivers = obj(&providers, "drivers")
        .into_iter()
        .map(|(kind, d)| {
            let file = d.get("file").and_then(|f| f.as_str()).and_then(assets::path);
            let label = d.get("label").and_then(|l| l.as_str()).unwrap_or(&kind).to_owned();
            let color = d.get("color").and_then(dark_light);
            (kind, Driver { file, label, color })
        })
        .collect();
    let instances = obj(&providers, "instances")
        .into_iter()
        .filter_map(|(id, i)| {
            let driver = i.get("driver")?.as_str()?.to_owned();
            let name = i.get("displayName").and_then(|n| n.as_str()).map(str::to_owned);
            Some((id, (driver, name)))
        })
        .collect();
    let projects = json("icons/project-icons.json");
    let colors = obj(&projects, "colors").into_iter().filter_map(|(name, v)| Some((name, dark_light(&v)?))).collect();
    let aliases =
        obj(&projects, "aliases").into_iter().filter_map(|(name, v)| Some((name, v.as_str()?.to_owned()))).collect();
    Catalog { drivers, instances, colors, aliases }
});

/// T3's lookup: instance id → driver (else the id is the driver kind) → mark; unknown → monogram.
pub fn provider_mark(provider: &str) -> ProviderMark {
    let catalog = &*CATALOG;
    let (kind, display) = match catalog.instances.get(provider) {
        Some((driver, name)) => (driver.as_str(), name.clone()),
        None => (provider, None),
    };
    match catalog.drivers.get(kind) {
        Some(driver) => {
            let label = display.unwrap_or_else(|| driver.label.clone());
            // The ACP registry mark is generic; T3 adds the instance's initials.
            let monogram = (kind == "acpRegistry").then(|| initials(provider));
            ProviderMark { glyph: driver.file, color: driver.color, label, monogram }
        }
        None => {
            ProviderMark { glyph: None, color: None, label: provider.to_owned(), monogram: Some(initials(provider)) }
        }
    }
}

/// T3's `providerInstanceInitials`: first letters of the first two words (`omp-firstmate` → `OF`),
/// else the first two letters.
pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let out: String = match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(2).collect(),
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
    };
    out.to_uppercase()
}

/// Lucide glyph for a stored T3 project icon name (T3's fallback: `folder-code`).
pub fn project_glyph(name: &str) -> &'static str {
    let stem = CATALOG.aliases.get(name).map_or(name, String::as_str);
    let path = format!("icons/{stem}.svg");
    assets::path(&path).unwrap_or(Icon::FolderCode.path())
}

/// T3 project colour name → hex for the current appearance.
pub fn project_color(name: Option<&str>, dark: bool) -> Option<u32> {
    let (d, l) = *CATALOG.colors.get(name?)?;
    Some(if dark { d } else { l })
}

pub fn provider_count() -> usize {
    CATALOG.drivers.len()
}

/// T3's colour order for generated project identities (`projectIdentity.ts`).
const IDENTITY_COLORS: [&str; 18] = [
    "gray", "red", "orange", "amber", "yellow", "lime", "green", "emerald", "teal", "cyan", "sky", "blue", "indigo",
    "violet", "purple", "fuchsia", "pink", "rose",
];

/// T3's project monogram for a project without an icon: first glyph of the first word, then the
/// first digit inside that word, else the first glyph of the last word, else its last glyph
/// (`demo` → `DO`, `Mobile App` → `MA`); empty → `PR`.
pub fn project_monogram(name: &str) -> String {
    let words: Vec<&str> = name.trim().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let Some(first) = words.first() else {
        return "PR".to_owned();
    };
    let head = first.chars().next().unwrap_or('P');
    let second = first
        .chars()
        .skip(1)
        .find(char::is_ascii_digit)
        .or_else(|| (words.len() > 1).then(|| words[words.len() - 1].chars().next()).flatten())
        .or_else(|| first.chars().last());
    head.to_uppercase().chain(second.into_iter().flat_map(char::to_uppercase)).collect()
}

/// T3's generated colour name for a project (`idx = (idx * 31 + c) % 18` over the lowercased name).
pub fn project_identity_color(name: &str) -> &'static str {
    let seed = name.trim().to_lowercase();
    let seed = if seed.is_empty() { "project".to_owned() } else { seed };
    let idx = seed.chars().fold(0u64, |idx, c| (idx * 31 + u64::from(c)) % 18);
    IDENTITY_COLORS[idx as usize]
}
