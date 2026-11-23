//! Specifications for ISO 8601 / RFC 3339 date-time validation,
//! algorithmic format rules (credit card Luhn, ULID, HexColor, SemVer), and conditional rules.

use jachai::prelude::*;
use serde_json::json;

#[test]
fn test_rfc3339_date_validation() {
    let s_date = string().date();
    assert!(s_date.validate(&json!("2026-11-22")).is_ok());
    assert!(s_date.validate(&json!("2024-02-29")).is_ok()); // Leap year
    assert!(s_date.validate(&json!("2023-02-29")).is_err()); // Non-leap year
    assert!(s_date.validate(&json!("2026-13-01")).is_err()); // Invalid month
    assert!(s_date.validate(&json!("2026-04-31")).is_err()); // April has 30 days
    assert!(s_date.validate(&json!("invalid-date")).is_err());
}

#[test]
fn test_rfc3339_time_validation() {
    let s_time = string().time();
    assert!(s_time.validate(&json!("14:30:00")).is_ok());
    assert!(s_time.validate(&json!("14:30:00.123456")).is_ok());
    assert!(s_time.validate(&json!("14:30:00Z")).is_ok());
    assert!(s_time.validate(&json!("14:30:00+05:30")).is_ok());
    assert!(s_time.validate(&json!("25:00:00")).is_err()); // Invalid hour
    assert!(s_time.validate(&json!("14:60:00")).is_err()); // Invalid minute
    assert!(s_time.validate(&json!("invalid-time")).is_err());
}

#[test]
fn test_rfc3339_datetime_validation() {
    let s_datetime = string().datetime();
    assert!(s_datetime.validate(&json!("2026-11-22T14:30:00Z")).is_ok());
    assert!(
        s_datetime
            .validate(&json!("2026-11-22T14:30:00.999+05:30"))
            .is_ok()
    );
    assert!(s_datetime.validate(&json!("2026-11-22 14:30:00Z")).is_ok()); // Space separator tolerated per RFC 3339
    assert!(s_datetime.validate(&json!("2026-11-22T25:00:00Z")).is_err());
    assert!(s_datetime.validate(&json!("not-a-datetime")).is_err());
}

#[test]
fn test_credit_card_luhn_validation() {
    let s_card = string().credit_card();
    // Valid standard test card numbers passing Luhn checksum (ISO/IEC 7812)
    assert!(s_card.validate(&json!("49927398716")).is_ok());
    assert!(s_card.validate(&json!("4992-7398-716")).is_ok()); // Formatting dashes allowed
    assert!(s_card.validate(&json!("4992 7398 716")).is_ok()); // Formatting spaces allowed
    assert!(s_card.validate(&json!("49927398717")).is_err()); // Invalid check digit
    assert!(s_card.validate(&json!("1234")).is_err()); // Too short
    assert!(s_card.validate(&json!("abc49927398716")).is_err()); // Non-digits
}

#[test]
fn test_ulid_validation() {
    let s_ulid = string().ulid();
    assert!(
        s_ulid
            .validate(&json!("01ARZ3NDEKTSV4RRFFQ69G5FAV"))
            .is_ok()
    );
    assert!(
        s_ulid
            .validate(&json!("01arz3ndektsv4rrffq69g5fav"))
            .is_ok()
    ); // Case-insensitive Crockford Base32
    assert!(
        s_ulid
            .validate(&json!("01ARZ3NDEKTSV4RRFFQ69G5FA"))
            .is_err()
    ); // 25 chars (too short)
    assert!(
        s_ulid
            .validate(&json!("01ARZ3NDEKTSV4RRFFQ69G5FAVU"))
            .is_err()
    ); // 27 chars (too long)
    assert!(
        s_ulid
            .validate(&json!("01ARZ3NDEKTSV4RRFFQ69G5FAI"))
            .is_err()
    ); // 'I' is not valid Crockford Base32
}

#[test]
fn test_hex_color_validation() {
    let s_color = string().hex_color();
    assert!(s_color.validate(&json!("#fff")).is_ok());
    assert!(s_color.validate(&json!("#FFF")).is_ok());
    assert!(s_color.validate(&json!("#ffffff")).is_ok());
    assert!(s_color.validate(&json!("#ffffffff")).is_ok()); // With alpha
    assert!(s_color.validate(&json!("ffffff")).is_ok()); // Optional '#' prefix
    assert!(s_color.validate(&json!("#ffff")).is_ok()); // 4-digit hex with alpha
    assert!(s_color.validate(&json!("#ffffg")).is_err()); // Invalid hex character
    assert!(s_color.validate(&json!("#ff")).is_err()); // Invalid length
}

#[test]
fn test_slug_validation() {
    let s_slug = string().slug();
    assert!(s_slug.validate(&json!("hello-world")).is_ok());
    assert!(s_slug.validate(&json!("user-profile-2026")).is_ok());
    assert!(s_slug.validate(&json!("single")).is_ok());
    assert!(s_slug.validate(&json!("hello--world")).is_err()); // Consecutive hyphens
    assert!(s_slug.validate(&json!("-hello")).is_err()); // Leading hyphen
    assert!(s_slug.validate(&json!("hello-")).is_err()); // Trailing hyphen
    assert!(s_slug.validate(&json!("Hello-World")).is_err()); // Uppercase
}

#[test]
fn test_semver_validation() {
    let s_semver = string().semver();
    assert!(s_semver.validate(&json!("1.0.0")).is_ok());
    assert!(s_semver.validate(&json!("0.1.0-alpha.1")).is_ok());
    assert!(s_semver.validate(&json!("2.1.3+build.42")).is_ok());
    assert!(s_semver.validate(&json!("1.0.0-beta.2+20260101")).is_ok());
    assert!(s_semver.validate(&json!("1.0")).is_err()); // Missing patch
    assert!(s_semver.validate(&json!("v1.0.0")).is_err()); // Leading 'v' not strict SemVer
    assert!(s_semver.validate(&json!("1.0.01")).is_err()); // Leading zero in version component
}

#[test]
fn test_base64_validation() {
    let s_b64 = string().base64();
    assert!(s_b64.validate(&json!("SGVsbG8gV29ybGQ=")).is_ok());
    assert!(s_b64.validate(&json!("")).is_ok()); // Empty is valid Base64
    assert!(s_b64.validate(&json!("SGVsbG8")).is_err()); // Invalid length padding
    assert!(s_b64.validate(&json!("invalid!base64")).is_err());
}

#[test]
fn test_json_string_validation() {
    let s_json = string().json_string();
    assert!(s_json.validate(&json!("{\"key\":\"val\"}")).is_ok());
    assert!(s_json.validate(&json!("[1, 2, 3]")).is_ok());
    assert!(s_json.validate(&json!("\"hello\"")).is_ok());
    assert!(s_json.validate(&json!("true")).is_ok());
    assert!(s_json.validate(&json!("not valid json")).is_err());
}

#[test]
fn test_in_list_and_not_in_list() {
    let s_in = string().in_list(&["admin", "moderator", "member"]);
    assert!(s_in.validate(&json!("admin")).is_ok());
    assert!(s_in.validate(&json!("guest")).is_err());

    let s_not_in = string().not_in_list(&["root", "admin"]);
    assert!(s_not_in.validate(&json!("john")).is_ok());
    assert!(s_not_in.validate(&json!("root")).is_err());
}

#[test]
fn test_number_between_port_decimal_places() {
    let n_between = number().between(10.0, 20.0);
    assert!(n_between.validate(&json!(10.0)).is_ok());
    assert!(n_between.validate(&json!(15.5)).is_ok());
    assert!(n_between.validate(&json!(20.0)).is_ok());
    assert!(n_between.validate(&json!(9.9)).is_err());
    assert!(n_between.validate(&json!(20.1)).is_err());

    let i_port = integer().port();
    assert!(i_port.validate(&json!(80)).is_ok());
    assert!(i_port.validate(&json!(443)).is_ok());
    assert!(i_port.validate(&json!(65535)).is_ok());
    assert!(i_port.validate(&json!(0)).is_err());
    assert!(i_port.validate(&json!(65536)).is_err());

    let n_decimals = number().decimal_places(2);
    assert!(n_decimals.validate(&json!(12.34)).is_ok());
    assert!(n_decimals.validate(&json!(12.3)).is_ok());
    assert!(n_decimals.validate(&json!(12)).is_ok());
    assert!(n_decimals.validate(&json!(12.345)).is_err());
}

#[test]
fn test_object_conditional_and_mutual_exclusion_rules() {
    // required_with: confirm_password required if password is provided
    let schema_rw = object()
        .field("password", string().optional())
        .field("confirm_password", string().optional())
        .required_with("confirm_password", "password");

    assert!(schema_rw.validate(&json!({})).is_ok());
    assert!(
        schema_rw
            .validate(&json!({"password": "secret", "confirm_password": "secret"}))
            .is_ok()
    );
    assert!(schema_rw.validate(&json!({"password": "secret"})).is_err());

    // prohibited_if: tax_exempt_reason prohibited if tax_exempt == false
    let schema_pi = object()
        .field("tax_exempt", boolean())
        .field("tax_exempt_reason", string().optional())
        .prohibited_if("tax_exempt_reason", "tax_exempt", json!(false));

    assert!(
        schema_pi
            .validate(&json!({"tax_exempt": true, "tax_exempt_reason": "Non-profit organization"}))
            .is_ok()
    );
    assert!(schema_pi.validate(&json!({"tax_exempt": false})).is_ok());
    assert!(
        schema_pi
            .validate(&json!({"tax_exempt": false, "tax_exempt_reason": "Not allowed"}))
            .is_err()
    );

    // prohibited_with: password and sso_token are mutually exclusive
    let schema_pw = object()
        .field("password", string().optional())
        .field("sso_token", string().optional())
        .prohibited_with("password", "sso_token");

    assert!(schema_pw.validate(&json!({"password": "secret"})).is_ok());
    assert!(
        schema_pw
            .validate(&json!({"sso_token": "token123"}))
            .is_ok()
    );
    assert!(schema_pw.validate(&json!({})).is_ok());
    assert!(
        schema_pw
            .validate(&json!({"password": "secret", "sso_token": "token123"}))
            .is_err()
    );
}

#[test]
fn test_date_aware_cross_field_comparisons() {
    let schema = object()
        .field("start_date", string().date())
        .field("end_date", string().date())
        .after_date_field("end_date", "start_date");

    assert!(
        schema
            .validate(&json!({"start_date": "2026-01-01", "end_date": "2026-01-10"}))
            .is_ok()
    );
    assert!(
        schema
            .validate(&json!({"start_date": "2026-01-10", "end_date": "2026-01-01"}))
            .is_err()
    );
    assert!(
        schema
            .validate(&json!({"start_date": "2026-01-01", "end_date": "2026-01-01"}))
            .is_err()
    );
}

#[test]
fn test_messages_provider_field_label() {
    let mut provider = SimpleMessagesProvider::new();
    provider.set_field_label("company.tax_id", "Tax Identification Number");

    let formatted = provider.format_message("company.tax_id", "required", &RuleParams::new(), None);
    assert_eq!(
        formatted,
        "The Tax Identification Number field is required."
    );
}
