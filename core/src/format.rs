use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Timelike};
use regex::{Captures, Regex};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];
const DAYS: [&str; 7] = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
];

// Longest tokens first within a shared prefix.
const TOKENS: &[&str] = &[
    "YYYY", "yyyy", "YY", "GGGG", "gggg", "Q", "MMMM", "MMM", "MM", "M", "Do", "DDDD", "DDD",
    "DD", "D", "dddd", "ddd", "dd", "d", "E", "WW", "W", "ww", "w", "HH", "H", "hh", "h", "kk",
    "k", "mm", "m", "ss", "s", "SSS", "A", "a", "ZZ", "Z", "X", "x"
];

fn locale_week(d: NaiveDate) -> (i32, i64) {
    // "en" locale: Sunday start, week 1 contains Jan 1.
    let sun = d - Duration::days(d.weekday().num_days_from_sunday() as i64);
    let year = (sun + Duration::days(6)).year();
    let jan1 = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let w1 = jan1 - Duration::days(jan1.weekday().num_days_from_sunday() as i64);
    (year, (sun - w1).num_days() / 7 + 1)
}

fn ordinal(n: u32) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

fn token(dt: &DateTime<Local>, t: &str) -> String {
    let d = dt.date_naive();
    let h12 = if dt.hour() % 12 == 0 { 12 } else { dt.hour() % 12 };
    let off = dt.offset().local_minus_utc() / 60;
    let off_str = |sep: &str| {
        format!(
            "{}{:02}{}{:02}",
            if off >= 0 { '+' } else { '-' },
            off.abs() / 60,
            sep,
            off.abs() % 60
        )
    };
    match t {
        "YYYY" | "yyyy" => format!("{:04}", d.year()),
        "YY" => format!("{:02}", d.year().rem_euclid(100)),
        "GGGG" => d.iso_week().year().to_string(),
        "gggg" => locale_week(d).0.to_string(),
        "Q" => ((d.month() - 1) / 3 + 1).to_string(),
        "MMMM" => MONTHS[d.month0() as usize].to_string(),
        "MMM" => MONTHS[d.month0() as usize][..3].to_string(),
        "MM" => format!("{:02}", d.month()),
        "M" => d.month().to_string(),
        "Do" => ordinal(d.day()),
        "DDDD" => format!("{:03}", d.ordinal()),
        "DDD" => d.ordinal().to_string(),
        "DD" => format!("{:02}", d.day()),
        "D" => d.day().to_string(),
        "dddd" => DAYS[d.weekday().num_days_from_sunday() as usize].to_string(),
        "ddd" => DAYS[d.weekday().num_days_from_sunday() as usize][..3].to_string(),
        "dd" => DAYS[d.weekday().num_days_from_sunday() as usize][..2].to_string(),
        "d" => d.weekday().num_days_from_sunday().to_string(),
        "E" => d.weekday().number_from_monday().to_string(),
        "WW" => format!("{:02}", d.iso_week().week()),
        "W" => d.iso_week().week().to_string(),
        "ww" => format!("{:02}", locale_week(d).1),
        "w" => locale_week(d).1.to_string(),
        "HH" => format!("{:02}", dt.hour()),
        "H" => dt.hour().to_string(),
        "hh" => format!("{h12:02}"),
        "h" => h12.to_string(),
        "kk" => format!("{:02}", if dt.hour() == 0 { 24 } else { dt.hour() }),
        "k" => (if dt.hour() == 0 { 24 } else { dt.hour() }).to_string(),
        "mm" => format!("{:02}", dt.minute()),
        "m" => dt.minute().to_string(),
        "ss" => format!("{:02}", dt.second()),
        "s" => dt.second().to_string(),
        "SSS" => format!("{:03}", dt.timestamp_subsec_millis()),
        "A" => (if dt.hour() < 12 { "AM" } else { "PM" }).into(),
        "a" => (if dt.hour() < 12 { "am" } else { "pm" }).into(),
        "ZZ" => off_str(""),
        "Z" => off_str(":"),
        "X" => dt.timestamp().to_string(),
        "x" => dt.timestamp_millis().to_string(),
        other => other.to_string(),
    }
}

/// A subset of moment.js formatting (what Obsidian users typically write).
pub fn moment_format(dt: &DateTime<Local>, fmt: &str) -> String {
    let mut out = String::new();
    let mut rest = fmt;
    'outer: while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('[') {
            if let Some(end) = r.find(']') {
                out.push_str(&r[..end]);
                rest = &r[end + 1..];
                continue;
            }
        }
        for t in TOKENS {
            if let Some(r) = rest.strip_prefix(t) {
                out.push_str(&token(dt, t));
                rest = r;
                continue 'outer;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

static GLOBAL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\{\{GLOBAL_VAR::([^}]+)\}\}").unwrap());
static DATE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\{\{DATE([+-]\d+)?(?::([^}]*))?\}\}").unwrap());
static TIME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\{\{TIME\}\}").unwrap());
static VALUE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\{\{VALUE(?::[^}]*)?\}\}").unwrap());
static OFF_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([+-]\d+)$").unwrap());

/// QuickAdd format syntax: `{{VALUE}}`, `{{VALUE:name}}`, `{{DATE}}`, `{{DATE:fmt}}`,
/// `{{DATE+n}}`, `{{DATE:fmt+n}}`, `{{TIME}}`, `{{GLOBAL_VAR::name}}` and literal `\n`.
pub fn format_text(
    tpl: &str,
    value: &str,
    now: &DateTime<Local>,
    globals: &HashMap<String, String>,
) -> String {
    let s = GLOBAL_RE.replace_all(tpl, |c: &Captures| {
        globals.get(c[1].trim()).cloned().unwrap_or_else(|| c[0].to_string())
    });
    let s = s.replace("\\n", "\n");
    let s = DATE_RE.replace_all(&s, |c: &Captures| {
        let mut fmt = c.get(2).map(|m| m.as_str().to_string());
        let mut off: i64 = c.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        if let Some(f) = fmt.clone() {
            if let Some(m) = OFF_RE.find(&f) {
                off = m.as_str().parse().unwrap_or(0);
                fmt = Some(f[..m.start()].to_string());
            }
        }
        let fmt = fmt.filter(|f| !f.is_empty()).unwrap_or_else(|| "YYYY-MM-DD".into());
        moment_format(&(*now + Duration::days(off)), &fmt)
    });
    let s = TIME_RE.replace_all(&s, |_: &Captures| moment_format(now, "HH:mm"));
    VALUE_RE.replace_all(&s, |_: &Captures| value.to_string()).into_owned()
}
