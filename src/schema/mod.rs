//! Composable schema builders, modifiers, and validation traits.

pub mod array;
pub mod boolean;
pub mod date;
pub mod r#enum;
pub mod file;
pub mod json_schema;
pub mod number;
pub mod object;
pub mod optional;
pub mod record;
pub mod string;
pub mod transform;
pub mod tuple;
pub mod union;

pub use array::ArraySchema;
pub use boolean::BooleanSchema;
pub use date::{DateFormat, DateSchema};
pub use r#enum::EnumSchema;
pub use file::{FileMetadata, FileSchema};
#[cfg(feature = "async")]
use futures_util::future::BoxFuture;
pub use json_schema::ToJsonSchema;
pub use number::{IntegerSchema, NumberSchema};
pub use object::{ObjectSchema, UnknownFieldPolicy};
pub use optional::{DefaultSchema, NullableSchema, OptionalSchema};
pub use record::RecordSchema;
use serde_json::Value;
pub use string::StringSchema;
pub use transform::{MapSchema, RefineSchema, TransformSchema};
pub use tuple::{Tuple2Schema, Tuple3Schema, Tuple4Schema, tuple, tuple2, tuple3, tuple4};
pub use union::{Union2Schema, Union3Schema, Union4Schema, union, union2, union3, union4};

use crate::{context::ValidationContext, error::ValidationError, messages::MessagesProvider};

/// Core schema contract for validating and decoding untrusted inputs into typed models.
pub trait Schema<T>: Send + Sync + 'static {
    /// Validates and decodes untrusted input using a default context.
    fn validate(&self, value: &Value) -> Result<T, ValidationError> {
        let context = ValidationContext::new();
        self.validate_path(
            value,
            "",
            &context,
            None,
            crate::messages::default_messages_provider(),
        )
    }

    /// Validates with an ambient metadata context.
    fn validate_with_context(
        &self,
        value: &Value,
        context: &ValidationContext,
    ) -> Result<T, ValidationError> {
        self.validate_path(
            value,
            "",
            context,
            None,
            crate::messages::default_messages_provider(),
        )
    }

    /// Internal evaluation method tracking the hierarchical path, parent data, and message provider.
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<T, ValidationError>;

    #[cfg(feature = "async")]
    /// Asynchronously validates and decodes untrusted input.
    fn validate_async<'a>(&'a self, value: &'a Value) -> BoxFuture<'a, Result<T, ValidationError>> {
        let context = ValidationContext::new();
        Box::pin(async move {
            self.validate_async_path(
                value,
                "",
                &context,
                None,
                crate::messages::default_messages_provider(),
            )
            .await
        })
    }

    #[cfg(feature = "async")]
    /// Asynchronously validates with an ambient metadata context.
    fn validate_async_with_context<'a>(
        &'a self,
        value: &'a Value,
        context: &'a ValidationContext,
    ) -> BoxFuture<'a, Result<T, ValidationError>> {
        Box::pin(async move {
            self.validate_async_path(
                value,
                "",
                context,
                None,
                crate::messages::default_messages_provider(),
            )
            .await
        })
    }

    #[cfg(feature = "async")]
    /// Internal async evaluation method.
    fn validate_async_path<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<T, ValidationError>> {
        Box::pin(async move { self.validate_path(value, path, context, parent_data, messages) })
    }

    /// Wraps this schema allowing `null` or missing inputs, returning `Option<T>`.
    fn optional(self) -> OptionalSchema<Self, T>
    where
        Self: Sized,
    {
        OptionalSchema::new(self)
    }

    /// Wraps this schema allowing explicit JSON `null`, returning `Option<T>`.
    fn nullable(self) -> NullableSchema<Self, T>
    where
        Self: Sized,
    {
        NullableSchema::new(self)
    }

    /// Provides a fallback default value if the input is `null`.
    fn default_value(self, default: T) -> DefaultSchema<Self, T>
    where
        Self: Sized,
        T: Clone,
    {
        DefaultSchema::new(self, default)
    }

    /// Provides a fallback default value if the input is `null`.
    ///
    /// Alias for [`Schema::default_value`].
    fn default(self, default: T) -> DefaultSchema<Self, T>
    where
        Self: Sized,
        T: Clone,
    {
        self.default_value(default)
    }

    /// Transforms the validated result `T` into another type `Out`.
    fn transform<Out, F>(self, transformer: F) -> TransformSchema<Self, T, Out, F>
    where
        Self: Sized,
        F: Fn(T) -> Result<Out, String> + Send + Sync + 'static,
    {
        TransformSchema::new(self, transformer)
    }

    /// Maps the validated result `T` into another type `Out` using an infallible mapping function.
    fn map<Out, F>(self, mapper: F) -> MapSchema<Self, T, Out, F>
    where
        Self: Sized,
        F: Fn(T) -> Out + Send + Sync + 'static,
    {
        MapSchema::new(self, mapper)
    }

    /// Refines the validated value with an additional closure predicate.
    fn refine<F>(self, predicate: F) -> RefineSchema<Self, T, F>
    where
        Self: Sized,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        RefineSchema::new(self, predicate, "refine", "Validation refinement failed.")
    }

    /// Refines the validated value with a custom rule name and error message.
    fn refine_with<F>(
        self,
        predicate: F,
        rule_name: impl Into<String>,
        message: impl Into<String>,
    ) -> RefineSchema<Self, T, F>
    where
        Self: Sized,
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        RefineSchema::new(self, predicate, rule_name, message)
    }

    /// Indicates whether this schema represents an optional or nullable field.
    fn is_optional(&self) -> bool {
        false
    }

    /// Generates a standard Draft 2020-12 / OpenAPI 3.1 JSON Schema representation.
    fn to_json_schema_value(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }
}

/// Creates a new date/time schema builder.
#[must_use]
pub fn date() -> DateSchema {
    DateSchema::new()
}

/// Creates a new file metadata schema builder.
#[must_use]
pub fn file() -> FileSchema {
    FileSchema::new()
}

/// Creates a new string schema builder.
#[must_use]
pub fn string() -> StringSchema {
    StringSchema::new()
}

/// Creates a new floating-point number schema builder (`f64`).
#[must_use]
pub fn number() -> NumberSchema {
    NumberSchema::new()
}

/// Creates a new integer number schema builder (`i64`).
#[must_use]
pub fn integer() -> IntegerSchema {
    IntegerSchema::new()
}

/// Creates a new boolean schema builder.
#[must_use]
pub fn boolean() -> BooleanSchema {
    BooleanSchema::new()
}

/// Creates a new object schema builder.
#[must_use]
pub fn object() -> ObjectSchema {
    ObjectSchema::new()
}

/// Creates a new array schema builder wrapping an element schema.
#[must_use]
pub fn array<S, T>(element_schema: S) -> ArraySchema<S, T>
where
    S: Schema<T>,
{
    ArraySchema::new(element_schema)
}

/// Creates a new record schema builder wrapping a value schema.
#[must_use]
pub fn record<S, V>(value_schema: S) -> RecordSchema<S, V>
where
    S: Schema<V>,
{
    RecordSchema::new(value_schema)
}

/// Creates a new enum schema builder parsing strings into `T: FromStr`.
#[must_use]
pub fn r#enum<T>() -> EnumSchema<T>
where
    T: std::str::FromStr + Send + Sync + 'static,
{
    EnumSchema::new()
}

/// Creates a new enum schema builder parsing strings into `T: FromStr` (ergonomic alias for `r#enum`).
#[must_use]
pub fn enum_of<T>() -> EnumSchema<T>
where
    T: std::str::FromStr + Send + Sync + 'static,
{
    r#enum()
}
