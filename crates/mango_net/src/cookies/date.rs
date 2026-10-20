//! HTTP date parsing for cookie `Expires` attributes (RFC 6265 §5.1.1 / RFC 7231 §7.1.1.1).

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Parses the RFC 1123 / RFC 850 / asctime date formats used by `Expires`.
pub fn parse_http_date(input: &str) -> Option<u64> {
    let cleaned: String = input.replace([',', '-'], " ").chars().collect();
    let tokens: Vec<String> = cleaned
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .collect();
    if tokens.len() < 3 {
        return None;
    }

    let mut day: Option<u32> = None;
    let mut month: Option<u32> = None;
    let mut year: Option<i64> = None;
    let mut time: Option<(u32, u32, u32)> = None;

    for token in &tokens {
        if time.is_none() && token.contains(':') {
            let mut parts = token.split(':');
            let h = parts.next().and_then(|v| v.parse::<u32>().ok());
            let m = parts.next().and_then(|v| v.parse::<u32>().ok());
            let s = parts
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            if let (Some(h), Some(m)) = (h, m) {
                time = Some((h, m, s));
                continue;
            }
        }
        if month.is_none()
            && let Some(idx) = MONTHS.iter().position(|m| token.starts_with(m))
        {
            month = Some(idx as u32 + 1);
            continue;
        }
        if let Ok(num) = token.parse::<i64>() {
            if day.is_none() && (1..=31).contains(&num) {
                day = Some(num as u32);
            } else if year.is_none() {
                // RFC 6265 §5.1.1: 70..=99 → 19xx, 00..=69 → 20xx.
                year = Some(match num {
                    0..=69 => num + 2000,
                    70..=99 => num + 1900,
                    _ => num,
                });
            }
        }
    }

    let (day, month, year, time) = (day?, month?, year?, time?);
    if year < 1601 {
        return None;
    }
    Some(
        days_from_civil(year, month, day) * 86_400
            + time.0 as u64 * 3600
            + time.1 as u64 * 60
            + time.2 as u64,
    )
}

/// Howard Hinnant's days-from-civil algorithm (days since 1970-01-01).
fn days_from_civil(y: i64, m: u32, d: u32) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64;
    let mp = ((m + 9) % 12) as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe as i64 - 719_468;
    days.max(0) as u64
}
