# jachai

Universal declarative schema definition, validation, sanitization, and structured error reporting engine for Rust.

[![Crates.io](https://img.shields.io/crates/v/jachai.svg)](https://crates.io/crates/jachai)
[![Documentation](https://docs.rs/jachai/badge.svg)](https://docs.rs/jachai)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

## Features

- **Declarative Schema Definitions**: Rich fluent schema types for primitives (`string`, `integer`, `number`, `boolean`), collections (`array`, `record`), composite records (`object`, `tuple2`, `tuple3`), unions, and enums.
- **In-Place Sanitization**: Mutate and normalize inputs during validation with `.trim()`, `.to_lowercase()`, `.to_uppercase()`, and `.empty_to_null()`.
- **Object Policies & Decoding**: Configurable handling of unknown fields (`Strip`, `Strict`, `Passthrough`) with direct strongly-typed decoding into target structs via `.validate_and_decode::<T>(&value)`.
- **Cross-Field Validations**: Define inter-field relationships on objects with `.same_as()`, `.different_from()`, `.required_if()`, `.required_without()`, `.after_date()`, and `.before_date()`.
- **Multi-Source Request Validation**: Coordinate validation across discrete input components (`params`, `query`, `body`, `headers`, `cookies`) with automatic error path prefixing via `RequestSchema`.
- **Standardized Error Reporting**: Machine-readable `ValidationIssue` collections with exact paths, rule names, parameters, and built-in RFC 9457 Problem Details formatting.
- **Convenient Developer Accessors**: Rapid issue inspection via `.first_message()`, `.messages()`, and `.field_errors()`.
- **Zero I/O Dependencies**: Pure in-memory synchronous evaluation, with optional `async` validation support when database or external rule queries are needed.
- **Zero Framework Coupling**: Designed to be integrated into any application, architecture, or environment without dependencies on web server runtimes.

## Installation

Add `jachai` to your `Cargo.toml`:

```toml
[dependencies]
jachai = "0.1.0"
```

To enable the declarative macro syntax and asynchronous validation rules (both enabled by default):

```toml
[dependencies]
jachai = { version = "0.1.0", features = ["async", "macros"] }
```

### Granular Feature Flags

`jachai` supports fine-grained modular features so consumers only compile and link what they need:

| Feature Flag | Description |
|---|---|
| `full` | Enables all schema modules, extended validation formats, macros, and async rules. |
| `schemas` | Enables all built-in schema builders (`string`, `number`, `date`, `color`, `crypto`, etc.). |
| `schema-string` | String schema builder, string format rules (email, URL, UUID, slug, semver, IP, etc.), and sanitization. |
| `schema-color` | CSS color format validation (`rgb()`, `rgba()`, `hsl()`, `hsla()`). |
| `schema-crypto` | Cryptocurrency addresses (`ethereum_address()`, `bitcoin_address()`). |
| `schema-business` | Tax and business identifiers (`eu_vat()`, `ein()`, `ssn()`, `issn()`, `imei()`). |
| `schema-identifiers` | System identifier formats (`mongodb_object_id()`, `bcp47()`, `cron()`, `ksuid()`). |
| `schema-date` | ISO 8601 / RFC 3339 dates, times, durations, and calendar checks (`weekend()`, `weekday()`). |
| `schema-file` | File upload metadata validation (MIME types, extensions, size bounds). |
| `schema-number` | Numeric range, safe integer, multiple-of, port, and geo-coordinate checks. |
| `async` | Asynchronous schema validation and async custom rule contracts. |
| `macros` | Declarative `schema! { ... }` and `request_schema! { ... }` macros. |
| `regex` | Regular expression pattern matching on string fields. |

## Quick Start

### Basic Object Validation

```rust
use jachai::prelude::*;
use serde_json::json;

let user_schema = object()
    .field("username", string().min_len(3).max_len(30).trim(true))
    .field("email", string().email().to_lowercase())
    .optional_field("age", integer().min(18).max(120));

let input = json!({
    "username": "  alice  ",
    "email": "Alice@Example.COM",
    "age": 25,
});

let validated = user_schema.validate(&input).unwrap();
assert_eq!(validated["username"], "alice");
assert_eq!(validated["email"], "alice@example.com");
```

### Strongly-Typed Decoding

```rust
use jachai::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct CreateUserDto {
    username: String,
    email: String,
    age: Option<i64>,
}

let user_schema = object()
    .field("username", string().min_len(3).trim(true))
    .field("email", string().email())
    .optional_field("age", integer().min(18));

let input = json!({
    "username": "  bob  ",
    "email": "bob@example.com",
    "age": 30,
});

let dto: CreateUserDto = user_schema.validate_and_decode(&input).unwrap();
assert_eq!(dto.username, "bob");
assert_eq!(dto.email, "bob@example.com");
assert_eq!(dto.age, Some(30));
```

### Declarative Macro Syntax

```rust
use jachai::prelude::*;
use serde_json::json;

let signup_schema = schema! {
    email: string().email().trim(true),
    password: string().min_len(8),
    age?: integer().min(18),
};

let input = json!({
    "email": "user@example.com",
    "password": "secret-password",
});

assert!(signup_schema.validate(&input).is_ok());
```

### Cross-Field Validation

```rust
use jachai::prelude::*;
use serde_json::json;

let password_schema = object()
    .field("password", string().min_len(8))
    .field("password_confirmation", string().min_len(8))
    .same_as("password_confirmation", "password");

let input = json!({
    "password": "secretpassword",
    "password_confirmation": "mismatched",
});

let err = password_schema.validate(&input).unwrap_err();
assert_eq!(err.first_issue().unwrap().rule, "same_as");
assert_eq!(
    err.first_message(),
    Some("The password confirmation and password fields must match.")
);
```

### Multi-Source Request Validation

```rust
use jachai::prelude::*;
use serde_json::json;

let req_schema = request_schema()
    .query(object().field("page", integer().min(1).default(1)))
    .body(object().field("title", string().min_len(1)));

let parts = RequestParts::new()
    .with_query(json!({ "page": 2 }))
    .with_body(json!({ "title": "My Post" }));

let validated = req_schema.validate(&parts).unwrap();
assert_eq!(validated.query.unwrap()["page"], 2);
assert_eq!(validated.body.unwrap()["title"], "My Post");
```

### RFC 9457 Problem Details

When validation fails, `ValidationError` can be serialized directly into RFC 9457 Problem Details JSON format:

```rust
use jachai::prelude::*;
use serde_json::json;

let s = object().field("email", string().email());
let err = s.validate(&json!({ "email": "invalid-email" })).unwrap_err();

let problem_details = err.to_problem_details(Some("/api/v1/users"));
assert_eq!(problem_details["status"], 422);
assert_eq!(problem_details["type"], "https://tools.ietf.org/html/rfc9457");
assert_eq!(problem_details["instance"], "/api/v1/users");
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
