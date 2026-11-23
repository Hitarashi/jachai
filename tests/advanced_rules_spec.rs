//! Tests for advanced format rules (E.164, JWT, MAC address, MIME type, hashes, control chars),
//! numeric/array guardrails, FileSchema, and JSON Schema export.

use jachai::{ToJsonSchema, prelude::*};
use serde_json::json;

#[test]
fn test_e164_phone_number() {
    let s = string().e164();
    assert!(s.validate(&json!("+14155552671")).is_ok());
    assert!(s.validate(&json!("+442071838750")).is_ok());
    assert!(s.validate(&json!("+8801711000000")).is_ok());
    assert!(s.validate(&json!("14155552671")).is_err()); // Missing leading '+'
    assert!(s.validate(&json!("+0123456")).is_err()); // Country code cannot start with 0
    assert!(s.validate(&json!("+14155552671999999")).is_err()); // Exceeds 15 digits
    assert!(s.validate(&json!("+1-415-555")).is_err()); // No formatting hyphens allowed in raw E.164
}

#[test]
fn test_jwt_validation() {
    let s = string().jwt();
    // Valid standard 3-part Base64URL JWT
    let valid_jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    assert!(s.validate(&json!(valid_jwt)).is_ok());

    // Unsigned token (empty signature with dot)
    let unsigned_jwt = "eyJhbGciOiJub25lIn0.eyJzdWIiOiIxMjM0NTY3ODkwIn0.";
    assert!(s.validate(&json!(unsigned_jwt)).is_ok());

    assert!(s.validate(&json!("only-two.parts")).is_err());
    assert!(s.validate(&json!("four.parts.here.extra")).is_err());
    assert!(s.validate(&json!("invalid!base64url.part2.part3")).is_err());
}

#[test]
fn test_mac_address_validation() {
    let s = string().mac_address();
    assert!(s.validate(&json!("00:1A:2B:3C:4D:5E")).is_ok());
    assert!(s.validate(&json!("00-1a-2b-3c-4d-5e")).is_ok());
    assert!(s.validate(&json!("001a.2b3c.4d5e")).is_ok()); // Cisco format
    assert!(s.validate(&json!("00:1A:2B:3C:4D:5E:6F:70")).is_ok()); // EUI-64
    assert!(s.validate(&json!("00:1A:2B:3C:4D")).is_err()); // Too short
    assert!(s.validate(&json!("00:1G:2B:3C:4D:5E")).is_err()); // Invalid hex 'G'
}

#[test]
fn test_mime_type_validation() {
    let s = string().mime_type();
    assert!(s.validate(&json!("application/json")).is_ok());
    assert!(s.validate(&json!("text/html; charset=utf-8")).is_ok());
    assert!(s.validate(&json!("image/svg+xml")).is_ok());
    assert!(
        s.validate(&json!("multipart/form-data; boundary=something"))
            .is_ok()
    );
    assert!(s.validate(&json!("not-a-mime")).is_err());
    assert!(s.validate(&json!("application/")).is_err());
    assert!(s.validate(&json!("/json")).is_err());
}

#[test]
fn test_base64_url_validation() {
    let s = string().base64_url();
    assert!(s.validate(&json!("SGVsbG8tV29ybGRf")).is_ok());
    assert!(s.validate(&json!("")).is_ok());
    assert!(s.validate(&json!("contains+plus")).is_err()); // Standard Base64 '+' rejected in Base64URL
    assert!(s.validate(&json!("contains/slash")).is_err()); // Standard Base64 '/' rejected in Base64URL
}

#[test]
fn test_cryptographic_hashes() {
    let s_sha256 = string().sha256();
    assert!(
        s_sha256
            .validate(&json!(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            ))
            .is_ok()
    );
    assert!(s_sha256.validate(&json!("tooshort")).is_err());
    assert!(
        s_sha256
            .validate(&json!(
                "g3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            ))
            .is_err()
    );

    let s_md5 = string().md5();
    assert!(
        s_md5
            .validate(&json!("d41d8cd98f00b204e9800998ecf8427e"))
            .is_ok()
    );
    assert!(
        s_md5
            .validate(&json!("d41d8cd98f00b204e9800998ecf8427"))
            .is_err()
    ); // 31 chars

    let s_sha512 = string().sha512();
    let valid_sha512 = "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e";
    assert!(s_sha512.validate(&json!(valid_sha512)).is_ok());
}

#[test]
fn test_non_control_character() {
    let s = string().non_control();
    assert!(s.validate(&json!("Hello World! 123")).is_ok());
    assert!(s.validate(&json!("Line with null byte \0")).is_err());
    assert!(s.validate(&json!("Line with bell \x07")).is_err());
}

#[test]
fn test_numeric_guardrails() {
    let n_finite = number().finite();
    assert!(n_finite.validate(&json!(42.5)).is_ok());

    let n_non_neg = number().non_negative();
    assert!(n_non_neg.validate(&json!(0.0)).is_ok());
    assert!(n_non_neg.validate(&json!(10.0)).is_ok());
    assert!(n_non_neg.validate(&json!(-0.01)).is_err());

    let n_non_pos = number().non_positive();
    assert!(n_non_pos.validate(&json!(0.0)).is_ok());
    assert!(n_non_pos.validate(&json!(-10.0)).is_ok());
    assert!(n_non_pos.validate(&json!(0.01)).is_err());

    let i_non_neg = integer().non_negative();
    assert!(i_non_neg.validate(&json!(0)).is_ok());
    assert!(i_non_neg.validate(&json!(5)).is_ok());
    assert!(i_non_neg.validate(&json!(-1)).is_err());

    let i_non_pos = integer().non_positive();
    assert!(i_non_pos.validate(&json!(0)).is_ok());
    assert!(i_non_pos.validate(&json!(-5)).is_ok());
    assert!(i_non_pos.validate(&json!(1)).is_err());
}

#[test]
fn test_array_guardrails() {
    let arr_between = array(string()).between(2, 4);
    assert!(arr_between.validate(&json!(["a", "b"])).is_ok());
    assert!(arr_between.validate(&json!(["a", "b", "c", "d"])).is_ok());
    assert!(arr_between.validate(&json!(["a"])).is_err());
    assert!(
        arr_between
            .validate(&json!(["a", "b", "c", "d", "e"]))
            .is_err()
    );

    let arr_subset = array(string()).subset_of(&["red", "green", "blue"]);
    assert!(arr_subset.validate(&json!(["red", "blue"])).is_ok());
    assert!(arr_subset.validate(&json!(["green"])).is_ok());
    assert!(arr_subset.validate(&json!(["red", "yellow"])).is_err());
}

#[test]
fn test_file_metadata_schema() {
    let file_validator = file()
        .min_size(1024)
        .max_size(10 * 1024 * 1024)
        .allowed_extensions(&["png", "jpg", "jpeg"])
        .allowed_mime_types(&["image/png", "image/jpeg"]);

    // Valid file payload
    let valid_payload = json!({
        "client_filename": "avatar.png",
        "size": 2048,
        "mime_type": "image/png"
    });
    let metadata = file_validator.validate(&valid_payload).expect("Valid file");
    assert_eq!(metadata.client_filename, "avatar.png");
    assert_eq!(metadata.size, 2048);
    assert_eq!(metadata.mime_type, "image/png");
    assert_eq!(metadata.extension, Some("png".to_string()));

    // Too small
    assert!(
        file_validator
            .validate(&json!({
                "client_filename": "small.png",
                "size": 500,
                "mime_type": "image/png"
            }))
            .is_err()
    );

    // Disallowed extension
    assert!(
        file_validator
            .validate(&json!({
                "client_filename": "script.sh",
                "size": 2048,
                "mime_type": "image/png"
            }))
            .is_err()
    );

    // Disallowed MIME type
    assert!(
        file_validator
            .validate(&json!({
                "client_filename": "avatar.png",
                "size": 2048,
                "mime_type": "application/x-executable"
            }))
            .is_err()
    );
}

#[test]
fn test_to_json_schema_export() {
    let user_schema = object()
        .field("id", integer().min(1))
        .field("name", string().min_len(2).max_len(50))
        .field("email", string().email())
        .field("role", string().optional());

    let schema_json = user_schema.to_json_schema();
    assert_eq!(schema_json["type"], "object");
    assert_eq!(schema_json["properties"]["id"]["type"], "integer");
    assert_eq!(schema_json["properties"]["id"]["minimum"], 1);
    assert_eq!(schema_json["properties"]["name"]["type"], "string");
    assert_eq!(schema_json["properties"]["name"]["minLength"], 2);
    assert_eq!(schema_json["properties"]["name"]["maxLength"], 50);
    assert_eq!(schema_json["properties"]["email"]["type"], "string");
    assert_eq!(schema_json["properties"]["email"]["format"], "email");
    assert_eq!(schema_json["required"], json!(["id", "name", "email"]));

    let arr_schema = array(string().uuid()).min_items(1);
    let arr_json = arr_schema.to_json_schema();
    assert_eq!(arr_json["type"], "array");
    assert_eq!(arr_json["items"]["type"], "string");
    assert_eq!(arr_json["items"]["format"], "uuid");
    assert_eq!(arr_json["minItems"], 1);
}

#[tokio::test]
async fn test_file_schema_comprehensive() {
    let f = file()
        .size_between(100, 1000)
        .allowed_extensions(&[".png", ".jpg"])
        .allowed_mime_types(&["image/png"])
        .with_message("File validation failed")
        .bail(true);

    // Default builder
    let default_file = <FileSchema as Default>::default();
    assert!(
        default_file
            .validate(&json!({"client_filename": "a.txt", "size": 10, "mime_type": "text/plain"}))
            .is_ok()
    );

    // Fallback fields: filename, detected_mime_type, explicit extension
    let fallback_payload = json!({
        "filename": "custom_name",
        "size": 500,
        "detected_mime_type": "image/png",
        "extension": "png"
    });
    let metadata = f.validate(&fallback_payload).expect("Valid file");
    assert_eq!(metadata.client_filename, "custom_name");
    assert_eq!(metadata.size, 500);
    assert_eq!(metadata.mime_type, "image/png");
    assert_eq!(metadata.extension, Some("png".to_string()));

    // client_content_type fallback
    let content_type_payload = json!({
        "client_filename": "avatar.png",
        "size": 500,
        "client_content_type": "image/png"
    });
    assert!(f.validate(&content_type_payload).is_ok());

    // Async validation
    let async_res = f.validate_async(&fallback_payload).await;
    assert!(async_res.is_ok());

    // Errors: Null, non-object, too small, too large, bad extension, bad mime
    assert!(f.validate(&json!(null)).is_err());
    assert!(f.validate(&json!("not-a-file")).is_err());
    assert!(
        f.validate(&json!({"client_filename": "a.png", "size": 50, "mime_type": "image/png"}))
            .is_err()
    );
    assert!(
        f.validate(&json!({"client_filename": "a.png", "size": 2000, "mime_type": "image/png"}))
            .is_err()
    );
    assert!(
        f.validate(&json!({"client_filename": "a.txt", "size": 500, "mime_type": "image/png"}))
            .is_err()
    );
    assert!(
        f.validate(&json!({"client_filename": "a.png", "size": 500, "mime_type": "text/plain"}))
            .is_err()
    );
    assert!(
        f.validate(
            &json!({"client_filename": "no_extension", "size": 500, "mime_type": "image/png"})
        )
        .is_err()
    );

    // Export JSON schema
    let file_json = f.to_json_schema();
    assert_eq!(file_json["type"], "object");
    assert!(file_json["properties"]["client_filename"].is_object());
}

#[test]
fn test_to_json_schema_all_variants() {
    let s_json = string().to_json_schema();
    assert_eq!(s_json["type"], "string");

    let num_schema = number().min(1.5).max(10.5).positive().multiple_of(0.5);
    let num_json = num_schema.to_json_schema();
    assert_eq!(num_json["type"], "number");
    assert_eq!(num_json["minimum"], 1.5);
    assert_eq!(num_json["maximum"], 10.5);
    assert_eq!(num_json["exclusiveMinimum"], 0);
    assert_eq!(num_json["multipleOf"], 0.5);

    let num_neg = number().negative().non_positive().range(0.0, 10.0);
    let num_neg_json = num_neg.to_json_schema();
    assert_eq!(num_neg_json["exclusiveMaximum"], 0);

    let int_json = integer().to_json_schema();
    assert_eq!(int_json["type"], "integer");

    let bool_json = boolean().to_json_schema();
    assert_eq!(bool_json["type"], "boolean");

    let date_dt = date().to_json_schema();
    assert_eq!(date_dt["format"], "date-time");

    let date_d = date().date_only().to_json_schema();
    assert_eq!(date_d["format"], "date");

    let date_t = date().time_only().to_json_schema();
    assert_eq!(date_t["format"], "time");

    let opt_json = string().optional().to_json_schema();
    assert_eq!(opt_json["type"], "string");

    let null_json = string().nullable().to_json_schema();
    assert_eq!(null_json["type"], json!(["string", "null"]));

    let def_json = string()
        .default_value("fallback".to_string())
        .to_json_schema();
    assert_eq!(def_json["type"], "string");

    let arr_opts = array(string())
        .max_items(10)
        .exact_items(5)
        .distinct()
        .to_json_schema();
    assert_eq!(arr_opts["maxItems"], 5);
    assert_eq!(arr_opts["uniqueItems"], true);
}

#[tokio::test]
async fn test_array_subset_and_number_finite_edge_cases() {
    let arr = array(string()).subset_of(&["cat", "dog"]).bail(true);
    assert!(arr.validate_async(&json!(["cat"])).await.is_ok());
    assert!(arr.validate_async(&json!(["cat", "fish"])).await.is_err());
    assert!(arr.validate(&json!([123])).is_err());

    let n = number().finite();
    assert!(n.validate(&json!(f64::NAN)).is_err());
    assert!(n.validate(&json!(f64::INFINITY)).is_err());
    assert!(n.validate(&json!(f64::NEG_INFINITY)).is_err());
}
