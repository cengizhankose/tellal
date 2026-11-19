//! RFC 3339 parsing and the compact relative times rows show (`şimdi`/`now`, `3dk`/`3m`).

use std::time::{SystemTime, UNIX_EPOCH};

use crate::strings::Strings;

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// T3's compact sidebar time: `now` (< 1 min), `5m`, `3h`, `2d` (floored, no upper bound), in
/// the table's units (`şimdi`, `5dk`, `3sa`, `2g`); `–` if T3 sent no parseable time.
pub fn relative(then: Option<i64>, now: i64, s: &Strings) -> String {
    let Some(then) = then else {
        return "–".to_owned();
    };
    let secs = (now - then).max(0);
    match secs {
        0..60 => s.now.to_owned(),
        60..3600 => format!("{}{}", secs / 60, s.min),
        3600..86400 => format!("{}{}", secs / 3600, s.hour),
        _ => format!("{}{}", secs / 86400, s.day),
    }
}

/// Working timer at minute resolution (`<1m`, `16m`, `1h 9m`), so it re-renders once a minute.
pub fn working(since: i64, now: i64, s: &Strings) -> String {
    let mins = (now - since).max(0) / 60;
    let (m, h) = (s.min, s.hour);
    match mins {
        0 => format!("<1{m}"),
        1..60 => format!("{mins}{m}"),
        _ if mins % 60 == 0 => format!("{}{h}", mins / 60),
        _ => format!("{}{h} {}{m}", mins / 60, mins % 60),
    }
}

/// Parses `YYYY-MM-DDTHH:MM:SS[.fff](Z|±HH:MM)` into Unix seconds.
pub fn parse_rfc3339_unix(s: &str) -> Option<i64> {
    let num = |start: usize, len: usize| -> Option<i64> { s.get(start..start + len)?.parse().ok() };
    let (year, month, day) = (num(0, 4)?, num(5, 2)?, num(8, 2)?);
    let (hour, minute, second) = (num(11, 2)?, num(14, 2)?, num(17, 2)?);
    let bytes = s.as_bytes();
    let mut i = 19;
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
    }
    let offset = match *bytes.get(i)? {
        b'Z' | b'z' => 0,
        sign @ (b'+' | b'-') => {
            let offset = num(i + 1, 2)? * 3600 + num(i + 4, 2)? * 60;
            if sign == b'+' { offset } else { -offset }
        }
        _ => return None,
    };
    Some(days_from_civil(year, month, day) * 86400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strings::{EN, TR};

    #[test]
    fn relative_and_working_in_both_languages() {
        let now = 1_000_000;
        assert_eq!(relative(Some(now - 5), now, &TR), "şimdi");
        assert_eq!(relative(Some(now - 5), now, &EN), "now");
        assert_eq!(relative(Some(now - 300), now, &TR), "5dk");
        assert_eq!(relative(Some(now - 300), now, &EN), "5m");
        assert_eq!(relative(Some(now - 3 * 3600), now, &EN), "3h");
        assert_eq!(relative(Some(now - 2 * 86400), now, &TR), "2g");
        assert_eq!(relative(Some(now - 2 * 86400), now, &EN), "2d");
        assert_eq!(working(now - 20, now, &TR), "<1dk");
        assert_eq!(working(now - 69 * 60, now, &TR), "1sa 9dk");
        assert_eq!(working(now - 69 * 60, now, &EN), "1h 9m");
        assert_eq!(working(now - 120 * 60, now, &EN), "2h");
    }
}
