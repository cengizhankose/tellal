//! English text → what the English voice reads, the counterpart of normalizer-tr in the Turkish
//! pipeline: URLs, hashes and paths leave speech (a path keeps its file name), then times,
//! dates, money, percentages, ordinals, versions, numbers with units, plain numbers and a few
//! symbols and abbreviations are written out as words. Markdown was already removed by
//! `extract::normalize`; only what the model reads changes, displayed text keeps the original.
//! [`BUILTIN`] respells words the voice misreads (applied after this normalization).

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// Respellings for the English voice (`Lexicon` format, see `lexicon.rs`).
pub const BUILTIN: &[(&str, &str)] = &[
    // Read-back heard "Poll request"; "pul request" is heard as "Pull request".
    ("pull request", "pul request"),
    ("pull requests", "pul requests"),
    // Read-back heard "NPN", "Magnium" and "End PMM" for "npm".
    ("npm", "N P M"),
    ("pnpm", "P N P M"),
    ("CLI", "C L I"),
    ("UI", "U I"),
    ("UX", "U X"),
    ("GUI", "G U I"),
    ("JSON", "jay son"),
    ("YAML", "yamel"),
    ("macOS", "mac O S"),
    ("README", "read me"),
    ("TTS", "T T S"),
    ("MCP", "M C P"),
    ("stdout", "standard out"),
    ("stderr", "standard error"),
    ("LGTM", "looks good to me"),
    ("WIP", "work in progress"),
    ("TODO", "to do"),
    ("TODOs", "to dos"),
    ("Wispr", "whisper"),
];

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve",
    "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
const SCALES: [(u64, &str); 4] =
    [(1_000_000_000_000, "trillion"), (1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand")];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November",
    "December",
];
/// Unit after a number → (singular, plural).
const UNITS: &[(&str, &str, &str)] = &[
    ("ms", "millisecond", "milliseconds"),
    ("s", "second", "seconds"),
    ("sec", "second", "seconds"),
    ("secs", "second", "seconds"),
    ("min", "minute", "minutes"),
    ("mins", "minute", "minutes"),
    ("h", "hour", "hours"),
    ("hr", "hour", "hours"),
    ("hrs", "hour", "hours"),
    ("d", "day", "days"),
    ("b", "byte", "bytes"),
    ("kb", "kilobyte", "kilobytes"),
    ("mb", "megabyte", "megabytes"),
    ("gb", "gigabyte", "gigabytes"),
    ("tb", "terabyte", "terabytes"),
    ("k", "thousand", "thousand"),
    ("m", "million", "million"),
    ("x", "time", "times"),
    ("px", "pixel", "pixels"),
    ("fps", "frame per second", "frames per second"),
    ("hz", "hertz", "hertz"),
    ("khz", "kilohertz", "kilohertz"),
    ("ghz", "gigahertz", "gigahertz"),
    ("loc", "line of code", "lines of code"),
];
/// Whole-word abbreviations and symbols (case-insensitive).
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("e.g.", "for example"),
    ("i.e.", "that is"),
    ("etc.", "et cetera"),
    ("vs.", "versus"),
    ("vs", "versus"),
    ("approx.", "approximately"),
    ("w/o", "without"),
    ("w/", "with"),
    ("&", "and"),
    ("+", "plus"),
    ("=", "equals"),
    ("->", "to"),
    ("=>", "to"),
    ("→", "to"),
    ("~", "about"),
    ("%", "percent"),
];

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static English normalizer regex")
}

static TIME: LazyLock<Regex> =
    LazyLock::new(|| re(r"\b(\d{1,2}):(\d{2})(?::\d{2})?(?:\s*([AaPp])(?:\.[Mm]\.|\s?[Mm]\b))?"));
static DATE: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d{4})-(\d{2})-(\d{2})\b"));
static MONEY: LazyLock<Regex> = LazyLock::new(|| re(r"\$(\d{1,3}(?:,\d{3})+|\d+)(?:\.(\d{1,2}))?\b"));
static PERCENT: LazyLock<Regex> = LazyLock::new(|| re(r"(\d+(?:\.\d+)?)\s?%"));
static ORDINAL: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d+)(?:st|nd|rd|th)\b"));
static VERSION: LazyLock<Regex> = LazyLock::new(|| re(r"\b[vV]?(\d+(?:\.\d+){2,})\b|\b[vV](\d+(?:\.\d+)?)\b"));
static RANGE: LazyLock<Regex> = LazyLock::new(|| re(r"\b(\d+)\s?[-–]\s?(\d+)\b"));
static ISSUE: LazyLock<Regex> = LazyLock::new(|| re(r"#(\d+)\b"));
static UNIT: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)\b(\d+(?:\.\d+)?)\s?([a-z]{1,3})\b"));
static NUMBER: LazyLock<Regex> = LazyLock::new(|| re(r"(^|[^\w.])(-?)(\d{1,3}(?:,\d{3})+|\d+)(?:\.(\d+))?\b"));
static GLUED: LazyLock<Regex> = LazyLock::new(|| re(r"\b([A-Za-z]+)(\d+)\b"));

static SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| re(r"\s+([.,;:!?])"));
/// Written English → spoken English (see the module docs).
pub fn normalize(text: &str) -> String {
    let words: Vec<String> = text.split_whitespace().filter_map(word).collect();
    let s = words.join(" ");
    let s = DATE.replace_all(&s, |c: &Captures| date(&c[1], &c[2], &c[3]).unwrap_or_else(|| c[0].replace('-', " ")));
    let s = TIME.replace_all(&s, |c: &Captures| time(&c[1], &c[2], c.get(3).map(|m| m.as_str())));
    let s = MONEY.replace_all(&s, |c: &Captures| money(&c[1], c.get(2).map(|m| m.as_str())));
    let s = PERCENT.replace_all(&s, |c: &Captures| format!("{} percent", decimal(&c[1])));
    let s = ORDINAL.replace_all(&s, |c: &Captures| c[1].parse().map_or_else(|_| c[0].to_owned(), ordinal));
    let s = VERSION.replace_all(&s, |c: &Captures| {
        let v = c.get(1).or_else(|| c.get(2)).map_or("", |m| m.as_str());
        let spoken = v.split('.').map(digits_or_number).collect::<Vec<_>>().join(" point ");
        if c[0].starts_with(['v', 'V']) { format!("version {spoken}") } else { spoken }
    });
    let s = RANGE.replace_all(&s, |c: &Captures| format!("{} to {}", number(&c[1]), number(&c[2])));
    let s = ISSUE.replace_all(&s, |c: &Captures| format!("number {}", number(&c[1])));
    let s = UNIT.replace_all(&s, |c: &Captures| {
        let n = &c[1];
        match UNITS.iter().find(|(u, _, _)| c[2].eq_ignore_ascii_case(u)) {
            Some((_, one, many)) => format!("{} {}", decimal(n), if n == "1" { one } else { many }),
            None => c[0].to_owned(),
        }
    });
    let s = GLUED.replace_all(&s, "$1 $2");
    let s = NUMBER.replace_all(&s, |c: &Captures| {
        let minus = if c[2].is_empty() { "" } else { "minus " };
        let int = number(&c[3]);
        match c.get(4) {
            Some(frac) => format!("{}{minus}{int} point {}", &c[1], spell_digits(frac.as_str())),
            None => format!("{}{minus}{int}", &c[1]),
        }
    });
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    SPACE_BEFORE_PUNCT.replace_all(&s, "$1").into_owned()
}

/// One whitespace-separated word: dropped (hash), replaced (URL, abbreviation) or reduced to its
/// file name (path); surrounding punctuation stays.
fn word(raw: &str) -> Option<String> {
    let lead = raw.len() - raw.trim_start_matches(['(', '"', '\'', '“', '‘']).len();
    let body = &raw[lead..];
    let core = body.trim_end_matches([',', ';', ':', '!', '?', ')', '"', '\'', '”', '’', '.']);
    let (pre, post) = (&raw[..lead], &body[core.len()..]);
    let lower = core.to_lowercase();
    // A trailing period belongs to abbreviations like "e.g."; "etc." mostly ends a sentence, so
    // its period stays.
    let dotted = format!("{lower}.");
    if body.len() > core.len()
        && body[core.len()..].starts_with('.')
        && let Some((k, spoken)) = ABBREVIATIONS.iter().find(|(k, _)| *k == dotted)
    {
        let rest = if *k == "etc." { post } else { &post[1..] };
        return Some(format!("{pre}{spoken}{rest}"));
    }
    if let Some((_, spoken)) = ABBREVIATIONS.iter().find(|(k, _)| *k == lower) {
        return Some(format!("{pre}{spoken}{post}"));
    }
    if core.contains("://") || lower.starts_with("www.") {
        return Some(format!("{pre}link{post}"));
    }
    if is_hash(core) {
        // The hash leaves; punctuation after it stays (re-attached to the previous word).
        return (!post.is_empty()).then(|| post.to_owned());
    }
    if core.contains('/') {
        let is_path = core.starts_with(['/', '~', '.']) || core.matches('/').count() >= 2 || has_extension(core);
        let spoken = if is_path {
            file_name(core.trim_end_matches('/').rsplit('/').next().unwrap_or_default())
        } else {
            core.replace('/', " ")
        };
        return Some(format!("{pre}{spoken}{post}"));
    }
    if has_extension(core) && !core.contains('@') && core.chars().next().is_some_and(char::is_alphabetic) {
        return Some(format!("{pre}{}{post}", file_name(core)));
    }
    Some(raw.to_owned())
}

const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "mjs", "json", "md", "py", "swift", "toml", "yml", "yaml", "sh", "html", "css",
    "txt", "lock", "sql", "go", "kt", "java", "c", "h", "cpp", "rb", "png", "jpg", "svg", "mp4", "wav", "log", "csv",
];

fn has_extension(s: &str) -> bool {
    s.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// `main.rs` → `main dot R S`, `notes.json` → `notes dot JSON` (the lexicon respells JSON).
fn file_name(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && EXTENSIONS.contains(&ext.to_lowercase().as_str()) => {
            let stem = stem.trim_start_matches('.').replace(['.', '_', '-'], " ");
            let ext = match ext.to_lowercase().as_str() {
                "json" => "JSON".to_owned(),
                "yaml" | "yml" => "yaml".to_owned(),
                "toml" => "toml".to_owned(),
                "lock" => "lock".to_owned(),
                "swift" => "swift".to_owned(),
                e => e.to_uppercase().chars().map(String::from).collect::<Vec<_>>().join(" "),
            };
            format!("{stem} dot {ext}")
        }
        _ => name.to_owned(),
    }
}

fn is_hash(s: &str) -> bool {
    s.len() >= 7 && s.bytes().all(|b| b.is_ascii_hexdigit()) && s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().any(|b| b.is_ascii_alphabetic())
}

/// Cardinal number in words ("1,250" → "one thousand two hundred fifty"); 13+ digit numbers and
/// numbers with leading zeros are read digit by digit.
fn number(written: &str) -> String {
    let digits: String = written.chars().filter(char::is_ascii_digit).collect();
    if digits.len() > 12 || (digits.len() > 1 && digits.starts_with('0')) {
        return spell_digits(&digits);
    }
    digits.parse().map_or_else(|_| spell_digits(&digits), cardinal)
}

/// Version parts: "0" → "oh"? No: plain cardinal, but "01" digit by digit.
fn digits_or_number(part: &str) -> String {
    number(part)
}

fn decimal(written: &str) -> String {
    match written.split_once('.') {
        Some((int, frac)) => format!("{} point {}", number(int), spell_digits(frac)),
        None => number(written),
    }
}

fn spell_digits(digits: &str) -> String {
    digits.chars().filter_map(|c| c.to_digit(10)).map(|d| ONES[d as usize]).collect::<Vec<_>>().join(" ")
}

pub(crate) fn cardinal(n: u64) -> String {
    if n < 20 {
        return ONES[n as usize].to_owned();
    }
    if n < 100 {
        let (t, o) = (n / 10, n % 10);
        return if o == 0 { TENS[t as usize].to_owned() } else { format!("{} {}", TENS[t as usize], ONES[o as usize]) };
    }
    if n < 1000 {
        let (h, r) = (n / 100, n % 100);
        return if r == 0 { format!("{} hundred", ONES[h as usize]) } else { format!("{} hundred {}", ONES[h as usize], cardinal(r)) };
    }
    let (scale, name) = SCALES.iter().copied().find(|(s, _)| n >= *s).expect("n >= 1000");
    let (head, rest) = (n / scale, n % scale);
    if rest == 0 { format!("{} {name}", cardinal(head)) } else { format!("{} {name} {}", cardinal(head), cardinal(rest)) }
}

fn ordinal(n: u64) -> String {
    let words = cardinal(n);
    let (head, last) = words.rsplit_once(' ').map_or(("", words.as_str()), |(h, l)| (h, l));
    let last = match last {
        "one" => "first".to_owned(),
        "two" => "second".to_owned(),
        "three" => "third".to_owned(),
        "five" => "fifth".to_owned(),
        "eight" => "eighth".to_owned(),
        "nine" => "ninth".to_owned(),
        "twelve" => "twelfth".to_owned(),
        l if l.ends_with('y') => format!("{}ieth", &l[..l.len() - 1]),
        l => format!("{l}th"),
    };
    if head.is_empty() { last } else { format!("{head} {last}") }
}

/// "14:30" → "fourteen thirty", "3:05 PM" → "three oh five PM", "9:00" → "nine o'clock".
fn time(h: &str, m: &str, half: Option<&str>) -> String {
    let (h, m): (u64, u64) = (h.parse().unwrap_or(0), m.parse().unwrap_or(0));
    let hour = cardinal(h);
    let minutes = match m {
        0 if half.is_some() => String::new(),
        0 if h > 12 || h == 0 => " hundred".to_owned(),
        0 => " o'clock".to_owned(),
        1..=9 => format!(" oh {}", ONES[m as usize]),
        _ => format!(" {}", cardinal(m)),
    };
    match half {
        Some(p) => format!("{hour}{minutes} {}M", p.to_uppercase()),
        None => format!("{hour}{minutes}"),
    }
}

/// "2026-11-19" → "November nineteenth, twenty twenty six".
fn date(y: &str, m: &str, d: &str) -> Option<String> {
    let (y, m, d): (u64, usize, u64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    let month = MONTHS.get(m.checked_sub(1)?)?;
    (1..=31).contains(&d).then(|| format!("{month} {}, {}", ordinal(d), year(y)))
}

fn year(y: u64) -> String {
    match y {
        2000..=2009 => cardinal(y),
        1100..=2099 if y % 100 == 0 => format!("{} hundred", cardinal(y / 100)),
        1100..=2099 if y % 100 < 10 => format!("{} oh {}", cardinal(y / 100), cardinal(y % 100)),
        1100..=2099 => format!("{} {}", cardinal(y / 100), cardinal(y % 100)),
        _ => cardinal(y),
    }
}

/// "$1,250" → "one thousand two hundred fifty dollars", "$1.50" → "one dollar fifty cents".
fn money(int: &str, cents: Option<&str>) -> String {
    let dollars = number(int);
    let unit = if dollars == "one" { "dollar" } else { "dollars" };
    match cents.and_then(|c| format!("{c:0<2}").parse::<u64>().ok()).filter(|&c| c > 0) {
        Some(c) => format!("{dollars} {unit} {} {}", cardinal(c), if c == 1 { "cent" } else { "cents" }),
        None => format!("{dollars} {unit}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_units() {
        assert_eq!(normalize("80 tasks, 20 stuck."), "eighty tasks, twenty stuck.");
        assert_eq!(normalize("npm test passed in 42 seconds."), "npm test passed in forty two seconds.");
        assert_eq!(normalize("The API answers in 120 ms."), "The API answers in one hundred twenty milliseconds.");
        assert_eq!(normalize("Took 1 min, 3.5s and 10GB."), "Took one minute, three point five seconds and ten gigabytes.");
        assert_eq!(normalize("18,000 invoices at 75%."), "eighteen thousand invoices at seventy five percent.");
        assert_eq!(normalize("Cost $1.50, then $1,250."), "Cost one dollar fifty cents, then one thousand two hundred fifty dollars.");
        assert_eq!(normalize("the 2nd and 21st try"), "the second and twenty first try");
        assert_eq!(normalize("3-5 retries, -4 delta"), "three to five retries, minus four delta");
        assert_eq!(normalize("Fixed #42 in 3x less time"), "Fixed number forty two in three times less time");
        assert_eq!(normalize("Upgraded to v2.0.1 and React 19"), "Upgraded to version two point zero point one and React nineteen");
    }

    #[test]
    fn times_and_dates() {
        assert_eq!(normalize("CI finished at 3:45 PM."), "CI finished at three forty five PM.");
        assert_eq!(normalize("Deployed at 14:30; again at 9:00 and 9:05 am."), "Deployed at fourteen thirty; again at nine o'clock and nine oh five AM.");
        assert_eq!(normalize("Released 2026-11-19."), "Released November nineteenth, twenty twenty six.");
    }

    #[test]
    fn code_paths_hashes_and_urls_leave_speech() {
        assert_eq!(normalize("Edited src/panel/voice.rs and ./build.sh."), "Edited voice dot R S and build dot S H.");
        assert_eq!(normalize("Pushed 3f9a2c1d to main."), "Pushed to main.");
        assert_eq!(normalize("See https://github.com/x/y for details."), "See link for details.");
        assert_eq!(normalize("Fixed it in commit 9b8c7d6e."), "Fixed it in commit.");
        assert_eq!(normalize("client/server and/or e.g. tests, etc."), "client server and or for example tests, et cetera.");
        assert_eq!(normalize("Updated package.json & README."), "Updated package dot JSON and README.");
    }

    #[test]
    fn plain_words_stay() {
        let s = "Okay, I'm collecting the errors first, then fixing the most common one.";
        assert_eq!(normalize(s), s);
        assert_eq!(normalize("Build finished on iOS; T3 is ready."), "Build finished on iOS; T three is ready.");
    }
}
