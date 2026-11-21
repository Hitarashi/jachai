# `jachai` Agent Instructions

## Scope

`jachai` provides universal declarative schema definition, validation, sanitization, and structured error reporting
for Rust. It has zero web framework coupling, zero network I/O, and operates across in-memory structures, serde values,
and optional multi-source HTTP request envelopes.

## Design Rules

1. **Framework Agnostic**:
    - Never mention any web frameworks (such as AdonisJS, Laravel, Express, Axum, Actix, Rails, or Django) in documentation,
      codebase comments, identifiers, or error messages.
    - Reference only official specifications (e.g. RFC 9457 Problem Details, RFC 5322 Email, RFC 4122 / RFC 9562 UUID, RFC 3986 URI, ISO 8601 / RFC 3339 Date-Time) and Rust types.
2. **Zero Allocation in Critical Validation Paths**:
    - Avoid unnecessary allocations when checking predicates, bounds, and lengths.
    - Use efficient data structures (e.g. reference-borrowing sets for distinct element checks).
3. **Ergonomic Declarative API**:
    - Primary schema builders: `jachai::schema::*` (`string`, `integer`, `number`, `boolean`, `array`, `object`, `record`, `tuple`, `union`, `enum_of`).
    - Multi-source request schema: `jachai::request_schema()` and `jachai::RequestParts`.
    - Macro shortcuts: `jachai::schema! { ... }` and `jachai::request_schema! { ... }`.
    - Primary error container: `jachai::ValidationError` and `jachai::ValidationIssue`.
4. **Structured Error Reporting**:
    - Errors maintain exact field paths (`items[0].email`, `query.page`), error rule identifiers, and contextual parameters.
    - Full RFC 9457 Problem Details serialization support.
    - Direct developer UX accessors: `.first_message()`, `.messages()`, `.field_errors()`.
5. **Code Style and Lints**:
    - Follow Google's Rust Style Guide.
    - Do not suppress Clippy warnings inline (`#[allow(...)]` or `#[expect(...)]`). Resolve the warning or restructure the code instead.
    - Avoid using leading underscores (`_`) on identifiers, variables, parameters, or struct fields.
    - Code formatting must be verified via `cargo +nightly fmt --check`.
