# Domain Glossary (`jachai`)

This glossary establishes the canonical ubiquitous language for `jachai`. These terms must be used consistently across code, tests, documentation, and architectural designs.

---

### Core Domain Terms

#### Schema
A declarative specification of the shape, type invariants, sanitization rules, and decoding target for an untrusted input node. A schema is a **deep module** presenting a minimal `.validate()` / `.validate_and_decode()` interface while encapsulating sanitization, validation predicates, and error handling.

#### Validation Issue
An atomic record of a validation failure. Contains the canonical hierarchical field path (`field`), rule identifier (`rule`), localized human-readable message (`message`), and contextual parameters (`params`).

#### Validation Error
A composite error container gathering one or more `ValidationIssue` items. Provides direct developer UX accessors (`.first_message()`, `.messages()`, `.field_errors()`) and serializes directly to RFC 9457 Problem Details.

#### Rule Context
Ambient execution context supplied to validation predicates. Exposes the canonical path of the field under validation, the ambient typemap context, parent object data, sibling values, and the active `MessagesProvider`.

#### Rule Parameters
A key-value map of contextual placeholders (`:min`, `:max`, `:field`, `:expected`, `:other`) populated when a validation rule fails, consumed by message providers for interpolation.

#### Validation Context
A type-safe ambient container (typemap) that carries environmental metadata (e.g. authenticated user ID, tenant ID, client IP) across schema validation passes without coupling schemas to specific runtime protocols or frameworks.

#### Messages Provider
An extensible service interface that resolves and formats human-readable error messages based on field paths, rule identifiers, rule parameters, and localized templates.

#### Sanitization
In-place transformation and normalization applied to input data before or during validation (e.g. whitespace trimming, empty string to null conversion, casing normalization).

#### Coercion
Weak-to-strong scalar conversion that parses strings or numbers from loosely typed transport formats (such as query strings or multipart forms) into expected typed values.

#### Unknown Field Policy
The fail-closed security policy on an object schema (`Strip`, `Strict`, `Passthrough`) determining whether undeclared properties in incoming payloads are dropped, rejected with an error, or retained.

#### Cross-Field Rule
An object-level validation constraint defining relationships between multiple sibling fields (e.g. `same_as`, `different_from`, `required_if`, `required_without`, `after_field`, `before_field`).

#### Request Parts
A multi-source HTTP request envelope partitioning input data into five standard components: `params`, `query`, `headers`, `cookies`, and `body`.

#### Request Schema
A composite schema that coordinates validation across multiple request parts, prefixing field paths with their component namespace (`query.page`, `body.user.email`).
