//! The embedded `crates/app/assets/**` tree (see `build.rs`), served to gpui's `svg()`.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(get(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ASSETS.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)).collect())
    }
}

pub fn get(path: &str) -> Option<&'static [u8]> {
    ASSETS.binary_search_by(|(p, _)| (*p).cmp(path)).ok().map(|ix| ASSETS[ix].1)
}

/// The interned asset path (a `&'static str`, so `svg().path(..)` never allocates).
pub fn path(path: &str) -> Option<&'static str> {
    ASSETS.binary_search_by(|(p, _)| (*p).cmp(path)).ok().map(|ix| ASSETS[ix].0)
}

pub fn count() -> usize {
    ASSETS.len()
}
