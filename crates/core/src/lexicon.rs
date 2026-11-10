//! Spoken respellings for the English/tech words agents write inside Turkish. The voice reads
//! foreign words with Turkish spelling rules ("build" → "bu il"), so they are respelled before
//! normalizer-tr runs. Only what the model reads changes; displayed text keeps the original.
//!
//! Keys match case-insensitively on whole words; a key ending in `*` also matches inflected forms
//! ("push*" turns "pushladım" into "puşladım"). A Turkish suffix after an apostrophe stays
//! ("PR'ı" → "pi ar'ı"). The user file (`Config.lexicon_path`, `written<TAB>spoken` per line, `#`
//! comments) adds entries and overrides built-in ones.

use std::collections::HashMap;
use std::path::Path;

use regex::Regex;

/// Built-in entries: words agents use often that the voice misreads. Words normalizer-tr already
/// reads right (API, GitHub, Claude, Codex, Rust, numbers, %, times) are not listed.
const BUILTIN: &[(&str, &str)] = &[
    ("pull request", "pul rikuest"),
    ("PR", "pi ar"),
    ("PRs", "pi arlar"),
    ("T3", "ti tri"),
    ("iOS", "ay o es"),
    ("macOS", "mek o es"),
    ("UI", "yu ay"),
    ("UX", "yu eks"),
    ("CI", "si ay"),
    ("CLI", "si el ay"),
    ("MCP", "em si pi"),
    ("TTS", "ti ti es"),
    ("URL", "yu ar el"),
    ("JSON", "ceysın"),
    ("HTML", "eyç ti em el"),
    ("CSS", "si es es"),
    ("SQL", "es kü el"),
    ("README", "ridmi"),
    ("Xcode", "eks kod"),
    ("TypeScript", "tayp skript"),
    ("Swift", "svift"),
    ("Wispr", "vispır"),
    ("Chrome", "krom"),
    // "bild" blurs into a following "bitti"; whisper read "bilt" back correctly in both test lines.
    ("build*", "bilt"),
    ("deploy*", "diploy"),
    ("merge*", "mörç"),
    ("commit*", "komit"),
    ("push*", "puş"),
    ("branch*", "brenç"),
    ("release*", "rilis"),
    ("feature*", "fiçır"),
    ("update*", "apdeyt"),
    // Exact, not a stem: "bug*" would also catch Turkish "bugün".
    ("bug", "bag"),
    ("bugs", "baglar"),
    ("fix*", "fiks"),
    ("issue*", "işu"),
    ("backend*", "bek end"),
    ("server*", "sörvır"),
    ("thread*", "tred"),
    ("agent*", "eycınt"),
    ("skill*", "skil"),
    ("token*", "tokın"),
    ("workflow*", "vörkflov"),
    ("check*", "çek"),
    ("stuck", "stak"),
    ("warning*", "vorning"),
    ("app", "ep"),
    ("apps", "epler"),
];

pub struct Lexicon {
    re: Option<Regex>,
    exact: HashMap<String, String>,
    prefix: HashMap<String, String>,
    pub entries: usize,
    pub user_entries: usize,
}

impl Lexicon {
    /// Built-in entries plus the user's file (missing file = built-ins only).
    pub fn load(user_file: &Path) -> anyhow::Result<Lexicon> {
        let mut map: Vec<(String, String)> = BUILTIN.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let mut user_entries = 0;
        if let Ok(text) = std::fs::read_to_string(user_file) {
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                let Some((k, v)) = line.split_once('\t') else {
                    anyhow::bail!("{}: expected `written<TAB>spoken`: {line:?}", user_file.display());
                };
                let k = k.trim().to_owned();
                map.retain(|(old, _)| old.to_lowercase() != k.to_lowercase());
                map.push((k, v.trim().to_owned()));
                user_entries += 1;
            }
        }
        Ok(Lexicon::from_entries(&map, user_entries))
    }

    fn from_entries(entries: &[(String, String)], user_entries: usize) -> Lexicon {
        let (mut exact, mut prefix) = (HashMap::new(), HashMap::new());
        let mut alts: Vec<(String, bool)> = Vec::new();
        for (k, v) in entries {
            let (key, is_prefix) = match k.strip_suffix('*') {
                Some(stem) => (stem, true),
                None => (k.as_str(), false),
            };
            let key = key.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
            if key.is_empty() {
                continue;
            }
            if is_prefix { &mut prefix } else { &mut exact }.insert(key.clone(), v.clone());
            alts.push((key, is_prefix));
        }
        // Longest first, so "pull request" wins over a shorter overlapping key.
        alts.sort_by_key(|(k, _)| std::cmp::Reverse(k.chars().count()));
        let pattern = alts
            .iter()
            .map(|(k, is_prefix)| {
                let words: Vec<String> = k.split(' ').map(regex::escape).collect();
                format!("{}{}", words.join(r"\s+"), if *is_prefix { "" } else { r"\b" })
            })
            .collect::<Vec<_>>()
            .join("|");
        let re = (!alts.is_empty()).then(|| Regex::new(&format!(r"(?i)\b(?:{pattern})")).expect("escaped lexicon regex"));
        Lexicon { re, exact, prefix, entries: alts.len(), user_entries }
    }

    /// Text with every lexicon word replaced by its spoken form.
    pub fn apply(&self, text: &str) -> String {
        let Some(re) = &self.re else { return text.to_owned() };
        re.replace_all(text, |caps: &regex::Captures| {
            let found = caps[0].split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
            self.exact.get(&found).or_else(|| self.prefix.get(&found)).cloned().unwrap_or_else(|| caps[0].to_owned())
        })
        .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(entries: &[(&str, &str)]) -> Lexicon {
        let v: Vec<(String, String)> = entries.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        Lexicon::from_entries(&v, 0)
    }

    #[test]
    fn whole_words_suffixes_and_prefix_stems() {
        let l = lex(&[("PR", "pi ar"), ("pull request", "pul rikuest"), ("push*", "puş"), ("app", "ep")]);
        assert_eq!(l.apply("Pull  Request açıldı"), "pul rikuest açıldı");
        assert_eq!(l.apply("PR'ı açtım, 2 PR kaldı"), "pi ar'ı açtım, 2 pi ar kaldı");
        assert_eq!(l.apply("pushladım ve push ettim"), "puşladım ve puş ettim");
        // whole-word keys never match inside longer words
        assert_eq!(l.apply("apple ve PRIME"), "apple ve PRIME");
    }

    #[test]
    fn builtin_leaves_turkish_words_alone() {
        let l = lex(BUILTIN);
        let turkish = "Bugün uygulama çekti, sunucu tamam; ağaç ve fikir kaldı.";
        assert_eq!(l.apply(turkish), turkish);
        assert_eq!(l.apply("Build bitti, T3 PR'ı açtı."), "bilt bitti, ti tri pi ar'ı açtı.");
    }
}
