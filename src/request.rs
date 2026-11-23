//! Multi-source HTTP request validation and composite request schemas.

use std::sync::Arc;

#[cfg(feature = "async")]
use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    context::ValidationContext, error::ValidationError, messages::MessagesProvider, schema::Schema,
};

/// Discrete source components of an incoming HTTP request envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPartKind {
    /// Route parameters (e.g. `:id`, `:slug`).
    Params = 0,
    /// Query string parameters parsed as structured values.
    Query = 1,
    /// Request headers represented as a map.
    Headers = 2,
    /// Request cookies represented as a map.
    Cookies = 3,
    /// Request body payload (JSON, Form, or text).
    Body = 4,
}

impl RequestPartKind {
    /// Array of all request part kinds in deterministic pipeline evaluation order.
    pub const ALL: [Self; 5] = [
        Self::Params,
        Self::Query,
        Self::Headers,
        Self::Cookies,
        Self::Body,
    ];

    /// Returns the canonical path namespace prefix for this request part.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Params => "params",
            Self::Query => "query",
            Self::Headers => "headers",
            Self::Cookies => "cookies",
            Self::Body => "body",
        }
    }
}

/// Raw or partially structured parts of an incoming HTTP request.
#[derive(Debug, Clone, Default)]
pub struct RequestParts {
    /// Route parameters (e.g. `:id`, `:slug`).
    pub params: Value,
    /// Query string parameters parsed as structured values.
    pub query: Value,
    /// Request headers represented as a map.
    pub headers: Value,
    /// Request cookies represented as a map.
    pub cookies: Value,
    /// Request body payload (JSON, Form, or text).
    pub body: Value,
}

impl RequestParts {
    /// Creates a new empty `RequestParts` bundle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set the route params component.
    #[must_use]
    pub fn with_params(mut self, params: impl Into<Value>) -> Self {
        self.params = params.into();
        self
    }

    /// Builder method to set the query parameters component.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<Value>) -> Self {
        self.query = query.into();
        self
    }

    /// Builder method to set the HTTP headers component.
    #[must_use]
    pub fn with_headers(mut self, headers: impl Into<Value>) -> Self {
        self.headers = headers.into();
        self
    }

    /// Builder method to set the cookies component.
    #[must_use]
    pub fn with_cookies(mut self, cookies: impl Into<Value>) -> Self {
        self.cookies = cookies.into();
        self
    }

    /// Builder method to set the request body component.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Value>) -> Self {
        self.body = body.into();
        self
    }

    /// Retrieves a reference to the request component matching `kind`.
    #[must_use]
    pub fn get_part(&self, kind: RequestPartKind) -> &Value {
        match kind {
            RequestPartKind::Params => &self.params,
            RequestPartKind::Query => &self.query,
            RequestPartKind::Headers => &self.headers,
            RequestPartKind::Cookies => &self.cookies,
            RequestPartKind::Body => &self.body,
        }
    }
}

/// Output bundle containing validated components of a multi-source HTTP request.
#[derive(Debug, Clone, Default)]
pub struct ValidatedRequest {
    /// Validated route parameters.
    pub params: Option<Value>,
    /// Validated query string parameters.
    pub query: Option<Value>,
    /// Validated HTTP headers.
    pub headers: Option<Value>,
    /// Validated cookies.
    pub cookies: Option<Value>,
    /// Validated request body.
    pub body: Option<Value>,
}

impl ValidatedRequest {
    /// Sets a validated component by part kind.
    pub fn set_part(&mut self, kind: RequestPartKind, value: Value) {
        match kind {
            RequestPartKind::Params => self.params = Some(value),
            RequestPartKind::Query => self.query = Some(value),
            RequestPartKind::Headers => self.headers = Some(value),
            RequestPartKind::Cookies => self.cookies = Some(value),
            RequestPartKind::Body => self.body = Some(value),
        }
    }

    /// Returns a reference to the validated component matching `kind`.
    #[must_use]
    pub fn get_part(&self, kind: RequestPartKind) -> Option<&Value> {
        match kind {
            RequestPartKind::Params => self.params.as_ref(),
            RequestPartKind::Query => self.query.as_ref(),
            RequestPartKind::Headers => self.headers.as_ref(),
            RequestPartKind::Cookies => self.cookies.as_ref(),
            RequestPartKind::Body => self.body.as_ref(),
        }
    }

    /// Deserializes a validated request component into `T: DeserializeOwned`.
    pub fn decode_part<T: DeserializeOwned>(
        &self,
        kind: RequestPartKind,
    ) -> Result<T, ValidationError> {
        let val = self.get_part(kind).unwrap_or(&Value::Null);
        serde_json::from_value(val.clone())
            .map_err(|err| ValidationError::single(kind.name(), "deserialization", err.to_string()))
    }

    /// Deserializes the validated body into `T: DeserializeOwned`.
    pub fn decode_body<T: DeserializeOwned>(&self) -> Result<T, ValidationError> {
        self.decode_part(RequestPartKind::Body)
    }

    /// Deserializes the validated query parameters into `T: DeserializeOwned`.
    pub fn decode_query<T: DeserializeOwned>(&self) -> Result<T, ValidationError> {
        self.decode_part(RequestPartKind::Query)
    }

    /// Deserializes the validated route parameters into `T: DeserializeOwned`.
    pub fn decode_params<T: DeserializeOwned>(&self) -> Result<T, ValidationError> {
        self.decode_part(RequestPartKind::Params)
    }

    /// Deserializes the validated headers into `T: DeserializeOwned`.
    pub fn decode_headers<T: DeserializeOwned>(&self) -> Result<T, ValidationError> {
        self.decode_part(RequestPartKind::Headers)
    }

    /// Deserializes the validated cookies into `T: DeserializeOwned`.
    pub fn decode_cookies<T: DeserializeOwned>(&self) -> Result<T, ValidationError> {
        self.decode_part(RequestPartKind::Cookies)
    }

    /// Converts the validated request parts into a unified `serde_json::Value` object.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(ref params) = self.params {
            map.insert("params".to_string(), params.clone());
        }
        if let Some(ref query) = self.query {
            map.insert("query".to_string(), query.clone());
        }
        if let Some(ref headers) = self.headers {
            map.insert("headers".to_string(), headers.clone());
        }
        if let Some(ref cookies) = self.cookies {
            map.insert("cookies".to_string(), cookies.clone());
        }
        if let Some(ref body) = self.body {
            map.insert("body".to_string(), body.clone());
        }
        Value::Object(map)
    }
}

impl From<&Value> for RequestParts {
    fn from(val: &Value) -> Self {
        match val {
            Value::Object(map) => Self {
                params: map.get("params").cloned().unwrap_or(Value::Null),
                query: map.get("query").cloned().unwrap_or(Value::Null),
                headers: map.get("headers").cloned().unwrap_or(Value::Null),
                cookies: map.get("cookies").cloned().unwrap_or(Value::Null),
                body: map.get("body").cloned().unwrap_or(Value::Null),
            },
            _ => Self::default(),
        }
    }
}

impl From<Value> for RequestParts {
    fn from(mut val: Value) -> Self {
        match val {
            Value::Object(ref mut map) => Self {
                params: map.remove("params").unwrap_or(Value::Null),
                query: map.remove("query").unwrap_or(Value::Null),
                headers: map.remove("headers").unwrap_or(Value::Null),
                cookies: map.remove("cookies").unwrap_or(Value::Null),
                body: map.remove("body").unwrap_or(Value::Null),
            },
            _ => Self::default(),
        }
    }
}

impl From<RequestParts> for Value {
    fn from(parts: RequestParts) -> Self {
        let mut map = serde_json::Map::new();
        map.insert("params".to_string(), parts.params);
        map.insert("query".to_string(), parts.query);
        map.insert("headers".to_string(), parts.headers);
        map.insert("cookies".to_string(), parts.cookies);
        map.insert("body".to_string(), parts.body);
        Value::Object(map)
    }
}

trait ErasedPartSchema: Send + Sync {
    fn validate_part(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError>;

    fn check_part(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError>;

    #[cfg(feature = "async")]
    fn validate_async_part<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<Value, ValidationError>>;
}

struct PartEntry<S, T> {
    schema: S,
    phantom: std::marker::PhantomData<T>,
}

impl<S, T> ErasedPartSchema for PartEntry<S, T>
where
    S: Schema<T>,
    T: serde::Serialize + Send + Sync + 'static,
{
    fn validate_part(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        messages: &dyn MessagesProvider,
    ) -> Result<Value, ValidationError> {
        let mut typed = self
            .schema
            .validate_path(value, path, context, None, messages)?;
        let any_mut = &mut typed as &mut dyn std::any::Any;
        if let Some(v) = any_mut.downcast_mut::<Value>() {
            return Ok(std::mem::take(v));
        }
        if let Some(s) = any_mut.downcast_mut::<String>() {
            return Ok(Value::String(std::mem::take(s)));
        }
        serde_json::to_value(&typed)
            .map_err(|err| ValidationError::single(path, "serialization", err.to_string()))
    }

    fn check_part(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        self.schema.check_path(value, path, context, None, messages)
    }

    #[cfg(feature = "async")]
    fn validate_async_part<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<Value, ValidationError>> {
        Box::pin(async move {
            let mut typed = self
                .schema
                .validate_async_path(value, path, context, None, messages)
                .await?;
            let any_mut = &mut typed as &mut dyn std::any::Any;
            if let Some(v) = any_mut.downcast_mut::<Value>() {
                return Ok(std::mem::take(v));
            }
            if let Some(s) = any_mut.downcast_mut::<String>() {
                return Ok(Value::String(std::mem::take(s)));
            }
            serde_json::to_value(&typed)
                .map_err(|err| ValidationError::single(path, "serialization", err.to_string()))
        })
    }
}

/// Composite request schema coordinating validation across query, params, headers, cookies, and body.
#[derive(Clone, Default)]
pub struct RequestSchema {
    parts: [Option<Arc<dyn ErasedPartSchema>>; 5],
    bail_on_fail: bool,
    coercer: Option<crate::coercion::Coercer>,
}

impl RequestSchema {
    /// Creates a new empty `RequestSchema`.
    #[must_use]
    pub fn new() -> Self {
        <Self as Default>::default()
    }

    /// Attaches a schema for validating route parameters.
    #[must_use]
    pub fn params<S, T>(mut self, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        self.parts[RequestPartKind::Params as usize] = Some(Arc::new(PartEntry {
            schema,
            phantom: std::marker::PhantomData,
        }));
        self
    }

    /// Attaches a schema for validating query string parameters.
    #[must_use]
    pub fn query<S, T>(mut self, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        self.parts[RequestPartKind::Query as usize] = Some(Arc::new(PartEntry {
            schema,
            phantom: std::marker::PhantomData,
        }));
        self
    }

    /// Attaches a schema for validating HTTP request headers.
    #[must_use]
    pub fn headers<S, T>(mut self, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        self.parts[RequestPartKind::Headers as usize] = Some(Arc::new(PartEntry {
            schema,
            phantom: std::marker::PhantomData,
        }));
        self
    }

    /// Attaches a schema for validating cookies.
    #[must_use]
    pub fn cookies<S, T>(mut self, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        self.parts[RequestPartKind::Cookies as usize] = Some(Arc::new(PartEntry {
            schema,
            phantom: std::marker::PhantomData,
        }));
        self
    }

    /// Attaches a schema for validating the request body.
    #[must_use]
    pub fn body<S, T>(mut self, schema: S) -> Self
    where
        S: Schema<T>,
        T: serde::Serialize + Send + Sync + 'static,
    {
        self.parts[RequestPartKind::Body as usize] = Some(Arc::new(PartEntry {
            schema,
            phantom: std::marker::PhantomData,
        }));
        self
    }

    /// Configures short-circuiting on failure across request components.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Configures a `Coercer` for in-place pre-sanitization of request components.
    #[must_use]
    pub fn coercer(mut self, coercer: crate::coercion::Coercer) -> Self {
        self.coercer = Some(coercer);
        self
    }

    /// Enables or disables in-place pre-sanitization of request components.
    #[must_use]
    pub fn sanitize(mut self, enabled: bool) -> Self {
        if enabled {
            self.coercer = Some(crate::coercion::Coercer::default());
        } else {
            self.coercer = None;
        }
        self
    }

    /// Validates all components of an incoming request bundle in a single pass.
    pub fn validate(&self, parts: &RequestParts) -> Result<ValidatedRequest, ValidationError> {
        let context = ValidationContext::new();
        self.validate_with_context(parts, &context)
    }

    /// Validates request components with an ambient metadata context.
    pub fn validate_with_context(
        &self,
        parts: &RequestParts,
        context: &ValidationContext,
    ) -> Result<ValidatedRequest, ValidationError> {
        let messages = crate::messages::default_messages_provider();
        let mut errors = ValidationError::new();
        let mut validated = ValidatedRequest::default();

        let mut sanitized_parts;
        let effective_parts = if let Some(coercer) = &self.coercer {
            sanitized_parts = parts.clone();
            coercer.sanitize_in_place(&mut sanitized_parts.params);
            coercer.sanitize_in_place(&mut sanitized_parts.query);
            coercer.sanitize_in_place(&mut sanitized_parts.headers);
            coercer.sanitize_in_place(&mut sanitized_parts.cookies);
            coercer.sanitize_in_place(&mut sanitized_parts.body);
            &sanitized_parts
        } else {
            parts
        };

        for kind in RequestPartKind::ALL {
            if let Some(schema) = &self.parts[kind as usize] {
                let input_val = effective_parts.get_part(kind);
                match schema.validate_part(input_val, kind.name(), context, messages) {
                    Ok(val) => validated.set_part(kind, val),
                    Err(err) => {
                        errors.merge(err);
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(validated)
        } else {
            Err(errors)
        }
    }

    #[cfg(feature = "async")]
    /// Asynchronously validates all request components in a single pass.
    pub async fn validate_async(
        &self,
        parts: &RequestParts,
    ) -> Result<ValidatedRequest, ValidationError> {
        let context = ValidationContext::new();
        self.validate_async_with_context(parts, &context).await
    }

    #[cfg(feature = "async")]
    /// Asynchronously validates request components with an ambient metadata context.
    pub async fn validate_async_with_context(
        &self,
        parts: &RequestParts,
        context: &ValidationContext,
    ) -> Result<ValidatedRequest, ValidationError> {
        let messages = crate::messages::default_messages_provider();
        let mut errors = ValidationError::new();
        let mut validated = ValidatedRequest::default();

        let mut sanitized_parts;
        let effective_parts = if let Some(coercer) = &self.coercer {
            sanitized_parts = parts.clone();
            coercer.sanitize_in_place(&mut sanitized_parts.params);
            coercer.sanitize_in_place(&mut sanitized_parts.query);
            coercer.sanitize_in_place(&mut sanitized_parts.headers);
            coercer.sanitize_in_place(&mut sanitized_parts.cookies);
            coercer.sanitize_in_place(&mut sanitized_parts.body);
            &sanitized_parts
        } else {
            parts
        };

        for kind in RequestPartKind::ALL {
            if let Some(schema) = &self.parts[kind as usize] {
                let input_val = effective_parts.get_part(kind);
                match schema
                    .validate_async_part(input_val, kind.name(), context, messages)
                    .await
                {
                    Ok(val) => validated.set_part(kind, val),
                    Err(err) => {
                        errors.merge(err);
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(validated)
        } else {
            Err(errors)
        }
    }

    /// Checks conformity of all request components without allocating validated output containers.
    pub fn check_request(&self, parts: &RequestParts) -> Result<(), ValidationError> {
        let context = ValidationContext::new();
        let messages = crate::messages::default_messages_provider();
        self.check_request_with_context(parts, &context, messages)
    }

    /// Checks conformity of request components with context and message provider.
    pub fn check_request_with_context(
        &self,
        parts: &RequestParts,
        context: &ValidationContext,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        let mut errors = ValidationError::new();

        let mut sanitized_parts;
        let effective_parts = if let Some(coercer) = &self.coercer {
            sanitized_parts = parts.clone();
            coercer.sanitize_in_place(&mut sanitized_parts.params);
            coercer.sanitize_in_place(&mut sanitized_parts.query);
            coercer.sanitize_in_place(&mut sanitized_parts.headers);
            coercer.sanitize_in_place(&mut sanitized_parts.cookies);
            coercer.sanitize_in_place(&mut sanitized_parts.body);
            &sanitized_parts
        } else {
            parts
        };

        for kind in RequestPartKind::ALL {
            if let Some(schema) = &self.parts[kind as usize] {
                let input_val = effective_parts.get_part(kind);
                if let Err(err) = schema.check_part(input_val, kind.name(), context, messages) {
                    errors.merge(err);
                    if self.bail_on_fail {
                        return Err(errors);
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Schema<ValidatedRequest> for RequestSchema {
    fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        let _ = (path, parent_data);
        let parts = RequestParts::from(value);
        self.check_request_with_context(&parts, context, messages)
    }

    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<ValidatedRequest, ValidationError> {
        let _ = parent_data;
        let parts = RequestParts::from(value);
        let mut validated = ValidatedRequest::default();
        let mut errors = ValidationError::new();

        let mut sanitized_parts;
        let effective_parts = if let Some(coercer) = &self.coercer {
            sanitized_parts = parts.clone();
            coercer.sanitize_in_place(&mut sanitized_parts.params);
            coercer.sanitize_in_place(&mut sanitized_parts.query);
            coercer.sanitize_in_place(&mut sanitized_parts.headers);
            coercer.sanitize_in_place(&mut sanitized_parts.cookies);
            coercer.sanitize_in_place(&mut sanitized_parts.body);
            &sanitized_parts
        } else {
            &parts
        };

        for kind in RequestPartKind::ALL {
            if let Some(schema) = &self.parts[kind as usize] {
                let input_val = effective_parts.get_part(kind);
                let part_path = if path.is_empty() {
                    kind.name().to_string()
                } else {
                    format!("{}.{}", path, kind.name())
                };
                match schema.validate_part(input_val, &part_path, context, messages) {
                    Ok(val) => validated.set_part(kind, val),
                    Err(err) => {
                        errors.merge(err);
                        if self.bail_on_fail {
                            return Err(errors);
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(validated)
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
    ) -> BoxFuture<'a, Result<ValidatedRequest, ValidationError>> {
        Box::pin(async move {
            let _ = parent_data;
            let parts = RequestParts::from(value);
            let mut validated = ValidatedRequest::default();
            let mut errors = ValidationError::new();

            let mut sanitized_parts;
            let effective_parts = if let Some(coercer) = &self.coercer {
                sanitized_parts = parts.clone();
                coercer.sanitize_in_place(&mut sanitized_parts.params);
                coercer.sanitize_in_place(&mut sanitized_parts.query);
                coercer.sanitize_in_place(&mut sanitized_parts.headers);
                coercer.sanitize_in_place(&mut sanitized_parts.cookies);
                coercer.sanitize_in_place(&mut sanitized_parts.body);
                &sanitized_parts
            } else {
                &parts
            };

            for kind in RequestPartKind::ALL {
                if let Some(schema) = &self.parts[kind as usize] {
                    let input_val = effective_parts.get_part(kind);
                    let part_path = if path.is_empty() {
                        kind.name().to_string()
                    } else {
                        format!("{}.{}", path, kind.name())
                    };
                    match schema
                        .validate_async_part(input_val, &part_path, context, messages)
                        .await
                    {
                        Ok(val) => validated.set_part(kind, val),
                        Err(err) => {
                            errors.merge(err);
                            if self.bail_on_fail {
                                return Err(errors);
                            }
                        }
                    }
                }
            }

            if errors.is_empty() {
                Ok(validated)
            } else {
                Err(errors)
            }
        })
    }
}

/// Helper function to create a new `RequestSchema`.
#[must_use]
pub fn request() -> RequestSchema {
    RequestSchema::new()
}

/// Helper function to create a new `RequestSchema`.
#[must_use]
pub fn request_schema() -> RequestSchema {
    RequestSchema::new()
}
