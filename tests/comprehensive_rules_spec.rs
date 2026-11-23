use jachai::prelude::*;
use serde_json::json;

#[test]
fn test_hostname_validation() {
    let schema = string().hostname();

    assert!(schema.validate(&json!("example.com")).is_ok());
    assert!(schema.validate(&json!("localhost")).is_ok());
    assert!(schema.validate(&json!("sub-domain.example.co.uk")).is_ok());
    assert!(schema.validate(&json!("sub-domain.example.co.uk.")).is_ok());
    assert!(schema.validate(&json!("123host.org")).is_ok());

    assert!(schema.validate(&json!("-invalid.com")).is_err());
    assert!(schema.validate(&json!("invalid-.com")).is_err());
    assert!(schema.validate(&json!("inv..alid.com")).is_err());
    assert!(schema.validate(&json!("")).is_err());
    let long_label = "a".repeat(64);
    assert!(
        schema
            .validate(&json!(format!("{long_label}.com")))
            .is_err()
    );
}

#[test]
fn test_hostname_rfc952_validation() {
    let schema = string().hostname_rfc952();

    assert!(schema.validate(&json!("example.com")).is_ok());
    assert!(schema.validate(&json!("host123.com")).is_ok());
    assert!(schema.validate(&json!("123host.com")).is_err()); // RFC 952 requires letter start
}

#[test]
fn test_domain_and_fqdn_validation() {
    let dom_schema = string().domain();

    assert!(dom_schema.validate(&json!("example.com")).is_ok());
    assert!(dom_schema.validate(&json!("sub.example.org")).is_ok());
    assert!(dom_schema.validate(&json!("localhost")).is_err());
    assert!(dom_schema.validate(&json!("example.c")).is_err()); // TLD too short
    assert!(dom_schema.validate(&json!("example.123")).is_err()); // TLD non-alpha

    let fqdn_schema = string().fqdn();
    assert!(fqdn_schema.validate(&json!("example.com.")).is_ok());
    assert!(fqdn_schema.validate(&json!("example.com")).is_ok());
    assert!(fqdn_schema.validate(&json!("localhost")).is_err());
}

#[test]
fn test_port_and_cidr_validation() {
    let port_schema = string().port();
    assert!(port_schema.validate(&json!("80")).is_ok());
    assert!(port_schema.validate(&json!("8080")).is_ok());
    assert!(port_schema.validate(&json!("65535")).is_ok());
    assert!(port_schema.validate(&json!("0")).is_err());
    assert!(port_schema.validate(&json!("65536")).is_err());
    assert!(port_schema.validate(&json!("abc")).is_err());
    assert!(port_schema.validate(&json!("-1")).is_err());

    let cidr_schema = string().cidr();
    assert!(cidr_schema.validate(&json!("192.168.1.0/24")).is_ok());
    assert!(cidr_schema.validate(&json!("10.0.0.0/8")).is_ok());
    assert!(cidr_schema.validate(&json!("2001:db8::/32")).is_ok());
    assert!(cidr_schema.validate(&json!("192.168.1.0/33")).is_err());
    assert!(cidr_schema.validate(&json!("2001:db8::/129")).is_err());
    assert!(cidr_schema.validate(&json!("192.168.1.0")).is_err());
    assert!(cidr_schema.validate(&json!("invalid/24")).is_err());
}

#[test]
fn test_data_uri_validation() {
    let schema = string().data_uri();

    assert!(
        schema
            .validate(&json!("data:text/plain;charset=utf-8,Hello%20World"))
            .is_ok()
    );
    assert!(
        schema
            .validate(&json!("data:text/plain;base64,SGVsbG8="))
            .is_ok()
    );
    assert!(schema.validate(&json!("data:,Hello")).is_ok());
    assert!(schema.validate(&json!("http://example.com")).is_err());
    assert!(
        schema
            .validate(&json!("data:invalid_base64;base64,!!@@##$$"))
            .is_err()
    );
}

#[test]
fn test_geographic_coordinates_validation() {
    let lat_str_schema = string().latitude();
    assert!(lat_str_schema.validate(&json!("40.7128")).is_ok());
    assert!(lat_str_schema.validate(&json!("-90.0")).is_ok());
    assert!(lat_str_schema.validate(&json!("90.0")).is_ok());
    assert!(lat_str_schema.validate(&json!("90.1")).is_err());
    assert!(lat_str_schema.validate(&json!("-90.1")).is_err());
    assert!(lat_str_schema.validate(&json!("not_a_num")).is_err());

    let lon_str_schema = string().longitude();
    assert!(lon_str_schema.validate(&json!("-74.0060")).is_ok());
    assert!(lon_str_schema.validate(&json!("-180.0")).is_ok());
    assert!(lon_str_schema.validate(&json!("180.0")).is_ok());
    assert!(lon_str_schema.validate(&json!("180.1")).is_err());
    assert!(lon_str_schema.validate(&json!("-180.1")).is_err());

    let coord_schema = string().coordinates();
    assert!(coord_schema.validate(&json!("40.7128,-74.0060")).is_ok());
    assert!(coord_schema.validate(&json!("40.7128, -74.0060")).is_ok());
    assert!(coord_schema.validate(&json!("-90.0, 180.0")).is_ok());
    assert!(coord_schema.validate(&json!("90.5, 50.0")).is_err());
    assert!(coord_schema.validate(&json!("50.0, 180.5")).is_err());
    assert!(coord_schema.validate(&json!("invalid")).is_err());

    let num_lat = number().latitude();
    assert!(num_lat.validate(&json!(45.0)).is_ok());
    assert!(num_lat.validate(&json!(-90.0)).is_ok());
    assert!(num_lat.validate(&json!(90.1)).is_err());

    let num_lon = number().longitude();
    assert!(num_lon.validate(&json!(120.0)).is_ok());
    assert!(num_lon.validate(&json!(-180.0)).is_ok());
    assert!(num_lon.validate(&json!(180.5)).is_err());

    let int_lat = integer().latitude();
    assert!(int_lat.validate(&json!(45)).is_ok());
    assert!(int_lat.validate(&json!(91)).is_err());

    let int_lon = integer().longitude();
    assert!(int_lon.validate(&json!(-120)).is_ok());
    assert!(int_lon.validate(&json!(181)).is_err());
}

#[test]
fn test_postal_code_validation() {
    let us_zip = string().zip_code();
    assert!(us_zip.validate(&json!("90210")).is_ok());
    assert!(us_zip.validate(&json!("12345-6789")).is_ok());
    assert!(us_zip.validate(&json!("9021")).is_err());
    assert!(us_zip.validate(&json!("902100")).is_err());
    assert!(us_zip.validate(&json!("12345-678")).is_err());
    assert!(us_zip.validate(&json!("abcde")).is_err());

    let ca_postal = string().postal_code("CA");
    assert!(ca_postal.validate(&json!("K1A 0B1")).is_ok());
    assert!(ca_postal.validate(&json!("K1A0B1")).is_ok());
    assert!(ca_postal.validate(&json!("123 456")).is_err());

    let gb_postal = string().postal_code("GB");
    assert!(gb_postal.validate(&json!("SW1A 1AA")).is_ok());
    assert!(gb_postal.validate(&json!("W1A 0AX")).is_ok());
    assert!(gb_postal.validate(&json!("M1 1AE")).is_ok());
    assert!(gb_postal.validate(&json!("12345")).is_err());

    let de_postal = string().postal_code("DE");
    assert!(de_postal.validate(&json!("10115")).is_ok());
    assert!(de_postal.validate(&json!("1011")).is_err());

    let au_postal = string().postal_code("AU");
    assert!(au_postal.validate(&json!("2000")).is_ok());
    assert!(au_postal.validate(&json!("200")).is_err());

    let nl_postal = string().postal_code("NL");
    assert!(nl_postal.validate(&json!("1012AB")).is_ok());
    assert!(nl_postal.validate(&json!("1012 AB")).is_ok());
    assert!(nl_postal.validate(&json!("1012")).is_err());

    let general_postal = string().postal_code("OTHER");
    assert!(general_postal.validate(&json!("12345")).is_ok());
    assert!(general_postal.validate(&json!("AB-123")).is_ok());
    assert!(general_postal.validate(&json!("AB")).is_err());
}

#[test]
fn test_financial_and_identifier_validation() {
    let iban_schema = string().iban();
    // Valid UK IBAN
    assert!(
        iban_schema
            .validate(&json!("GB82 WEST 1234 5698 7654 32"))
            .is_ok()
    );
    // Valid German IBAN
    assert!(
        iban_schema
            .validate(&json!("DE89 3704 0044 0532 0130 00"))
            .is_ok()
    );
    // Invalid IBAN (corrupted check digit)
    assert!(
        iban_schema
            .validate(&json!("GB83 WEST 1234 5698 7654 32"))
            .is_err()
    );
    assert!(iban_schema.validate(&json!("GB12")).is_err());

    let bic_schema = string().bic();
    assert!(bic_schema.validate(&json!("DEUTDEDD")).is_ok());
    assert!(bic_schema.validate(&json!("DEUTDEDD500")).is_ok());
    assert!(bic_schema.validate(&json!("BOFAGB2L")).is_ok());
    assert!(bic_schema.validate(&json!("BARCGB22XXX")).is_ok());
    assert!(bic_schema.validate(&json!("DEUTDE")).is_err());
    assert!(bic_schema.validate(&json!("12345678")).is_err());

    let swift_schema = string().swift();
    assert!(swift_schema.validate(&json!("DEUTDEDD")).is_ok());

    let isbn_schema = string().isbn();
    assert!(isbn_schema.validate(&json!("0-306-40615-2")).is_ok()); // ISBN-10
    assert!(isbn_schema.validate(&json!("0306406152")).is_ok());
    assert!(isbn_schema.validate(&json!("0-8044-2957-X")).is_ok()); // ISBN-10 with X
    assert!(isbn_schema.validate(&json!("978-0-306-40615-7")).is_ok()); // ISBN-13
    assert!(isbn_schema.validate(&json!("9780306406157")).is_ok());
    assert!(isbn_schema.validate(&json!("0-306-40615-3")).is_err()); // bad check digit
    assert!(isbn_schema.validate(&json!("978-0-306-40615-8")).is_err()); // bad check digit

    let isin_schema = string().isin();
    assert!(isin_schema.validate(&json!("US0378331005")).is_ok()); // Apple ISIN
    assert!(isin_schema.validate(&json!("GB0002374006")).is_ok());
    assert!(isin_schema.validate(&json!("US0378331006")).is_err()); // bad checksum

    let cuid_schema = string().cuid();
    assert!(
        cuid_schema
            .validate(&json!("ch72gsb320000udocfh2aook8"))
            .is_ok()
    );
    assert!(
        cuid_schema
            .validate(&json!("c000000000000000000000000"))
            .is_ok()
    );
    assert!(
        cuid_schema
            .validate(&json!("d000000000000000000000000"))
            .is_err()
    );
    assert!(cuid_schema.validate(&json!("c123")).is_err());

    let cuid2_schema = string().cuid2();
    assert!(
        cuid2_schema
            .validate(&json!("tz4a98xxat96iws9zmbrgj3a"))
            .is_ok()
    );
    assert!(
        cuid2_schema
            .validate(&json!("1z4a98xxat96iws9zmbrgj3a"))
            .is_err()
    );

    let nanoid_schema = string().nanoid();
    assert!(
        nanoid_schema
            .validate(&json!("V1StGXR8_Z5jdHi6B-myT"))
            .is_ok()
    );
    assert!(nanoid_schema.validate(&json!("invalid@char!")).is_err());
    assert!(
        string()
            .nanoid_len(10)
            .validate(&json!("0123456789"))
            .is_ok()
    );
    assert!(string().nanoid_len(10).validate(&json!("01234")).is_err());

    let country_schema = string().country_code();
    assert!(country_schema.validate(&json!("US")).is_ok());
    assert!(country_schema.validate(&json!("USA")).is_ok());
    assert!(country_schema.validate(&json!("U")).is_err());
    assert!(country_schema.validate(&json!("USAA")).is_err());

    let currency_schema = string().currency_code();
    assert!(currency_schema.validate(&json!("USD")).is_ok());
    assert!(currency_schema.validate(&json!("EUR")).is_ok());
    assert!(currency_schema.validate(&json!("US")).is_err());
    assert!(currency_schema.validate(&json!("USDT")).is_err());
}

#[test]
fn test_string_primitives_hex_digits_words_does_not_contain() {
    let hex_schema = string().hex();
    assert!(hex_schema.validate(&json!("deadbeef")).is_ok());
    assert!(hex_schema.validate(&json!("0x1234ABCD")).is_ok());
    assert!(hex_schema.validate(&json!("CAFE0123")).is_ok());
    assert!(hex_schema.validate(&json!("xyz")).is_err());
    assert!(hex_schema.validate(&json!("")).is_err());

    let digits_schema = string().digits();
    assert!(digits_schema.validate(&json!("1234567890")).is_ok());
    assert!(digits_schema.validate(&json!("123a45")).is_err());
    assert!(digits_schema.validate(&json!("")).is_err());

    let words_schema = string().words(2, 4);
    assert!(words_schema.validate(&json!("hello world")).is_ok());
    assert!(
        words_schema
            .validate(&json!("hello beautiful world"))
            .is_ok()
    );
    assert!(words_schema.validate(&json!("hello")).is_err());
    assert!(
        words_schema
            .validate(&json!("one two three four five"))
            .is_err()
    );

    let min_words_schema = string().min_words(2);
    assert!(min_words_schema.validate(&json!("one two")).is_ok());
    assert!(min_words_schema.validate(&json!("one")).is_err());

    let max_words_schema = string().max_words(2);
    assert!(max_words_schema.validate(&json!("one two")).is_ok());
    assert!(max_words_schema.validate(&json!("one two three")).is_err());

    let not_contain_schema = string().does_not_contain("forbidden");
    assert!(
        not_contain_schema
            .validate(&json!("allowed content"))
            .is_ok()
    );
    assert!(
        not_contain_schema
            .validate(&json!("contains forbidden content"))
            .is_err()
    );
}

#[test]
fn test_safe_integer_validation() {
    let num_schema = number().safe_integer();
    assert!(num_schema.validate(&json!(0.0)).is_ok());
    assert!(num_schema.validate(&json!(9_007_199_254_740_991.0)).is_ok());
    assert!(
        num_schema
            .validate(&json!(-9_007_199_254_740_991.0))
            .is_ok()
    );
    assert!(
        num_schema
            .validate(&json!(9_007_199_254_740_992.0))
            .is_err()
    );
    assert!(num_schema.validate(&json!(1.5)).is_err());

    let int_schema = integer().safe_integer();
    assert!(int_schema.validate(&json!(42)).is_ok());
    assert!(
        int_schema
            .validate(&json!(9_007_199_254_740_991_i64))
            .is_ok()
    );
}

#[test]
fn test_array_contains_combinators() {
    let schema = array(string())
        .contains(json!("admin"))
        .contains_all([json!("editor"), json!("viewer")])
        .contains_any([json!("super"), json!("owner")]);

    assert!(
        schema
            .validate(&json!(["admin", "editor", "viewer", "super"]))
            .is_ok()
    );
    assert!(
        schema
            .validate(&json!(["admin", "editor", "viewer", "owner"]))
            .is_ok()
    );

    // Missing admin
    assert!(
        schema
            .validate(&json!(["editor", "viewer", "super"]))
            .is_err()
    );
    // Missing viewer from contains_all
    assert!(
        schema
            .validate(&json!(["admin", "editor", "super"]))
            .is_err()
    );
    // Missing both super and owner from contains_any
    assert!(
        schema
            .validate(&json!(["admin", "editor", "viewer"]))
            .is_err()
    );
}

#[test]
fn test_object_exclusivity_rules() {
    let at_least_one = object()
        .field("email", string().optional())
        .field("phone", string().optional())
        .at_least_one_of(&["email", "phone"]);

    assert!(
        at_least_one
            .validate(&json!({"email": "test@example.com"}))
            .is_ok()
    );
    assert!(
        at_least_one
            .validate(&json!({"phone": "+1234567890"}))
            .is_ok()
    );
    assert!(
        at_least_one
            .validate(&json!({"email": "test@example.com", "phone": "+1234567890"}))
            .is_ok()
    );
    assert!(at_least_one.validate(&json!({})).is_err());
    assert!(
        at_least_one
            .validate(&json!({"email": null, "phone": ""}))
            .is_err()
    );

    let exactly_one = object()
        .field("credit_card", string().optional())
        .field("paypal", string().optional())
        .exactly_one_of(&["credit_card", "paypal"]);

    assert!(
        exactly_one
            .validate(&json!({"credit_card": "4111"}))
            .is_ok()
    );
    assert!(
        exactly_one
            .validate(&json!({"paypal": "account@paypal.com"}))
            .is_ok()
    );
    assert!(exactly_one.validate(&json!({})).is_err());
    assert!(
        exactly_one
            .validate(&json!({"credit_card": "4111", "paypal": "account@paypal.com"}))
            .is_err()
    );

    let exclusive = object()
        .field("monthly", boolean().optional())
        .field("annual", boolean().optional())
        .field("lifetime", boolean().optional())
        .mutually_exclusive(&["monthly", "annual", "lifetime"]);

    assert!(exclusive.validate(&json!({})).is_ok());
    assert!(exclusive.validate(&json!({"monthly": true})).is_ok());
    assert!(exclusive.validate(&json!({"annual": true})).is_ok());
    assert!(
        exclusive
            .validate(&json!({"monthly": true, "annual": true}))
            .is_err()
    );
}

#[tokio::test]
async fn test_async_array_contains_and_number_geo() {
    let arr_schema = array(string())
        .contains(json!("admin"))
        .contains_all([json!("editor")])
        .contains_any([json!("super")]);

    assert!(
        arr_schema
            .validate_async(&json!(["admin", "editor", "super"]))
            .await
            .is_ok()
    );
    assert!(
        arr_schema
            .validate_async(&json!(["editor", "super"]))
            .await
            .is_err()
    );
    assert!(
        arr_schema
            .validate_async(&json!(["admin", "super"]))
            .await
            .is_err()
    );
    assert!(
        arr_schema
            .validate_async(&json!(["admin", "editor"]))
            .await
            .is_err()
    );

    let num_schema = number().safe_integer().latitude().longitude();
    assert!(num_schema.validate_async(&json!(45.0)).await.is_ok());
    assert!(num_schema.validate_async(&json!(95.0)).await.is_err());

    let int_schema = integer().safe_integer().latitude().longitude();
    assert!(int_schema.validate_async(&json!(45)).await.is_ok());
    assert!(int_schema.validate_async(&json!(95)).await.is_err());
}

#[test]
fn test_json_schema_new_rules() {
    let s = string()
        .hostname()
        .domain()
        .fqdn()
        .data_uri()
        .hex()
        .digits();
    let json_val = s.to_json_schema();
    assert_eq!(json_val["type"], "string");

    let n = number().safe_integer().latitude().longitude();
    let n_json = n.to_json_schema();
    assert_eq!(n_json["type"], "integer");
    assert_eq!(n_json["minimum"], -180);
    assert_eq!(n_json["maximum"], 180);
}

#[test]
fn test_message_formatting_for_new_rules() {
    let issue = string()
        .hostname()
        .validate(&json!("-invalid.com"))
        .unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must be a valid hostname.")
    );

    let issue = string().domain().validate(&json!("localhost")).unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must be a valid domain name.")
    );

    let issue = string().safe_integer().validate(&json!("1.5")).unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must be a safe integer.")
    );

    let issue = string()
        .iban()
        .validate(&json!("INVALID_IBAN"))
        .unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must be a valid International Bank Account Number (IBAN).")
    );

    let issue = string().bic().validate(&json!("INV")).unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must be a valid Bank Identifier Code (BIC/SWIFT).")
    );

    let issue = string()
        .words(3, 5)
        .validate(&json!("only two"))
        .unwrap_err();
    assert_eq!(
        issue.first_message(),
        Some("The value field must contain between 3 and 5 words.")
    );
}

#[test]
fn test_object_projection_with_exclusivity_rules() {
    let base = object()
        .field("email", string().optional())
        .field("phone", string().optional())
        .field("role", string().optional())
        .at_least_one_of(&["email", "phone"])
        .mutually_exclusive(&["email", "phone"]);

    let picked = base.clone().pick(&["email", "phone"]);
    assert!(
        picked
            .validate(&json!({"email": "test@example.com"}))
            .is_ok()
    );
    assert!(
        picked
            .validate(&json!({"email": "a", "phone": "b"}))
            .is_err()
    );

    let omitted = base.omit(&["role"]);
    assert!(
        omitted
            .validate(&json!({"email": "test@example.com"}))
            .is_ok()
    );
}

#[test]
fn test_color_specifications() {
    let rgb_schema = string().rgb();
    assert!(rgb_schema.validate(&json!("rgb(255, 0, 128)")).is_ok());
    assert!(rgb_schema.validate(&json!("rgb(100%, 0%, 50%)")).is_ok());
    assert!(rgb_schema.validate(&json!("rgb(255 0 128)")).is_ok());
    assert!(rgb_schema.validate(&json!("rgb(256, 0, 0)")).is_err());
    assert!(rgb_schema.validate(&json!("rgb(255, 0)")).is_err());
    assert!(rgb_schema.validate(&json!("not-a-color")).is_err());

    let rgba_schema = string().rgba();
    assert!(
        rgba_schema
            .validate(&json!("rgba(255, 0, 128, 0.5)"))
            .is_ok()
    );
    assert!(
        rgba_schema
            .validate(&json!("rgba(255, 0, 128, 50%)"))
            .is_ok()
    );
    assert!(
        rgba_schema
            .validate(&json!("rgba(255 0 128 / 0.5)"))
            .is_ok()
    );
    assert!(
        rgba_schema
            .validate(&json!("rgba(255, 0, 128, 1.5)"))
            .is_err()
    );
    assert!(rgba_schema.validate(&json!("rgb(255, 0, 128)")).is_err());

    let hsl_schema = string().hsl();
    assert!(hsl_schema.validate(&json!("hsl(360, 100%, 50%)")).is_ok());
    assert!(hsl_schema.validate(&json!("hsl(180deg, 50%, 50%)")).is_ok());
    assert!(hsl_schema.validate(&json!("hsl(360 100% 50%)")).is_ok());
    assert!(hsl_schema.validate(&json!("hsl(400, 50%, 50%)")).is_err());

    let hsla_schema = string().hsla();
    assert!(
        hsla_schema
            .validate(&json!("hsla(360, 100%, 50%, 0.8)"))
            .is_ok()
    );
    assert!(
        hsla_schema
            .validate(&json!("hsla(360 100% 50% / 80%)"))
            .is_ok()
    );
    assert!(
        hsla_schema
            .validate(&json!("hsla(360, 100%, 50%, 2)"))
            .is_err()
    );
}

#[test]
fn test_crypto_addresses() {
    let eth_schema = string().ethereum_address();
    assert!(
        eth_schema
            .validate(&json!("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"))
            .is_ok()
    );
    assert!(
        eth_schema
            .validate(&json!("0x0000000000000000000000000000000000000000"))
            .is_ok()
    );
    assert!(eth_schema.validate(&json!("0x123")).is_err());
    assert!(eth_schema.validate(&json!("not-an-eth-address")).is_err());

    let btc_schema = string().bitcoin_address();
    assert!(
        btc_schema
            .validate(&json!("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"))
            .is_ok()
    );
    assert!(
        btc_schema
            .validate(&json!("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"))
            .is_ok()
    );
    assert!(
        btc_schema
            .validate(&json!("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"))
            .is_ok()
    );
    assert!(btc_schema.validate(&json!("not-a-btc-address")).is_err());
    assert!(btc_schema.validate(&json!("1Il0Oinvalid")).is_err());
}

#[test]
fn test_tax_business_identifiers() {
    let vat_schema = string().eu_vat();
    assert!(vat_schema.validate(&json!("DE123456789")).is_ok());
    assert!(vat_schema.validate(&json!("FR12345678901")).is_ok());
    assert!(vat_schema.validate(&json!("NL123456789B01")).is_ok());
    assert!(vat_schema.validate(&json!("US123456789")).is_err());
    assert!(vat_schema.validate(&json!("DE1")).is_err());

    let ein_schema = string().ein();
    assert!(ein_schema.validate(&json!("12-3456789")).is_ok());
    assert!(ein_schema.validate(&json!("123456789")).is_ok());
    assert!(ein_schema.validate(&json!("123-45678")).is_err());
    assert!(ein_schema.validate(&json!("12-345678a")).is_err());

    let ssn_schema = string().ssn();
    assert!(ssn_schema.validate(&json!("123-45-6789")).is_ok());
    assert!(ssn_schema.validate(&json!("123456789")).is_ok());
    assert!(ssn_schema.validate(&json!("000-12-3456")).is_err());
    assert!(ssn_schema.validate(&json!("666-12-3456")).is_err());
    assert!(ssn_schema.validate(&json!("900-12-3456")).is_err());
    assert!(ssn_schema.validate(&json!("123-00-6789")).is_err());
    assert!(ssn_schema.validate(&json!("123-45-0000")).is_err());
}

#[test]
fn test_standard_system_identifiers() {
    let mongo_schema = string().mongodb_object_id();
    assert!(
        mongo_schema
            .validate(&json!("507f1f77bcf86cd799439011"))
            .is_ok()
    );
    assert!(
        mongo_schema
            .validate(&json!("507f1f77bcf86cd79943901z"))
            .is_err()
    );
    assert!(mongo_schema.validate(&json!("123")).is_err());

    let bcp_schema = string().bcp47();
    assert!(bcp_schema.validate(&json!("en")).is_ok());
    assert!(bcp_schema.validate(&json!("en-US")).is_ok());
    assert!(bcp_schema.validate(&json!("zh-Hans-CN")).is_ok());
    assert!(bcp_schema.validate(&json!("es-419")).is_ok());
    assert!(bcp_schema.validate(&json!("bn-BD")).is_ok());
    assert!(bcp_schema.validate(&json!("123")).is_err());
    assert!(
        bcp_schema
            .validate(&json!("en-toolongsubtag12345"))
            .is_err()
    );

    let cron_schema = string().cron();
    assert!(cron_schema.validate(&json!("0 0 * * *")).is_ok());
    assert!(cron_schema.validate(&json!("*/15 * * * *")).is_ok());
    assert!(cron_schema.validate(&json!("0 12 * * 1-5")).is_ok());
    assert!(cron_schema.validate(&json!("0 0 1 1 * 2026")).is_ok());
    assert!(cron_schema.validate(&json!("invalid cron")).is_err());
    assert!(cron_schema.validate(&json!("0 0")).is_err());

    let ksuid_schema = string().ksuid();
    assert!(
        ksuid_schema
            .validate(&json!("0ujssZwY8NafAYfdisMmMtRQEk2"))
            .is_ok()
    );
    assert!(ksuid_schema.validate(&json!("short")).is_err());

    let issn_schema = string().issn();
    assert!(issn_schema.validate(&json!("0378-5955")).is_ok());
    assert!(issn_schema.validate(&json!("03785955")).is_ok());
    assert!(issn_schema.validate(&json!("2434-561X")).is_ok());
    assert!(issn_schema.validate(&json!("0378-5954")).is_err());

    let imei_schema = string().imei();
    assert!(imei_schema.validate(&json!("490154203237518")).is_ok());
    assert!(imei_schema.validate(&json!("490154203237519")).is_err());
    assert!(imei_schema.validate(&json!("12345")).is_err());
}

#[test]
fn test_temporal_calendar_rules() {
    let weekend_date = date().date_only().weekend();
    assert!(weekend_date.validate(&json!("2026-11-22")).is_ok()); // Sunday
    assert!(weekend_date.validate(&json!("2026-11-20")).is_err()); // Friday

    let weekday_date = date().date_only().weekday();
    assert!(weekday_date.validate(&json!("2026-11-20")).is_ok()); // Friday
    assert!(weekday_date.validate(&json!("2026-11-22")).is_err()); // Sunday

    let dur_date = date().duration();
    assert!(dur_date.validate(&json!("P3Y6M4DT12H30M5S")).is_ok());
    assert!(dur_date.validate(&json!("PT15M")).is_ok());
    assert!(dur_date.validate(&json!("P1D")).is_ok());
    assert!(dur_date.validate(&json!("P4W")).is_ok());
    assert!(dur_date.validate(&json!("P")).is_err());
    assert!(dur_date.validate(&json!("PT")).is_err());
    assert!(dur_date.validate(&json!("invalid")).is_err());

    let str_dur = string().duration();
    assert!(str_dur.validate(&json!("PT1H30M")).is_ok());
    assert!(str_dur.validate(&json!("bad")).is_err());

    let str_weekend = string().weekend();
    assert!(str_weekend.validate(&json!("2026-11-22")).is_ok());
    assert!(str_weekend.validate(&json!("2026-11-20")).is_err());

    let str_weekday = string().weekday();
    assert!(str_weekday.validate(&json!("2026-11-20")).is_ok());
    assert!(str_weekday.validate(&json!("2026-11-22")).is_err());
}

#[test]
fn test_sanitizers_and_normalizers() {
    let s_start = string().trim_start();
    assert_eq!(s_start.validate(&json!("  hello  ")).unwrap(), "hello  ");

    let s_end = string().trim_end();
    assert_eq!(s_end.validate(&json!("  hello  ")).unwrap(), "  hello");

    let s_escape = string().escape_html();
    assert_eq!(
        s_escape
            .validate(&json!("<script>alert('XSS') & \"test\"</script>"))
            .unwrap(),
        "&lt;script&gt;alert(&#x27;XSS&#x27;) &amp; &quot;test&quot;&lt;/script&gt;"
    );

    let s_norm_email = string().normalize_email();
    assert_eq!(
        s_norm_email
            .validate(&json!("  User@EXAMPLE.COM  "))
            .unwrap(),
        "User@example.com"
    );
}

#[test]
fn test_object_confirmed_rule() {
    let user_schema = object()
        .field("password", string())
        .field("password_confirmation", string())
        .confirmed("password");

    assert!(
        user_schema
            .validate(&json!({
                "password": "secret",
                "password_confirmation": "secret"
            }))
            .is_ok()
    );

    let err = user_schema
        .validate(&json!({
            "password": "secret",
            "password_confirmation": "wrong"
        }))
        .unwrap_err();
    assert_eq!(
        err.first_message(),
        Some("The password confirmation does not match.")
    );

    let err_missing = user_schema
        .validate(&json!({
            "password": "secret"
        }))
        .unwrap_err();
    assert!(
        err_missing
            .field_errors()
            .contains_key("password_confirmation")
    );
}
