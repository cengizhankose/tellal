//! Pure speakable-text extraction: (thread status, title, messages) → (Turkish text, rule name),
//! and one live assistant message → its spoken line.

use crate::t3::Message;

/// Max chars of the completed/idle summary.
const SUMMARY_MAX_CHARS: usize = 220;
/// Max chars spoken for one live message without a `🔊` line.
const LIVE_MAX_CHARS: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extracted {
    pub text: String,
    /// `speaker_line` | `summary` | `running` | `stopped` | `empty` (catch-up);
    /// `speaker_line` | `first_sentence` | `empty` (live).
    pub rule: &'static str,
}

/// The normalized `🔊` line of a message, if it has a non-empty one.
fn speaker_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim_start).find(|l| l.starts_with('🔊'))?;
    Some(normalize(line.trim_start_matches('🔊'))).filter(|t| !t.is_empty())
}

/// What to say for one new assistant message: its `🔊` line, else its first sentence (at most
/// [`LIVE_MAX_CHARS`] chars). `rule == "empty"` (empty text) when nothing is speakable.
pub fn live_line(text: &str) -> Extracted {
    if let Some(text) = speaker_line(text) {
        return Extracted { text, rule: "speaker_line" };
    }
    match split_sentences(&normalize(text)).into_iter().next() {
        Some(first) if first.chars().count() > LIVE_MAX_CHARS => {
            Extracted { text: truncate_words(&first, LIVE_MAX_CHARS), rule: "first_sentence" }
        }
        Some(first) => Extracted { text: first, rule: "first_sentence" },
        None => Extracted { text: String::new(), rule: "empty" },
    }
}

/// Picks what to say about a thread. Messages are oldest → newest.
pub fn extract(status: &str, title: &str, messages: &[Message]) -> Extracted {
    let latest_run = messages.last().and_then(|m| m.run_id.as_deref());
    let last_assistant_of_run = messages
        .iter()
        .rev()
        .take_while(|m| latest_run.is_none() || m.run_id.as_deref() == latest_run)
        .find(|m| m.role == "assistant");
    if let Some(text) = last_assistant_of_run.and_then(|m| speaker_line(&m.text)) {
        return Extracted { text, rule: "speaker_line" };
    }
    let last_assistant = messages.iter().rev().find(|m| m.role == "assistant").map(|m| normalize(&m.text));
    let last_assistant = last_assistant.filter(|t| !t.is_empty());
    match status {
        "running" => {
            if let Some(t) = &last_assistant
                && let Some(first) = split_sentences(t).into_iter().next()
            {
                return Extracted { text: format!("Hâlâ çalışıyor. {first}"), rule: "running" };
            }
        }
        "interrupted" | "cancelled" => {
            let text = match last_assistant.as_deref().map(summary) {
                Some(s) => format!("Durduruldu. {s}"),
                None => "Durduruldu.".to_owned(),
            };
            return Extracted { text, rule: "stopped" };
        }
        _ => {
            if let Some(t) = &last_assistant {
                return Extracted { text: summary(t), rule: "summary" };
            }
        }
    }
    Extracted { text: format!("{} için mesaj yok.", normalize(title)), rule: "empty" }
}

/// First 1–2 sentences, at most [`SUMMARY_MAX_CHARS`] chars.
fn summary(normalized: &str) -> String {
    let sentences = split_sentences(normalized);
    let Some(first) = sentences.first() else { return String::new() };
    if first.chars().count() > SUMMARY_MAX_CHARS {
        return truncate_words(first, SUMMARY_MAX_CHARS);
    }
    match sentences.get(1) {
        Some(second) if first.chars().count() + 1 + second.chars().count() <= SUMMARY_MAX_CHARS => {
            format!("{first} {second}")
        }
        _ => first.clone(),
    }
}

fn truncate_words(s: &str, max: usize) -> String {
    let cut = s.char_indices().nth(max).map_or(s.len(), |(i, _)| i);
    let head = &s[..cut];
    let head = head.rfind(' ').map_or(head, |i| &head[..i]);
    format!("{}.", head.trim_end_matches([',', ';', ':', '-', ' ']))
}

/// Splits on `.`, `!`, `?`, `…` followed by whitespace (so `1.5` and `v2.0` stay intact).
pub fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(c, '.' | '!' | '?' | '…') {
            // Swallow runs like "?!" or "...".
            let mut end = i + c.len_utf8();
            while let Some(&(j, n)) = chars.peek() {
                if matches!(n, '.' | '!' | '?' | '…' | '"' | '\'' | ')' | '”' | '’') {
                    end = j + n.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            if chars.peek().is_none_or(|&(_, n)| n.is_whitespace()) {
                let s = text[start..end].trim();
                if !s.is_empty() {
                    out.push(s.to_owned());
                }
                start = end;
            }
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

/// Markdown/chat text → plain speakable text (see crate docs for the rules).
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || line.starts_with('👀') || line.starts_with('|') || is_rule_line(line) {
            continue;
        }
        let line = strip_block_markers(line);
        let line = clean_inline(line);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            // Headings and bullets rarely end with punctuation; give the voice a pause.
            if !out.ends_with(['.', '!', '?', '…', ':', ';', ',']) {
                out.push('.');
            }
            out.push(' ');
        }
        out.push_str(line);
    }
    out
}

fn is_rule_line(line: &str) -> bool {
    line.len() >= 3 && line.chars().all(|c| matches!(c, '-' | '*' | '_' | '=' | ' '))
}

fn strip_block_markers(mut line: &str) -> &str {
    loop {
        let before = line;
        line = line.trim_start_matches('#').trim_start_matches('>').trim_start();
        for bullet in ["- [ ] ", "- [x] ", "- ", "* ", "+ ", "• "] {
            if let Some(rest) = line.strip_prefix(bullet) {
                line = rest.trim_start();
            }
        }
        let digits = line.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && digits <= 3 {
            if let Some(rest) = line[digits..].strip_prefix(". ").or_else(|| line[digits..].strip_prefix(") ")) {
                line = rest.trim_start();
            }
        }
        if line == before {
            return line;
        }
    }
}

/// Inline cleanup: links/images → label, inline code, bare URLs, emphasis markers, emoji.
fn clean_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        // ![alt](url) / [label](url)
        if c == '[' || (c == '!' && rest[1..].starts_with('[')) {
            let open = if c == '!' { 1 } else { 0 };
            if let Some((label, consumed)) = parse_link(&rest[open..]) {
                out.push_str(&clean_inline(label));
                rest = &rest[open + consumed..];
                continue;
            }
        }
        if c == '`' {
            let ticks = rest.bytes().take_while(|&b| b == b'`').count();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence) {
                let code = rest[ticks..ticks + end].trim();
                out.push_str(&speak_code(code));
                rest = &rest[ticks + end + ticks..];
                continue;
            }
            rest = &rest[ticks..];
            continue;
        }
        if (rest.starts_with("http://") || rest.starts_with("https://")) && at_word_start(&out) {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let url = &rest[..end];
            let trail = url.len() - url.trim_end_matches(['.', ',', ')', ';', ':', '!', '?']).len();
            out.push_str("link");
            rest = &rest[end - trail..];
            continue;
        }
        rest = &rest[c.len_utf8()..];
        match c {
            '*' | '~' => {}
            '_' => {
                let prev_word = out.chars().last().is_some_and(char::is_alphanumeric);
                let next_word = rest.chars().next().is_some_and(char::is_alphanumeric);
                if prev_word && next_word {
                    out.push(' ');
                }
            }
            c if is_emoji(c) => {}
            c if c.is_whitespace() => {
                if !out.ends_with(' ') && !out.is_empty() {
                    out.push(' ');
                }
            }
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

fn at_word_start(out: &str) -> bool {
    out.chars().last().is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '"' | '\''))
}

/// `[label](target)` at the start of `s` → (label, bytes consumed).
fn parse_link(s: &str) -> Option<(&str, usize)> {
    let close = s.find("](")?;
    let label = &s[1..close];
    if label.contains(['[', ']']) {
        return None;
    }
    let end = s[close + 2..].find(')')?;
    Some((label, close + 2 + end + 1))
}

/// Inline code: URL/hash → dropped, path → basename, anything else spoken as-is.
fn speak_code(code: &str) -> String {
    if code.contains("://") || is_hash(code) {
        return String::new();
    }
    if code.contains('/') && !code.contains(' ') {
        return code.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_owned();
    }
    code.to_owned()
}

fn is_hash(s: &str) -> bool {
    s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()) && s.bytes().any(|b| b.is_ascii_digit())
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF   // pictographs, emoticons, transport, symbols & pictographs ext.
        | 0x2600..=0x27BF   // misc symbols, dingbats
        | 0x2B00..=0x2BFF   // arrows & stars (⭐, ⬆)
        | 0x2190..=0x21FF   // arrows (→)
        | 0x2300..=0x23FF   // misc technical (⌛, ⏳)
        | 0xFE00..=0xFE0F   // variation selectors
        | 0x200D            // zero-width joiner
        | 0x20E3            // keycap
        | 0xE0020..=0xE007F // tags
    )
}
