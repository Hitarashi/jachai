//! Contract and generator for producing standard Draft 2020-12 and OpenAPI 3.1 JSON Schema representations.

use serde_json::Value;

use crate::schema::{
    Schema,
    array::ArraySchema,
    boolean::BooleanSchema,
    date::DateSchema,
    file::FileSchema,
    number::{IntegerSchema, NumberSchema},
    object::ObjectSchema,
    optional::{DefaultSchema, NullableSchema, OptionalSchema},
    string::StringSchema,
};

/// Conversion trait allowing declarative schemas to export standard JSON Schema objects.
pub trait ToJsonSchema {
    /// Generates a standard JSON Schema (`serde_json::Value`) representing this schema's constraints.
    fn to_json_schema(&self) -> Value;
}

impl ToJsonSchema for StringSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for NumberSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for IntegerSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for BooleanSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl<S: Schema<T>, T: Send + Sync + 'static> ToJsonSchema for ArraySchema<S, T> {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for ObjectSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for DateSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl ToJsonSchema for FileSchema {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl<S: Schema<T>, T: Send + Sync + 'static> ToJsonSchema for OptionalSchema<S, T> {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl<S: Schema<T>, T: Send + Sync + 'static> ToJsonSchema for NullableSchema<S, T> {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}

impl<S: Schema<T>, T: Clone + Send + Sync + 'static> ToJsonSchema for DefaultSchema<S, T> {
    fn to_json_schema(&self) -> Value {
        self.to_json_schema_value()
    }
}
