//! ISO 8601 and RFC 3339 calendar date, time, and datetime validation engine.

use serde_json::Value;

use crate::{
    context::ValidationContext,
    error::{RuleParams, ValidationError},
    messages::MessagesProvider,
    rule::RuleContext,
    schema::Schema,
};

/// Format modes supported by the date schema validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFormat {
    /// Full RFC 3339 date-time format (e.g. `2026-11-22T14:30:00Z`).
    #[default]
    DateTime,
    /// Calendar date format (e.g. `2026-11-22`).
    DateOnly,
    /// Clock time format (e.g. `14:30:00Z`).
    TimeOnly,
    /// ISO 8601 duration format (e.g. `P3Y6M4DT12H30M5S`, `PT15M`, `P1D`).
    Duration,
}

/// Fluent schema builder for validating ISO 8601 and RFC 3339 date and time strings.
#[derive(Clone, Default)]
pub struct DateSchema {
    format: DateFormat,
    must_be_weekend: bool,
    must_be_weekday: bool,
    bail_on_fail: bool,
    inline_message: Option<String>,
}

impl DateSchema {
    /// Creates a new `DateSchema` defaulting to full datetime format.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: DateFormat::DateTime,
            must_be_weekend: false,
            must_be_weekday: false,
            bail_on_fail: false,
            inline_message: None,
        }
    }

    /// Enforces date-only format (`YYYY-MM-DD`).
    #[must_use]
    pub fn date_only(mut self) -> Self {
        self.format = DateFormat::DateOnly;
        self
    }

    /// Enforces time-only format (`HH:MM:SS[...]`).
    #[must_use]
    pub fn time_only(mut self) -> Self {
        self.format = DateFormat::TimeOnly;
        self
    }

    /// Enforces ISO 8601 duration format (`P...`).
    #[must_use]
    pub fn duration(mut self) -> Self {
        self.format = DateFormat::Duration;
        self
    }

    /// Asserts that the date falls on a weekend (Saturday or Sunday).
    #[must_use]
    pub fn weekend(mut self) -> Self {
        self.must_be_weekend = true;
        self
    }

    /// Asserts that the date falls on a weekday (Monday through Friday).
    #[must_use]
    pub fn weekday(mut self) -> Self {
        self.must_be_weekday = true;
        self
    }

    /// Configures short-circuiting on failure.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Sets an inline custom message override.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inline_message = Some(message.into());
        self
    }
}

impl Schema<String> for DateSchema {
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<String, ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
        let inline = self.inline_message.as_deref();

        let s = match value {
            Value::String(s) => s.as_str(),
            _ => {
                let issue = rule_ctx.create_issue("string", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let is_valid = match self.format {
            DateFormat::DateTime => is_valid_rfc3339_datetime(s),
            DateFormat::DateOnly => is_valid_rfc3339_date(s),
            DateFormat::TimeOnly => is_valid_rfc3339_time(s),
            DateFormat::Duration => is_valid_iso8601_duration(s),
        };

        if !is_valid {
            let rule_name = match self.format {
                DateFormat::DateTime => "datetime",
                DateFormat::DateOnly => "date",
                DateFormat::TimeOnly => "time",
                DateFormat::Duration => "duration",
            };
            let issue = rule_ctx.create_issue(rule_name, RuleParams::new(), inline);
            return Err(ValidationError::single(
                issue.field,
                issue.rule,
                issue.message,
            ));
        }

        let mut errors = ValidationError::new();
        if self.must_be_weekend && !is_weekend_date(s) {
            errors.push(rule_ctx.create_issue("weekend", RuleParams::new(), inline));
            if self.bail_on_fail {
                return Err(errors);
            }
        }

        if self.must_be_weekday && !is_weekday_date(s) {
            errors.push(rule_ctx.create_issue("weekday", RuleParams::new(), inline));
            if self.bail_on_fail {
                return Err(errors);
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(s.to_string())
    }

    fn to_json_schema_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("string".to_string()));
        let fmt = match self.format {
            DateFormat::DateTime => "date-time",
            DateFormat::DateOnly => "date",
            DateFormat::TimeOnly => "time",
            DateFormat::Duration => "duration",
        };
        map.insert("format".to_string(), Value::String(fmt.to_string()));
        Value::Object(map)
    }
}

/// Checks whether a year is a leap year in the Gregorian calendar.
#[must_use]
pub const fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Returns the number of days in a given Gregorian month (1-12) for a given year.
#[must_use]
pub const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Validates that a string is an ISO 8601 calendar date (`YYYY-MM-DD`).
#[must_use]
pub fn is_valid_rfc3339_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 10 {
        return false;
    }

    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }

    let year = match parse_4_digits(&bytes[0..4]) {
        Some(y) => y,
        None => return false,
    };
    let month = match parse_2_digits(&bytes[5..7]) {
        Some(m) if (1..=12).contains(&m) => m,
        _ => return false,
    };
    let day = match parse_2_digits(&bytes[8..10]) {
        Some(d) => d,
        None => return false,
    };

    day >= 1 && day <= days_in_month(year, month)
}

/// Validates that a string is an ISO 8601 clock time (`HH:MM:SS[.fractional](Z|[+-]HH:MM)`).
#[must_use]
pub fn is_valid_rfc3339_time(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 8 {
        return false;
    }

    if bytes[2] != b':' || bytes[5] != b':' {
        return false;
    }

    if !matches!(parse_2_digits(&bytes[0..2]), Some(h) if h < 24) {
        return false;
    }

    if !matches!(parse_2_digits(&bytes[3..5]), Some(m) if m < 60) {
        return false;
    }

    if !matches!(parse_2_digits(&bytes[6..8]), Some(sec) if sec <= 60) {
        return false;
    }

    let mut cursor = 8;

    // Optional fractional seconds
    if cursor < bytes.len() && bytes[cursor] == b'.' {
        cursor += 1;
        let frac_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == frac_start {
            return false; // Point without digits
        }
    }

    if cursor == bytes.len() {
        return true; // Local time without timezone
    }

    // Timezone offset: 'Z', 'z', or '[+-]HH:MM'
    let remaining = &bytes[cursor..];
    if remaining == b"Z" || remaining == b"z" {
        return true;
    }

    if remaining.len() == 6
        && (remaining[0] == b'+' || remaining[0] == b'-')
        && remaining[3] == b':'
    {
        if !matches!(parse_2_digits(&remaining[1..3]), Some(h) if h < 24) {
            return false;
        }
        if !matches!(parse_2_digits(&remaining[4..6]), Some(m) if m < 60) {
            return false;
        }
        return true;
    }

    false
}

/// Validates that a string is a complete RFC 3339 date-time.
#[must_use]
pub fn is_valid_rfc3339_datetime(s: &str) -> bool {
    if s.len() < 19 {
        return false;
    }

    let separator = s.as_bytes()[10];
    if separator != b'T' && separator != b't' && separator != b' ' {
        return false;
    }

    let date_part = &s[0..10];
    let time_part = &s[11..];

    is_valid_rfc3339_date(date_part) && is_valid_rfc3339_time(time_part)
}

/// Parses an RFC 3339 date or datetime string into a normalized epoch second timestamp.
#[must_use]
pub fn parse_rfc3339_timestamp(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    if bytes.len() == 10 {
        if !is_valid_rfc3339_date(s) {
            return None;
        }
        let year = parse_4_digits(&bytes[0..4])? as i64;
        let month = parse_2_digits(&bytes[5..7])? as i64;
        let day = parse_2_digits(&bytes[8..10])? as i64;

        let days = days_from_civil(year, month, day);
        return Some(days * 86_400);
    }

    if !is_valid_rfc3339_datetime(s) {
        return None;
    }

    let year = parse_4_digits(&bytes[0..4])? as i64;
    let month = parse_2_digits(&bytes[5..7])? as i64;
    let day = parse_2_digits(&bytes[8..10])? as i64;

    let hour = parse_2_digits(&bytes[11..13])? as i64;
    let minute = parse_2_digits(&bytes[14..16])? as i64;
    let second = parse_2_digits(&bytes[17..19])? as i64;

    let mut cursor = 19;
    if cursor < bytes.len() && bytes[cursor] == b'.' {
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
    }

    let offset_seconds =
        if cursor == bytes.len() || &bytes[cursor..] == b"Z" || &bytes[cursor..] == b"z" {
            0
        } else {
            let tz = &bytes[cursor..];
            if tz.len() == 6 && (tz[0] == b'+' || tz[0] == b'-') && tz[3] == b':' {
                let offset_h = parse_2_digits(&tz[1..3])? as i64;
                let offset_m = parse_2_digits(&tz[4..6])? as i64;
                let total_offset = offset_h * 3600 + offset_m * 60;
                if tz[0] == b'+' {
                    total_offset
                } else {
                    -total_offset
                }
            } else {
                0
            }
        };

    let days = days_from_civil(year, month, day);
    let seconds_since_midnight = hour * 3600 + minute * 60 + second;
    Some(days * 86_400 + seconds_since_midnight - offset_seconds)
}

const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - (month <= 2) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (month + (if month > 2 { -3 } else { 9 })) + 2) / 5 + day - 1;
    let doe = (yoe * 365 + yoe / 4 - yoe / 100) as i64 + doy;
    era * 146_097 + doe - 719_468
}

const fn parse_2_digits(slice: &[u8]) -> Option<u32> {
    if slice.len() != 2 {
        return None;
    }
    let d1 = slice[0];
    let d2 = slice[1];
    if d1.is_ascii_digit() && d2.is_ascii_digit() {
        Some((d1 - b'0') as u32 * 10 + (d2 - b'0') as u32)
    } else {
        None
    }
}

const fn parse_4_digits(slice: &[u8]) -> Option<u32> {
    if slice.len() != 4 {
        return None;
    }
    let d1 = slice[0];
    let d2 = slice[1];
    let d3 = slice[2];
    let d4 = slice[3];
    if d1.is_ascii_digit() && d2.is_ascii_digit() && d3.is_ascii_digit() && d4.is_ascii_digit() {
        Some(
            (d1 - b'0') as u32 * 1000
                + (d2 - b'0') as u32 * 100
                + (d3 - b'0') as u32 * 10
                + (d4 - b'0') as u32,
        )
    } else {
        None
    }
}

/// Validates whether a date string falls on a weekend (Saturday or Sunday).
#[must_use]
pub fn is_weekend_date(s: &str) -> bool {
    if let Some(ts) = parse_rfc3339_timestamp(s) {
        let days = ts.div_euclid(86_400);
        let dow = ((days + 4).rem_euclid(7)) as u32;
        dow == 0 || dow == 6
    } else {
        false
    }
}

/// Validates whether a date string falls on a weekday (Monday through Friday).
#[must_use]
pub fn is_weekday_date(s: &str) -> bool {
    if let Some(ts) = parse_rfc3339_timestamp(s) {
        let days = ts.div_euclid(86_400);
        let dow = ((days + 4).rem_euclid(7)) as u32;
        (1..=5).contains(&dow)
    } else {
        false
    }
}

/// Validates whether a string complies with the ISO 8601 Duration format.
#[must_use]
pub fn is_valid_iso8601_duration(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || (bytes[0] != b'P' && bytes[0] != b'p') {
        return false;
    }
    let rest = &bytes[1..];
    if rest.is_empty() {
        return false;
    }
    if rest.ends_with(b"W") || rest.ends_with(b"w") {
        let num = &rest[..rest.len() - 1];
        return !num.is_empty() && num.iter().all(u8::is_ascii_digit);
    }
    let (date_part, time_part) = match rest.iter().position(|&b| b == b'T' || b == b't') {
        Some(idx) => (&rest[..idx], Some(&rest[idx + 1..])),
        None => (rest, None),
    };
    let mut date_components_count = 0usize;
    if !date_part.is_empty() {
        let mut cursor = 0;
        while cursor < date_part.len() {
            let start = cursor;
            while cursor < date_part.len() && date_part[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor == start || cursor >= date_part.len() {
                return false;
            }
            let des = date_part[cursor];
            if des != b'Y'
                && des != b'y'
                && des != b'M'
                && des != b'm'
                && des != b'D'
                && des != b'd'
            {
                return false;
            }
            date_components_count += 1;
            cursor += 1;
        }
    }
    let mut time_components_count = 0usize;
    if let Some(time_bytes) = time_part {
        if time_bytes.is_empty() {
            return false;
        }
        let mut cursor = 0;
        while cursor < time_bytes.len() {
            let start = cursor;
            let mut has_fraction = false;
            while cursor < time_bytes.len()
                && (time_bytes[cursor].is_ascii_digit()
                    || (!has_fraction
                        && (time_bytes[cursor] == b'.' || time_bytes[cursor] == b',')))
            {
                if time_bytes[cursor] == b'.' || time_bytes[cursor] == b',' {
                    has_fraction = true;
                }
                cursor += 1;
            }
            if cursor == start || cursor >= time_bytes.len() {
                return false;
            }
            let des = time_bytes[cursor];
            if des != b'H'
                && des != b'h'
                && des != b'M'
                && des != b'm'
                && des != b'S'
                && des != b's'
            {
                return false;
            }
            time_components_count += 1;
            cursor += 1;
        }
    }
    date_components_count + time_components_count > 0
}
