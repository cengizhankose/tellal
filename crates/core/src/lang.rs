//! Which language a line is in (per utterance, for `VoicePref::Auto`) and macOS's preferred
//! language (`Lang::system`).

use crate::Lang;

/// Turkish-only letters; a word containing one counts for Turkish.
const TURKISH_LETTERS: &[char] = &['ç', 'ğ', 'ı', 'ö', 'ş', 'ü', 'Ç', 'Ğ', 'İ', 'Ö', 'Ş', 'Ü'];
/// Frequent Turkish words written without Turkish letters (what an agent writes in Turkish).
const TURKISH_WORDS: &[&str] = &[
    "ve", "bir", "bu", "su", "ile", "de", "da", "ki", "mi", "mu", "ne", "var", "yok", "gibi", "ama", "sonra", "daha",
    "tamam", "evet", "hayir", "kadar", "olarak", "bitti", "hata", "hatalar", "neden", "nasil", "simdi", "hepsi",
    "oldu", "yine", "sadece", "bunu", "buna", "onu", "ona", "icin", "ise", "veya", "yani", "zaten", "artik", "henuz",
    "tekrar", "yeni", "eski", "dosya", "dosyalar", "kod", "testler", "testleri", "sorun", "hazir", "devam",
    "bekliyor", "geldi", "gitti", "aldim", "ettim", "yaptim", "baktim", "kaldi", "durum", "sunu", "biraz", "bence",
];
/// Turkish verb endings a non-Turkish word rarely has (forms with Turkish letters are caught above).
const TURKISH_ENDINGS: &[&str] = &["iyor", "uyor", "yorum", "yoruz", "acak", "ecek", "uldu", "ildi", "lendi", "dim"];
const ENGLISH_WORDS: &[&str] = &[
    "the", "and", "is", "are", "was", "were", "to", "of", "in", "on", "for", "with", "it", "this", "that", "i", "you",
    "we", "they", "not", "now", "be", "has", "have", "had", "will", "can", "all", "from", "at", "by", "an", "or",
    "but", "done", "still", "so", "my", "your", "our", "its", "it's", "i'm", "should", "would", "let", "me", "no",
    "yes", "ok", "okay", "finished", "running", "failed", "passed", "fixed", "added", "found", "there", "here",
    "what", "which", "want", "need", "next", "then", "first", "after", "before", "into", "out", "up", "about",
    "just", "only", "more", "some", "any", "every", "one", "two", "three", "stopped", "waiting", "ready", "left",
];

/// The language of `text`: Turkish letters and Turkish words/endings vote for Turkish, English
/// stopwords for English; a tie (including no evidence) is `fallback`.
pub fn detect(text: &str, fallback: Lang) -> Lang {
    let (mut tr, mut en) = (0usize, 0usize);
    for raw in text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’')) {
        let word = raw.trim_matches(['\'', '’']);
        if word.is_empty() {
            continue;
        }
        if word.contains(TURKISH_LETTERS) {
            tr += 1;
            continue;
        }
        let lower = word.to_lowercase().replace('’', "'");
        if TURKISH_WORDS.contains(&lower.as_str()) || TURKISH_ENDINGS.iter().any(|e| lower.len() > e.len() + 2 && lower.ends_with(e)) {
            tr += 1;
        } else if ENGLISH_WORDS.contains(&lower.as_str()) {
            en += 1;
        }
    }
    match tr.cmp(&en) {
        std::cmp::Ordering::Greater => Lang::Tr,
        std::cmp::Ordering::Less => Lang::En,
        std::cmp::Ordering::Equal => fallback,
    }
}

/// macOS's first preferred language (`CFLocaleCopyPreferredLanguages`, e.g. `tr-TR`, `en-US`);
/// `None` when unavailable.
#[cfg(target_os = "macos")]
pub fn preferred_language() -> Option<String> {
    use std::ffi::{CStr, c_char, c_void};
    type CFTypeRef = *const c_void;
    const UTF8: u32 = 0x0800_0100;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> CFTypeRef;
        fn CFArrayGetCount(array: CFTypeRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: isize) -> CFTypeRef;
        fn CFStringGetCString(s: CFTypeRef, buf: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFRelease(cf: CFTypeRef);
    }
    // SAFETY: CoreFoundation's documented "Copy" rule: we own the returned array and release it;
    // the element is borrowed from the array while it is alive; the buffer outlives the call.
    unsafe {
        let langs = CFLocaleCopyPreferredLanguages();
        if langs.is_null() {
            return None;
        }
        let mut buf = [0 as c_char; 64];
        let first = (CFArrayGetCount(langs) > 0).then(|| CFArrayGetValueAtIndex(langs, 0));
        let ok = first.is_some_and(|s| !s.is_null() && CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) != 0);
        CFRelease(langs);
        ok.then(|| CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn preferred_language() -> Option<String> {
    std::env::var("LANG").ok().filter(|l| !l.is_empty())
}

/// `tr*` → Turkish, anything else (or nothing) → English.
pub fn from_tag(tag: Option<&str>) -> Lang {
    if tag.is_some_and(|t| t.trim().to_ascii_lowercase().starts_with("tr")) { Lang::Tr } else { Lang::En }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkish_and_english_lines() {
        for tr in [
            "Tamam, önce hataları topluyorum.",
            "Build bitti; iOS simülatöründe ekran görüntüsü aldım.",
            "80 görevin 20'si takıldı.",
            "Pull request açıldı, deploy için onayını bekliyorum.",
            "Hata yok, testler de tamam.",
            "Hâlâ çalışıyor. Göç betiğini yazdım.",
            "Durduruldu.",
        ] {
            assert_eq!(detect(tr, Lang::En), Lang::Tr, "{tr}");
        }
        for en in [
            "Okay, I'm collecting the errors first.",
            "80 tasks: 20 stuck, 30 failed, the rest finished with warnings.",
            "npm test passed in 42 seconds.",
            "The PR is ready for review.",
            "Onboarding is now 3 screens instead of 5.",
            "Stopped.",
            "Still running. Deployed at 14:30.",
        ] {
            assert_eq!(detect(en, Lang::Tr), Lang::En, "{en}");
        }
    }

    #[test]
    fn ties_use_the_fallback() {
        assert_eq!(detect("PR #42", Lang::Tr), Lang::Tr);
        assert_eq!(detect("PR #42", Lang::En), Lang::En);
        assert_eq!(detect("", Lang::Tr), Lang::Tr);
    }

    #[test]
    fn system_tags() {
        assert_eq!(from_tag(Some("tr-TR")), Lang::Tr);
        assert_eq!(from_tag(Some("TR")), Lang::Tr);
        assert_eq!(from_tag(Some("en-GB")), Lang::En);
        assert_eq!(from_tag(Some("de")), Lang::En);
        assert_eq!(from_tag(None), Lang::En);
    }
}
