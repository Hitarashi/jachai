//! Universal declarative schema definition, validation, sanitization, and structured error reporting.
//!
//! # Overview
//!
//! `jachai` provides universal schema definition, input sanitization, data coercion,
//! validation rule evaluation, and hierarchical field-level error reporting. It operates as an
//! environment- and transport-agnostic data validation engine, capable of validating arbitrary
//! in-memory structures, serialized data formats, and multi-source request components.
//!
//! # Architecture & Features
//!
//! - **Universal & Decoupled Core**: Validates arbitrary in-memory data structures, JSON values,
//!   or structured representations with zero I/O, compiling cleanly to `wasm32-unknown-unknown`.
//! - **"Parse, Don't Validate"**: Normalizes untrusted, weakly-typed input (trimming strings,
//!   empty-string-to-null conversion, scalar coercion) and decodes directly into strictly typed Rust models.
//! - **Fail-Closed Security**: Silently strips undeclared fields by default (`strip`) to prevent
//!   mass-assignment vulnerabilities, with opt-in `.strict()` and `.passthrough()` modes.
//! - **Authoring Surfaces**:
//!   1. Fluent Schema Builders (`schema::object()`, `schema::string()`).
//!   2. Declarative Macro DSL ([`schema!`], [`request_schema!`]).
//! - **Dual-Mode Execution**: Pure schemas execute synchronously with zero runtime overhead via
//!   [`Schema::validate`], while asynchronous rules are enabled under the `async` feature flag.
//! - **Cross-Field Relationships**: Native combinators (`.same_as()`, `.required_if()`, `.after_field()`, `.before_field()`).
//! - **Ambient Metadata (`ValidationContext`)**: Typemap container allowing custom rules to inspect
//!   runtime metadata (such as current user ID or tenant ID).
//! - **RFC 9457 & Problem Details**: Built-in support for generating standardized RFC 9457 Problem Details
//!   and simple JSON error arrays.

pub mod coercion;
pub mod context;
pub mod error;
pub mod messages;
pub mod request;
pub mod rule;
pub mod schema;

#[cfg(feature = "macros")]
pub mod macros;

pub use coercion::Coercer;
pub use context::ValidationContext;
pub use error::{RuleParams, ValidationError, ValidationIssue};
pub use messages::{MessagesProvider, SimpleMessagesProvider};
pub use request::{
    RequestPartKind, RequestParts, RequestSchema, ValidatedRequest, request, request_schema,
};
#[cfg(feature = "async")]
pub use rule::{AsyncCustomRule, SharedAsyncCustomRule};
pub use rule::{
    CrossFieldRuleValidator, CustomRule, RuleContext, SharedCrossFieldRuleValidator,
    SharedCustomRule,
};
pub use schema::{
    DateFormat, DateSchema, FileMetadata, FileSchema, Schema, ToJsonSchema, UnknownFieldPolicy,
    array, boolean, date, r#enum, enum_of, file, integer, number, object, record, string, tuple,
    tuple2, tuple3, tuple4, union, union2, union3, union4,
};

/// Convenient prelude module importing essential traits, builders, and types.
pub mod prelude {
    #[cfg(feature = "macros")]
    pub use crate::schema;
    #[cfg(feature = "async")]
    pub use crate::{AsyncCustomRule, SharedAsyncCustomRule};
    pub use crate::{
        Coercer, CrossFieldRuleValidator, CustomRule, DateFormat, DateSchema, FileMetadata,
        FileSchema, MessagesProvider, RequestPartKind, RequestParts, RequestSchema, RuleContext,
        RuleParams, Schema, SharedCrossFieldRuleValidator, SimpleMessagesProvider, ToJsonSchema,
        UnknownFieldPolicy, ValidatedRequest, ValidationContext, ValidationError, ValidationIssue,
        array, boolean, date, r#enum, enum_of, file, integer, number, object, record, request,
        request_schema, string, tuple, tuple2, tuple3, tuple4, union, union2, union3, union4,
    };
}
