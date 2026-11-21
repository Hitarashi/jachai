//! Type coercion, sanitization, and normalization for incoming untrusted payloads.

use serde_json::{Number, Value};

/// Configurable sanitization and coercion processor for untrusted input trees.
///
/// `Coercer` allows configuring and executing tree normalization and sanitization
/// (such as empty string to `Null` conversion and whitespace trimming) in-place
/// with zero heap allocations, as well as providing type coercion helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coercer {
    empty_strings_to_null: bool,
    trim_strings: bool,
}

impl Default for Coercer {
    fn default() -> Self {
        Self {
            empty_strings_to_null: true,
            trim_strings: true,
        }
    }
}

impl Coercer {
    /// Creates a new default `Coercer`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures whether empty strings (or strings containing only whitespace) should be converted to `Null`.
    #[must_use]
    pub fn empty_strings_to_null(mut self, enabled: bool) -> Self {
        self.empty_strings_to_null = enabled;
        self
    }

    /// Configures whether string values should be trimmed of leading and trailing whitespace.
    #[must_use]
    pub fn trim_strings(mut self, enabled: bool) -> Self {
        self.trim_strings = enabled;
        self
    }

    /// Normalizes and sanitizes a mutable JSON value in-place without tree cloning.
    pub fn sanitize_in_place(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                let trimmed = s.trim();
                if self.empty_strings_to_null && trimmed.is_empty() {
                    *value = Value::Null;
                } else if self.trim_strings && trimmed.len() != s.len() {
                    *s = trimmed.to_string();
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.sanitize_in_place(item);
                }
            }
            Value::Object(map) => {
                for item in map.values_mut() {
                    self.sanitize_in_place(item);
                }
            }
            _ => {}
        }
    }

    /// Consumes and sanitizes a JSON value, reusing allocated buffers where possible.
    #[must_use]
    pub fn sanitize(&self, mut value: Value) -> Value {
        self.sanitize_in_place(&mut value);
        value
    }

    /// Sanitizes a string slice by trimming leading and trailing whitespace.
    #[must_use]
    pub fn trim_str(input: &str) -> &str {
        input.trim()
    }

    /// Attempts to coerce a JSON value into a `bool`.
    ///
    /// Accepts native booleans, numeric 0 and 1, and case-insensitive truthy/falsy strings
    /// (`"true"`, `"1"`, `"yes"`, `"on"` / `"false"`, `"0"`, `"no"`, `"off"`).
    #[must_use]
    pub fn to_bool(value: &Value) -> Option<bool> {
        match value {
            Value::Bool(b) => Some(*b),
            Value::Number(num) => {
                if let Some(int_val) = num.as_i64() {
                    match int_val {
                        1 => Some(true),
                        0 => Some(false),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            Value::String(s) => {
                let normalized = s.trim().to_ascii_lowercase();
                match normalized.as_str() {
                    "true" | "1" | "yes" | "on" => Some(true),
                    "false" | "0" | "no" | "off" => Some(false),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Attempts to coerce a JSON value into a `Number`.
    ///
    /// Converts numeric strings (such as `"42"` or `"3.14"`) from query strings or forms into numbers.
    #[must_use]
    pub fn to_number(value: &Value) -> Option<Number> {
        match value {
            Value::Number(num) => Some(num.clone()),
            Value::String(s) => {
                let trimmed = s.trim();
                if let Ok(int_val) = trimmed.parse::<i64>() {
                    Some(Number::from(int_val))
                } else if let Ok(uint_val) = trimmed.parse::<u64>() {
                    Some(Number::from(uint_val))
                } else if let Ok(float_val) = trimmed.parse::<f64>() {
                    Number::from_f64(float_val)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Attempts to coerce a JSON value into a 64-bit signed integer.
    #[must_use]
    pub fn to_i64(value: &Value) -> Option<i64> {
        match value {
            Value::Number(num) => num.as_i64(),
            Value::String(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }

    /// Attempts to coerce a JSON value into a 64-bit unsigned integer.
    #[must_use]
    pub fn to_u64(value: &Value) -> Option<u64> {
        match value {
            Value::Number(num) => num.as_u64(),
            Value::String(s) => s.trim().parse::<u64>().ok(),
            _ => None,
        }
    }

    /// Attempts to coerce a JSON value into a 64-bit float.
    #[must_use]
    pub fn to_f64(value: &Value) -> Option<f64> {
        match value {
            Value::Number(num) => num.as_f64(),
            Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    }

    /// Attempts to coerce a JSON value into an owned string.
    #[must_use]
    pub fn to_string(value: &Value) -> Option<String> {
        match value {
            Value::String(s) => Some(s.clone()),
            Value::Number(num) => Some(num.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Ensures an array value is produced, wrapping a single scalar if necessary.
    #[must_use]
    pub fn to_array(value: &Value) -> Option<Vec<Value>> {
        match value {
            Value::Array(items) => Some(items.clone()),
            Value::Null => None,
            scalar => Some(vec![scalar.clone()]),
        }
    }
}

/// Sanitizes a string slice by trimming leading and trailing whitespace.
#[must_use]
pub fn trim_whitespace(input: &str) -> &str {
    Coercer::trim_str(input)
}

/// Normalizes empty string values `""` into JSON `Null`.
///
/// Critical for form submissions where omitted text fields are submitted as empty strings
/// rather than null or missing keys.
#[must_use]
pub fn empty_string_to_null(value: &Value) -> Value {
    match value {
        Value::String(string_val) if string_val.trim().is_empty() => Value::Null,
        other => other.clone(),
    }
}

/// Recursively traverses a JSON value, normalizing empty strings `""` to `Null`.
#[must_use]
pub fn normalize_empty_strings(value: &Value) -> Value {
    Coercer::new()
        .empty_strings_to_null(true)
        .trim_strings(false)
        .sanitize(value.clone())
}

/// Recursively traverses a JSON value in-place, normalizing empty strings `""` to `Null`.
pub fn normalize_in_place(value: &mut Value) {
    Coercer::new()
        .empty_strings_to_null(true)
        .trim_strings(false)
        .sanitize_in_place(value);
}

/// Attempts to coerce a JSON value into a `Number`.
///
/// Converts numeric strings (such as `"42"` or `"3.14"`) from query strings or forms into numbers.
#[must_use]
pub fn coerce_to_number(value: &Value) -> Option<Number> {
    Coercer::to_number(value)
}

/// Attempts to coerce a JSON value into a boolean.
///
/// Converts truthy strings (`"true"`, `"1"`, `"yes"`, `"on"`) and falsy strings (`"false"`, `"0"`, `"no"`, `"off"`).
#[must_use]
pub fn coerce_to_bool(value: &Value) -> Option<bool> {
    Coercer::to_bool(value)
}

/// Attempts to coerce a JSON value into an owned string.
#[must_use]
pub fn coerce_to_string(value: &Value) -> Option<String> {
    Coercer::to_string(value)
}

/// Ensures an array value is produced, wrapping a single scalar if necessary.
#[must_use]
pub fn coerce_to_array(value: &Value) -> Option<Vec<Value>> {
    Coercer::to_array(value)
}
