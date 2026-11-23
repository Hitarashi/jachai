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

macro_rules! define_tuple_schema {
    (
        $struct_name:ident,
        $builder_name:ident,
        $len:expr,
        $len_str:expr,
        $( ($idx:tt, $field:ident, $schema_ty:ident, $val_ty:ident) ),+
    ) => {
        /// Tuple schema validating a fixed-length heterogeneous sequence.
        #[derive(Clone)]
        pub struct $struct_name<$( $schema_ty, )+ $( $val_ty, )+> {
            $( $field: $schema_ty, )+
            bail_on_fail: bool,
            inline_message: Option<String>,
            phantom: std::marker::PhantomData<($( $val_ty, )+)>,
        }

        impl<$( $schema_ty, )+ $( $val_ty, )+> $struct_name<$( $schema_ty, )+ $( $val_ty, )+>
        where
            $( $schema_ty: Schema<$val_ty>, )+
        {
            /// Creates a new tuple schema.
            #[must_use]
            pub fn new($( $field: $schema_ty, )+) -> Self {
                Self {
                    $( $field, )+
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

        impl<$( $schema_ty, )+ $( $val_ty, )+> Schema<($( $val_ty, )+)>
            for $struct_name<$( $schema_ty, )+ $( $val_ty, )+>
        where
            $( $schema_ty: Schema<$val_ty>, )+
            $( $val_ty: Send + Sync + 'static, )+
        {
            fn validate_path(
                &self,
                value: &Value,
                path: &str,
                context: &ValidationContext,
                parent_data: Option<&Value>,
                messages: &dyn MessagesProvider,
            ) -> Result<($( $val_ty, )+), ValidationError> {
                let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                let inline = self.inline_message.as_deref();

                let items = match value {
                    Value::Array(arr) if arr.len() == $len => arr,
                    _ => {
                        let mut params = RuleParams::new();
                        params.insert("length", $len_str);
                        let issue = rule_ctx.create_issue("exact_items", params, inline);
                        return Err(ValidationError::single(
                            issue.field,
                            issue.rule,
                            issue.message,
                        ));
                    }
                };

                let mut errors = ValidationError::new();

                $(
                    let sub_path = if path.is_empty() {
                        std::borrow::Cow::Borrowed(concat!("[", stringify!($idx), "]"))
                    } else {
                        std::borrow::Cow::Owned(format!("{}[{}]", path, $idx))
                    };
                    let res = self.$field.validate_path(
                        &items[$idx],
                        &sub_path,
                        context,
                        Some(value),
                        messages,
                    );
                    let $field = match res {
                        Ok(val) => Some(val),
                        Err(err) => {
                            errors.merge(err);
                            if self.bail_on_fail {
                                return Err(errors);
                            }
                            None
                        }
                    };
                )+

                if let ($( Some($field), )+) = ($( $field, )+) {
                    Ok(($( $field, )+))
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
                let items = match value {
                    Value::Array(arr) if arr.len() == $len => arr,
                    _ => {
                        return self.validate_path(value, path, context, parent_data, messages).map(|_| ());
                    }
                };

                let mut errors = ValidationError::new();
                $(
                    let sub_path = if path.is_empty() {
                        std::borrow::Cow::Borrowed(concat!("[", stringify!($idx), "]"))
                    } else {
                        std::borrow::Cow::Owned(format!("{}[{}]", path, $idx))
                    };
                    if let Err(err) = self.$field.check_path(
                        &items[$idx],
                        &sub_path,
                        context,
                        Some(value),
                        messages,
                    ) {
                        errors.merge(err);
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                )+

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
            ) -> BoxFuture<'a, Result<($( $val_ty, )+), ValidationError>> {
                Box::pin(async move {
                    let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                    let inline = self.inline_message.as_deref();

                    let items = match value {
                        Value::Array(arr) if arr.len() == $len => arr,
                        _ => {
                            let mut params = RuleParams::new();
                            params.insert("length", $len_str);
                            let issue = rule_ctx.create_issue("exact_items", params, inline);
                            return Err(ValidationError::single(
                                issue.field,
                                issue.rule,
                                issue.message,
                            ));
                        }
                    };

                    let mut errors = ValidationError::new();

                    $(
                        let sub_path = if path.is_empty() {
                            format!("[{}]", $idx)
                        } else {
                            format!("{}[{}]", path, $idx)
                        };
                        let res = self
                            .$field
                            .validate_async_path(
                                &items[$idx],
                                &sub_path,
                                context,
                                Some(value),
                                messages,
                            )
                            .await;
                        let $field = match res {
                            Ok(val) => Some(val),
                            Err(err) => {
                                errors.merge(err);
                                if self.bail_on_fail {
                                    return Err(errors);
                                }
                                None
                            }
                        };
                    )+

                    if let ($( Some($field), )+) = ($( $field, )+) {
                        Ok(($( $field, )+))
                    } else {
                        Err(errors)
                    }
                })
            }
        }

        /// Creates a tuple schema builder.
        #[must_use]
        pub fn $builder_name<$( $schema_ty, )+ $( $val_ty, )+>(
            $( $field: $schema_ty, )+
        ) -> $struct_name<$( $schema_ty, )+ $( $val_ty, )+>
        where
            $( $schema_ty: Schema<$val_ty>, )+
        {
            $struct_name::new($( $field, )+)
        }
    };
}

define_tuple_schema!(
    Tuple2Schema,
    tuple2,
    2,
    "2",
    (0, first, S1, T1),
    (1, second, S2, T2)
);

define_tuple_schema!(
    Tuple3Schema,
    tuple3,
    3,
    "3",
    (0, first, S1, T1),
    (1, second, S2, T2),
    (2, third, S3, T3)
);

define_tuple_schema!(
    Tuple4Schema,
    tuple4,
    4,
    "4",
    (0, first, S1, T1),
    (1, second, S2, T2),
    (2, third, S3, T3),
    (3, fourth, S4, T4)
);

/// Creates a 2-element tuple schema builder (ergonomic alias for `tuple2`).
#[must_use]
pub fn tuple<S1, S2, T1, T2>(first: S1, second: S2) -> Tuple2Schema<S1, S2, T1, T2>
where
    S1: Schema<T1>,
    S2: Schema<T2>,
{
    tuple2(first, second)
}

macro_rules! impl_schema_for_rust_tuple {
    ($len:expr, $len_str:expr, $( ($idx:tt, $item:ident, $schema_ty:ident, $val_ty:ident) ),+) => {
        impl<$( $schema_ty, )+ $( $val_ty, )+> Schema<($( $val_ty, )+)> for ($( $schema_ty, )+)
        where
            $( $schema_ty: Schema<$val_ty>, )+
            $( $val_ty: Send + Sync + 'static, )+
        {
            fn validate_path(
                &self,
                value: &Value,
                path: &str,
                context: &ValidationContext,
                parent_data: Option<&Value>,
                messages: &dyn MessagesProvider,
            ) -> Result<($( $val_ty, )+), ValidationError> {
                let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                let items = match value {
                    Value::Array(arr) if arr.len() == $len => arr,
                    _ => {
                        let mut params = RuleParams::new();
                        params.insert("length", $len_str);
                        let issue = rule_ctx.create_issue("exact_items", params, None);
                        return Err(ValidationError::single(
                            issue.field,
                            issue.rule,
                            issue.message,
                        ));
                    }
                };

                let mut errors = ValidationError::new();
                $(
                    let sub_path = if path.is_empty() {
                        std::borrow::Cow::Borrowed(concat!("[", stringify!($idx), "]"))
                    } else {
                        std::borrow::Cow::Owned(format!("{}[{}]", path, $idx))
                    };
                    let res = self.$idx.validate_path(
                        &items[$idx],
                        &sub_path,
                        context,
                        Some(value),
                        messages,
                    );
                    let $item = match res {
                        Ok(v) => Some(v),
                        Err(e) => {
                            errors.merge(e);
                            None
                        }
                    };
                )+

                if let ($( Some($item), )+) = ($( $item, )+) {
                    Ok(($( $item, )+))
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
                let items = match value {
                    Value::Array(arr) if arr.len() == $len => arr,
                    _ => {
                        return self.validate_path(value, path, context, parent_data, messages).map(|_| ());
                    }
                };

                let mut errors = ValidationError::new();
                $(
                    let sub_path = if path.is_empty() {
                        std::borrow::Cow::Borrowed(concat!("[", stringify!($idx), "]"))
                    } else {
                        std::borrow::Cow::Owned(format!("{}[{}]", path, $idx))
                    };
                    if let Err(e) = self.$idx.check_path(
                        &items[$idx],
                        &sub_path,
                        context,
                        Some(value),
                        messages,
                    ) {
                        errors.merge(e);
                    }
                )+

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
            ) -> BoxFuture<'a, Result<($( $val_ty, )+), ValidationError>> {
                Box::pin(async move {
                    let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                    let items = match value {
                        Value::Array(arr) if arr.len() == $len => arr,
                        _ => {
                            let mut params = RuleParams::new();
                            params.insert("length", $len_str);
                            let issue = rule_ctx.create_issue("exact_items", params, None);
                            return Err(ValidationError::single(
                                issue.field,
                                issue.rule,
                                issue.message,
                            ));
                        }
                    };

                    let mut errors = ValidationError::new();
                    $(
                        let sub_path = if path.is_empty() {
                            format!("[{}]", $idx)
                        } else {
                            format!("{}[{}]", path, $idx)
                        };
                        let res = self
                            .$idx
                            .validate_async_path(
                                &items[$idx],
                                &sub_path,
                                context,
                                Some(value),
                                messages,
                            )
                            .await;
                        let $item = match res {
                            Ok(v) => Some(v),
                            Err(e) => {
                                errors.merge(e);
                                None
                            }
                        };
                    )+

                    if let ($( Some($item), )+) = ($( $item, )+) {
                        Ok(($( $item, )+))
                    } else {
                        Err(errors)
                    }
                })
            }
        }
    };
}

impl_schema_for_rust_tuple!(2, "2", (0, item0, S0, T0), (1, item1, S1, T1));
impl_schema_for_rust_tuple!(
    3,
    "3",
    (0, item0, S0, T0),
    (1, item1, S1, T1),
    (2, item2, S2, T2)
);
impl_schema_for_rust_tuple!(
    4,
    "4",
    (0, item0, S0, T0),
    (1, item1, S1, T1),
    (2, item2, S2, T2),
    (3, item3, S3, T3)
);
impl_schema_for_rust_tuple!(
    5,
    "5",
    (0, item0, S0, T0),
    (1, item1, S1, T1),
    (2, item2, S2, T2),
    (3, item3, S3, T3),
    (4, item4, S4, T4)
);
impl_schema_for_rust_tuple!(
    6,
    "6",
    (0, item0, S0, T0),
    (1, item1, S1, T1),
    (2, item2, S2, T2),
    (3, item3, S3, T3),
    (4, item4, S4, T4),
    (5, item5, S5, T5)
);
