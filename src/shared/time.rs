//! UTC date/time formatting without a date crate.

const SECS_PER_DAY: i64 = 86_400;

/// `YYYY-MM-DD HH:MM` for a Unix timestamp.
pub fn fmt_time(secs: i64) -> String {
    let (y, m, d) = civil(secs.div_euclid(SECS_PER_DAY));
    let rem = secs.rem_euclid(SECS_PER_DAY);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, rem / 3600, rem % 3600 / 60)
}

/// The current time as `YYYY-MM-DD HH:MM:SS`.
pub fn now_string() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    format!("{}:{:02}", fmt_time(secs), secs.rem_euclid(60))
}

/// Days since 1970-01-01 to (year, month, day). Howard Hinnant's algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::fmt_time;

    #[test]
    fn formats_known_timestamps() {
        assert_eq!(fmt_time(0), "1970-01-01 00:00");
        assert_eq!(fmt_time(1_700_000_000), "2023-11-14 22:13");
    }
}
