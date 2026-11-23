//! Union schema combinators for validating multiple alternative schemas.

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

macro_rules! define_union_schema {
    (
        $struct_name:ident,
        $builder_name:ident,
        $( ($field:ident, $schema_ty:ident) ),+
    ) => {
        /// Union schema combinator validating multiple alternative schemas for type `T`.
        #[derive(Clone)]
        pub struct $struct_name<$( $schema_ty, )+ T> {
            $( $field: $schema_ty, )+
            inline_message: Option<String>,
            phantom: std::marker::PhantomData<T>,
        }

        impl<$( $schema_ty, )+ T> $struct_name<$( $schema_ty, )+ T>
        where
            $( $schema_ty: Schema<T>, )+
        {
            /// Creates a new union schema.
            #[must_use]
            pub fn new($( $field: $schema_ty, )+) -> Self {
                Self {
                    $( $field, )+
                    inline_message: None,
                    phantom: std::marker::PhantomData,
                }
            }

            /// Sets an inline custom message override.
            #[must_use]
            pub fn with_message(mut self, message: impl Into<String>) -> Self {
                self.inline_message = Some(message.into());
                self
            }
        }

        impl<$( $schema_ty, )+ T> Schema<T> for $struct_name<$( $schema_ty, )+ T>
        where
            $( $schema_ty: Schema<T>, )+
            T: Send + Sync + 'static,
        {
            fn validate_path(
                &self,
                value: &Value,
                path: &str,
                context: &ValidationContext,
                parent_data: Option<&Value>,
                messages: &dyn MessagesProvider,
            ) -> Result<T, ValidationError> {
                let mut combined_errors = ValidationError::new();
                $(
                    match self.$field.validate_path(value, path, context, parent_data, messages) {
                        Ok(val) => return Ok(val),
                        Err(err) => combined_errors.merge(err),
                    }
                )+

                if let Some(msg) = &self.inline_message {
                    let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                    let issue = rule_ctx.create_issue("union", RuleParams::new(), Some(msg));
                    Err(ValidationError::single(issue.field, issue.rule, issue.message))
                } else {
                    Err(combined_errors)
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
                $(
                    if self.$field.check_path(value, path, context, parent_data, messages).is_ok() {
                        return Ok(());
                    }
                )+
                self.validate_path(value, path, context, parent_data, messages).map(|_| ())
            }

            #[cfg(feature = "async")]
            fn validate_async_path<'a>(
                &'a self,
                value: &'a Value,
                path: &'a str,
                context: &'a ValidationContext,
                parent_data: Option<&'a Value>,
                messages: &'a dyn MessagesProvider,
            ) -> BoxFuture<'a, Result<T, ValidationError>> {
                Box::pin(async move {
                    let mut combined_errors = ValidationError::new();
                    $(
                        match self.$field.validate_async_path(value, path, context, parent_data, messages).await {
                            Ok(val) => return Ok(val),
                            Err(err) => combined_errors.merge(err),
                        }
                    )+

                    if let Some(msg) = &self.inline_message {
                        let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                        let issue = rule_ctx.create_issue("union", RuleParams::new(), Some(msg));
                        Err(ValidationError::single(issue.field, issue.rule, issue.message))
                    } else {
                        Err(combined_errors)
                    }
                })
            }
        }

        /// Creates a union schema builder.
        #[must_use]
        pub fn $builder_name<$( $schema_ty, )+ T>(
            $( $field: $schema_ty, )+
        ) -> $struct_name<$( $schema_ty, )+ T>
        where
            $( $schema_ty: Schema<T>, )+
        {
            $struct_name::new($( $field, )+)
        }
    };
}

define_union_schema!(Union2Schema, union2, (first, S1), (second, S2));
define_union_schema!(Union3Schema, union3, (first, S1), (second, S2), (third, S3));
define_union_schema!(
    Union4Schema,
    union4,
    (first, S1),
    (second, S2),
    (third, S3),
    (fourth, S4)
);

/// Creates a binary union schema builder (ergonomic alias for `union2`).
#[must_use]
pub fn union<S1, S2, T>(first: S1, second: S2) -> Union2Schema<S1, S2, T>
where
    S1: Schema<T>,
    S2: Schema<T>,
{
    union2(first, second)
}
