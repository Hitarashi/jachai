//! Tests for ObjectSchema composition (extend, merge, pick, omit, partial)
//! and multi-field conditional rules (required_unless, required_with_all, required_without_all).

use jachai::prelude::*;
use serde_json::json;

#[test]
fn test_object_extend_and_merge() {
    let pagination = object()
        .field("page", integer().min(1))
        .field("limit", integer().between(1, 100));

    let filter = object()
        .field("search", string().optional())
        .field("sort", string().optional());

    let combined = pagination.extend(filter);

    assert!(
        combined
            .validate(&json!({
                "page": 2,
                "limit": 50,
                "search": "rust",
                "sort": "asc"
            }))
            .is_ok()
    );

    // Missing page should fail
    assert!(
        combined
            .validate(&json!({
                "limit": 50,
                "search": "rust"
            }))
            .is_err()
    );

    // Merge alias
    let s1 = object().field("a", integer());
    let s2 = object().field("b", string());
    let merged = s1.merge(s2);
    assert!(merged.validate(&json!({"a": 1, "b": "hello"})).is_ok());
    assert!(merged.validate(&json!({"a": 1})).is_err());
}

#[test]
fn test_object_pick_and_omit() {
    let user_schema = object()
        .field("id", integer())
        .field("name", string().min_len(2))
        .field("email", string().email())
        .field("created_at", string().date());

    // Pick only id and email
    let picked = user_schema.clone().pick(&["id", "email"]);
    assert!(
        picked
            .validate(&json!({
                "id": 1,
                "email": "user@example.com"
            }))
            .is_ok()
    );
    // Extra fields stripped by default strip policy
    assert!(
        picked
            .validate(&json!({
                "id": 1,
                "email": "user@example.com",
                "name": "ignored"
            }))
            .is_ok()
    );
    // Missing email should fail
    assert!(picked.validate(&json!({"id": 1})).is_err());

    // Omit id and created_at (e.g. for registration input)
    let omitted = user_schema.omit(&["id", "created_at"]);
    assert!(
        omitted
            .validate(&json!({
                "name": "Alice",
                "email": "alice@example.com"
            }))
            .is_ok()
    );
    assert!(
        omitted
            .validate(&json!({
                "name": "Alice"
            }))
            .is_err()
    );
}

#[test]
fn test_object_partial() {
    let full_schema = object()
        .field("username", string().min_len(3))
        .field("email", string().email())
        .field("age", integer().min(18));

    let patch_schema = full_schema.partial();

    // All empty payload passes in partial mode
    assert!(patch_schema.validate(&json!({})).is_ok());

    // Partial update of single field validates constraints
    assert!(
        patch_schema
            .validate(&json!({"username": "john_doe"}))
            .is_ok()
    );
    assert!(patch_schema.validate(&json!({"username": "jo"})).is_err());
    assert!(
        patch_schema
            .validate(&json!({"email": "not-an-email"}))
            .is_err()
    );
}

#[test]
fn test_required_unless_rule() {
    // reason is required unless status is "active"
    let schema = object()
        .field("status", string())
        .field("reason", string().optional())
        .required_unless("reason", "status", json!("active"));

    assert!(schema.validate(&json!({"status": "active"})).is_ok());
    assert!(
        schema
            .validate(&json!({"status": "suspended", "reason": "non-payment"}))
            .is_ok()
    );
    assert!(schema.validate(&json!({"status": "suspended"})).is_err());
}

#[test]
fn test_required_with_all_rule() {
    // postal_code is required if BOTH city and state are provided
    let schema = object()
        .field("city", string().optional())
        .field("state", string().optional())
        .field("postal_code", string().optional())
        .required_with_all("postal_code", &["city", "state"]);

    // Neither city nor state: OK
    assert!(schema.validate(&json!({})).is_ok());
    // Only city provided: OK
    assert!(schema.validate(&json!({"city": "Metropolis"})).is_ok());
    // Both provided with postal_code: OK
    assert!(
        schema
            .validate(&json!({"city": "Metropolis", "state": "NY", "postal_code": "10001"}))
            .is_ok()
    );
    // Both provided WITHOUT postal_code: Err
    assert!(
        schema
            .validate(&json!({"city": "Metropolis", "state": "NY"}))
            .is_err()
    );
}

#[test]
fn test_required_without_all_rule() {
    // phone_or_email fallback: contact_info required if NONE of [phone, email] are present
    let schema = object()
        .field("phone", string().optional())
        .field("email", string().optional())
        .field("fallback_id", string().optional())
        .required_without_all("fallback_id", &["phone", "email"]);

    // If phone is present: OK
    assert!(schema.validate(&json!({"phone": "+14155552671"})).is_ok());
    // If email is present: OK
    assert!(
        schema
            .validate(&json!({"email": "test@example.com"}))
            .is_ok()
    );
    // If neither is present, fallback_id must be provided
    assert!(schema.validate(&json!({"fallback_id": "user_123"})).is_ok());
    // If none are present: Err
    assert!(schema.validate(&json!({})).is_err());
}
