//! Object schema builder for validating structured dictionaries, cross-field rules, and fail-closed security.

use std::sync::Arc;

#[cfg(feature = "async")]
use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::{
    coercion::Coercer,
    context::ValidationContext,
    error::{RuleParams, ValidationError, ValidationIssue},
    messages::{MessagesProvider, SimpleMessagesProvider},
    rule::{CrossFieldRuleValidator, RuleContext},
    schema::Schema,
};

/// Policy governing how object schemas handle undeclared/unexpected fields in incoming payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnknownFieldPolicy {
    /// Omit undeclared fields from the decoded output to prevent mass-assignment vulnerabilities.
    #[default]
    Strip,
    /// Reject payloads containing any undeclared field with an `unknown_field` validation error.
    Strict,
    /// Preserve all undeclared fields in the decoded output.
    Passthrough,
}

trait ErasedField: Send + Sync {
    fn is_optional(&self) -> bool;

    fn to_json_schema(&self) -> Value;

    fn validate_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError>;

    fn check_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError>;

    #[cfg(feature = "async")]
    fn validate_async_field<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<Value, ValidationError>>;
}

#[inline]
fn typed_to_value<T: serde::Serialize + 'static>(
    mut typed: T,
    path: &str,
) -> Result<Value, ValidationError> {
    let any_mut = &mut typed as &mut dyn std::any::Any;
    if let Some(v) = any_mut.downcast_mut::<Value>() {
        return Ok(std::mem::take(v));
    }
    if let Some(s) = any_mut.downcast_mut::<String>() {
        return Ok(Value::String(std::mem::take(s)));
    }
    if let Some(n) = any_mut.downcast_mut::<i64>() {
        return Ok(Value::Number((*n).into()));
    }
    if let Some(n) = any_mut.downcast_mut::<u64>() {
        return Ok(Value::Number((*n).into()));
    }
    if let Some(n) = any_mut.downcast_mut::<f64>() {
        return Ok(serde_json::Number::from_f64(*n)
            .map(Value::Number)
            .unwrap_or(Value::Null));
    }
    if let Some(b) = any_mut.downcast_mut::<bool>() {
        return Ok(Value::Bool(*b));
    }
    serde_json::to_value(&typed).map_err(|err| {
        let issue = ValidationIssue::new(path, "serialization", err.to_string());
        ValidationError::single(issue.field, issue.rule, issue.message)
    })
}

struct FieldEntry<S, T> {
    schema: S,
    optional: bool,
    phantom: std::marker::PhantomData<T>,
}

impl<S, T> ErasedField for FieldEntry<S, T>
where
    S: Schema<T>,
    T: serde::Serialize + Send + Sync + 'static,
{
    fn is_optional(&self) -> bool {
        self.optional || self.schema.is_optional()
    }

    fn to_json_schema(&self) -> Value {
        self.schema.to_json_schema_value()
    }

    fn validate_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError> {
        let typed_result =
            self.schema
                .validate_path(value, path, context, parent_data, messages)?;
        typed_to_value(typed_result, path)
    }

    fn check_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        self.schema
            .check_path(value, path, context, parent_data, messages)
    }

    #[cfg(feature = "async")]
    fn validate_async_field<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<Value, ValidationError>> {
        Box::pin(async move {
            let typed_result = self
                .schema
                .validate_async_path(value, path, context, parent_data, messages)
                .await?;
            typed_to_value(typed_result, path)
        })
    }
}

struct OptionalWrapper {
    inner: Arc<dyn ErasedField>,
}

impl ErasedField for OptionalWrapper {
    fn is_optional(&self) -> bool {
        true
    }

    fn to_json_schema(&self) -> Value {
        self.inner.to_json_schema()
    }

    fn validate_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError> {
        self.inner
            .validate_field(value, path, context, parent_data, messages)
    }

    fn check_field(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        self.inner
            .check_field(value, path, context, parent_data, messages)
    }

    #[cfg(feature = "async")]
    fn validate_async_field<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<Value, ValidationError>> {
        self.inner
            .validate_async_field(value, path, context, parent_data, messages)
    }
}

struct CompositeMessagesProvider<'a> {
    local: &'a SimpleMessagesProvider,
    parent: &'a dyn MessagesProvider,
}

impl<'a> MessagesProvider for CompositeMessagesProvider<'a> {
    fn format_message(
        &self,
        field: &str,
        rule: &str,
        params: &RuleParams,
        inline_override: Option<&str>,
    ) -> String {
        if let Some(inline) = inline_override {
            return crate::messages::interpolate_placeholders(inline, field, rule, params);
        }
        if let Some(template) = self.local.get_override_template(field, rule) {
            return crate::messages::interpolate_placeholders(template, field, rule, params);
        }
        self.parent.format_message(field, rule, params, None)
    }
}

#[derive(Clone)]
enum CrossFieldRule {
    SameAs {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    DifferentFrom {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    RequiredIf {
        field: String,
        other: String,
        expected: Value,
        custom_message: Option<String>,
    },
    RequiredWithout {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    AfterField {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    BeforeField {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    RequiredWith {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    ProhibitedIf {
        field: String,
        other: String,
        expected: Value,
        custom_message: Option<String>,
    },
    ProhibitedWith {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    AfterDateField {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    BeforeDateField {
        field: String,
        other: String,
        custom_message: Option<String>,
    },
    RequiredUnless {
        field: String,
        other: String,
        expected: Value,
        custom_message: Option<String>,
    },
    RequiredWithAll {
        field: String,
        others: Vec<String>,
        custom_message: Option<String>,
    },
    RequiredWithoutAll {
        field: String,
        others: Vec<String>,
        custom_message: Option<String>,
    },
    AtLeastOneOf {
        fields: Vec<String>,
        custom_message: Option<String>,
    },
    ExactlyOneOf {
        fields: Vec<String>,
        custom_message: Option<String>,
    },
    MutuallyExclusive {
        fields: Vec<String>,
        custom_message: Option<String>,
    },
    Confirmed {
        field: String,
        custom_message: Option<String>,
    },
}

impl CrossFieldRuleValidator for CrossFieldRule {
    fn evaluate(
        &self,
        map: &Map<String, Value>,
        path: &str,
        rule_ctx: &RuleContext<'_>,
    ) -> Option<ValidationIssue> {
        match self {
            Self::SameAs {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if val_a != val_b {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("other", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "same_as",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::DifferentFrom {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if !val_a.is_null() && !val_b.is_null() && val_a == val_b {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("other", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "different_from",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::RequiredIf {
                field,
                other,
                expected,
                custom_message,
            } => {
                let dependency = map.get(other).unwrap_or(&Value::Null);
                if dependency == expected {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("other", other.clone());
                        params.insert("value", expected.to_string());
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_if",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::RequiredWithout {
                field,
                other,
                custom_message,
            } => {
                let dependency = map.get(other).unwrap_or(&Value::Null);
                let dependency_is_absent = dependency.is_null()
                    || (dependency.is_string()
                        && dependency.as_str().unwrap_or("").trim().is_empty());
                if dependency_is_absent {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("other", other.clone());
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_without",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::AfterField {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if let (Some(sa), Some(sb)) = (val_a.as_str(), val_b.as_str())
                    && sa <= sb
                {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("target", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "after",
                        params,
                        custom_message.as_deref(),
                    ))
                } else if let (Some(na), Some(nb)) = (val_a.as_f64(), val_b.as_f64())
                    && na <= nb
                {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("target", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "after",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::BeforeField {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if let (Some(sa), Some(sb)) = (val_a.as_str(), val_b.as_str())
                    && sa >= sb
                {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("target", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "before",
                        params,
                        custom_message.as_deref(),
                    ))
                } else if let (Some(na), Some(nb)) = (val_a.as_f64(), val_b.as_f64())
                    && na >= nb
                {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("target", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "before",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::RequiredWith {
                field,
                other,
                custom_message,
            } => {
                let dependency = map.get(other).unwrap_or(&Value::Null);
                let dependency_is_present = !dependency.is_null()
                    && !(dependency.is_string()
                        && dependency.as_str().unwrap_or("").trim().is_empty());
                if dependency_is_present {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("other", other.clone());
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_with",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::ProhibitedIf {
                field,
                other,
                expected,
                custom_message,
            } => {
                let dependency = map.get(other).unwrap_or(&Value::Null);
                if dependency == expected {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let target_is_present = !target.is_null()
                        && !(target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if target_is_present {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("other", other.clone());
                        params.insert("value", expected.to_string());
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "prohibited_if",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::ProhibitedWith {
                field,
                other,
                custom_message,
            } => {
                let target = map.get(field).unwrap_or(&Value::Null);
                let target_is_present = !target.is_null()
                    && !(target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                let dependency = map.get(other).unwrap_or(&Value::Null);
                let dependency_is_present = !dependency.is_null()
                    && !(dependency.is_string()
                        && dependency.as_str().unwrap_or("").trim().is_empty());
                if target_is_present && dependency_is_present {
                    let full_path = qualify_path(path, field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("other", other.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "prohibited_with",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::AfterDateField {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if let (Some(sa), Some(sb)) = (val_a.as_str(), val_b.as_str()) {
                    if let (Some(ta), Some(tb)) = (
                        crate::schema::date::parse_rfc3339_timestamp(sa),
                        crate::schema::date::parse_rfc3339_timestamp(sb),
                    ) {
                        if ta <= tb {
                            let full_path = qualify_path(path, field);
                            let mut params = RuleParams::new();
                            params.insert("field", field.clone());
                            params.insert("target", other.clone());
                            Some(rule_ctx.create_issue_at(
                                &full_path,
                                "after_date",
                                params,
                                custom_message.as_deref(),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::BeforeDateField {
                field,
                other,
                custom_message,
            } => {
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(other).unwrap_or(&Value::Null);
                if let (Some(sa), Some(sb)) = (val_a.as_str(), val_b.as_str()) {
                    if let (Some(ta), Some(tb)) = (
                        crate::schema::date::parse_rfc3339_timestamp(sa),
                        crate::schema::date::parse_rfc3339_timestamp(sb),
                    ) {
                        if ta >= tb {
                            let full_path = qualify_path(path, field);
                            let mut params = RuleParams::new();
                            params.insert("field", field.clone());
                            params.insert("target", other.clone());
                            Some(rule_ctx.create_issue_at(
                                &full_path,
                                "before_date",
                                params,
                                custom_message.as_deref(),
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::RequiredUnless {
                field,
                other,
                expected,
                custom_message,
            } => {
                let dependency = map.get(other).unwrap_or(&Value::Null);
                if dependency != expected {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("other", other.clone());
                        params.insert("value", expected.to_string());
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_unless",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::RequiredWithAll {
                field,
                others,
                custom_message,
            } => {
                let all_present = others.iter().all(|o| {
                    let val = map.get(o).unwrap_or(&Value::Null);
                    !val.is_null()
                        && !(val.is_string() && val.as_str().unwrap_or("").trim().is_empty())
                });
                if all_present {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("others", others.join(", "));
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_with_all",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::RequiredWithoutAll {
                field,
                others,
                custom_message,
            } => {
                let none_present = others.iter().all(|o| {
                    let val = map.get(o).unwrap_or(&Value::Null);
                    val.is_null()
                        || (val.is_string() && val.as_str().unwrap_or("").trim().is_empty())
                });
                if none_present {
                    let target = map.get(field).unwrap_or(&Value::Null);
                    let is_missing = target.is_null()
                        || (target.is_string() && target.as_str().unwrap_or("").trim().is_empty());
                    if is_missing {
                        let full_path = qualify_path(path, field);
                        let mut params = RuleParams::new();
                        params.insert("field", field.clone());
                        params.insert("others", others.join(", "));
                        Some(rule_ctx.create_issue_at(
                            &full_path,
                            "required_without_all",
                            params,
                            custom_message.as_deref(),
                        ))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Self::AtLeastOneOf {
                fields,
                custom_message,
            } => {
                let any_present = fields.iter().any(|f| {
                    let val = map.get(f).unwrap_or(&Value::Null);
                    !val.is_null()
                        && !(val.is_string() && val.as_str().unwrap_or("").trim().is_empty())
                });
                if !any_present {
                    let full_path =
                        qualify_path(path, fields.first().map(String::as_str).unwrap_or(""));
                    let mut params = RuleParams::new();
                    params.insert("fields", fields.join(", "));
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "at_least_one_of",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::ExactlyOneOf {
                fields,
                custom_message,
            } => {
                let present_count = fields
                    .iter()
                    .filter(|f| {
                        let val = map.get(f.as_str()).unwrap_or(&Value::Null);
                        !val.is_null()
                            && !(val.is_string() && val.as_str().unwrap_or("").trim().is_empty())
                    })
                    .count();
                if present_count != 1 {
                    let full_path =
                        qualify_path(path, fields.first().map(String::as_str).unwrap_or(""));
                    let mut params = RuleParams::new();
                    params.insert("fields", fields.join(", "));
                    params.insert("actual", present_count.to_string());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "exactly_one_of",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::MutuallyExclusive {
                fields,
                custom_message,
            } => {
                let present_count = fields
                    .iter()
                    .filter(|f| {
                        let val = map.get(f.as_str()).unwrap_or(&Value::Null);
                        !val.is_null()
                            && !(val.is_string() && val.as_str().unwrap_or("").trim().is_empty())
                    })
                    .count();
                if present_count > 1 {
                    let full_path =
                        qualify_path(path, fields.first().map(String::as_str).unwrap_or(""));
                    let mut params = RuleParams::new();
                    params.insert("fields", fields.join(", "));
                    params.insert("actual", present_count.to_string());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "mutually_exclusive",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
            Self::Confirmed {
                field,
                custom_message,
            } => {
                let confirmation_field = format!("{field}_confirmation");
                let val_a = map.get(field).unwrap_or(&Value::Null);
                let val_b = map.get(&confirmation_field).unwrap_or(&Value::Null);
                if val_a != val_b {
                    let full_path = qualify_path(path, &confirmation_field);
                    let mut params = RuleParams::new();
                    params.insert("field", field.clone());
                    params.insert("other", confirmation_field.clone());
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        "confirmed",
                        params,
                        custom_message.as_deref(),
                    ))
                } else {
                    None
                }
            }
        }
    }

    fn references_only(&self, keys: &[&str]) -> bool {
        match self {
            Self::SameAs { field, other, .. }
            | Self::DifferentFrom { field, other, .. }
            | Self::RequiredIf { field, other, .. }
            | Self::RequiredUnless { field, other, .. }
            | Self::RequiredWithout { field, other, .. }
            | Self::AfterField { field, other, .. }
            | Self::BeforeField { field, other, .. }
            | Self::RequiredWith { field, other, .. }
            | Self::ProhibitedIf { field, other, .. }
            | Self::ProhibitedWith { field, other, .. }
            | Self::AfterDateField { field, other, .. }
            | Self::BeforeDateField { field, other, .. } => {
                keys.contains(&field.as_str()) && keys.contains(&other.as_str())
            }
            Self::Confirmed { field, .. } => {
                let conf = format!("{field}_confirmation");
                keys.contains(&field.as_str()) && keys.contains(&conf.as_str())
            }
            Self::RequiredWithAll { field, others, .. }
            | Self::RequiredWithoutAll { field, others, .. } => {
                keys.contains(&field.as_str()) && others.iter().all(|o| keys.contains(&o.as_str()))
            }
            Self::AtLeastOneOf { fields, .. }
            | Self::ExactlyOneOf { fields, .. }
            | Self::MutuallyExclusive { fields, .. } => {
                fields.iter().all(|f| keys.contains(&f.as_str()))
            }
        }
    }

    fn references_any(&self, keys: &[&str]) -> bool {
        match self {
            Self::SameAs { field, other, .. }
            | Self::DifferentFrom { field, other, .. }
            | Self::RequiredIf { field, other, .. }
            | Self::RequiredUnless { field, other, .. }
            | Self::RequiredWithout { field, other, .. }
            | Self::AfterField { field, other, .. }
            | Self::BeforeField { field, other, .. }
            | Self::RequiredWith { field, other, .. }
            | Self::ProhibitedIf { field, other, .. }
            | Self::ProhibitedWith { field, other, .. }
            | Self::AfterDateField { field, other, .. }
            | Self::BeforeDateField { field, other, .. } => {
                keys.contains(&field.as_str()) || keys.contains(&other.as_str())
            }
            Self::Confirmed { field, .. } => {
                let conf = format!("{field}_confirmation");
                keys.contains(&field.as_str()) || keys.contains(&conf.as_str())
            }
            Self::RequiredWithAll { field, others, .. }
            | Self::RequiredWithoutAll { field, others, .. } => {
                keys.contains(&field.as_str()) || others.iter().any(|o| keys.contains(&o.as_str()))
            }
            Self::AtLeastOneOf { fields, .. }
            | Self::ExactlyOneOf { fields, .. }
            | Self::MutuallyExclusive { fields, .. } => {
                fields.iter().any(|f| keys.contains(&f.as_str()))
            }
        }
    }
}

/// Fluent schema builder for validating JSON objects, maps, and domain structures.
#[derive(Clone)]
pub struct ObjectSchema {
    fields: Vec<(String, Arc<dyn ErasedField>)>,
    unknown_policy: UnknownFieldPolicy,
    cross_rules: Vec<Arc<dyn CrossFieldRuleValidator>>,
    custom_messages: SimpleMessagesProvider,
    bail_on_fail: bool,
    inline_message: Option<String>,
    coercer: Option<Coercer>,
}

impl Default for ObjectSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectSchema {
    /// Creates a new `ObjectSchema` defaulting to fail-closed `strip` policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            unknown_policy: UnknownFieldPolicy::Strip,
            cross_rules: Vec::new(),
            custom_messages: SimpleMessagesProvider::default(),
            bail_on_fail: false,
            inline_message: None,
            coercer: None,
        }
    }

    /// Registers a required field schema.
    #[must_use]
    pub fn field<S, T>(mut self, name: impl Into<String>, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        let field_name = name.into();
        let entry = Arc::new(FieldEntry {
            schema,
            optional: false,
            phantom: std::marker::PhantomData,
        });
        if let Some(existing) = self.fields.iter_mut().find(|(k, _)| k == &field_name) {
            existing.1 = entry;
        } else {
            self.fields.push((field_name, entry));
        }
        self
    }

    /// Registers an optional field schema.
    #[must_use]
    pub fn optional_field<S, T>(mut self, name: impl Into<String>, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        let field_name = name.into();
        let entry = Arc::new(FieldEntry {
            schema,
            optional: true,
            phantom: std::marker::PhantomData,
        });
        if let Some(existing) = self.fields.iter_mut().find(|(k, _)| k == &field_name) {
            existing.1 = entry;
        } else {
            self.fields.push((field_name, entry));
        }
        self
    }

    /// Configures the schema to silently strip undeclared fields (default).
    #[must_use]
    pub fn strip(mut self) -> Self {
        self.unknown_policy = UnknownFieldPolicy::Strip;
        self
    }

    /// Configures the schema to strictly reject undeclared fields with `422 Unprocessable Entity`.
    #[must_use]
    pub fn strict(mut self) -> Self {
        self.unknown_policy = UnknownFieldPolicy::Strict;
        self
    }

    /// Configures the schema to preserve undeclared fields in the decoded output.
    #[must_use]
    pub fn passthrough(mut self) -> Self {
        self.unknown_policy = UnknownFieldPolicy::Passthrough;
        self
    }

    /// Configures short-circuiting on failure across fields.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Sets an inline custom error message override.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inline_message = Some(message.into());
        self
    }

    /// Overrides error messages for specific fields or rules.
    #[must_use]
    pub fn messages(mut self, overrides: &[(&str, &str)]) -> Self {
        for (key, template) in overrides {
            if let Some((field, rule)) = key.split_once('.') {
                if rule == "*" {
                    self.custom_messages.set_field_wildcard(field, *template);
                } else {
                    self.custom_messages.set_field_rule(field, rule, *template);
                }
            } else {
                self.custom_messages.set_rule_default(*key, *template);
            }
        }
        self
    }

    /// Asserts that `field` and `other` contain identical values (e.g. password confirmation).
    #[must_use]
    pub fn same_as(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules.push(Arc::new(CrossFieldRule::SameAs {
            field: field.into(),
            other: other.into(),
            custom_message: None,
        }));
        self
    }

    /// Asserts that `field` and `other` contain different values.
    #[must_use]
    pub fn different_from(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::DifferentFrom {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is present and non-null when `other` matches `expected`.
    #[must_use]
    pub fn required_if(
        mut self,
        field: impl Into<String>,
        other: impl Into<String>,
        expected: impl Into<Value>,
    ) -> Self {
        self.cross_rules.push(Arc::new(CrossFieldRule::RequiredIf {
            field: field.into(),
            other: other.into(),
            expected: expected.into(),
            custom_message: None,
        }));
        self
    }

    /// Asserts that `field` is present and non-null when `other` is absent or null.
    #[must_use]
    pub fn required_without(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::RequiredWithout {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is temporally or numerically after `other`.
    #[must_use]
    pub fn after_field(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules.push(Arc::new(CrossFieldRule::AfterField {
            field: field.into(),
            other: other.into(),
            custom_message: None,
        }));
        self
    }

    /// Asserts that `field` is temporally or numerically before `other`.
    #[must_use]
    pub fn before_field(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules.push(Arc::new(CrossFieldRule::BeforeField {
            field: field.into(),
            other: other.into(),
            custom_message: None,
        }));
        self
    }

    /// Asserts that `field` is temporally after `other`.
    ///
    /// Alias for [`ObjectSchema::after_field`].
    #[must_use]
    pub fn after_date(self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.after_field(field, other)
    }

    /// Asserts that `field` is temporally before `other`.
    ///
    /// Alias for [`ObjectSchema::before_field`].
    #[must_use]
    pub fn before_date(self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.before_field(field, other)
    }

    /// Asserts that `field` is present and non-null when `other` is present and non-null.
    #[must_use]
    pub fn required_with(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::RequiredWith {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is absent/null when `other` matches `expected`.
    #[must_use]
    pub fn prohibited_if(
        mut self,
        field: impl Into<String>,
        other: impl Into<String>,
        expected: impl Into<Value>,
    ) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::ProhibitedIf {
                field: field.into(),
                other: other.into(),
                expected: expected.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` and `other` cannot both be present and non-null simultaneously.
    #[must_use]
    pub fn prohibited_with(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::ProhibitedWith {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is temporally after `other` based on RFC 3339 timestamp parsing.
    #[must_use]
    pub fn after_date_field(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::AfterDateField {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is temporally before `other` based on RFC 3339 timestamp parsing.
    #[must_use]
    pub fn before_date_field(mut self, field: impl Into<String>, other: impl Into<String>) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::BeforeDateField {
                field: field.into(),
                other: other.into(),
                custom_message: None,
            }));
        self
    }

    /// Extends this schema by merging another schema's fields and cross-field rules.
    #[must_use]
    pub fn extend(mut self, other: Self) -> Self {
        for (name, field_def) in other.fields {
            if let Some(existing) = self.fields.iter_mut().find(|(k, _)| k == &name) {
                existing.1 = field_def;
            } else {
                self.fields.push((name, field_def));
            }
        }
        self.cross_rules.extend(other.cross_rules);
        self
    }

    /// Extends this schema by merging another schema's fields and cross-field rules.
    ///
    /// Alias for [`ObjectSchema::extend`].
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        self.extend(other)
    }

    /// Derives a new `ObjectSchema` retaining only the specified field keys.
    #[must_use]
    pub fn pick(mut self, keys: &[&str]) -> Self {
        self.fields.retain(|(k, _)| keys.contains(&k.as_str()));
        self.cross_rules.retain(|rule| rule.references_only(keys));
        self
    }

    /// Derives a new `ObjectSchema` excluding the specified field keys.
    #[must_use]
    pub fn omit(mut self, keys: &[&str]) -> Self {
        self.fields.retain(|(k, _)| !keys.contains(&k.as_str()));
        self.cross_rules.retain(|rule| !rule.references_any(keys));
        self
    }

    /// Derives a new `ObjectSchema` where all declared fields are converted to optional.
    #[must_use]
    pub fn partial(mut self) -> Self {
        for (_, field_def) in &mut self.fields {
            if !field_def.is_optional() {
                *field_def = Arc::new(OptionalWrapper {
                    inner: field_def.clone(),
                });
            }
        }
        self
    }

    /// Asserts that `field` is present and non-null unless `other` equals `expected`.
    #[must_use]
    pub fn required_unless(
        mut self,
        field: impl Into<String>,
        other: impl Into<String>,
        expected: impl Into<Value>,
    ) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::RequiredUnless {
                field: field.into(),
                other: other.into(),
                expected: expected.into(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is present and non-null when ALL fields in `others` are present and non-null.
    #[must_use]
    pub fn required_with_all(mut self, field: impl Into<String>, others: &[&str]) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::RequiredWithAll {
                field: field.into(),
                others: others.iter().map(|s| (*s).to_string()).collect(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `field` is present and non-null when NONE of the fields in `others` are present.
    #[must_use]
    pub fn required_without_all(mut self, field: impl Into<String>, others: &[&str]) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::RequiredWithoutAll {
                field: field.into(),
                others: others.iter().map(|s| (*s).to_string()).collect(),
                custom_message: None,
            }));
        self
    }

    /// Enforces that at least one of the specified fields is present and non-empty.
    #[must_use]
    pub fn at_least_one_of(mut self, fields: &[&str]) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::AtLeastOneOf {
                fields: fields.iter().map(|s| (*s).to_string()).collect(),
                custom_message: None,
            }));
        self
    }

    /// Enforces that exactly one of the specified fields is present and non-empty.
    #[must_use]
    pub fn exactly_one_of(mut self, fields: &[&str]) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::ExactlyOneOf {
                fields: fields.iter().map(|s| (*s).to_string()).collect(),
                custom_message: None,
            }));
        self
    }

    /// Enforces that the specified fields are mutually exclusive (at most one may be present and non-empty).
    #[must_use]
    pub fn mutually_exclusive(mut self, fields: &[&str]) -> Self {
        self.cross_rules
            .push(Arc::new(CrossFieldRule::MutuallyExclusive {
                fields: fields.iter().map(|s| (*s).to_string()).collect(),
                custom_message: None,
            }));
        self
    }

    /// Asserts that `{field}_confirmation` is present and matches `{field}`.
    #[must_use]
    pub fn confirmed(mut self, field: impl Into<String>) -> Self {
        self.cross_rules.push(Arc::new(CrossFieldRule::Confirmed {
            field: field.into(),
            custom_message: None,
        }));
        self
    }

    /// Attaches an arbitrary custom multi-field rule implementing `CrossFieldRuleValidator`.
    #[must_use]
    pub fn cross_rule<R: CrossFieldRuleValidator + 'static>(mut self, rule: R) -> Self {
        self.cross_rules.push(Arc::new(rule));
        self
    }

    /// Attaches a custom closure-based multi-field refinement rule.
    #[must_use]
    pub fn cross_refine<F>(
        mut self,
        fields: &[&str],
        predicate: F,
        rule_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self
    where
        F: Fn(&Map<String, Value>) -> bool + Send + Sync + 'static,
    {
        struct ClosureCrossRule<F> {
            fields: Vec<String>,
            predicate: F,
            rule_name: String,
            message: String,
        }

        impl<F> CrossFieldRuleValidator for ClosureCrossRule<F>
        where
            F: Fn(&Map<String, Value>) -> bool + Send + Sync + 'static,
        {
            fn evaluate(
                &self,
                map: &Map<String, Value>,
                path: &str,
                rule_ctx: &RuleContext<'_>,
            ) -> Option<ValidationIssue> {
                if !(self.predicate)(map) {
                    let target_field = self.fields.first().map(String::as_str).unwrap_or("");
                    let full_path = qualify_path(path, target_field);
                    let mut params = RuleParams::new();
                    params.insert("fields", self.fields.join(", "));
                    Some(rule_ctx.create_issue_at(
                        &full_path,
                        &self.rule_name,
                        params,
                        Some(&self.message),
                    ))
                } else {
                    None
                }
            }

            fn references_only(&self, keys: &[&str]) -> bool {
                self.fields.iter().all(|f| keys.contains(&f.as_str()))
            }

            fn references_any(&self, keys: &[&str]) -> bool {
                self.fields.iter().any(|f| keys.contains(&f.as_str()))
            }
        }

        self.cross_rules.push(Arc::new(ClosureCrossRule {
            fields: fields.iter().map(|s| (*s).to_string()).collect(),
            predicate,
            rule_name: rule_name.into(),
            message: message.into(),
        }));
        self
    }

    /// Configures a `Coercer` for in-place pre-sanitization of the object payload before field validation.
    #[must_use]
    pub fn coercer(mut self, coercer: Coercer) -> Self {
        self.coercer = Some(coercer);
        self
    }

    /// Enables or disables in-place pre-sanitization of the object payload before field validation.
    #[must_use]
    pub fn sanitize(mut self, enabled: bool) -> Self {
        if enabled {
            self.coercer = Some(Coercer::default());
        } else {
            self.coercer = None;
        }
        self
    }

    /// Validates the payload and decodes directly into a typed struct `T: DeserializeOwned`.
    pub fn decode<T: DeserializeOwned>(&self, value: &Value) -> Result<T, ValidationError> {
        let validated_json = self.validate(value)?;
        serde_json::from_value(validated_json).map_err(|err| {
            let issue = ValidationIssue::new("", "deserialization", err.to_string());
            ValidationError::single(issue.field, issue.rule, issue.message)
        })
    }

    /// Validates the payload and decodes directly into a typed struct `T: DeserializeOwned`.
    ///
    /// Alias for [`ObjectSchema::decode`].
    pub fn validate_and_decode<T: DeserializeOwned>(
        &self,
        value: &Value,
    ) -> Result<T, ValidationError> {
        self.decode(value)
    }

    /// Converts this `ObjectSchema` into a `Schema<T>` that automatically decodes into `T: DeserializeOwned`.
    pub fn decode_into<T>(
        self,
    ) -> crate::schema::TransformSchema<
        Self,
        Value,
        T,
        impl Fn(Value) -> Result<T, String> + Send + Sync + 'static,
    >
    where
        T: DeserializeOwned + Send + Sync + 'static,
    {
        self.transform(|val| serde_json::from_value(val).map_err(|err| err.to_string()))
    }

    #[cfg(feature = "async")]
    /// Asynchronously validates the payload and decodes directly into `T: DeserializeOwned`.
    pub async fn decode_async<T: DeserializeOwned>(
        &self,
        value: &Value,
    ) -> Result<T, ValidationError> {
        let validated_json = self.validate_async(value).await?;
        serde_json::from_value(validated_json).map_err(|err| {
            let issue = ValidationIssue::new("", "deserialization", err.to_string());
            ValidationError::single(issue.field, issue.rule, issue.message)
        })
    }

    /// Checks that the JSON value conforms to this object schema without allocating a sanitized output map.
    pub fn check(&self, value: &Value) -> Result<(), ValidationError> {
        let context = ValidationContext::new();
        self.check_path(
            value,
            "",
            &context,
            None,
            crate::messages::default_messages_provider(),
        )
    }

    /// Internal evaluation method checking conformity without allocating sanitized output maps.
    pub fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        let composite = CompositeMessagesProvider {
            local: &self.custom_messages,
            parent: messages,
        };
        let rule_ctx = RuleContext::new(path, context, parent_data, &composite);
        let inline = self.inline_message.as_deref();

        let sanitized_storage;
        let map = match value {
            Value::Object(obj) => {
                if let Some(coercer) = &self.coercer {
                    let mut cloned = value.clone();
                    coercer.sanitize_in_place(&mut cloned);
                    sanitized_storage = cloned;
                    if let Value::Object(ref sanitized_obj) = sanitized_storage {
                        sanitized_obj
                    } else {
                        obj
                    }
                } else {
                    obj
                }
            }
            Value::Null => {
                let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();

        if self.unknown_policy == UnknownFieldPolicy::Strict {
            for key in map.keys() {
                if !self.fields.iter().any(|(k, _)| k == key) {
                    let field_path = qualify_path(path, key);
                    let mut params = RuleParams::new();
                    params.insert("field", key.clone());
                    let issue = rule_ctx.create_issue("unknown_field", params, inline);
                    errors.push(ValidationIssue {
                        field: field_path.into_owned(),
                        rule: "unknown_field".to_string(),
                        message: issue.message,
                        params: issue.params,
                    });
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                }
            }
        }

        for (field_name, field_def) in &self.fields {
            let field_path = qualify_path(path, field_name);
            let input_val = map.get(field_name).unwrap_or(&Value::Null);

            if input_val.is_null() && field_def.is_optional() {
                continue;
            }

            if let Err(field_err) =
                field_def.check_field(input_val, &field_path, context, Some(value), &composite)
            {
                errors.merge(field_err);
                if self.bail_on_fail {
                    return Err(errors);
                }
            }
        }

        self.evaluate_cross_rules(map, path, &mut errors, &rule_ctx);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn evaluate_cross_rules(
        &self,
        map: &Map<String, Value>,
        path: &str,
        errors: &mut ValidationError,
        rule_ctx: &RuleContext<'_>,
    ) {
        for cross_rule in &self.cross_rules {
            if let Some(issue) = cross_rule.evaluate(map, path, rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    return;
                }
            }
        }
    }
}

impl Schema<Value> for ObjectSchema {
    fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        self.check_path(value, path, context, parent_data, messages)
    }

    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError> {
        let composite = CompositeMessagesProvider {
            local: &self.custom_messages,
            parent: messages,
        };
        let rule_ctx = RuleContext::new(path, context, parent_data, &composite);
        let inline = self.inline_message.as_deref();

        let sanitized_storage;
        let map = match value {
            Value::Object(obj) => {
                if let Some(coercer) = &self.coercer {
                    let mut cloned = value.clone();
                    coercer.sanitize_in_place(&mut cloned);
                    sanitized_storage = cloned;
                    if let Value::Object(ref sanitized_obj) = sanitized_storage {
                        sanitized_obj
                    } else {
                        obj
                    }
                } else {
                    obj
                }
            }
            Value::Null => {
                let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();
        let mut result_map = Map::with_capacity(self.fields.len());

        // 1. Check for unexpected unknown fields
        if self.unknown_policy == UnknownFieldPolicy::Strict {
            for key in map.keys() {
                if !self.fields.iter().any(|(k, _)| k == key) {
                    let field_path = qualify_path(path, key);
                    let mut params = RuleParams::new();
                    params.insert("field", key.clone());
                    let issue = rule_ctx.create_issue("unknown_field", params, inline);
                    errors.push(ValidationIssue {
                        field: field_path.into_owned(),
                        rule: "unknown_field".to_string(),
                        message: issue.message,
                        params: issue.params,
                    });
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                }
            }
        }

        // 2. Validate declared fields
        for (field_name, field_def) in &self.fields {
            let field_path = qualify_path(path, field_name);
            let input_val = map.get(field_name).unwrap_or(&Value::Null);

            if input_val.is_null() && field_def.is_optional() {
                continue;
            }

            match field_def.validate_field(input_val, &field_path, context, Some(value), &composite)
            {
                Ok(valid_val) => {
                    result_map.insert(field_name.clone(), valid_val);
                }
                Err(field_err) => {
                    errors.merge(field_err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                }
            }
        }

        // 3. Passthrough extra fields if policy is Passthrough
        if self.unknown_policy == UnknownFieldPolicy::Passthrough {
            for (key, val) in map {
                if !self.fields.iter().any(|(k, _)| k == key) {
                    result_map.insert(key.clone(), val.clone());
                }
            }
        }

        // 4. Cross-field validation rules
        self.evaluate_cross_rules(map, path, &mut errors, &rule_ctx);

        if errors.is_empty() {
            Ok(Value::Object(result_map))
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
    ) -> BoxFuture<'a, Result<Value, ValidationError>> {
        Box::pin(async move {
            let composite = CompositeMessagesProvider {
                local: &self.custom_messages,
                parent: messages,
            };
            let rule_ctx = RuleContext::new(path, context, parent_data, &composite);
            let inline = self.inline_message.as_deref();

            let sanitized_storage;
            let map = match value {
                Value::Object(obj) => {
                    if let Some(coercer) = &self.coercer {
                        let mut cloned = value.clone();
                        coercer.sanitize_in_place(&mut cloned);
                        sanitized_storage = cloned;
                        if let Value::Object(ref sanitized_obj) = sanitized_storage {
                            sanitized_obj
                        } else {
                            obj
                        }
                    } else {
                        obj
                    }
                }
                Value::Null => {
                    let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
                _ => {
                    let issue = rule_ctx.create_issue("required", RuleParams::new(), inline);
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            };

            let mut errors = ValidationError::new();
            let mut result_map = Map::with_capacity(self.fields.len());

            if self.unknown_policy == UnknownFieldPolicy::Strict {
                for key in map.keys() {
                    if !self.fields.iter().any(|(k, _)| k == key) {
                        let field_path = qualify_path(path, key);
                        let mut params = RuleParams::new();
                        params.insert("field", key.clone());
                        let issue = rule_ctx.create_issue("unknown_field", params, inline);
                        errors.push(ValidationIssue {
                            field: field_path.into_owned(),
                            rule: "unknown_field".to_string(),
                            message: issue.message,
                            params: issue.params,
                        });
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                }
            }

            for (field_name, field_def) in &self.fields {
                let field_path = qualify_path(path, field_name);
                let input_val = map.get(field_name).unwrap_or(&Value::Null);

                if input_val.is_null() && field_def.is_optional() {
                    continue;
                }

                match field_def
                    .validate_async_field(input_val, &field_path, context, Some(value), &composite)
                    .await
                {
                    Ok(valid_val) => {
                        result_map.insert(field_name.clone(), valid_val);
                    }
                    Err(field_err) => {
                        errors.merge(field_err);
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                }
            }

            if self.unknown_policy == UnknownFieldPolicy::Passthrough {
                for (key, val) in map {
                    if !self.fields.iter().any(|(k, _)| k == key) {
                        result_map.insert(key.clone(), val.clone());
                    }
                }
            }

            self.evaluate_cross_rules(map, path, &mut errors, &rule_ctx);

            if errors.is_empty() {
                Ok(Value::Object(result_map))
            } else {
                Err(errors)
            }
        })
    }

    fn to_json_schema_value(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for (name, field_def) in &self.fields {
            properties.insert(name.clone(), field_def.to_json_schema());
            if !field_def.is_optional() {
                required.push(Value::String(name.clone()));
            }
        }
        let mut schema_map = serde_json::Map::new();
        schema_map.insert("type".to_string(), Value::String("object".to_string()));
        schema_map.insert("properties".to_string(), Value::Object(properties));
        if !required.is_empty() {
            schema_map.insert("required".to_string(), Value::Array(required));
        }
        Value::Object(schema_map)
    }
}

#[inline]
fn qualify_path<'a>(base_path: &'a str, field_name: &'a str) -> std::borrow::Cow<'a, str> {
    if base_path.is_empty() {
        std::borrow::Cow::Borrowed(field_name)
    } else {
        std::borrow::Cow::Owned(format!("{base_path}.{field_name}"))
    }
}
