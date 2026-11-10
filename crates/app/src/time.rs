//! RFC 3339 parsing and the Turkish relative times rows show (`şimdi`, `3 dk`, `2 sa`, `dün`).

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// T3's compact sidebar time in Turkish: `şimdi` (< 1 min), `5dk`, `3sa`, `2g` (floored, no
/// upper bound); `–` if T3 sent no parseable time.
pub fn relative(then: Option<i64>, now: i64) -> String {
    let Some(then) = then else {
        return "–".to_owned();
    };
    let secs = (now - then).max(0);
    match secs {
        0..60 => "şimdi".to_owned(),
        60..3600 => format!("{}dk", secs / 60),
        3600..86400 => format!("{}sa", secs / 3600),
        _ => format!("{}g", secs / 86400),
    }
}

/// Working timer at minute resolution (`<1dk`, `16dk`, `1sa 9dk`), so it re-renders once a minute.
pub fn working(since: i64, now: i64) -> String {
    let mins = (now - since).max(0) / 60;
    match mins {
        0 => "<1dk".to_owned(),
        1..60 => format!("{mins}dk"),
        _ if mins % 60 == 0 => format!("{}sa", mins / 60),
        _ => format!("{}sa {}dk", mins / 60, mins % 60),
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
