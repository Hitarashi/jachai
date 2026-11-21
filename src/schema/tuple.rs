//! Tuple schema builders for fixed-length heterogeneous sequences.

#[cfg(feature = "async")]
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::{
    context::ValidationContext,
    error::{RuleParams, ValidationError},
    messages::MessagesProvider,
    rule::RuleContext,
    schema::Schema,
};

/// Tuple schema validating a 2-element heterogeneous sequence `(T1, T2)`.
#[derive(Clone)]
pub struct Tuple2Schema<S1, S2, T1, T2> {
    first: S1,
    second: S2,
    bail_on_fail: bool,
    inline_message: Option<String>,
    phantom: std::marker::PhantomData<(T1, T2)>,
}

impl<S1, S2, T1, T2> Tuple2Schema<S1, S2, T1, T2>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
{
    /// Creates a new `Tuple2Schema`.
    #[must_use]
    pub fn new(first: S1, second: S2) -> Self {
        Self {
            first,
            second,
            bail_on_fail: false,
            inline_message: None,
            phantom: std::marker::PhantomData,
        }
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

impl<S1, S2, T1, T2> Schema<(T1, T2)> for Tuple2Schema<S1, S2, T1, T2>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static,
{
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(T1, T2), ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
        let inline = self.inline_message.as_deref();

        let items = match value {
            Value::Array(arr) if arr.len() == 2 => arr,
            _ => {
                let mut params = RuleParams::new();
                params.insert("length", "2");
                let issue = rule_ctx.create_issue("exact_items", params, inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();

        let path1 = if path.is_empty() {
            "[0]".to_string()
        } else {
            format!("{path}[0]")
        };
        let path2 = if path.is_empty() {
            "[1]".to_string()
        } else {
            format!("{path}[1]")
        };

        let res1 = self
            .first
            .validate_path(&items[0], &path1, context, Some(value), messages);
        let res2 = self
            .second
            .validate_path(&items[1], &path2, context, Some(value), messages);

        let valid1 = match res1 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let valid2 = match res2 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        if let (Some(v1), Some(v2)) = (valid1, valid2) {
            Ok((v1, v2))
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
    ) -> BoxFuture<'a, Result<(T1, T2), ValidationError>> {
        Box::pin(async move {
            let rule_ctx = RuleContext::new(path, context, parent_data, messages);
            let inline = self.inline_message.as_deref();

            let items = match value {
                Value::Array(arr) if arr.len() == 2 => arr,
                _ => {
                    let mut params = RuleParams::new();
                    params.insert("length", "2");
                    let issue = rule_ctx.create_issue("exact_items", params, inline);
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            };

            let mut errors = ValidationError::new();

            let path1 = if path.is_empty() {
                "[0]".to_string()
            } else {
                format!("{path}[0]")
            };
            let path2 = if path.is_empty() {
                "[1]".to_string()
            } else {
                format!("{path}[1]")
            };

            let res1 = self
                .first
                .validate_async_path(&items[0], &path1, context, Some(value), messages)
                .await;
            let res2 = self
                .second
                .validate_async_path(&items[1], &path2, context, Some(value), messages)
                .await;

            let valid1 = match res1 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let valid2 = match res2 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            if let (Some(v1), Some(v2)) = (valid1, valid2) {
                Ok((v1, v2))
            } else {
                Err(errors)
            }
        })
    }
}

/// Tuple schema validating a 3-element heterogeneous sequence `(T1, T2, T3)`.
#[derive(Clone)]
pub struct Tuple3Schema<S1, S2, S3, T1, T2, T3> {
    first: S1,
    second: S2,
    third: S3,
    bail_on_fail: bool,
    inline_message: Option<String>,
    phantom: std::marker::PhantomData<(T1, T2, T3)>,
}

impl<S1, S2, S3, T1, T2, T3> Tuple3Schema<S1, S2, S3, T1, T2, T3>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
{
    /// Creates a new `Tuple3Schema`.
    #[must_use]
    pub fn new(first: S1, second: S2, third: S3) -> Self {
        Self {
            first,
            second,
            third,
            bail_on_fail: false,
            inline_message: None,
            phantom: std::marker::PhantomData,
        }
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

impl<S1, S2, S3, T1, T2, T3> Schema<(T1, T2, T3)> for Tuple3Schema<S1, S2, S3, T1, T2, T3>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static,
    T3: Send + Sync + 'static,
{
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(T1, T2, T3), ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
        let inline = self.inline_message.as_deref();

        let items = match value {
            Value::Array(arr) if arr.len() == 3 => arr,
            _ => {
                let mut params = RuleParams::new();
                params.insert("length", "3");
                let issue = rule_ctx.create_issue("exact_items", params, inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();

        let path1 = if path.is_empty() {
            "[0]".to_string()
        } else {
            format!("{path}[0]")
        };
        let path2 = if path.is_empty() {
            "[1]".to_string()
        } else {
            format!("{path}[1]")
        };
        let path3 = if path.is_empty() {
            "[2]".to_string()
        } else {
            format!("{path}[2]")
        };

        let res1 = self
            .first
            .validate_path(&items[0], &path1, context, Some(value), messages);
        let valid1 = match res1 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let res2 = self
            .second
            .validate_path(&items[1], &path2, context, Some(value), messages);
        let valid2 = match res2 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let res3 = self
            .third
            .validate_path(&items[2], &path3, context, Some(value), messages);
        let valid3 = match res3 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        if let (Some(v1), Some(v2), Some(v3)) = (valid1, valid2, valid3) {
            Ok((v1, v2, v3))
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
    ) -> BoxFuture<'a, Result<(T1, T2, T3), ValidationError>> {
        Box::pin(async move {
            let rule_ctx = RuleContext::new(path, context, parent_data, messages);
            let inline = self.inline_message.as_deref();

            let items = match value {
                Value::Array(arr) if arr.len() == 3 => arr,
                _ => {
                    let mut params = RuleParams::new();
                    params.insert("length", "3");
                    let issue = rule_ctx.create_issue("exact_items", params, inline);
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            };

            let mut errors = ValidationError::new();

            let path1 = if path.is_empty() {
                "[0]".to_string()
            } else {
                format!("{path}[0]")
            };
            let path2 = if path.is_empty() {
                "[1]".to_string()
            } else {
                format!("{path}[1]")
            };
            let path3 = if path.is_empty() {
                "[2]".to_string()
            } else {
                format!("{path}[2]")
            };

            let res1 = self
                .first
                .validate_async_path(&items[0], &path1, context, Some(value), messages)
                .await;
            let valid1 = match res1 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let res2 = self
                .second
                .validate_async_path(&items[1], &path2, context, Some(value), messages)
                .await;
            let valid2 = match res2 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let res3 = self
                .third
                .validate_async_path(&items[2], &path3, context, Some(value), messages)
                .await;
            let valid3 = match res3 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            if let (Some(v1), Some(v2), Some(v3)) = (valid1, valid2, valid3) {
                Ok((v1, v2, v3))
            } else {
                Err(errors)
            }
        })
    }
}

/// Tuple schema validating a 4-element heterogeneous sequence `(T1, T2, T3, T4)`.
#[derive(Clone)]
pub struct Tuple4Schema<S1, S2, S3, S4, T1, T2, T3, T4> {
    first: S1,
    second: S2,
    third: S3,
    fourth: S4,
    bail_on_fail: bool,
    inline_message: Option<String>,
    phantom: std::marker::PhantomData<(T1, T2, T3, T4)>,
}

impl<S1, S2, S3, S4, T1, T2, T3, T4> Tuple4Schema<S1, S2, S3, S4, T1, T2, T3, T4>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
    S4: Schema<T4>,
{
    /// Creates a new `Tuple4Schema`.
    #[must_use]
    pub fn new(first: S1, second: S2, third: S3, fourth: S4) -> Self {
        Self {
            first,
            second,
            third,
            fourth,
            bail_on_fail: false,
            inline_message: None,
            phantom: std::marker::PhantomData,
        }
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

impl<S1, S2, S3, S4, T1, T2, T3, T4> Schema<(T1, T2, T3, T4)>
    for Tuple4Schema<S1, S2, S3, S4, T1, T2, T3, T4>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
    S4: Schema<T4>,
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static,
    T3: Send + Sync + 'static,
    T4: Send + Sync + 'static,
{
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(T1, T2, T3, T4), ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
        let inline = self.inline_message.as_deref();

        let items = match value {
            Value::Array(arr) if arr.len() == 4 => arr,
            _ => {
                let mut params = RuleParams::new();
                params.insert("length", "4");
                let issue = rule_ctx.create_issue("exact_items", params, inline);
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let mut errors = ValidationError::new();

        let path1 = if path.is_empty() {
            "[0]".to_string()
        } else {
            format!("{path}[0]")
        };
        let path2 = if path.is_empty() {
            "[1]".to_string()
        } else {
            format!("{path}[1]")
        };
        let path3 = if path.is_empty() {
            "[2]".to_string()
        } else {
            format!("{path}[2]")
        };
        let path4 = if path.is_empty() {
            "[3]".to_string()
        } else {
            format!("{path}[3]")
        };

        let res1 = self
            .first
            .validate_path(&items[0], &path1, context, Some(value), messages);
        let valid1 = match res1 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let res2 = self
            .second
            .validate_path(&items[1], &path2, context, Some(value), messages);
        let valid2 = match res2 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let res3 = self
            .third
            .validate_path(&items[2], &path3, context, Some(value), messages);
        let valid3 = match res3 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        let res4 = self
            .fourth
            .validate_path(&items[3], &path4, context, Some(value), messages);
        let valid4 = match res4 {
            Ok(val) => Some(val),
            Err(err) => {
                errors.merge(err);
                if self.bail_on_fail {
                    return Err(errors);
                }
                None
            }
        };

        if let (Some(v1), Some(v2), Some(v3), Some(v4)) = (valid1, valid2, valid3, valid4) {
            Ok((v1, v2, v3, v4))
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
    ) -> BoxFuture<'a, Result<(T1, T2, T3, T4), ValidationError>> {
        Box::pin(async move {
            let rule_ctx = RuleContext::new(path, context, parent_data, messages);
            let inline = self.inline_message.as_deref();

            let items = match value {
                Value::Array(arr) if arr.len() == 4 => arr,
                _ => {
                    let mut params = RuleParams::new();
                    params.insert("length", "4");
                    let issue = rule_ctx.create_issue("exact_items", params, inline);
                    return Err(ValidationError::single(
                        issue.field,
                        issue.rule,
                        issue.message,
                    ));
                }
            };

            let mut errors = ValidationError::new();

            let path1 = if path.is_empty() {
                "[0]".to_string()
            } else {
                format!("{path}[0]")
            };
            let path2 = if path.is_empty() {
                "[1]".to_string()
            } else {
                format!("{path}[1]")
            };
            let path3 = if path.is_empty() {
                "[2]".to_string()
            } else {
                format!("{path}[2]")
            };
            let path4 = if path.is_empty() {
                "[3]".to_string()
            } else {
                format!("{path}[3]")
            };

            let res1 = self
                .first
                .validate_async_path(&items[0], &path1, context, Some(value), messages)
                .await;
            let valid1 = match res1 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let res2 = self
                .second
                .validate_async_path(&items[1], &path2, context, Some(value), messages)
                .await;
            let valid2 = match res2 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let res3 = self
                .third
                .validate_async_path(&items[2], &path3, context, Some(value), messages)
                .await;
            let valid3 = match res3 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            let res4 = self
                .fourth
                .validate_async_path(&items[3], &path4, context, Some(value), messages)
                .await;
            let valid4 = match res4 {
                Ok(val) => Some(val),
                Err(err) => {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                    None
                }
            };

            if let (Some(v1), Some(v2), Some(v3), Some(v4)) = (valid1, valid2, valid3, valid4) {
                Ok((v1, v2, v3, v4))
            } else {
                Err(errors)
            }
        })
    }
}

/// Creates a 2-element tuple schema builder.
#[must_use]
pub fn tuple2<S1, S2, T1, T2>(first: S1, second: S2) -> Tuple2Schema<S1, S2, T1, T2>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
{
    Tuple2Schema::new(first, second)
}

/// Creates a 3-element tuple schema builder.
#[must_use]
pub fn tuple3<S1, S2, S3, T1, T2, T3>(
    first: S1,
    second: S2,
    third: S3,
) -> Tuple3Schema<S1, S2, S3, T1, T2, T3>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
{
    Tuple3Schema::new(first, second, third)
}

/// Creates a 4-element tuple schema builder.
#[must_use]
pub fn tuple4<S1, S2, S3, S4, T1, T2, T3, T4>(
    first: S1,
    second: S2,
    third: S3,
    fourth: S4,
) -> Tuple4Schema<S1, S2, S3, S4, T1, T2, T3, T4>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
    S3: Schema<T3>,
    S4: Schema<T4>,
{
    Tuple4Schema::new(first, second, third, fourth)
}

/// Creates a 2-element tuple schema builder (ergonomic alias for `tuple2`).
#[must_use]
pub fn tuple<S1, S2, T1, T2>(first: S1, second: S2) -> Tuple2Schema<S1, S2, T1, T2>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
{
    tuple2(first, second)
}
