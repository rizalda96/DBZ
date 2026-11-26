//! SQL Server values as the text cells the grids show, and catalogue columns as the
//! designer's type, length and default.

use tiberius::time::{Date, DateTime, DateTime2, DateTimeOffset, SmallDateTime, Time};
use tiberius::ColumnData;

use crate::domain::Cell;

/// Binary values longer than this show as their size, not as hex.
const MAX_HEX_BYTES: usize = 4096;
/// Days from 0001-01-01 (where `date` and `datetime2` count from) to 1970-01-01.
const DAYS_0001_TO_1970: i64 = 719_162;
/// Days from 0001-01-01 to 1900-01-01 (where `datetime` and `smalldatetime` count from).
const DAYS_0001_TO_1900: i64 = 693_595;
const SECS_PER_DAY: u64 = 86_400;

pub(super) fn cell(v: ColumnData<'static>) -> Cell {
    match v {
        ColumnData::U8(x) => x.map(|n| n.to_string()),
        ColumnData::I16(x) => x.map(|n| n.to_string()),
        ColumnData::I32(x) => x.map(|n| n.to_string()),
        ColumnData::I64(x) => x.map(|n| n.to_string()),
        ColumnData::F32(x) => x.map(|n| n.to_string()),
        ColumnData::F64(x) => x.map(|n| n.to_string()),
        ColumnData::Bit(x) => x.map(|b| if b { "1" } else { "0" }.to_string()),
        ColumnData::String(x) => x.map(|s| s.into_owned()),
        ColumnData::Guid(x) => x.map(|g| g.to_string().to_uppercase()),
        ColumnData::Binary(x) => x.map(|b| binary(&b)),
        ColumnData::Numeric(x) => x.map(|n| n.to_string()),
        ColumnData::Xml(x) => x.map(|x| x.to_string()),
        ColumnData::DateTime(x) => x.map(datetime),
        ColumnData::SmallDateTime(x) => x.map(small_datetime),
        ColumnData::Time(x) => x.map(time),
        ColumnData::Date(x) => x.map(date),
        ColumnData::DateTime2(x) => x.map(datetime2),
        ColumnData::DateTimeOffset(x) => x.map(datetime_offset),
    }
}

/// `0x0A1B…`, the way T-SQL writes binary literals.
fn binary(b: &[u8]) -> String {
    if b.len() > MAX_HEX_BYTES {
        return format!("(BINARY) {} bytes", b.len());
    }
    let hex: String = b.iter().map(|byte| format!("{:02X}", byte)).collect();
    format!("0x{}", hex)
}

/// "YYYY-MM-DD" of a day counted from 0001-01-01 (proleptic Gregorian calendar).
fn day_text(days_from_0001: i64) -> String {
    // Howard Hinnant's days-to-civil algorithm, from 1970-01-01.
    let z = days_from_0001 - DAYS_0001_TO_1970 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// "HH:MM:SS" plus `digits` fraction digits of `nanos`.
fn clock_text(secs: u64, nanos: u64, digits: u32) -> String {
    let hms = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    if digits == 0 {
        return hms;
    }
    let frac = nanos / 10u64.pow(9 - digits);
    format!("{}.{:0width$}", hms, frac, width = digits as usize)
}

fn date(d: Date) -> String {
    day_text(i64::from(d.days()))
}

/// `datetime` keeps 1/300 s; shown in milliseconds, which it accepts back.
fn datetime(d: DateTime) -> String {
    let ms = (u64::from(d.seconds_fragments()) * 1000 + 150) / 300;
    format!("{} {}", day_text(DAYS_0001_TO_1900 + i64::from(d.days())), clock_text(ms / 1000, ms % 1000 * 1_000_000, 3))
}

fn small_datetime(d: SmallDateTime) -> String {
    let secs = u64::from(d.seconds_fragments()) * 60;
    format!("{} {}", day_text(DAYS_0001_TO_1900 + i64::from(d.days())), clock_text(secs, 0, 0))
}

/// (seconds, nanoseconds) since midnight, and the fraction digits the type keeps.
fn time_parts(t: Time) -> (u64, u64, u32) {
    let scale = u32::from(t.scale()).min(7);
    let per_sec = 10u64.pow(scale);
    let nanos = t.increments() % per_sec * 10u64.pow(9 - scale);
    (t.increments() / per_sec, nanos, scale)
}

fn time(t: Time) -> String {
    let (secs, nanos, digits) = time_parts(t);
    clock_text(secs, nanos, digits)
}

fn datetime2(d: DateTime2) -> String {
    let (secs, nanos, digits) = time_parts(d.time());
    format!("{} {}", date(d.date()), clock_text(secs, nanos, digits))
}

/// The server sends UTC plus an offset; shown in the offset's local time, as SSMS does.
fn datetime_offset(d: DateTimeOffset) -> String {
    let dt = d.datetime2();
    let (secs, nanos, digits) = time_parts(dt.time());
    let offset = i64::from(d.offset()) * 60;
    let local = i64::from(dt.date().days()) * SECS_PER_DAY as i64 + secs as i64 + offset;
    let (days, secs) = (local.div_euclid(SECS_PER_DAY as i64), local.rem_euclid(SECS_PER_DAY as i64) as u64);
    let sign = if offset < 0 { '-' } else { '+' };
    let mins = d.offset().unsigned_abs();
    format!("{} {} {}{:02}:{:02}", day_text(days), clock_text(secs, nanos, digits), sign, mins / 60, mins % 60)
}

/// (type, length, decimals) from `sys.columns`: byte lengths become characters for
/// the N types, -1 becomes `max`, and decimals and times keep their precision.
pub(super) fn column_type(name: &str, max_length: i64, precision: &str, scale: &str) -> (String, String, String) {
    let chars = |per_char: i64| if max_length < 0 { "max".to_string() } else { (max_length / per_char).to_string() };
    let (length, decimals) = match name.to_ascii_lowercase().as_str() {
        "varchar" | "char" | "varbinary" | "binary" => (chars(1), String::new()),
        "nvarchar" | "nchar" => (chars(2), String::new()),
        "decimal" | "numeric" => (precision.to_string(), scale.to_string()),
        "datetime2" | "time" | "datetimeoffset" => (scale.to_string(), String::new()),
        _ => (String::new(), String::new()),
    };
    (name.to_string(), length, decimals)
}

/// A default as the designer shows it: `((0))` → `0`, `(getdate())` → `getdate()`.
pub(super) fn default_text(definition: &str) -> String {
    let mut s = definition.trim();
    while s.len() >= 2 && encloses(s) {
        s = s[1..s.len() - 1].trim();
    }
    s.to_string()
}

/// Whether `s` is one parenthesised group from its first character to its last.
fn encloses(s: &str) -> bool {
    if !s.starts_with('(') {
        return false;
    }
    let (mut depth, mut quoted) = (0, false);
    for (i, c) in s.char_indices() {
        match c {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return i == s.len() - 1;
                }
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{cell, column_type, default_text};
    use tiberius::time::{Date, DateTime, DateTime2, DateTimeOffset, SmallDateTime, Time};
    use tiberius::ColumnData;

    #[test]
    fn dates_and_times() {
        assert_eq!(cell(ColumnData::Date(Some(Date::new(0)))).as_deref(), Some("0001-01-01"));
        // 2024-02-29 is 738_944 days after 0001-01-01.
        assert_eq!(cell(ColumnData::Date(Some(Date::new(738_944)))).as_deref(), Some("2024-02-29"));
        // 1900-01-01 12:00:00.003 (1/300 s fragments).
        assert_eq!(cell(ColumnData::DateTime(Some(DateTime::new(0, 300 * 43_200 + 1)))).as_deref(), Some("1900-01-01 12:00:00.003"));
        assert_eq!(cell(ColumnData::SmallDateTime(Some(SmallDateTime::new(1, 61)))).as_deref(), Some("1900-01-02 01:01:00"));
        let t = Time::new(3_600 * 10_000_000 + 1_234_567, 7);
        assert_eq!(cell(ColumnData::Time(Some(t))).as_deref(), Some("01:00:00.1234567"));
        assert_eq!(cell(ColumnData::DateTime2(Some(DateTime2::new(Date::new(738_944), Time::new(5, 0))))).as_deref(), Some("2024-02-29 00:00:05"));
        // 23:30 UTC at +07:00 is 06:30 the next day.
        let utc = DateTime2::new(Date::new(738_944), Time::new(23 * 3_600 + 1_800, 0));
        assert_eq!(cell(ColumnData::DateTimeOffset(Some(DateTimeOffset::new(utc, 420)))).as_deref(), Some("2024-03-01 06:30:00 +07:00"));
        assert_eq!(cell(ColumnData::DateTimeOffset(Some(DateTimeOffset::new(utc, -90)))).as_deref(), Some("2024-02-29 22:00:00 -01:30"));
    }

    #[test]
    fn other_values() {
        assert_eq!(cell(ColumnData::Bit(Some(true))).as_deref(), Some("1"));
        assert_eq!(cell(ColumnData::I32(None)), None);
        assert_eq!(cell(ColumnData::Binary(Some(vec![0x0a, 0xff].into()))).as_deref(), Some("0x0AFF"));
        assert_eq!(cell(ColumnData::Binary(Some(vec![0; 5000].into()))).as_deref(), Some("(BINARY) 5000 bytes"));
    }

    #[test]
    fn column_types_and_defaults() {
        assert_eq!(column_type("nvarchar", 100, "0", "0"), ("nvarchar".into(), "50".into(), String::new()));
        assert_eq!(column_type("varbinary", -1, "0", "0").1, "max");
        assert_eq!(column_type("decimal", 9, "18", "2"), ("decimal".into(), "18".into(), "2".into()));
        assert_eq!(column_type("int", 4, "10", "0"), ("int".into(), String::new(), String::new()));
        assert_eq!(default_text("((0))"), "0");
        assert_eq!(default_text("(getdate())"), "getdate()");
        assert_eq!(default_text("(N'a)b')"), "N'a)b'");
        assert_eq!(default_text("((1)+(2))"), "(1)+(2)");
    }
}
