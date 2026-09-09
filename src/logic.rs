//! Date/number helpers (pure Rust plus a single `js_sys` read of the local
//! clock) and small non-panicking DOM helpers.
//!
//! Nothing in this module ever panics: all parsing is fallible and all array
//! access is bounds-checked.

use anyhow::{Result, anyhow};
use wasm_bindgen::JsCast;

/// Calendar date with 1-based month and day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ymd {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Ymd {
    /// Build a validated date; `None` for impossible dates such as 31 Feb.
    pub fn new(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        if day == 0 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self { year, month, day })
    }

    /// Today's date in the browser's local timezone.
    pub fn today() -> Result<Self> {
        let date = js_sys::Date::new_0();
        let year = date.get_full_year() as i32;
        // JavaScript months are 0-based (January = 0).
        let month = date.get_month() + 1;
        let day = date.get_date();
        Self::new(year, month, day).ok_or_else(|| anyhow!("failed to read today's date"))
    }

    /// Format as `YYYY-MM-DD` (the value format used by `<input type="date">`).
    pub fn fmt(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Parse a `YYYY-MM-DD` string; `None` for anything invalid.
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.trim().split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u32>().ok()?;
        let day = parts[2].parse::<u32>().ok()?;
        Self::new(year, month, day)
    }
}

/// Gregorian leap-year rule.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `month` (1-12) of `year`; 0 for an invalid month.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Same day one calendar month earlier, clamped to the target month's last
/// day. Examples: 31 Mar -> 28/29 Feb; 30 May -> 30 Apr; 1 Jan -> 1 Dec (prev
/// year).
pub fn previous_month_clamped(today: Ymd) -> Ymd {
    let (year, month) = if today.month == 1 {
        (today.year - 1, 12)
    } else {
        (today.year, today.month - 1)
    };
    let day = today.day.min(days_in_month(year, month));
    Ymd { year, month, day }
}

/// Index of a month inside the Indian financial year: April = 1 .. March = 12.
pub fn finance_month_index(month: u32) -> Option<u32> {
    if !(1..=12).contains(&month) {
        return None;
    }
    Some(if month >= 4 { month - 3 } else { month + 9 })
}

/// Financial year that an invoice dated in `(year, month)` belongs to:
/// e.g. Aug 2026 -> 2026, Jan 2026 -> 2025.
pub fn finance_year(year: i32, month: u32) -> Option<i32> {
    if !(1..=12).contains(&month) {
        return None;
    }
    Some(if month >= 4 { year } else { year - 1 })
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Full English month name, or `None` for an invalid month.
pub fn month_name(month: u32) -> Option<&'static str> {
    MONTH_NAMES.get(month.checked_sub(1)? as usize).copied()
}

/// Parse a non-negative finite number as entered by the user.
pub fn parse_amount(s: &str) -> Option<f64> {
    let value: f64 = s.trim().parse().ok()?;
    if value.is_finite() && value >= 0.0 {
        Some(value)
    } else {
        None
    }
}

/// `hours * rate`; `None` while either side is missing or invalid.
pub fn total_amount(hours: &str, rate: &str) -> Option<f64> {
    let total = parse_amount(hours)? * parse_amount(rate)?;
    if total.is_finite() { Some(total) } else { None }
}

/// Format money with exactly two decimals, e.g. `718865.96`.
pub fn fmt_amount(value: f64) -> String {
    format!("{value:.2}")
}

/// Display form of a user-entered numeric field (e.g. quantity or rate): the
/// value rounded to exactly two decimal places when it parses, otherwise the
/// raw text unchanged (empty/in-progress/invalid input is never mangled).
///
/// Display-only: calculations must keep using the unrounded values via
/// [`parse_amount`], never this formatted string.
pub fn fmt_decimal(s: &str) -> String {
    let trimmed = s.trim();
    // Rust's f64 parser accepts "9." and "1.5e" as complete numbers; keep
    // them raw so an in-progress entry (e.g. typing "9." on the way to "9.5")
    // is not shown as a rounded value before the user finishes.
    if trimmed.ends_with('.') || trimmed.ends_with('e') || trimmed.ends_with('E') {
        return s.to_owned();
    }
    match parse_amount(s) {
        Some(value) => format!("{value:.2}"),
        None => s.to_owned(),
    }
}

/// Percent-encode a string so it can be embedded in a `data:` URL.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Value of the `<input>` element that fired `ev`, or "" when unavailable.
pub fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

/// Value of the `<textarea>` element that fired `ev`, or "" when unavailable.
pub fn textarea_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|area| area.value())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_decimal_rounds_to_two_places() {
        assert_eq!(fmt_decimal("9.6666"), "9.67");
        assert_eq!(fmt_decimal("1125.3333"), "1125.33");
        assert_eq!(fmt_decimal("5"), "5.00");
        assert_eq!(fmt_decimal("0"), "0.00");
        assert_eq!(fmt_decimal("2.5"), "2.50");
        assert_eq!(fmt_decimal(" 7 "), "7.00");
    }

    #[test]
    fn fmt_decimal_leaves_unparseable_input_untouched() {
        assert_eq!(fmt_decimal(""), "");
        assert_eq!(fmt_decimal("9."), "9.");
        assert_eq!(fmt_decimal("1.5e"), "1.5e");
        assert_eq!(fmt_decimal("abc"), "abc");
        assert_eq!(fmt_decimal("-3"), "-3");
    }

    #[test]
    fn fmt_decimal_does_not_affect_calculation_input() {
        // totals keep using the full-precision values
        let total = total_amount("9.6666", "1125.3333").expect("valid inputs");
        assert!((total - 10878.14687778).abs() < 1e-6);
        // ...while the display shows the rounded form
        assert_eq!(fmt_decimal("9.6666"), "9.67");
    }
}
