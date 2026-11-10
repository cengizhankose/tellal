//! Embeds every file under `assets/` (SVG icons, provider glyphs, JSON maps) into the binary as a
//! sorted `(path, bytes)` table that `assets::Assets` serves to gpui.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("svg" | "json")) {
            let rel = path.strip_prefix(root).expect("asset under root");
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            out.push((rel, path));
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("assets");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let mut src = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        let _ = writeln!(src, "    ({rel:?}, include_bytes!({:?})),", path.display().to_string());
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir")).join("assets.rs");
    std::fs::write(out, src).expect("write assets.rs");
}
