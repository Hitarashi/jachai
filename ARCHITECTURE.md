# `jachai` Architecture Guide

## 1. Architectural Philosophy

`jachai` is an environment-agnostic, zero-I/O declarative schema validation, sanitization, and structured error reporting engine for Rust.

### Core Tenets

1. **Strict Framework Agnosticism**:
   `jachai` contains zero web framework dependencies, identifiers, or concepts. It operates purely across in-memory structures and `serde_json::Value` trees.
2. **"Parse, Don't Validate"**:
   Rather than performing boolean checks on loosely typed inputs, `jachai` sanitizes, normalizes, validates, and decodes directly into strongly typed Rust structs in a single pass via `.validate_and_decode::<T>(&payload)`.
3. **Fail-Closed Security**:
   By default, all objects enforce `UnknownFieldPolicy::Strip` to eliminate mass-assignment vulnerabilities. Developers can opt into `Strict` (reject unknown keys) or `Passthrough` (retain undeclared keys).
4. **Zero-Allocation Critical Paths**:
   Data normalization (such as whitespace trimming and empty-string-to-null conversion) operates in-place (`&mut Value`) through the `Coercer` engine. Set lookups for distinct item verification avoid allocating owned clones.
5. **Exact Path Error Propagation & RFC 9457**:
   Every failure records its precise hierarchical path (`items[0].email`, `query.page`). Errors serialize to RFC 9457 Problem Details or JSON API errors with zero loss of contextual metadata.

---

## 2. Deep Module Decomposition

Following deep module design principles, `jachai` encapsulates complexity behind simple, expressive interfaces:

```
                            ┌────────────────────────┐
                            │    Schema<T> Trait     │
                            └───────────┬────────────┘
                                        │
        ┌───────────────┬───────────────┼───────────────┬───────────────┐
        │               │               │               │               │
  ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼─────┐
  │ Primitives│   │  Object   │   │Combinators│   │  Request  │   │  Coercion │
  │  String,  │   │  Schema   │   │  Tuple2..4│   │  Schema   │   │  Coercer  │
  │  Integer, │   │Cross-Field│   │  Union2..4│   │  Unified  │   │ In-Place  │
  │  Number,  │   │ Policies  │   │  Record   │   │ Part Loop │   │Sanitization
  │  Boolean  │   │           │   │  Enum_of  │   │Prefix Pths│   │           │
  └───────────┘   └───────────┘   └───────────┘   └───────────┘   └───────────┘
```

### 2.1 Multi-Source Request Engine (`src/request.rs`)

Multi-source envelopes (`params`, `query`, `headers`, `cookies`, `body`) are managed via:
- `RequestPartKind`: An enumeration (`Params`, `Query`, `Headers`, `Cookies`, `Body`) defining display names, namespaced path prefixes, and canonical lookup keys.
- Unified Dispatch: A single loop iterates over `RequestPartKind::ALL` for both synchronous and asynchronous validation, eliminating duplicated dispatch boilerplate and ensuring identical prefixing logic.
- `Schema<ValidatedRequest>` Integration: `RequestSchema` implements the universal `Schema` trait directly, accepting structured request values or `RequestParts` bundles.
- Zero-Copy Downcasting: Part evaluation downcasts existing JSON `Value` instances directly to eliminate redundant intermediate `serde` serialization round-trips.
- `ValidatedRequest`: Provides strongly typed deserialization via `.decode_part::<T>(kind)` and `.decode_body::<T>()`, as well as unified JSON export via `.to_value()`.

### 2.2 Cross-Field Validation Engine (`src/schema/object.rs`, `src/rule.rs`)

Cross-field dependencies (`.same_as()`, `.different_from()`, `.required_if()`, `.required_without()`, `.after_date()`, `.before_date()`) are encapsulated in:
- `CrossFieldRuleValidator` Trait Seam: Open contract defining `.evaluate()`, `.references_only()`, and `.references_any()`, enabling caller-defined custom multi-field rules (`.cross_rule()`) and closure predicates (`.cross_refine()`).
- `CompositeMessagesProvider`: Wraps schema-level inline messages and ambient caller-provided providers, ensuring custom localization is preserved throughout the validation tree.
- `RuleContext::create_issue_at()`: Emits validation issues targeted at arbitrary sibling paths.

### 2.3 Sanitization & Coercion Engine (`src/coercion.rs`, `src/schema/object.rs`, `src/request.rs`)

The `Coercer` struct provides configurable, in-place sanitization:
- In-place mutation walks JSON trees recursively without cloning subtrees.
- Ingress Pre-Sanitization: Both `ObjectSchema` and `RequestSchema` integrate `.coercer()` and `.sanitize(true)` directly at their entry seams, normalizing untrusted payloads in-place before leaf field rule dispatch.
- Scalar coercion helpers (`to_bool`, `to_number`, `to_i64`, `to_u64`, `to_f64`, `to_string`, `to_array`) support truthy/falsy strings, numeric strings, and scalar wrapping.

### 2.4 Generalized Combinators (`src/schema/tuple.rs`, `src/schema/union.rs`)

- Native Rust Tuple Schemas: Standard Rust tuples `(S0, S1)`, `(S0, S1, S2)`, up to 6+ elements implement `Schema` directly without requiring specialized wrapper types.
- Macro-Driven Tuple Schemas: `Tuple2Schema`, `Tuple3Schema`, `Tuple4Schema` generated via a consolidated macro kernel that eliminates hundreds of lines of duplicate error indexing and bail handling.
- Macro-Driven Union Schemas: Alternative schema matching for 2, 3, and 4 variants (`Union2Schema`, `Union3Schema`, `Union4Schema`) generated via a unified combinator macro with automatic issue merging and inline error overrides.
- Enum Schemas: String-to-type parsing for types implementing `std::str::FromStr`, accessible via `r#enum::<T>()` or the ergonomic `enum_of::<T>()`.

---

## 3. Error Reporting Model (`src/error.rs`)

Errors are captured within `ValidationError`, containing zero or more `ValidationIssue` structs:

- `ValidationIssue`: Stores `field` (canonical path), `rule` (machine identifier), `message` (formatted text), and `params` (interpolated placeholders).
- Problem Details: `ValidationError::to_problem_details(instance)` outputs RFC 9457 JSON with `invalid-params`.
- Developer Ergonomics: Direct accessors `.first_message()`, `.messages()`, and `.field_errors()` (grouping messages by field name).
