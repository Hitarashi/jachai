//! Number schemas for floating-point and integer validation with string coercion.

use std::sync::Arc;

use serde_json::Value;
#[cfg(feature = "async")]
use {crate::rule::AsyncCustomRule, futures_util::future::BoxFuture};

use crate::{
    coercion::coerce_to_number,
    context::ValidationContext,
    error::{RuleParams, ValidationError, ValidationIssue},
    messages::MessagesProvider,
    rule::{CustomRule, RuleContext},
    schema::Schema,
};

#[derive(Clone)]
enum NumberPredicate {
    Min(f64),
    Max(f64),
    Range(f64, f64),
    Integer,
    Positive,
    Negative,
    NonNegative,
    NonPositive,
    Finite,
    MultipleOf(f64),
    DecimalPlaces(u32),
    SafeInteger,
    Latitude,
    Longitude,
    Custom(Arc<dyn CustomRule<f64>>),
}

/// Fluent schema builder for validating numbers (`f64`).
#[derive(Clone)]
pub struct NumberSchema {
    predicates: Vec<NumberPredicate>,
    bail_on_fail: bool,
    inline_message: Option<String>,
    #[cfg(feature = "async")]
    async_rules: Vec<Arc<dyn AsyncCustomRule<f64>>>,
}

impl Default for NumberSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl NumberSchema {
    /// Creates a new `NumberSchema`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
            bail_on_fail: false,
            inline_message: None,
            #[cfg(feature = "async")]
            async_rules: Vec::new(),
        }
    }

    /// Configures whether rule evaluation on this field should stop on the first failure.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Sets an inline custom error message override for this number field.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inline_message = Some(message.into());
        self
    }

    /// Enforces a minimum value constraint.
    #[must_use]
    pub fn min(mut self, minimum: f64) -> Self {
        self.predicates.push(NumberPredicate::Min(minimum));
        self
    }

    /// Enforces a maximum value constraint.
    #[must_use]
    pub fn max(mut self, maximum: f64) -> Self {
        self.predicates.push(NumberPredicate::Max(maximum));
        self
    }

    /// Enforces that the value falls within an inclusive numeric range `[min, max]`.
    #[must_use]
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.predicates.push(NumberPredicate::Range(min, max));
        self
    }

    /// Enforces that the value falls within an inclusive numeric range `[min, max]`.
    ///
    /// Ergonomic alias for `range(min, max)`.
    #[must_use]
    pub fn between(self, min: f64, max: f64) -> Self {
        self.range(min, max)
    }

    /// Enforces that the number has at most `max_places` decimal places.
    #[must_use]
    pub fn decimal_places(mut self, max_places: u32) -> Self {
        self.predicates
            .push(NumberPredicate::DecimalPlaces(max_places));
        self
    }

    /// Enforces that the number has no fractional component.
    #[must_use]
    pub fn int(mut self) -> Self {
        self.predicates.push(NumberPredicate::Integer);
        self
    }

    /// Enforces that the number is strictly positive (`> 0.0`).
    #[must_use]
    pub fn positive(mut self) -> Self {
        self.predicates.push(NumberPredicate::Positive);
        self
    }

    /// Enforces that the number is strictly negative (`< 0.0`).
    #[must_use]
    pub fn negative(mut self) -> Self {
        self.predicates.push(NumberPredicate::Negative);
        self
    }

    /// Enforces that the number is non-negative (`>= 0.0`).
    #[must_use]
    pub fn non_negative(mut self) -> Self {
        self.predicates.push(NumberPredicate::NonNegative);
        self
    }

    /// Enforces that the number is non-positive (`<= 0.0`).
    #[must_use]
    pub fn non_positive(mut self) -> Self {
        self.predicates.push(NumberPredicate::NonPositive);
        self
    }

    /// Enforces that the number is finite (neither infinite nor NaN).
    #[must_use]
    pub fn finite(mut self) -> Self {
        self.predicates.push(NumberPredicate::Finite);
        self
    }

    /// Enforces that the number is an exact multiple of the specified step.
    #[must_use]
    pub fn multiple_of(mut self, step: f64) -> Self {
        self.predicates.push(NumberPredicate::MultipleOf(step));
        self
    }

    /// Enforces that the number is an IEEE 754 safe integer in [-(2^53 - 1), 2^53 - 1].
    #[must_use]
    pub fn safe_integer(mut self) -> Self {
        self.predicates.push(NumberPredicate::SafeInteger);
        self
    }

    /// Enforces that the number is a valid latitude in [-90.0, 90.0].
    #[must_use]
    pub fn latitude(mut self) -> Self {
        self.predicates.push(NumberPredicate::Latitude);
        self
    }

    /// Enforces that the number is a valid longitude in [-180.0, 180.0].
    #[must_use]
    pub fn longitude(mut self) -> Self {
        self.predicates.push(NumberPredicate::Longitude);
        self
    }

    /// Attaches a custom synchronous validation rule.
    #[must_use]
    pub fn rule<R: CustomRule<f64> + 'static>(mut self, rule: R) -> Self {
        self.predicates
            .push(NumberPredicate::Custom(Arc::new(rule)));
        self
    }

    #[cfg(feature = "async")]
    /// Attaches an asynchronous custom validation rule.
    #[must_use]
    pub fn rule_async<R: AsyncCustomRule<f64> + 'static>(mut self, rule: R) -> Self {
        self.async_rules.push(Arc::new(rule));
        self
    }

    fn evaluate_predicate(
        &self,
        predicate: &NumberPredicate,
        value: f64,
        rule_context: &RuleContext<'_>,
    ) -> Option<ValidationIssue> {
        let inline = self.inline_message.as_deref();

        match predicate {
            NumberPredicate::Min(min) => {
                if value < *min {
                    let mut params = RuleParams::new();
                    params.insert("min", min.to_string());
                    params.insert("actual", value.to_string());
                    Some(rule_context.create_issue("min", params, inline))
                } else {
                    None
                }
            }
            NumberPredicate::Max(max) => {
                if value > *max {
                    let mut params = RuleParams::new();
                    params.insert("max", max.to_string());
                    params.insert("actual", value.to_string());
                    Some(rule_context.create_issue("max", params, inline))
                } else {
                    None
                }
            }
            NumberPredicate::Range(min, max) => {
                if value < *min || value > *max {
                    let mut params = RuleParams::new();
                    params.insert("min", min.to_string());
                    params.insert("max", max.to_string());
                    params.insert("actual", value.to_string());
                    Some(rule_context.create_issue("range", params, inline))
                } else {
                    None
                }
            }
            NumberPredicate::Integer => {
                if value.fract() != 0.0 {
                    Some(rule_context.create_issue("integer", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Positive => {
                if value <= 0.0 {
                    Some(rule_context.create_issue("positive", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Negative => {
                if value >= 0.0 {
                    Some(rule_context.create_issue("negative", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::NonNegative => {
                if value < 0.0 {
                    Some(rule_context.create_issue("non_negative", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::NonPositive => {
                if value > 0.0 {
                    Some(rule_context.create_issue("non_positive", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Finite => {
                if !value.is_finite() {
                    Some(rule_context.create_issue("finite", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::MultipleOf(step) => {
                let remainder = (value / *step).fract();
                if remainder.abs() > f64::EPSILON && (1.0 - remainder).abs() > f64::EPSILON {
                    let mut params = RuleParams::new();
                    params.insert("step", step.to_string());
                    Some(rule_context.create_issue("multiple_of", params, inline))
                } else {
                    None
                }
            }
            NumberPredicate::DecimalPlaces(max_places) => {
                let s = value.to_string();
                let decimal_count = s.split_once('.').map(|(_, frac)| frac.len()).unwrap_or(0);
                if decimal_count > *max_places as usize {
                    let mut params = RuleParams::new();
                    params.insert("max", max_places.to_string());
                    params.insert("actual", decimal_count.to_string());
                    Some(rule_context.create_issue("decimal_places", params, inline))
                } else {
                    None
                }
            }
            NumberPredicate::SafeInteger => {
                const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
                if value.fract() != 0.0 || value < -MAX_SAFE_INTEGER || value > MAX_SAFE_INTEGER {
                    Some(rule_context.create_issue("safe_integer", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Latitude => {
                if !(-90.0..=90.0).contains(&value) {
                    Some(rule_context.create_issue("latitude", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Longitude => {
                if !(-180.0..=180.0).contains(&value) {
                    Some(rule_context.create_issue("longitude", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            NumberPredicate::Custom(rule) => rule.validate(&value, rule_context).err(),
        }
    }

    #[inline(always)]
    fn matches_predicate(&self, predicate: &NumberPredicate, value: f64) -> bool {
        match predicate {
            NumberPredicate::Min(min) => value >= *min,
            NumberPredicate::Max(max) => value <= *max,
            NumberPredicate::Range(min, max) => value >= *min && value <= *max,
            NumberPredicate::Integer => value.fract() == 0.0,
            NumberPredicate::Positive => value > 0.0,
            NumberPredicate::Negative => value < 0.0,
            NumberPredicate::NonPositive => value <= 0.0,
            NumberPredicate::NonNegative => value >= 0.0,
            NumberPredicate::MultipleOf(step) => {
                if *step == 0.0 {
                    false
                } else {
                    let rem = value % *step;
                    rem.abs() < 1e-9 || (*step - rem.abs()).abs() < 1e-9
                }
            }
            NumberPredicate::Finite => value.is_finite(),
            NumberPredicate::SafeInteger => {
                value.fract() == 0.0 && (-9007199254740991.0..=9007199254740991.0).contains(&value)
            }
            NumberPredicate::Latitude => (-90.0..=90.0).contains(&value),
            NumberPredicate::Longitude => (-180.0..=180.0).contains(&value),
            _ => false,
        }
    }

    /// Checks that a 64-bit float conforms to all bounds rules without requiring `Value` wrappers.
    pub fn check_f64(&self, input: f64) -> Result<(), ValidationError> {
        if self
            .predicates
            .iter()
            .all(|predicate| self.matches_predicate(predicate, input))
        {
            return Ok(());
        }

        let context = ValidationContext::new();
        let rule_ctx = RuleContext::new(
            "",
            &context,
            None,
            crate::messages::default_messages_provider(),
        );

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, input, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates a 64-bit float, returning it on success.
    pub fn validate_f64(&self, input: f64) -> Result<f64, ValidationError> {
        self.check_f64(input).map(|_| input)
    }
}

impl Schema<f64> for NumberSchema {
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<f64, ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);

        let parsed_num = match value {
            Value::Number(num) => num.as_f64().unwrap_or(0.0),
            Value::String(_) => match coerce_to_number(value) {
                Some(num) => num.as_f64().unwrap_or(0.0),
                None => {
                    let issue = rule_ctx.create_issue(
                        "number",
                        RuleParams::new(),
                        self.inline_message.as_deref(),
                    );
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            },
            Value::Null => {
                let issue = rule_ctx.create_issue(
                    "required",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue(
                    "number",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, parsed_num, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(parsed_num)
        } else {
            Err(errors)
        }
    }

    fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        if let Value::Number(num) = value {
            let parsed_num = num.as_f64().unwrap_or(0.0);
            if self
                .predicates
                .iter()
                .all(|predicate| self.matches_predicate(predicate, parsed_num))
            {
                return Ok(());
            }
        }

        let rule_ctx = RuleContext::new(path, context, parent_data, messages);

        let parsed_num = match value {
            Value::Number(num) => num.as_f64().unwrap_or(0.0),
            Value::String(_) => match coerce_to_number(value) {
                Some(num) => num.as_f64().unwrap_or(0.0),
                None => {
                    let issue = rule_ctx.create_issue(
                        "number",
                        RuleParams::new(),
                        self.inline_message.as_deref(),
                    );
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            },
            Value::Null => {
                let issue = rule_ctx.create_issue(
                    "required",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue(
                    "number",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, parsed_num, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[cfg(feature = "async")]
    fn validate_async_path<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<f64, ValidationError>> {
        Box::pin(async move {
            let num = self.validate_path(value, path, context, parent_data, messages)?;
            let rule_ctx = RuleContext::new(path, context, parent_data, messages);
            let mut errors = ValidationError::new();

            for async_rule in &self.async_rules {
                match async_rule.validate(&num, &rule_ctx).await {
                    Ok(()) => {}
                    Err(issue) => {
                        errors.push(issue);
                        if self.bail_on_fail {
                            break;
                        }
                    }
                }
            }

            if errors.is_empty() {
                Ok(num)
            } else {
                Err(errors)
            }
        })
    }

    fn to_json_schema_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("number".to_string()));
        for pred in &self.predicates {
            match pred {
                NumberPredicate::Min(min) => {
                    let num_val = if min.fract() == 0.0 {
                        serde_json::Number::from(*min as i64)
                    } else {
                        serde_json::Number::from_f64(*min).unwrap_or(0.into())
                    };
                    map.insert("minimum".to_string(), Value::Number(num_val));
                }
                NumberPredicate::Max(max) => {
                    let num_val = if max.fract() == 0.0 {
                        serde_json::Number::from(*max as i64)
                    } else {
                        serde_json::Number::from_f64(*max).unwrap_or(0.into())
                    };
                    map.insert("maximum".to_string(), Value::Number(num_val));
                }
                NumberPredicate::Range(min, max) => {
                    let min_val = if min.fract() == 0.0 {
                        serde_json::Number::from(*min as i64)
                    } else {
                        serde_json::Number::from_f64(*min).unwrap_or(0.into())
                    };
                    let max_val = if max.fract() == 0.0 {
                        serde_json::Number::from(*max as i64)
                    } else {
                        serde_json::Number::from_f64(*max).unwrap_or(0.into())
                    };
                    map.insert("minimum".to_string(), Value::Number(min_val));
                    map.insert("maximum".to_string(), Value::Number(max_val));
                }
                NumberPredicate::Integer => {
                    map.insert("type".to_string(), Value::String("integer".to_string()));
                }
                NumberPredicate::Positive => {
                    map.insert("exclusiveMinimum".to_string(), Value::Number(0.into()));
                }
                NumberPredicate::Negative => {
                    map.insert("exclusiveMaximum".to_string(), Value::Number(0.into()));
                }
                NumberPredicate::NonNegative => {
                    map.insert("minimum".to_string(), Value::Number(0.into()));
                }
                NumberPredicate::NonPositive => {
                    map.insert("maximum".to_string(), Value::Number(0.into()));
                }
                NumberPredicate::MultipleOf(step) => {
                    if let Some(n) = serde_json::Number::from_f64(*step) {
                        map.insert("multipleOf".to_string(), Value::Number(n));
                    }
                }
                NumberPredicate::SafeInteger => {
                    map.insert("type".to_string(), Value::String("integer".to_string()));
                    map.insert(
                        "minimum".to_string(),
                        Value::Number(serde_json::Number::from(-9_007_199_254_740_991_i64)),
                    );
                    map.insert(
                        "maximum".to_string(),
                        Value::Number(serde_json::Number::from(9_007_199_254_740_991_i64)),
                    );
                }
                NumberPredicate::Latitude => {
                    map.insert("minimum".to_string(), Value::Number((-90).into()));
                    map.insert("maximum".to_string(), Value::Number(90.into()));
                }
                NumberPredicate::Longitude => {
                    map.insert("minimum".to_string(), Value::Number((-180).into()));
                    map.insert("maximum".to_string(), Value::Number(180.into()));
                }
                _ => {}
            }
        }
        Value::Object(map)
    }
}

/// Dedicated integer schema builder validating into `i64`.
#[derive(Clone)]
pub struct IntegerSchema {
    inner: NumberSchema,
}

impl Default for IntegerSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegerSchema {
    /// Creates a new `IntegerSchema`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: NumberSchema::new().int(),
        }
    }

    /// Enforces a minimum integer constraint.
    #[must_use]
    pub fn min(mut self, minimum: i64) -> Self {
        self.inner = self.inner.min(minimum as f64);
        self
    }

    /// Enforces a maximum integer constraint.
    #[must_use]
    pub fn max(mut self, maximum: i64) -> Self {
        self.inner = self.inner.max(maximum as f64);
        self
    }

    /// Enforces an inclusive integer range `[min, max]`.
    #[must_use]
    pub fn range(mut self, min: i64, max: i64) -> Self {
        self.inner = self.inner.range(min as f64, max as f64);
        self
    }

    /// Enforces an inclusive integer range `[min, max]`.
    ///
    /// Ergonomic alias for `range(min, max)`.
    #[must_use]
    pub fn between(self, min: i64, max: i64) -> Self {
        self.range(min, max)
    }

    /// Validates that the integer represents a valid network port (1 to 65535).
    #[must_use]
    pub fn port(self) -> Self {
        self.between(1, 65535)
    }

    /// Enforces that the integer is strictly positive (`> 0`).
    #[must_use]
    pub fn positive(mut self) -> Self {
        self.inner = self.inner.positive();
        self
    }

    /// Enforces that the integer is strictly negative (`< 0`).
    #[must_use]
    pub fn negative(mut self) -> Self {
        self.inner = self.inner.negative();
        self
    }

    /// Enforces that the integer is non-negative (`>= 0`).
    #[must_use]
    pub fn non_negative(mut self) -> Self {
        self.inner = self.inner.non_negative();
        self
    }

    /// Enforces that the integer is non-positive (`<= 0`).
    #[must_use]
    pub fn non_positive(mut self) -> Self {
        self.inner = self.inner.non_positive();
        self
    }

    /// Enforces that the integer is a multiple of `step`.
    #[must_use]
    pub fn multiple_of(mut self, step: i64) -> Self {
        self.inner = self.inner.multiple_of(step as f64);
        self
    }

    /// Enforces that the integer is in the safe integer range [-(2^53 - 1), 2^53 - 1].
    #[must_use]
    pub fn safe_integer(mut self) -> Self {
        self.inner = self.inner.safe_integer();
        self
    }

    /// Enforces that the integer is a valid latitude in [-90, 90].
    #[must_use]
    pub fn latitude(mut self) -> Self {
        self.inner = self.inner.latitude();
        self
    }

    /// Enforces that the integer is a valid longitude in [-180, 180].
    #[must_use]
    pub fn longitude(mut self) -> Self {
        self.inner = self.inner.longitude();
        self
    }

    /// Configures short-circuiting on first failure.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.inner = self.inner.bail(enabled);
        self
    }

    /// Sets an inline custom message override.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inner = self.inner.with_message(message);
        self
    }

    /// Checks that a 64-bit integer conforms to all bounds rules without requiring `Value` wrappers.
    pub fn check_i64(&self, input: i64) -> Result<(), ValidationError> {
        let val_f64 = input as f64;
        for predicate in &self.inner.predicates {
            match predicate {
                NumberPredicate::Integer => continue,
                NumberPredicate::Min(min) => {
                    if val_f64 < *min {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::Max(max) => {
                    if val_f64 > *max {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::Range(min, max) => {
                    if val_f64 < *min || val_f64 > *max {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::Positive => {
                    if input <= 0 {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::Negative => {
                    if input >= 0 {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::NonPositive => {
                    if input > 0 {
                        return self.inner.check_f64(val_f64);
                    }
                }
                NumberPredicate::NonNegative => {
                    if input < 0 {
                        return self.inner.check_f64(val_f64);
                    }
                }
                _ => {
                    if !self.inner.matches_predicate(predicate, val_f64) {
                        return self.inner.check_f64(val_f64);
                    }
                }
            }
        }
        Ok(())
    }

    /// Validates a 64-bit integer, returning it on success.
    pub fn validate_i64(&self, input: i64) -> Result<i64, ValidationError> {
        self.inner.check_f64(input as f64).map(|_| input)
    }
}

impl Schema<i64> for IntegerSchema {
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<i64, ValidationError> {
        let float_val = self
            .inner
            .validate_path(value, path, context, parent_data, messages)?;
        Ok(float_val as i64)
    }

    fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        self.inner
            .check_path(value, path, context, parent_data, messages)
    }

    #[cfg(feature = "async")]
    fn validate_async_path<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<i64, ValidationError>> {
        Box::pin(async move {
            let float_val = self
                .inner
                .validate_async_path(value, path, context, parent_data, messages)
                .await?;
            Ok(float_val as i64)
        })
    }

    fn to_json_schema_value(&self) -> Value {
        let mut val = self.inner.to_json_schema_value();
        if let Value::Object(ref mut map) = val {
            map.insert("type".to_string(), Value::String("integer".to_string()));
        }
        val
    }
}
