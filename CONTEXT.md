# `jachai` Context

## Purpose

`jachai` provides universal declarative schema definition, validation, sanitization, and structured error reporting
for Rust. It enables developers to define strongly typed data contracts, validate nested data trees with exact field
paths, sanitize inputs in-place, and format violations into RFC 9457 compliant error payloads.

## Underlying Standards & Specifications

- **RFC 9457**: Problem Details for HTTP APIs (`application/problem+json`). `ValidationError::to_problem_details()` formats validation failures into compliant structures with an `invalid-params` extension array.
- **RFC 5322**: Internet Message Format for email address syntax validation.
- **RFC 4122 / RFC 9562**: Universally Unique IDentifiers (UUID) validation.
- **RFC 3986**: Uniform Resource Identifier (URI) syntax and scheme validation.
- **RFC 3339 / ISO 8601**: Date and time representation formats for temporal validations.

## Core Responsibilities

1. **Declarative Schema Definitions**:
    - Primitives: `string()`, `integer()`, `number()`, `boolean()`.
    - Combinators: `array()`, `object()`, `record()`, `tuple()` (`tuple2()`, `tuple3()`, `tuple4()`), `union()` (`union2()`, `union3()`, `union4()`), `enum_of()` (`r#enum()`).
    - Modifiers: `.optional()`, `.nullable()`, `.default(v)`, `.bail(bool)`, `.transform(f)`, `.refine(f)`.
2. **Input Sanitization & Transformations**:
    - `Coercer` engine: configurable in-place tree sanitization with zero allocations (`Coercer::sanitize_in_place`, `Coercer::to_bool`, `Coercer::to_number`, etc.).
    - String mutations: `.trim()`, `.without_trim()`, `.to_lowercase()`, `.to_uppercase()`, `.empty_to_null()`.
    - Object unknown-field policies: `Strip` (default), `Strict` (rejects excess keys), and `Passthrough`.
    - Type decoding: `.validate_and_decode::<T>(&value)` deserializes valid and sanitized data directly into target structs.
3. **Cross-Field Validations**:
    - Cross-field rules on objects: `.same_as()`, `.different_from()`, `.required_if()`, `.required_without()`, `.after_date()`, `.before_date()`.
4. **Multi-Source Request Validation**:
    - `RequestSchema`, `RequestParts`, and `RequestPartKind` validate separate input sources (`params`, `query`, `body`, `headers`, `cookies`) with unified dispatch and automatic path prefixing (e.g. `query.page`, `body.user.email`).
5. **Structured Error Reporting**:
    - `ValidationError` collection with `ValidationIssue` entries.
    - Developer convenience accessors: `.first_message()`, `.messages()`, `.field_errors()`.
    - Custom error messaging and localization providers via `MessagesProvider`.

## Boundaries & Non-Responsibilities

- **Protocol & Network Agnostic**: Operates purely in-memory over data structures and standard `serde_json::Value` trees. Zero network sockets, async runtime locks, or transport bindings.
- **Zero Web Framework Coupling**: Does not depend on any web application framework or HTTP server runtime.
