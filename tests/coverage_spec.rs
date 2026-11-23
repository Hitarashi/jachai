//! Targeted test suite to exercise all internal modules, edge cases, and async methods for maximum test coverage.

use jachai::{coercion::*, prelude::*};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct MockDto {
    name: String,
    score: i64,
}

#[test]
fn test_coercion_edge_cases() {
    assert_eq!(trim_whitespace("  hello  "), "hello");

    let empty_str = json!("   ");
    assert_eq!(empty_string_to_null(&empty_str), serde_json::Value::Null);
    let non_empty_str = json!("val");
    assert_eq!(empty_string_to_null(&non_empty_str), non_empty_str);

    let nested_tree = json!({
        "a": "  ",
        "b": ["", "valid", "   "],
        "c": { "inner": "  ", "num": 10 }
    });
    let normalized = normalize_empty_strings(&nested_tree);
    assert_eq!(normalized["a"], serde_json::Value::Null);
    assert_eq!(normalized["b"][0], serde_json::Value::Null);
    assert_eq!(normalized["b"][1], "valid");
    assert_eq!(normalized["b"][2], serde_json::Value::Null);
    assert_eq!(normalized["c"]["inner"], serde_json::Value::Null);
    assert_eq!(normalized["c"]["num"], 10);

    // Number coercions
    assert_eq!(coerce_to_number(&json!(42)).unwrap().as_i64(), Some(42));
    assert_eq!(coerce_to_number(&json!("100")).unwrap().as_i64(), Some(100));
    assert_eq!(
        coerce_to_number(&json!("18446744073709551615"))
            .unwrap()
            .as_u64(),
        Some(18446744073709551615)
    );
    assert!(
        (coerce_to_number(&json!("12.345"))
            .unwrap()
            .as_f64()
            .unwrap()
            - 12.345)
            .abs()
            < 1e-4
    );
    assert!(coerce_to_number(&json!("not-a-number")).is_none());
    assert!(coerce_to_number(&json!(true)).is_none());

    // Bool coercions
    assert_eq!(coerce_to_bool(&json!(true)), Some(true));
    assert_eq!(coerce_to_bool(&json!(false)), Some(false));
    assert_eq!(coerce_to_bool(&json!(1)), Some(true));
    assert_eq!(coerce_to_bool(&json!(0)), Some(false));
    assert_eq!(coerce_to_bool(&json!(2)), None);
    assert_eq!(coerce_to_bool(&json!(1.5)), None);
    assert_eq!(coerce_to_bool(&json!("1")), Some(true));
    assert_eq!(coerce_to_bool(&json!("yes")), Some(true));
    assert_eq!(coerce_to_bool(&json!("0")), Some(false));
    assert_eq!(coerce_to_bool(&json!("no")), Some(false));
    assert_eq!(coerce_to_bool(&json!("invalid")), None);
    assert_eq!(coerce_to_bool(&json!([])), None);

    // String coercions
    assert_eq!(coerce_to_string(&json!("test")), Some("test".to_string()));
    assert_eq!(coerce_to_string(&json!(123)), Some("123".to_string()));
    assert_eq!(coerce_to_string(&json!(true)), Some("true".to_string()));
    assert_eq!(coerce_to_string(&json!([])), None);

    // Array coercions
    assert_eq!(
        coerce_to_array(&json!([1, 2, 3])),
        Some(vec![json!(1), json!(2), json!(3)])
    );
    assert_eq!(coerce_to_array(&json!(null)), None);
    assert_eq!(
        coerce_to_array(&json!("single")),
        Some(vec![json!("single")])
    );
}

#[test]
fn test_validation_context_full_api() {
    #[derive(Debug, PartialEq, Eq, Clone)]
    struct Dummy(i32);

    let mut ctx = ValidationContext::new();
    assert!(ctx.is_empty());
    assert_eq!(ctx.len(), 0);

    ctx.insert(Dummy(42));
    assert!(!ctx.is_empty());
    assert_eq!(ctx.len(), 1);
    assert!(ctx.contains::<Dummy>());
    assert_eq!(ctx.get::<Dummy>(), Some(&Dummy(42)));

    let removed = ctx.remove::<Dummy>();
    assert!(removed);
    assert!(!ctx.contains::<Dummy>());

    ctx.insert(Dummy(999));
    ctx.clear();
    assert!(ctx.is_empty());

    let formatted = format!("{ctx:?}");
    assert!(formatted.contains("ValidationContext"));
}

#[test]
fn test_error_and_issue_full_api() {
    let mut issue = ValidationIssue::new("field", "rule", "message");
    assert_eq!(issue.field, "field");
    assert_eq!(issue.rule, "rule");
    assert_eq!(issue.message, "message");

    let mut params = RuleParams::new();
    params.insert("k", "v");
    issue = issue.with_params(params.clone());
    assert_eq!(issue.params.get("k"), Some("v"));

    let mut err = ValidationError::single("f", "r", "m");
    assert_eq!(err.len(), 1);
    assert!(!err.is_empty());
    assert_eq!(err.first_issue().unwrap().field, "f");

    err.push(ValidationIssue::new("f2", "r2", "m2"));
    assert_eq!(err.len(), 2);
    assert_eq!(err.issues().len(), 2);
    assert_eq!(err.issues_mut().len(), 2);

    let mut other_err = ValidationError::new();
    other_err.push(ValidationIssue::new("f3", "r3", "m3"));
    err.merge(other_err);
    assert_eq!(err.len(), 3);

    // Iterators
    let mut count = 0;
    for _ in &err {
        count += 1;
    }
    assert_eq!(count, 3);

    let issues_vec = err.into_issues();
    assert_eq!(issues_vec.len(), 3);

    let reconstructed = ValidationError::from_issues(issues_vec);
    assert_eq!(reconstructed.len(), 3);

    // Display & Error
    let display_str = format!("{reconstructed}");
    assert!(display_str.contains("Validation failed with 3 errors"));
    let empty_err = ValidationError::new();
    assert_eq!(format!("{empty_err}"), "Validation failed with 0 issues");

    // JSON serialization
    let json_val = reconstructed.to_json_value();
    assert!(json_val.get("errors").is_some());
    let json_str = reconstructed.to_json_string();
    assert!(json_str.contains("errors"));

    // Problem details without instance
    let problem_no_instance = reconstructed.to_problem_details(None);
    assert!(problem_no_instance.get("instance").is_none());

    // UX Helpers: first_message, messages, field_errors
    assert_eq!(reconstructed.first_message(), Some("m"));
    assert_eq!(reconstructed.messages(), vec!["m", "m2", "m3"]);
    let field_errs = reconstructed.field_errors();
    assert_eq!(field_errs.get("f").unwrap(), &vec!["m".to_string()]);
    assert_eq!(field_errs.get("f2").unwrap(), &vec!["m2".to_string()]);
    assert_eq!(field_errs.get("f3").unwrap(), &vec!["m3".to_string()]);
    assert_eq!(empty_err.first_message(), None);
    assert!(empty_err.messages().is_empty());
    assert!(empty_err.field_errors().is_empty());
}

#[test]
fn test_rule_params_api() {
    let mut params = RuleParams::new();
    assert!(params.is_empty());
    params.insert("param1", "val1");
    assert_eq!(params.len(), 1);
    assert!(params.contains("param1"));
    assert_eq!(params.get("param1"), Some("val1"));

    let removed = params.remove("param1");
    assert_eq!(removed, Some("val1".to_string()));
    assert!(!params.contains("param1"));
}

#[test]
fn test_request_parts_fluent_builders_and_decodes() {
    let parts = RequestParts::new()
        .with_params(json!({ "id": 1 }))
        .with_query(json!({ "q": "search" }))
        .with_headers(json!({ "x-key": "secret" }))
        .with_cookies(json!({ "auth": "token" }))
        .with_body(json!({ "name": "Alice", "score": 95 }));

    assert_eq!(parts.params["id"], 1);
    assert_eq!(parts.query["q"], "search");
    assert_eq!(parts.headers["x-key"], "secret");
    assert_eq!(parts.cookies["auth"], "token");
    assert_eq!(parts.body["score"], 95);

    let req_schema = request()
        .params(object().field("id", integer()))
        .query(object().field("q", string()))
        .headers(object().field("x-key", string()))
        .cookies(object().field("auth", string()))
        .body(object().field("name", string()).field("score", integer()))
        .bail(true);

    let validated = req_schema.validate(&parts).unwrap();
    let body: MockDto = validated.decode_body().unwrap();
    assert_eq!(body.name, "Alice");
    assert_eq!(body.score, 95);

    let decoded_headers: serde_json::Value = validated.decode_headers().unwrap();
    assert_eq!(decoded_headers["x-key"], "secret");

    let decoded_cookies: serde_json::Value = validated.decode_cookies().unwrap();
    assert_eq!(decoded_cookies["auth"], "token");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_all_schema_types() {
    // Array async
    let arr_schema = array(string().min_len(2))
        .min_items(1)
        .max_items(5)
        .exact_items(2)
        .distinct()
        .not_empty()
        .bail(true)
        .with_message("Array error");

    let res_arr = arr_schema.validate_async(&json!(["ab", "cd"])).await;
    assert_eq!(res_arr.unwrap(), vec!["ab".to_string(), "cd".to_string()]);

    let err_arr = arr_schema.validate_async(&json!(["a"])).await.unwrap_err();
    assert!(!err_arr.is_empty());

    // Record async
    let rec_schema = record(integer().positive())
        .bail(true)
        .with_message("Record error");

    let valid_rec = json!({ "k1": 10, "k2": 20 });
    assert!(rec_schema.validate_async(&valid_rec).await.is_ok());

    let invalid_rec = json!({ "k1": -5 });
    assert!(rec_schema.validate_async(&invalid_rec).await.is_err());

    // Tuple2 async
    let tup_schema = tuple2(string(), integer()).bail(true);
    let valid_tup = json!(["hello", 42]);
    let (s, n) = tup_schema.validate_async(&valid_tup).await.unwrap();
    assert_eq!(s, "hello");
    assert_eq!(n, 42);

    let invalid_tup = json!(["hello"]);
    assert!(tup_schema.validate_async(&invalid_tup).await.is_err());

    // Union2 async
    let uni_schema = union2(integer().map(|i| i.to_string()), string());
    assert_eq!(uni_schema.validate_async(&json!(123)).await.unwrap(), "123");
    assert_eq!(
        uni_schema.validate_async(&json!("abc")).await.unwrap(),
        "abc"
    );

    // Optional, Nullable, Default async
    let opt_s = string().optional();
    assert_eq!(opt_s.validate_async(&json!(null)).await.unwrap(), None);

    let nul_s = integer().nullable();
    assert_eq!(nul_s.validate_async(&json!(null)).await.unwrap(), None);

    let def_s = string().default_value("fallback".to_string());
    assert_eq!(
        def_s.validate_async(&json!(null)).await.unwrap(),
        "fallback"
    );

    // Transform and Refine async
    let trans_s = string().transform(|s| Ok(s.len()));
    assert_eq!(trans_s.validate_async(&json!("hello")).await.unwrap(), 5);

    let ref_s = integer().refine(|n| *n > 0);
    assert_eq!(ref_s.validate_async(&json!(10)).await.unwrap(), 10);
    assert!(ref_s.validate_async(&json!(-10)).await.is_err());

    // Object async & decode_async
    let obj_s = object()
        .field("name", string())
        .field("score", integer())
        .bail(true)
        .with_message("Object failure");

    let valid_obj = json!({ "name": "Hero", "score": 100 });
    let dto: MockDto = obj_s.decode_async(&valid_obj).await.unwrap();
    assert_eq!(dto.name, "Hero");
    assert_eq!(dto.score, 100);

    let refined_obj =
        obj_s.refine(|val| val.get("score").and_then(|s| s.as_i64()).unwrap_or(0) >= 50);
    let failing_refine = json!({ "name": "Low", "score": 10 });
    assert!(refined_obj.validate_async(&failing_refine).await.is_err());
}

#[test]
fn test_rule_context_helpers_and_siblings() {
    let mut ambient_ctx = ValidationContext::new();
    ambient_ctx.insert("tenant_123".to_string());

    let parent_json = json!({
        "password": "secret",
        "confirm": "secret",
        "role": "admin"
    });

    let messages = SimpleMessagesProvider::default();
    let rule_ctx = RuleContext::new("confirm", &ambient_ctx, Some(&parent_json), &messages);

    assert_eq!(rule_ctx.path(), "confirm");
    assert_eq!(
        rule_ctx.get_ambient::<String>(),
        Some(&"tenant_123".to_string())
    );
    assert_eq!(rule_ctx.context().len(), 1);
    assert!(rule_ctx.parent_data().is_some());
    assert_eq!(
        rule_ctx.get_sibling("role").and_then(|v| v.as_str()),
        Some("admin")
    );
    assert_eq!(rule_ctx.get_sibling("non_existent"), None);

    let issue = rule_ctx.create_issue("custom_rule", RuleParams::new(), Some("Manual override"));
    assert_eq!(issue.message, "Manual override");
    assert_eq!(issue.field, "confirm");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_enum_schema_all_branches() {
    #[derive(Debug, PartialEq, Eq)]
    enum Status {
        Active,
        Inactive,
    }

    impl std::str::FromStr for Status {
        type Err = &'static str;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "active" => Ok(Self::Active),
                "inactive" => Ok(Self::Inactive),
                _ => Err("invalid status"),
            }
        }
    }

    let default_enum = r#enum::<Status>().bail(true).with_message("Status error");

    // Async validation
    assert_eq!(
        default_enum.validate_async(&json!("active")).await.unwrap(),
        Status::Active
    );

    // Parse error without preconfigured variants
    let err_unregistered = default_enum.validate(&json!("banned")).unwrap_err();
    assert_eq!(err_unregistered.issues()[0].message, "Status error");

    // Null and non-string inputs
    assert!(default_enum.validate(&json!(null)).is_err());
    assert!(default_enum.validate(&json!(12345)).is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_array_and_number_async_and_predicates() {
    let s = array(integer().positive())
        .min_items(2)
        .max_items(4)
        .exact_items(3)
        .bail(true);

    // Async with failing exact items
    let err_exact = s.validate_async(&json!([1, 2])).await.unwrap_err();
    assert_eq!(err_exact.issues()[0].rule, "exact_items");

    // Async with failing element
    let err_elem = s.validate_async(&json!([1, -2, 3])).await.unwrap_err();
    assert_eq!(err_elem.issues()[0].field, "[1]");

    // Number float predicates and async
    let num_s = number()
        .min(5.5)
        .max(10.5)
        .range(6.0, 10.0)
        .multiple_of(0.5)
        .bail(true)
        .with_message("Float number error");

    assert_eq!(num_s.validate_async(&json!(7.5)).await.unwrap(), 7.5);
    let err_mult = num_s.validate_async(&json!(7.3)).await.unwrap_err();
    assert_eq!(err_mult.issues()[0].message, "Float number error");

    // Integer negative
    let int_neg = integer()
        .negative()
        .bail(true)
        .with_message("Must be negative");
    assert_eq!(int_neg.validate(&json!(-10)).unwrap(), -10);
    let err_pos = int_neg.validate(&json!(10)).unwrap_err();
    assert_eq!(err_pos.issues()[0].message, "Must be negative");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_ultimate_coverage_boost() {
    // 1. ValidationError display with exactly 1 issue
    let single_err = ValidationError::single("email", "email", "Invalid email format");
    let display_single = format!("{single_err}");
    assert!(display_single.contains("Validation error: email: Invalid email format"));

    // 2. RuleParams builder and removal
    let mut params = RuleParams::new().with("key1", "val1").with("key2", "val2");
    assert_eq!(params.len(), 2);
    assert_eq!(params.remove("key1"), Some("val1".to_string()));
    assert_eq!(params.len(), 1);

    // 3. NumberSchema rule_async and bail
    struct AsyncNumLimitRule;
    impl AsyncCustomRule<f64> for AsyncNumLimitRule {
        fn validate<'a>(
            &'a self,
            value: &'a f64,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            Box::pin(async move {
                if *value > 50.0 {
                    Err(ValidationIssue::new(
                        ctx.path(),
                        "custom_async",
                        "Too large",
                    ))
                } else {
                    Ok(())
                }
            })
        }
    }

    let num_async_s = number().min(10.0).bail(true).rule_async(AsyncNumLimitRule);
    assert_eq!(
        num_async_s.validate_async(&json!(25.0)).await.unwrap(),
        25.0
    );
    let err_num_async = num_async_s.validate_async(&json!(60.0)).await.unwrap_err();
    assert_eq!(err_num_async.issues()[0].rule, "custom_async");

    // NumberSchema bail on predicate failure stops before evaluating async rules
    let err_num_bail = num_async_s.validate_async(&json!(5.0)).await.unwrap_err();
    assert_eq!(err_num_bail.issues()[0].rule, "min");

    // 4. IntegerSchema multiple_of and with_message
    let int_mult = integer()
        .multiple_of(5)
        .with_message("Multiple of 5 required");
    assert_eq!(int_mult.validate(&json!(15)).unwrap(), 15);
    assert_eq!(int_mult.validate_async(&json!(15)).await.unwrap(), 15);
    let err_int_mult = int_mult.validate(&json!(14)).unwrap_err();
    assert_eq!(err_int_mult.issues()[0].message, "Multiple of 5 required");

    // 5. RecordSchema sync and async bail, and async invalid types
    let rec_bail = record(integer().min(10)).bail(true);
    let err_rec_sync = rec_bail.validate(&json!({ "a": 5, "b": 2 })).unwrap_err();
    assert_eq!(err_rec_sync.issues().len(), 1);

    let err_rec_async_bail = rec_bail
        .validate_async(&json!({ "a": 5, "b": 2 }))
        .await
        .unwrap_err();
    assert_eq!(err_rec_async_bail.issues().len(), 1);

    assert!(rec_bail.validate_async(&json!(null)).await.is_err());
    assert!(rec_bail.validate_async(&json!("not_an_obj")).await.is_err());

    // 6. ArraySchema sync and async bail, and async invalid types
    let arr_bail = array(integer().min(10)).bail(true);
    let err_arr_sync = arr_bail.validate(&json!([5, 2])).unwrap_err();
    assert_eq!(err_arr_sync.issues().len(), 1);

    let err_arr_async_bail = arr_bail.validate_async(&json!([5, 2])).await.unwrap_err();
    assert_eq!(err_arr_async_bail.issues().len(), 1);

    assert!(arr_bail.validate_async(&json!(null)).await.is_err());
    assert!(arr_bail.validate_async(&json!("not_an_arr")).await.is_err());

    // 7. Schema::validate_async_with_context
    let v_ctx = ValidationContext::new();
    assert_eq!(
        string()
            .validate_async_with_context(&json!("hello"), &v_ctx)
            .await
            .unwrap(),
        "hello"
    );
    assert_eq!(
        integer()
            .validate_async_with_context(&json!(123), &v_ctx)
            .await
            .unwrap(),
        123
    );

    // 8. EnumSchema variants, bail, with_message, async null & non-string
    #[derive(Debug, PartialEq, Eq)]
    enum LocalRole {
        Admin,
        Member,
    }

    impl std::str::FromStr for LocalRole {
        type Err = &'static str;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "admin" => Ok(Self::Admin),
                "member" => Ok(Self::Member),
                _ => Err("unknown"),
            }
        }
    }

    let enum_s = r#enum::<LocalRole>()
        .variants(&["admin", "member"])
        .bail(true)
        .with_message("Unknown role");
    assert!(enum_s.validate_async(&json!(null)).await.is_err());
    assert!(enum_s.validate_async(&json!(123)).await.is_err());
    assert!(enum_s.validate_async(&json!("invalid_role")).await.is_err());
    assert_eq!(
        enum_s.validate_async(&json!("admin")).await.unwrap(),
        LocalRole::Admin
    );

    // 9. Coercion standalone helpers: normalize_empty_strings, empty_string_to_null
    let norm_val = jachai::coercion::normalize_empty_strings(&json!({ "a": "", "b": "val" }));
    assert_eq!(norm_val["a"], serde_json::Value::Null);
    assert_eq!(norm_val["b"], "val");
    assert_eq!(
        jachai::coercion::empty_string_to_null(&json!("not_empty")),
        json!("not_empty")
    );
    assert_eq!(
        jachai::coercion::empty_string_to_null(&json!(123)),
        json!(123)
    );

    // 10. StringSchema predicates failure coverage: starts_with, ends_with, contains, alpha, alphanumeric, ascii
    let s_prefix = string().starts_with("pre_");
    assert!(s_prefix.validate(&json!("bad_prefix")).is_err());

    let s_suffix = string().ends_with("_suf");
    assert!(s_suffix.validate(&json!("bad_suffix")).is_err());

    let s_contain = string().contains("needle");
    assert!(s_contain.validate(&json!("haystack")).is_err());

    let s_alpha = string().alpha();
    assert!(s_alpha.validate(&json!("abc123")).is_err());

    let s_alphanum = string().alphanumeric();
    assert!(s_alphanum.validate(&json!("abc@123")).is_err());

    let s_ascii = string().ascii();
    assert!(s_ascii.validate(&json!("hello\u{1F600}")).is_err());

    // 11. String refine_async with inline custom message
    let s_refine_custom = string()
        .with_message("Inline refine fail")
        .refine_async(|candidate| {
            let _ = candidate;
            Box::pin(async move { Err("ignored reason".to_string()) })
        });
    let err_refine_custom = s_refine_custom
        .validate_async(&json!("valid_str"))
        .await
        .unwrap_err();
    assert_eq!(
        err_refine_custom.first_message(),
        Some("Inline refine fail")
    );

    // 12. RefineSchema with_rule and with_message, async failure
    let refine_s = integer()
        .refine(|n| *n > 0)
        .with_rule("strictly_positive")
        .with_message("Value must be strictly positive");
    let err_refine_async = refine_s.validate_async(&json!(-5)).await.unwrap_err();
    assert_eq!(err_refine_async.issues()[0].rule, "strictly_positive");
    assert_eq!(
        err_refine_async.issues()[0].message,
        "Value must be strictly positive"
    );

    // 13. TransformSchema async failure
    let transform_s = integer().transform(|n| {
        if n == 0 {
            Err("Cannot be zero".to_string())
        } else {
            Ok(n * 2)
        }
    });
    let err_transform_async = transform_s.validate_async(&json!(0)).await.unwrap_err();
    assert_eq!(err_transform_async.issues()[0].rule, "transform");
    assert_eq!(err_transform_async.issues()[0].message, "Cannot be zero");
}

#[derive(Debug, Clone)]
struct FailingSerialize;

impl serde::Serialize for FailingSerialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let _ = serializer;
        Err(serde::ser::Error::custom("forced serialization failure"))
    }
}

#[derive(Clone)]
struct FailingSchema;

impl Schema<FailingSerialize> for FailingSchema {
    fn validate_path(
        &self,
        value: &serde_json::Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&serde_json::Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<FailingSerialize, ValidationError> {
        let _ = (value, path, context, parent_data, messages);
        Ok(FailingSerialize)
    }

    #[cfg(feature = "async")]
    fn validate_async_path<'a>(
        &'a self,
        value: &'a serde_json::Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a serde_json::Value>,
        messages: &'a dyn MessagesProvider,
    ) -> futures_util::future::BoxFuture<'a, Result<FailingSerialize, ValidationError>> {
        let _ = (value, path, context, parent_data, messages);
        Box::pin(async move { Ok(FailingSerialize) })
    }
}

#[tokio::test]
async fn test_uncovered_branches_sweep() {
    // 1. Coercer conversions returning None
    assert_eq!(Coercer::to_i64(&json!(true)), None);
    assert_eq!(Coercer::to_u64(&json!(false)), None);
    assert_eq!(Coercer::to_f64(&json!([])), None);

    // 2. ValidationIssue::with_param
    let issue_param = ValidationIssue::new("field", "rule", "msg").with_param("k", "v");
    assert_eq!(issue_param.params.get("k"), Some("v"));

    // 3. ValidationError::with_issue
    let err_with_issue = ValidationError::new().with_issue(ValidationIssue::new("f", "r", "m"));
    assert_eq!(err_with_issue.issues().len(), 1);

    // 4. prefix_paths edge cases (empty prefix, and empty issue field)
    let mut err_empty_prefix = ValidationError::single("field", "rule", "msg");
    err_empty_prefix.prefix_paths("");
    assert_eq!(err_empty_prefix.issues()[0].field, "field");

    let mut err_empty_field = ValidationError::single("", "rule", "msg");
    err_empty_field.prefix_paths("root");
    assert_eq!(err_empty_field.issues()[0].field, "root");

    // 5. MessagesProvider set_field_wildcard
    let mut provider = SimpleMessagesProvider::new();
    provider.set_field_wildcard("user.email", "Wildcard for :field");
    let formatted_wildcard = provider.format_message(
        "user.email",
        "some_unconfigured_rule",
        &RuleParams::new(),
        None,
    );
    assert_eq!(formatted_wildcard, "Wildcard for email");

    // 6. ValidatedRequest decode error
    let val_req = ValidatedRequest::default();
    let decode_res: Result<MockDto, _> = val_req.decode_body();
    assert!(decode_res.is_err());

    // 7. CustomRule Fn implementation
    let sync_closure_rule = |val: &String, ctx: &RuleContext<'_>| {
        if val.is_empty() {
            Err(ctx.create_issue("empty", RuleParams::new(), None))
        } else {
            Ok(())
        }
    };
    let val_ctx = ValidationContext::new();
    let msg_prov = SimpleMessagesProvider::new();
    let rule_ctx = RuleContext::new("name", &val_ctx, None, &msg_prov);
    assert!(CustomRule::validate(&sync_closure_rule, &"hello".to_string(), &rule_ctx).is_ok());
    assert!(CustomRule::validate(&sync_closure_rule, &"".to_string(), &rule_ctx).is_err());

    // 8. AsyncCustomRule Fn implementation
    fn async_closure_rule<'a>(
        val: &'a i32,
        ctx: &'a RuleContext<'a>,
    ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
        let is_negative = *val < 0;
        let err = if is_negative {
            Some(ctx.create_issue("positive", RuleParams::new(), None))
        } else {
            None
        };
        Box::pin(async move {
            if let Some(issue) = err {
                Err(issue)
            } else {
                Ok(())
            }
        })
    }
    assert!(
        AsyncCustomRule::validate(&async_closure_rule, &10, &rule_ctx)
            .await
            .is_ok()
    );
    assert!(
        AsyncCustomRule::validate(&async_closure_rule, &-5, &rule_ctx)
            .await
            .is_err()
    );

    // 9. Schema defaults
    #[derive(Debug, PartialEq, Eq)]
    enum LocalStatus {
        Active,
    }
    impl std::str::FromStr for LocalStatus {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s == "active" {
                Ok(Self::Active)
            } else {
                Err("invalid".to_string())
            }
        }
    }
    let _ = <jachai::schema::number::NumberSchema as Default>::default();
    let _ = <jachai::schema::r#enum::EnumSchema<LocalStatus> as Default>::default();
    let _ = <jachai::schema::object::ObjectSchema as Default>::default();
    let _ = <jachai::schema::string::StringSchema as Default>::default();

    // 10. NumberSchema custom rule
    let num_rule_schema = number().rule(|num: &f64, ctx: &RuleContext<'_>| {
        if *num < 10.0 {
            Err(ctx.create_issue("min_ten", RuleParams::new(), None))
        } else {
            Ok(())
        }
    });
    assert!(num_rule_schema.validate(&json!(5.0)).is_err());
    assert!(num_rule_schema.validate(&json!(15.0)).is_ok());

    // 11. StringSchema custom rule and invalid regex
    let str_rule_schema = string().rule(|s: &String, ctx: &RuleContext<'_>| {
        if s.is_empty() {
            Err(ctx.create_issue("non_empty", RuleParams::new(), None))
        } else {
            Ok(())
        }
    });
    assert!(str_rule_schema.validate(&json!("")).is_err());
    assert!(str_rule_schema.validate(&json!("val")).is_ok());

    let str_invalid_regex = string().regex("[unclosed-group");
    assert!(str_invalid_regex.validate(&json!("anything")).is_ok());

    // 12. Object strip and message mappings
    let obj_strip = object().strip().field("key", string());
    let stripped_val = obj_strip
        .validate(&json!({"key": "val", "extra": 123}))
        .unwrap();
    assert_eq!(stripped_val, json!({"key": "val"}));

    let obj_msgs = object().field("email", string().email()).messages(&[
        ("email.*", "Wildcard field message"),
        ("email.email", "Specific field message"),
    ]);
    let err_msg_specific = obj_msgs
        .validate(&json!({"email": "not_an_email"}))
        .unwrap_err();
    assert_eq!(
        err_msg_specific.first_message(),
        Some("Specific field message")
    );

    let obj_msgs_wc = object()
        .field("email", string().email())
        .messages(&[("email.*", "Wildcard only message")]);
    let err_msg_wc = obj_msgs_wc
        .validate(&json!({"email": "not_an_email"}))
        .unwrap_err();
    assert_eq!(err_msg_wc.first_message(), Some("Wildcard only message"));

    let obj_inline_override = object()
        .field(
            "name",
            string().min_len(5).with_message("Inline field message"),
        )
        .messages(&[("name.min_len", "Configured message")]);
    let err_inline = obj_inline_override
        .validate(&json!({"name": "abc"}))
        .unwrap_err();
    assert_eq!(err_inline.first_message(), Some("Inline field message"));

    // 13. Object bail on null and bail on failing fields (sync and async)
    let obj_bail_null = object().bail(true).field("a", string());
    assert!(obj_bail_null.validate(&serde_json::Value::Null).is_err());
    assert!(
        obj_bail_null
            .validate_async(&serde_json::Value::Null)
            .await
            .is_err()
    );

    let obj_bail_fields = object()
        .bail(true)
        .field("a", string().email())
        .field("b", string().email());
    let err_bail_sync = obj_bail_fields
        .validate(&json!({"a": "bad", "b": "bad"}))
        .unwrap_err();
    assert_eq!(err_bail_sync.issues().len(), 1);

    let err_bail_async = obj_bail_fields
        .validate_async(&json!({"a": "bad", "b": "bad"}))
        .await
        .unwrap_err();
    assert_eq!(err_bail_async.issues().len(), 1);

    // 14. Object decode error mapping (sync and async)
    #[derive(Debug, Deserialize)]
    struct MandatoryAge {
        pub age: u32,
    }
    let dummy_age = MandatoryAge { age: 25 };
    assert_eq!(dummy_age.age, 25);
    let obj_decode_fail = object().field("age", string());
    let err_decode_sync = obj_decode_fail
        .decode::<MandatoryAge>(&json!({"age": "not_a_number"}))
        .unwrap_err();
    assert_eq!(err_decode_sync.issues()[0].rule, "deserialization");

    let err_decode_async = obj_decode_fail
        .decode_async::<MandatoryAge>(&json!({"age": "not_a_number"}))
        .await
        .unwrap_err();
    assert_eq!(err_decode_async.issues()[0].rule, "deserialization");

    // 15. Array bail on min, max, exact (sync and async)
    let arr_min_bail = array(string()).min_items(2).bail(true);
    assert!(arr_min_bail.validate(&json!(["one"])).is_err());
    assert!(arr_min_bail.validate_async(&json!(["one"])).await.is_err());

    let arr_max_bail = array(string()).max_items(1).bail(true);
    assert!(arr_max_bail.validate(&json!(["one", "two"])).is_err());
    assert!(
        arr_max_bail
            .validate_async(&json!(["one", "two"]))
            .await
            .is_err()
    );

    let arr_exact_bail = array(string()).exact_items(2).bail(true);
    assert!(arr_exact_bail.validate(&json!(["one"])).is_err());
    assert!(
        arr_exact_bail
            .validate_async(&json!(["one"]))
            .await
            .is_err()
    );

    // 16. Nested path formatting in array, record, tuple
    let nested_arr = object().field("items", array(string().email()));
    let err_narr = nested_arr
        .validate(&json!({"items": ["bad_email"]}))
        .unwrap_err();
    assert_eq!(err_narr.issues()[0].field, "items[0]");
    let err_narr_async = nested_arr
        .validate_async(&json!({"items": ["bad_email"]}))
        .await
        .unwrap_err();
    assert_eq!(err_narr_async.issues()[0].field, "items[0]");

    let nested_rec = object().field("meta", record(string().email()));
    let err_nrec = nested_rec
        .validate(&json!({"meta": {"admin": "bad"}}))
        .unwrap_err();
    assert_eq!(err_nrec.issues()[0].field, "meta.admin");
    let err_nrec_async = nested_rec
        .validate_async(&json!({"meta": {"admin": "bad"}}))
        .await
        .unwrap_err();
    assert_eq!(err_nrec_async.issues()[0].field, "meta.admin");

    let rec_async_bail = record(string().email()).bail(true);
    let err_rec_bail = rec_async_bail
        .validate_async(&json!({"a": "bad", "b": "bad"}))
        .await
        .unwrap_err();
    assert_eq!(err_rec_bail.issues().len(), 1);

    let nested_tup = object().field("coords", tuple2(string().email(), string().email()));
    let err_ntup = nested_tup
        .validate(&json!({"coords": ["bad1", "bad2"]}))
        .unwrap_err();
    assert_eq!(err_ntup.issues()[0].field, "coords[0]");
    assert_eq!(err_ntup.issues()[1].field, "coords[1]");
    let err_ntup_async = nested_tup
        .validate_async(&json!({"coords": ["bad1", "bad2"]}))
        .await
        .unwrap_err();
    assert_eq!(err_ntup_async.issues()[0].field, "coords[0]");
    assert_eq!(err_ntup_async.issues()[1].field, "coords[1]");

    let tup_async_bail = tuple2(string().email(), string().email()).bail(true);
    let err_tbail = tup_async_bail
        .validate_async(&json!(["bad1", "bad2"]))
        .await
        .unwrap_err();
    assert_eq!(err_tbail.issues().len(), 1);

    let tup_async_no_bail = tuple2(string().email(), string().email());
    let err_t_nobail = tup_async_no_bail
        .validate_async(&json!(["bad1", "bad2"]))
        .await
        .unwrap_err();
    assert_eq!(err_t_nobail.issues().len(), 2);

    // 17. Async union inline message failure
    let union_inline_async =
        union(string().email(), string().uuid()).with_message("Must be valid email or uuid");
    let err_u_inline = union_inline_async
        .validate_async(&json!("invalid_token"))
        .await
        .unwrap_err();
    assert_eq!(
        err_u_inline.first_message(),
        Some("Must be valid email or uuid")
    );

    // 18. Serialization failure in ObjectSchema and RequestSchema (sync and async)
    let obj_fail = object().field("failing", FailingSchema);
    let err_obj_fail = obj_fail.validate(&json!({"failing": true})).unwrap_err();
    assert_eq!(err_obj_fail.issues()[0].rule, "serialization");

    let err_obj_fail_async = obj_fail
        .validate_async(&json!({"failing": true}))
        .await
        .unwrap_err();
    assert_eq!(err_obj_fail_async.issues()[0].rule, "serialization");

    // 19. Final edge cases for 99%+ coverage:
    // a. array not_empty bail (sync and async)
    let arr_ne_bail = array(string()).not_empty().bail(true);
    assert!(arr_ne_bail.validate(&json!([])).is_err());
    assert!(arr_ne_bail.validate_async(&json!([])).await.is_err());

    // b. IntegerSchema default
    let _ = <jachai::schema::number::IntegerSchema as Default>::default();

    // c. number and string sync fail early return in async validation when bail=true
    struct DummyAsyncNumRule;
    impl AsyncCustomRule<f64> for DummyAsyncNumRule {
        fn validate<'a>(
            &'a self,
            num: &'a f64,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            let _ = (num, ctx);
            Box::pin(async move { Ok(()) })
        }
    }

    struct DummyAsyncStrRule;
    impl AsyncCustomRule<String> for DummyAsyncStrRule {
        fn validate<'a>(
            &'a self,
            s: &'a String,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            let _ = (s, ctx);
            Box::pin(async move { Ok(()) })
        }
    }

    let num_sync_fail_async = number().min(10.0).bail(true).rule_async(DummyAsyncNumRule);
    assert!(
        num_sync_fail_async
            .validate_async(&json!(2.0))
            .await
            .is_err()
    );

    let str_sync_fail_async = string().min_len(5).bail(true).rule_async(DummyAsyncStrRule);
    assert!(
        str_sync_fail_async
            .validate_async(&json!("abc"))
            .await
            .is_err()
    );

    // d. tuple3 and tuple4 async validation edge cases (bail and non-bail failures)
    let t3_async = tuple3(string().email(), string().email(), string().email());
    assert!(
        t3_async
            .validate_async(&json!(["bad1", "bad2", "bad3"]))
            .await
            .is_err()
    );

    let t3_async_bail = tuple3(string().email(), string().email(), string().email()).bail(true);
    assert!(
        t3_async_bail
            .validate_async(&json!(["bad1", "bad2", "bad3"]))
            .await
            .is_err()
    );
    assert!(
        t3_async_bail
            .validate_async(&json!(["test@example.com", "bad2", "bad3"]))
            .await
            .is_err()
    );

    let t4_async = tuple4(
        string().email(),
        string().email(),
        string().email(),
        string().email(),
    );
    assert!(
        t4_async
            .validate_async(&json!(["bad1", "bad2", "bad3", "bad4"]))
            .await
            .is_err()
    );

    // e. object non-object payload failure and strict async bail
    let obj_non_obj = object().field("field", string());
    assert!(obj_non_obj.validate(&json!("primitive_string")).is_err());
    assert!(obj_non_obj.validate_async(&json!(12345)).await.is_err());

    let obj_strict_bail_async = object().strict().bail(true).field("known", string());
    assert!(
        obj_strict_bail_async
            .validate_async(&json!({"known": "val", "unknown": "unexpected"}))
            .await
            .is_err()
    );

    // f. tuple3 and tuple4 sync bail and nested path formatting
    let t3_sync_bail = tuple3(string().email(), string().email(), string().email()).bail(true);
    assert!(
        t3_sync_bail
            .validate(&json!(["bad1", "bad2", "bad3"]))
            .is_err()
    );
    assert!(
        t3_sync_bail
            .validate(&json!(["test@example.com", "bad2", "bad3"]))
            .is_err()
    );
    assert!(
        t3_sync_bail
            .validate(&json!(["test@example.com", "test@example.com", "bad3"]))
            .is_err()
    );

    let t3_async_bail_item2 =
        tuple3(string().email(), string().email(), string().email()).bail(true);
    assert!(
        t3_async_bail_item2
            .validate_async(&json!(["test@example.com", "bad2", "bad3"]))
            .await
            .is_err()
    );
    assert!(
        t3_async_bail_item2
            .validate_async(&json!(["test@example.com", "test@example.com", "bad3"]))
            .await
            .is_err()
    );

    let nested_t3 = object().field(
        "coords",
        tuple3(string().email(), string().email(), string().email()),
    );
    let err_nt3 = nested_t3
        .validate(&json!({"coords": ["bad1", "bad2", "bad3"]}))
        .unwrap_err();
    assert_eq!(err_nt3.issues()[0].field, "coords[0]");
    let err_nt3_async = nested_t3
        .validate_async(&json!({"coords": ["bad1", "bad2", "bad3"]}))
        .await
        .unwrap_err();
    assert_eq!(err_nt3_async.issues()[0].field, "coords[0]");

    let t4_sync_bail = tuple4(
        string().email(),
        string().email(),
        string().email(),
        string().email(),
    )
    .bail(true);
    assert!(
        t4_sync_bail
            .validate(&json!(["bad1", "bad2", "bad3", "bad4"]))
            .is_err()
    );
    assert!(
        t4_sync_bail
            .validate(&json!(["test@example.com", "bad2", "bad3", "bad4"]))
            .is_err()
    );
    assert!(
        t4_sync_bail
            .validate(&json!([
                "test@example.com",
                "test@example.com",
                "bad3",
                "bad4"
            ]))
            .is_err()
    );
    assert!(
        t4_sync_bail
            .validate(&json!([
                "test@example.com",
                "test@example.com",
                "test@example.com",
                "bad4"
            ]))
            .is_err()
    );

    let t4_async_bail_cases = tuple4(
        string().email(),
        string().email(),
        string().email(),
        string().email(),
    )
    .bail(true);
    assert!(
        t4_async_bail_cases
            .validate_async(&json!(["test@example.com", "bad2", "bad3", "bad4"]))
            .await
            .is_err()
    );
    assert!(
        t4_async_bail_cases
            .validate_async(&json!([
                "test@example.com",
                "test@example.com",
                "bad3",
                "bad4"
            ]))
            .await
            .is_err()
    );
    assert!(
        t4_async_bail_cases
            .validate_async(&json!([
                "test@example.com",
                "test@example.com",
                "test@example.com",
                "bad4"
            ]))
            .await
            .is_err()
    );

    let nested_t4 = object().field(
        "quad",
        tuple4(
            string().email(),
            string().email(),
            string().email(),
            string().email(),
        ),
    );
    let err_nt4 = nested_t4
        .validate(&json!({"quad": ["bad1", "bad2", "bad3", "bad4"]}))
        .unwrap_err();
    assert_eq!(err_nt4.issues()[0].field, "quad[0]");
    let err_nt4_async = nested_t4
        .validate_async(&json!({"quad": ["bad1", "bad2", "bad3", "bad4"]}))
        .await
        .unwrap_err();
    assert_eq!(err_nt4_async.issues()[0].field, "quad[0]");

    // g. union3 and union4 async inline messages
    let u3_msg =
        union3(string().email(), string().uuid(), string().ip()).with_message("Union3 failed");
    let err_u3 = u3_msg
        .validate_async(&json!("invalid_u3"))
        .await
        .unwrap_err();
    assert_eq!(err_u3.first_message(), Some("Union3 failed"));

    let u4_msg = union4(
        string().email(),
        string().uuid(),
        string().ip(),
        string().url(),
    )
    .with_message("Union4 failed");
    let err_u4 = u4_msg
        .validate_async(&json!("invalid_u4"))
        .await
        .unwrap_err();
    assert_eq!(err_u4.first_message(), Some("Union4 failed"));
}

#[tokio::test]
async fn test_date_and_format_max_coverage() {
    use jachai::schema::date::{
        DateSchema, days_in_month, is_leap_year, is_valid_rfc3339_date, is_valid_rfc3339_datetime,
        is_valid_rfc3339_time, parse_rfc3339_timestamp,
    };

    // 1. Standalone DateSchema builder and methods
    let d_default = <DateSchema as Default>::default();
    assert!(d_default.validate(&json!("2026-11-22T14:30:00Z")).is_ok());

    let d_fn = date();
    assert!(d_fn.validate(&json!("2026-11-22T14:30:00Z")).is_ok());

    let d_custom = DateSchema::new()
        .with_message("Invalid date format custom")
        .bail(true);
    let err_custom = d_custom.validate(&json!("not-a-date")).unwrap_err();
    assert_eq!(
        err_custom.first_message(),
        Some("Invalid date format custom")
    );

    let err_type = d_custom.validate(&json!(12345)).unwrap_err();
    assert_eq!(err_type.issues()[0].rule, "string");

    let d_date_only = DateSchema::new().date_only();
    assert!(d_date_only.validate(&json!("2026-11-22")).is_ok());
    assert!(d_date_only.validate(&json!("2026-13-40")).is_err());
    assert!(d_date_only.validate(&json!(true)).is_err());

    let d_time_only = DateSchema::new().time_only();
    assert!(d_time_only.validate(&json!("14:30:00Z")).is_ok());
    assert!(d_time_only.validate(&json!("99:99:99")).is_err());
    assert!(d_time_only.validate(&json!(false)).is_err());

    // 2. Pure algorithmic calendar and time helper coverage
    assert_eq!(days_in_month(2026, 13), 0);
    assert!(is_leap_year(2000));
    assert!(!is_leap_year(1900));
    assert!(is_leap_year(2024));
    assert!(!is_leap_year(2023));

    // Date branches
    assert!(!is_valid_rfc3339_date("short"));
    assert!(!is_valid_rfc3339_date("2026/11/22"));
    assert!(!is_valid_rfc3339_date("abcd-11-22"));
    assert!(!is_valid_rfc3339_date("2026-ab-22"));
    assert!(!is_valid_rfc3339_date("2026-11-cd"));

    // Time branches
    assert!(!is_valid_rfc3339_time("12:30")); // Too short
    assert!(!is_valid_rfc3339_time("12-30-00")); // Missing colons
    assert!(!is_valid_rfc3339_time("ab:30:00")); // Non-digit hours
    assert!(!is_valid_rfc3339_time("12:cd:00")); // Non-digit minutes
    assert!(!is_valid_rfc3339_time("12:30:ef")); // Non-digit seconds
    assert!(!is_valid_rfc3339_time("12:30:70")); // Seconds > 60
    assert!(is_valid_rfc3339_time("23:59:60Z")); // Leap second 60
    assert!(!is_valid_rfc3339_time("12:30:00.")); // Dot without digits
    assert!(is_valid_rfc3339_time("12:30:00.123")); // Fractional local
    assert!(!is_valid_rfc3339_time("12:30:00+ab:00")); // Non-digit offset hour
    assert!(!is_valid_rfc3339_time("12:30:00+25:00")); // Offset hour >= 24
    assert!(!is_valid_rfc3339_time("12:30:00+05:cd")); // Non-digit offset min
    assert!(!is_valid_rfc3339_time("12:30:00+05:65")); // Offset min >= 60
    assert!(!is_valid_rfc3339_time("12:30:00#05:00")); // Invalid offset prefix
    assert!(!is_valid_rfc3339_time("12:30:00+0500")); // Missing offset colon

    // DateTime branches
    assert!(!is_valid_rfc3339_datetime("2026-11-22")); // Too short
    assert!(!is_valid_rfc3339_datetime("2026-11-22X14:30:00Z")); // Invalid separator
    assert!(is_valid_rfc3339_datetime("2026-11-22t14:30:00z")); // Lowercase separator & timezone

    // 3. parse_rfc3339_timestamp branches
    assert!(parse_rfc3339_timestamp("invalid").is_none());
    assert!(parse_rfc3339_timestamp("2026-11-22").is_some());
    assert!(parse_rfc3339_timestamp("2026-13-40").is_none());
    assert!(parse_rfc3339_timestamp("2026-11-22T14:30:00Z").is_some());
    assert!(parse_rfc3339_timestamp("2026-11-22T14:30:00z").is_some());
    assert!(parse_rfc3339_timestamp("2026-11-22T14:30:00.123456+05:30").is_some());
    assert!(parse_rfc3339_timestamp("2026-11-22T14:30:00-08:00").is_some());
    assert!(parse_rfc3339_timestamp("2026-11-22T14:30:00+99:99").is_none());
    assert!(parse_rfc3339_timestamp("2026-11-22T25:00:00Z").is_none());

    // 4. ObjectSchema cross-field rules edge coverage
    let obj_cross = object()
        .field("f1", string().optional())
        .field("f2", string().optional())
        .required_with("f1", "f2")
        .prohibited_if("f1", "f2", json!("blocked"))
        .prohibited_with("f1", "f2")
        .after_date_field("f1", "f2")
        .before_date_field("f1", "f2");

    assert!(obj_cross.validate(&json!({})).is_ok());

    let obj_before = object()
        .field("start_date", string().date())
        .field("end_date", string().date())
        .before_date_field("start_date", "end_date");
    assert!(
        obj_before
            .validate(&json!({"start_date": "2026-01-01", "end_date": "2026-01-10"}))
            .is_ok()
    );
    assert!(
        obj_before
            .validate(&json!({"start_date": "2026-01-10", "end_date": "2026-01-01"}))
            .is_err()
    );
    assert!(
        obj_before
            .validate(&json!({"start_date": "2026-01-01", "end_date": "2026-01-01"}))
            .is_err()
    );

    let obj_after = object()
        .passthrough()
        .after_date_field("end_date", "start_date");
    assert!(
        obj_after
            .validate(&json!({"end_date": "not-date", "start_date": "2026-01-01"}))
            .is_ok()
    );
    assert!(
        obj_after
            .validate(&json!({"end_date": 123, "start_date": 456}))
            .is_ok()
    );

    let obj_before_opt = object()
        .passthrough()
        .before_date_field("start_date", "end_date");
    assert!(
        obj_before_opt
            .validate(&json!({"start_date": "not-date", "end_date": "2026-01-01"}))
            .is_ok()
    );
    assert!(
        obj_before_opt
            .validate(&json!({"start_date": 123, "end_date": 456}))
            .is_ok()
    );

    // 5. String predicates edge coverage (semver, slug, credit_card, base64)
    // Slug empty
    assert!(string().slug().validate(&json!("")).is_err());

    // SemVer branches
    let sem_schema = string().semver();
    assert!(sem_schema.validate(&json!("")).is_err());
    assert!(sem_schema.validate(&json!("-1.0.0")).is_err());
    assert!(sem_schema.validate(&json!("+1.0.0")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0+")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0+invalid!chars")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0-")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0-invalid!chars")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0-01")).is_err());
    assert!(sem_schema.validate(&json!("1.0.0.0")).is_err());
    assert!(sem_schema.validate(&json!("1.a.0")).is_err());
    assert!(sem_schema.validate(&json!("01.0.0")).is_err());

    // Base64 branches
    let b64_schema = string().base64();
    assert!(b64_schema.validate(&json!("AAAAAA==")).is_ok());
    assert!(b64_schema.validate(&json!("AAAAAA=")).is_err());
    assert!(b64_schema.validate(&json!("AAAA!A==")).is_err());

    // Credit card branches
    let cc_schema = string().credit_card();
    let super_long_card = "4".repeat(40);
    assert!(cc_schema.validate(&json!(super_long_card)).is_err());

    // ULID branches
    let ulid_schema = string().ulid();
    assert!(
        ulid_schema
            .validate(&json!("81ARZ3NDEKTSV4RRFFQ69G5FAV"))
            .is_err()
    ); // First char > 7

    // 6. Array and Number async bail coverage
    let arr_bail_1 = array(string()).not_empty().bail(true);
    assert!(arr_bail_1.validate_async(&json!([])).await.is_err());

    let arr_bail_2 = array(string()).min_items(2).bail(true);
    assert!(arr_bail_2.validate_async(&json!(["a"])).await.is_err());

    let arr_bail_3 = array(string()).max_items(1).bail(true);
    assert!(arr_bail_3.validate_async(&json!(["a", "b"])).await.is_err());

    let arr_bail_4 = array(string()).exact_items(3).bail(true);
    assert!(arr_bail_4.validate_async(&json!(["a", "b"])).await.is_err());

    struct FailRule;
    impl AsyncCustomRule<f64> for FailRule {
        fn validate<'a>(
            &'a self,
            value: &'a f64,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            let unused_val = value;
            let _ = unused_val;
            Box::pin(async move { Err(ctx.create_issue("fail", RuleParams::new(), None)) })
        }
    }
    let num_bail = number().bail(true).rule_async(FailRule);
    assert!(num_bail.validate_async(&json!(42.0)).await.is_err());
}

#[tokio::test]
async fn test_max_coverage_final_push() {
    // 1. Array sync and async bail & failure paths
    let arr_sync = array(string().email()).bail(true);
    assert!(arr_sync.validate(&json!(null)).is_err());
    assert!(arr_sync.validate(&json!(123)).is_err());
    assert!(arr_sync.validate(&json!(["bad1", "bad2"])).is_err());

    let arr_async = array(string().email()).bail(true);
    assert!(arr_async.validate_async(&json!(null)).await.is_err());
    assert!(arr_async.validate_async(&json!(123)).await.is_err());
    assert!(
        arr_async
            .validate_async(&json!(["bad1", "bad2"]))
            .await
            .is_err()
    );

    let arr_dist_sync = array(string()).distinct().bail(true);
    assert!(arr_dist_sync.validate(&json!(["a", "a"])).is_err());

    let arr_dist_async = array(string()).distinct().bail(true);
    assert!(
        arr_dist_async
            .validate_async(&json!(["a", "a"]))
            .await
            .is_err()
    );

    let arr_sub_sync = array(string()).subset_of(&["a"]).bail(true);
    assert!(arr_sub_sync.validate(&json!(["b"])).is_err());

    let arr_sub_async = array(string()).subset_of(&["a"]).bail(true);
    assert!(arr_sub_async.validate_async(&json!(["b"])).await.is_err());

    let arr_exact_sync = array(string()).exact_items(2).bail(true);
    assert!(arr_exact_sync.validate(&json!(["a"])).is_err());

    let arr_max_sync = array(string()).max_items(1).bail(true);
    assert!(arr_max_sync.validate(&json!(["a", "b"])).is_err());

    let arr_min_sync = array(string()).min_items(2).bail(true);
    assert!(arr_min_sync.validate(&json!(["a"])).is_err());

    let arr_empty_sync = array(string()).not_empty().bail(true);
    assert!(arr_empty_sync.validate(&json!([])).is_err());

    // 2. Tuple 2, 3, 4 with bail, with_message, sync and async edge paths
    let t2 = tuple2(string().email(), integer().min(10))
        .with_message("t2 custom error")
        .bail(true);
    assert!(t2.validate(&json!(null)).is_err());
    assert!(t2.validate(&json!("not-array")).is_err());
    assert!(t2.validate(&json!(["single"])).is_err());
    assert!(t2.validate(&json!(["bad_email", 100])).is_err());
    assert!(t2.validate(&json!(["test@example.com", 5])).is_err());
    assert!(t2.validate_async(&json!(null)).await.is_err());
    assert!(t2.validate_async(&json!("not-array")).await.is_err());
    assert!(t2.validate_async(&json!(["single"])).await.is_err());
    assert!(t2.validate_async(&json!(["bad_email", 100])).await.is_err());
    assert!(
        t2.validate_async(&json!(["test@example.com", 5]))
            .await
            .is_err()
    );

    let t3 = tuple3(string().email(), integer().min(10), boolean())
        .with_message("t3 custom error")
        .bail(true);
    assert!(t3.validate(&json!(null)).is_err());
    assert!(t3.validate(&json!("not-array")).is_err());
    assert!(t3.validate(&json!(["one", 2])).is_err());
    assert!(t3.validate(&json!(["bad_email", 100, true])).is_err());
    assert!(t3.validate(&json!(["test@example.com", 5, true])).is_err());
    assert!(
        t3.validate(&json!(["test@example.com", 20, "bad_bool"]))
            .is_err()
    );
    assert!(t3.validate_async(&json!(null)).await.is_err());
    assert!(t3.validate_async(&json!("not-array")).await.is_err());
    assert!(t3.validate_async(&json!(["one", 2])).await.is_err());
    assert!(
        t3.validate_async(&json!(["bad_email", 100, true]))
            .await
            .is_err()
    );
    assert!(
        t3.validate_async(&json!(["test@example.com", 5, true]))
            .await
            .is_err()
    );
    assert!(
        t3.validate_async(&json!(["test@example.com", 20, "bad_bool"]))
            .await
            .is_err()
    );

    let t4 = tuple4(
        string().email(),
        integer().min(10),
        boolean(),
        string().uuid(),
    )
    .with_message("t4 custom error")
    .bail(true);
    assert!(t4.validate(&json!(null)).is_err());
    assert!(t4.validate(&json!("not-array")).is_err());
    assert!(t4.validate(&json!(["one", 2, true])).is_err());
    assert!(
        t4.validate(&json!(["bad_email", 100, true, "123"]))
            .is_err()
    );
    assert!(
        t4.validate(&json!(["test@example.com", 5, true, "123"]))
            .is_err()
    );
    assert!(
        t4.validate(&json!(["test@example.com", 20, "bad_bool", "123"]))
            .is_err()
    );
    assert!(
        t4.validate(&json!(["test@example.com", 20, true, "bad_uuid"]))
            .is_err()
    );
    assert!(t4.validate_async(&json!(null)).await.is_err());
    assert!(t4.validate_async(&json!("not-array")).await.is_err());
    assert!(t4.validate_async(&json!(["one", 2, true])).await.is_err());
    assert!(
        t4.validate_async(&json!(["bad_email", 100, true, "123"]))
            .await
            .is_err()
    );
    assert!(
        t4.validate_async(&json!(["test@example.com", 5, true, "123"]))
            .await
            .is_err()
    );
    assert!(
        t4.validate_async(&json!(["test@example.com", 20, "bad_bool", "123"]))
            .await
            .is_err()
    );
    assert!(
        t4.validate_async(&json!(["test@example.com", 20, true, "bad_uuid"]))
            .await
            .is_err()
    );

    // 3. Object async bail and invalid inputs
    let obj = object()
        .field("email", string().email())
        .field("age", integer().min(18))
        .bail(true);
    assert!(obj.validate(&json!(null)).is_err());
    assert!(obj.validate(&json!("not-obj")).is_err());
    assert!(obj.validate(&json!({"email": "bad", "age": 10})).is_err());
    assert!(obj.validate_async(&json!(null)).await.is_err());
    assert!(obj.validate_async(&json!("not-obj")).await.is_err());
    assert!(
        obj.validate_async(&json!({"email": "bad", "age": 10}))
            .await
            .is_err()
    );

    // 4. Object cross-rules async bail
    let obj_cross = object()
        .field("a", string())
        .field("b", string())
        .same_as("a", "b")
        .bail(true);
    assert!(
        obj_cross
            .validate_async(&json!({"a": "foo", "b": "bar"}))
            .await
            .is_err()
    );

    // 5. Tuple constructor aliases and success paths
    let tup_alias = tuple(string(), integer());
    assert!(tup_alias.validate(&json!(["hello", 42])).is_ok());

    let t3_ok = tuple3(string(), integer(), boolean());
    assert!(t3_ok.validate(&json!(["hello", 42, true])).is_ok());
    assert!(
        t3_ok
            .validate_async(&json!(["hello", 42, true]))
            .await
            .is_ok()
    );

    let t4_ok = tuple4(string(), integer(), boolean(), string());
    assert!(t4_ok.validate(&json!(["hello", 42, true, "world"])).is_ok());
    assert!(
        t4_ok
            .validate_async(&json!(["hello", 42, true, "world"]))
            .await
            .is_ok()
    );

    // 6. Optional, Nullable, Default sync and async non-null values
    let opt_schema = string().optional();
    assert_eq!(
        opt_schema.validate_async(&json!("hello")).await.unwrap(),
        Some("hello".to_string())
    );

    let null_schema = string().nullable();
    assert_eq!(
        null_schema.validate_async(&json!("hello")).await.unwrap(),
        Some("hello".to_string())
    );
    assert_eq!(
        null_schema.validate_async(&json!(null)).await.unwrap(),
        None
    );

    let def_schema = string().default_value("default_val".to_string());
    assert_eq!(
        def_schema.validate(&json!("provided")).unwrap(),
        "provided".to_string()
    );
    assert_eq!(
        def_schema.validate_async(&json!("provided")).await.unwrap(),
        "provided".to_string()
    );
    assert_eq!(
        def_schema.validate_async(&json!(null)).await.unwrap(),
        "default_val".to_string()
    );

    let def_alias = string().default("default_val".to_string());
    assert_eq!(
        def_alias.validate(&json!(null)).unwrap(),
        "default_val".to_string()
    );

    // 7. Array range, between, exact_items builder and element async success
    let arr_r = array(string()).range(1, 3);
    assert!(arr_r.validate(&json!(["a"])).is_ok());
    let arr_b = array(string()).between(1, 3);
    assert!(arr_b.validate(&json!(["a", "b"])).is_ok());
    let arr_ex = array(string()).exact_items(2);
    assert!(arr_ex.validate(&json!(["a", "b"])).is_ok());

    let arr_async_ok = array(string()).min_items(1);
    assert!(
        arr_async_ok
            .validate_async(&json!(["a", "b"]))
            .await
            .is_ok()
    );

    // 8. NullableSchema to_json_schema with existing array
    let null_arr_schema = array(string()).nullable();
    let null_arr_json = null_arr_schema.to_json_schema();
    assert!(null_arr_json["type"].is_array());
}

#[test]
fn test_string_to_json_schema_all_predicates() {
    let s = string()
        .ip()
        .ipv6()
        .date()
        .time()
        .datetime()
        .credit_card()
        .ulid()
        .hex_color()
        .slug()
        .semver()
        .base64()
        .json_string()
        .e164()
        .jwt()
        .mac_address()
        .mime_type()
        .base64_url()
        .sha256()
        .sha512()
        .md5()
        .non_control()
        .alpha()
        .alphanumeric()
        .ascii()
        .starts_with("pre")
        .ends_with("post")
        .contains("mid")
        .in_list(&["opt1", "opt2"])
        .not_in_list(&["bad"])
        .regex("^[a-z]+$")
        .exact_len(20);

    let val = s.to_json_schema();
    assert_eq!(val["type"], "string");
    assert_eq!(val["minLength"], 20);
    assert_eq!(val["maxLength"], 20);
    assert_eq!(val["pattern"], "^[a-z]+$");
    assert!(val["enum"].is_array());
}

#[test]
fn test_cross_field_rules_projection() {
    let base_obj = object()
        .field("f1", string())
        .field("f2", string())
        .field("f3", string())
        .field("f4", string())
        .same_as("f1", "f2")
        .different_from("f1", "f2")
        .required_if("f1", "f2", "val")
        .required_without("f1", "f2")
        .after_field("f2", "f1")
        .before_field("f1", "f2")
        .after_date("f2", "f1")
        .before_date("f1", "f2")
        .required_with("f1", "f2")
        .prohibited_if("f1", "f2", "val")
        .prohibited_with("f1", "f2")
        .after_date_field("f2", "f1")
        .before_date_field("f1", "f2")
        .required_unless("f1", "f2", "val")
        .required_with_all("f1", &["f2", "f3"])
        .required_without_all("f1", &["f2", "f3"]);

    // pick containing f1 and f2 retains rules referencing f1 and f2
    let picked = base_obj.clone().pick(&["f1", "f2"]);
    assert_eq!(picked.to_json_schema()["type"], "object");

    // omit f1 drops all rules referencing f1
    let omitted = base_obj.omit(&["f1"]);
    assert_eq!(omitted.to_json_schema()["type"], "object");
}

#[tokio::test]
async fn test_tuple_full_bail_and_failure_matrix() {
    let t2_no_bail = tuple2(string().email(), integer().min(10));
    assert!(t2_no_bail.validate(&json!(["bad", 5])).is_err());
    assert!(t2_no_bail.validate_async(&json!(["bad", 5])).await.is_err());

    let t2_bail = tuple2(string().email(), integer().min(10)).bail(true);
    assert!(t2_bail.validate(&json!(["test@example.com", 5])).is_err());
    assert!(
        t2_bail
            .validate_async(&json!(["test@example.com", 5]))
            .await
            .is_err()
    );

    let t3_no_bail = tuple3(string().email(), integer().min(10), boolean());
    assert!(t3_no_bail.validate(&json!(["bad", 5, "not_bool"])).is_err());
    assert!(
        t3_no_bail
            .validate_async(&json!(["bad", 5, "not_bool"]))
            .await
            .is_err()
    );

    let t3_bail = tuple3(string().email(), integer().min(10), boolean()).bail(true);
    assert!(t3_bail.validate(&json!(["bad", 10, true])).is_err());
    assert!(
        t3_bail
            .validate(&json!(["test@example.com", 5, true]))
            .is_err()
    );
    assert!(
        t3_bail
            .validate(&json!(["test@example.com", 10, "bad_bool"]))
            .is_err()
    );
    assert!(
        t3_bail
            .validate_async(&json!(["bad", 10, true]))
            .await
            .is_err()
    );
    assert!(
        t3_bail
            .validate_async(&json!(["test@example.com", 5, true]))
            .await
            .is_err()
    );
    assert!(
        t3_bail
            .validate_async(&json!(["test@example.com", 10, "bad_bool"]))
            .await
            .is_err()
    );

    let t4_no_bail = tuple4(
        string().email(),
        integer().min(10),
        boolean(),
        string().uuid(),
    );
    assert!(
        t4_no_bail
            .validate(&json!(["bad", 5, "not_bool", "bad_uuid"]))
            .is_err()
    );
    assert!(
        t4_no_bail
            .validate_async(&json!(["bad", 5, "not_bool", "bad_uuid"]))
            .await
            .is_err()
    );

    let t4_bail = tuple4(
        string().email(),
        integer().min(10),
        boolean(),
        string().uuid(),
    )
    .bail(true);
    assert!(
        t4_bail
            .validate(&json!([
                "bad",
                10,
                true,
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .is_err()
    );
    assert!(
        t4_bail
            .validate(&json!([
                "test@example.com",
                5,
                true,
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .is_err()
    );
    assert!(
        t4_bail
            .validate(&json!([
                "test@example.com",
                10,
                "bad_bool",
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .is_err()
    );
    assert!(
        t4_bail
            .validate(&json!(["test@example.com", 10, true, "bad_uuid"]))
            .is_err()
    );
    assert!(
        t4_bail
            .validate_async(&json!([
                "bad",
                10,
                true,
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .await
            .is_err()
    );
    assert!(
        t4_bail
            .validate_async(&json!([
                "test@example.com",
                5,
                true,
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .await
            .is_err()
    );
    assert!(
        t4_bail
            .validate_async(&json!([
                "test@example.com",
                10,
                "bad_bool",
                "550e8400-e29b-41d4-a716-446655440000"
            ]))
            .await
            .is_err()
    );
    assert!(
        t4_bail
            .validate_async(&json!(["test@example.com", 10, true, "bad_uuid"]))
            .await
            .is_err()
    );
}

#[test]
fn test_all_new_rules_edge_cases_and_branches() {
    let host = string().hostname();
    assert!(host.validate(&json!("")).is_err());
    assert!(host.validate(&json!("-bad.com")).is_err());
    assert!(host.validate(&json!("bad-.com")).is_err());
    assert!(host.validate(&json!("bad..com")).is_err());
    let label64 = "a".repeat(64);
    assert!(host.validate(&json!(format!("{label64}.com"))).is_err());
    let host254 = format!("{}.com", "a".repeat(251));
    assert!(host.validate(&json!(host254)).is_err());

    let dom = string().domain();
    assert!(dom.validate(&json!("single")).is_err());
    assert!(dom.validate(&json!("example.c")).is_err());
    assert!(dom.validate(&json!("example.123")).is_err());

    let fqdn = string().fqdn();
    assert!(fqdn.validate(&json!("")).is_err());
    assert!(fqdn.validate(&json!("a")).is_err());
    let fqdn255 = "a".repeat(255);
    assert!(fqdn.validate(&json!(fqdn255)).is_err());

    let port = string().port();
    assert!(port.validate(&json!("")).is_err());
    assert!(port.validate(&json!("not_num")).is_err());
    assert!(port.validate(&json!("0")).is_err());
    assert!(port.validate(&json!("99999")).is_err());

    let cidr = string().cidr();
    assert!(cidr.validate(&json!("no_slash")).is_err());
    assert!(cidr.validate(&json!("192.168.1.1/abc")).is_err());
    assert!(cidr.validate(&json!("192.168.1.1/33")).is_err());
    assert!(cidr.validate(&json!("2001:db8::1/129")).is_err());
    assert!(cidr.validate(&json!("999.999.999.999/24")).is_err());

    let data_uri = string().data_uri();
    assert!(data_uri.validate(&json!("not_data_uri")).is_err());
    assert!(data_uri.validate(&json!("data:no_comma")).is_err());
    assert!(
        data_uri
            .validate(&json!("data:text/plain;base64,invalid!@#"))
            .is_err()
    );
    assert!(
        data_uri
            .validate(&json!("data:bad/mime/type,hello"))
            .is_err()
    );

    let postal = string().postal_code("US");
    assert!(postal.validate(&json!("   ")).is_err());
    assert!(postal.validate(&json!("1234")).is_err());
    assert!(postal.validate(&json!("123456")).is_err());
    assert!(postal.validate(&json!("12345-678")).is_err());
    assert!(postal.validate(&json!("12345_6789")).is_err());

    let postal_ca = string().postal_code("CA");
    assert!(postal_ca.validate(&json!("123456")).is_err());
    assert!(postal_ca.validate(&json!("A1A1A")).is_err());

    let postal_gb = string().postal_code("GB");
    assert!(postal_gb.validate(&json!("1234")).is_err());
    assert!(postal_gb.validate(&json!("SW1A1AAA")).is_err());

    let postal_de = string().postal_code("DE");
    assert!(postal_de.validate(&json!("1234")).is_err());

    let postal_au = string().postal_code("AU");
    assert!(postal_au.validate(&json!("123")).is_err());

    let postal_nl = string().postal_code("NL");
    assert!(postal_nl.validate(&json!("12345")).is_err());

    let postal_gen = string().postal_code("XYZ");
    assert!(postal_gen.validate(&json!("12")).is_err());
    assert!(postal_gen.validate(&json!("12345678901")).is_err());
    assert!(postal_gen.validate(&json!("ABCDEF")).is_err()); // no digits

    let iban = string().iban();
    assert!(iban.validate(&json!("GB1234")).is_err());
    assert!(iban.validate(&json!("1282WEST12345698765432")).is_err()); // non-alpha country
    assert!(iban.validate(&json!("GBXXWEST12345698765432")).is_err()); // non-digit check
    assert!(iban.validate(&json!("GB82WEST12345698765432@#$")).is_err()); // invalid chars

    let bic = string().bic();
    assert!(bic.validate(&json!("SHORT")).is_err());
    assert!(bic.validate(&json!("TOOLONGCODE123")).is_err());
    assert!(bic.validate(&json!("1234GB2L")).is_err()); // non-alpha bank
    assert!(bic.validate(&json!("BOFA122L")).is_err()); // non-alpha country

    let isbn10 = string().isbn10();
    assert!(isbn10.validate(&json!("123")).is_err());
    assert!(isbn10.validate(&json!("030640615A")).is_err());
    assert!(isbn10.validate(&json!("0306406151")).is_err()); // bad check digit

    let isbn13 = string().isbn13();
    assert!(isbn13.validate(&json!("123")).is_err());
    assert!(isbn13.validate(&json!("9700306406157")).is_err()); // doesn't start with 978 or 979
    assert!(isbn13.validate(&json!("978030640615A")).is_err()); // non digit

    let isin = string().isin();
    assert!(isin.validate(&json!("SHORT")).is_err());
    assert!(isin.validate(&json!("120378331005")).is_err()); // non-alpha country
    assert!(isin.validate(&json!("US037833100A")).is_err()); // non-digit check
    assert!(isin.validate(&json!("US037833100@#")).is_err()); // invalid chars

    let cuid = string().cuid();
    assert!(cuid.validate(&json!("a000000000000000000000000")).is_err());
    assert!(
        cuid.validate(&json!("c000000000000000000000000123"))
            .is_err()
    );
    assert!(cuid.validate(&json!("c00000000000000000000000A")).is_err());

    let cuid2 = string().cuid2();
    assert!(cuid2.validate(&json!("123456789012345678901234")).is_err());
    assert!(cuid2.validate(&json!("a1234567890123456789012")).is_err()); // 23 chars
    assert!(
        cuid2
            .validate(&json!("a123456789012345678901234567890123"))
            .is_err()
    ); // 33 chars

    let nanoid = string().nanoid();
    assert!(nanoid.validate(&json!("short")).is_err());
    assert!(nanoid.validate(&json!("V1StGXR8_Z5jdHi6B-myT@#$")).is_err());

    let country = string().country_code();
    assert!(country.validate(&json!("A")).is_err());
    assert!(country.validate(&json!("ABCD")).is_err());
    assert!(country.validate(&json!("12")).is_err());

    let curr = string().currency_code();
    assert!(curr.validate(&json!("US")).is_err());
    assert!(curr.validate(&json!("USDT")).is_err());
    assert!(curr.validate(&json!("123")).is_err());

    let hex_s = string().hex();
    assert!(hex_s.validate(&json!("")).is_err());
    assert!(hex_s.validate(&json!("0x")).is_err());
    assert!(hex_s.validate(&json!("0X")).is_err());
    assert!(hex_s.validate(&json!("deadbeefG")).is_err());

    let dig_s = string().digits();
    assert!(dig_s.validate(&json!("")).is_err());
    assert!(dig_s.validate(&json!("123a")).is_err());

    let obj = object()
        .field("a", string().optional())
        .field("b", string().optional())
        .at_least_one_of(&["a", "b"])
        .exactly_one_of(&["a", "b"])
        .mutually_exclusive(&["a", "b"]);
    assert!(obj.validate(&json!({"a": "hello"})).is_ok());
}

#[tokio::test]
async fn test_precision_coverage_sweep() {
    // Array bail branches sync
    let arr_sync_bail = array(string())
        .contains(json!("a"))
        .contains_all([json!("b")])
        .contains_any([json!("c")])
        .bail(true);
    assert!(arr_sync_bail.validate(&json!([])).is_err());

    let arr_sync_all_bail = array(string()).contains_all([json!("b")]).bail(true);
    assert!(arr_sync_all_bail.validate(&json!([])).is_err());

    let arr_sync_any_bail = array(string()).contains_any([json!("c")]).bail(true);
    assert!(arr_sync_any_bail.validate(&json!([])).is_err());

    // Array bail branches async
    let arr_min_bail = array(string()).min_items(2).bail(true);
    assert!(arr_min_bail.validate_async(&json!(["a"])).await.is_err());

    let arr_max_bail = array(string()).max_items(1).bail(true);
    assert!(
        arr_max_bail
            .validate_async(&json!(["a", "b"]))
            .await
            .is_err()
    );

    let arr_exact_bail = array(string()).exact_items(2).bail(true);
    assert!(arr_exact_bail.validate_async(&json!(["a"])).await.is_err());

    let arr_sub_bail = array(string()).subset_of(&["a"]).bail(true);
    assert!(arr_sub_bail.validate_async(&json!(["b"])).await.is_err());

    // Number json schema and edge cases
    let num_all = number()
        .min(1.5)
        .max(10.5)
        .range(1.5, 9.5)
        .positive()
        .negative()
        .non_negative()
        .non_positive()
        .multiple_of(0.25)
        .safe_integer()
        .latitude()
        .longitude();
    let num_json = num_all.to_json_schema();
    assert_eq!(num_json["type"], "integer");

    let int_json = integer().to_json_schema();
    assert_eq!(int_json["type"], "integer");

    // Number decimal_places on integer or without fractional
    let dec = number().decimal_places(2);
    assert!(dec.validate(&json!(100)).is_ok());

    // Number async with bail
    let num_async_bail = number().min(10.0).max(5.0).bail(true);
    assert!(num_async_bail.validate_async(&json!(2.0)).await.is_err());

    // Object prohibited_with / prohibited_if custom messages
    let obj_proh = object()
        .field("a", string().optional())
        .field("b", string().optional())
        .prohibited_if("a", "b", json!("stop"))
        .prohibited_with("a", "b");
    assert!(obj_proh.validate(&json!({"a": "x", "b": "stop"})).is_err());
    assert!(obj_proh.validate(&json!({"a": "x", "b": "y"})).is_err());

    // Object json schema
    let obj_json = object()
        .field("name", string())
        .field("age", integer().optional())
        .to_json_schema();
    assert_eq!(obj_json["type"], "object");

    // String without_trim and json schema formats
    let s_wt = string().without_trim();
    assert_eq!(s_wt.validate(&json!("  test  ")).unwrap(), "  test  ");

    let s_formats = string().date().time().datetime().e164().regex("^[a-z]+$");
    let sf_json = s_formats.to_json_schema();
    assert_eq!(sf_json["type"], "string");
}

#[tokio::test]
async fn test_reach_98_coverage() {
    // Array bails in sync and async
    let arr_cnt = array(string()).contains(json!("target")).bail(true);
    assert!(arr_cnt.validate(&json!(["other"])).is_err());
    assert!(arr_cnt.validate_async(&json!(["other"])).await.is_err());

    let arr_cnt_all = array(string())
        .contains_all([json!("a"), json!("b")])
        .bail(true);
    assert!(arr_cnt_all.validate(&json!(["a"])).is_err());
    assert!(arr_cnt_all.validate_async(&json!(["a"])).await.is_err());

    let arr_cnt_any = array(string())
        .contains_any([json!("x"), json!("y")])
        .bail(true);
    assert!(arr_cnt_any.validate(&json!(["a"])).is_err());
    assert!(arr_cnt_any.validate_async(&json!(["a"])).await.is_err());

    let arr_ne = array(string()).not_empty().bail(true);
    assert!(arr_ne.validate_async(&json!([])).await.is_err());

    let arr_max = array(string()).max_items(1).bail(true);
    assert!(arr_max.validate_async(&json!(["a", "b"])).await.is_err());

    let arr_exact = array(string()).exact_items(2).bail(true);
    assert!(arr_exact.validate_async(&json!(["a"])).await.is_err());

    let arr_sub = array(string()).subset_of(&["a"]).bail(true);
    assert!(arr_sub.validate_async(&json!(["b"])).await.is_err());

    // File max_size bail
    let file_schema = file().max_size(10).bail(true);
    assert!(
        file_schema
            .validate(&json!({"name": "test.png", "size": 100, "mime_type": "image/png"}))
            .is_err()
    );

    // Schema trait defaults
    struct CustomMinimal;
    impl Schema<String> for CustomMinimal {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<String, ValidationError> {
            let has_ctx = context.is_empty();
            let has_parent = parent_data.is_none();
            let has_msg = messages.format_message(path, "dummy", &RuleParams::new(), None);
            let has_val = value.is_null();
            if has_ctx && has_parent && has_val && has_msg.is_empty() {
                Ok("ok".to_string())
            } else {
                Ok("fallback".to_string())
            }
        }
    }
    let minimal = CustomMinimal;
    assert!(!minimal.is_optional());
    assert_eq!(minimal.to_json_schema_value(), json!({}));

    // Number predicates and json schema branches
    let num_async_bail = number().min(10.0).bail(true);
    assert!(num_async_bail.validate_async(&json!(5.0)).await.is_err());

    let num_finite_fail = number().finite();
    assert!(num_finite_fail.validate(&json!("not-num")).is_err());

    let num_max_int = number()
        .max(10.0)
        .multiple_of(2.5)
        .finite()
        .decimal_places(2);
    let num_max_json = num_max_int.to_json_schema();
    assert_eq!(num_max_json["maximum"], 10);
    assert_eq!(num_max_json["multipleOf"], 2.5);

    let int_schema = integer().min(5);
    let int_json = int_schema.to_json_schema();
    assert_eq!(int_json["type"], "integer");

    // Optional schema branches
    let opt_schema = string().optional();
    assert!(opt_schema.is_optional());

    let null_schema = string().nullable();
    assert!(null_schema.is_optional());

    struct MultiType;
    impl Schema<String> for MultiType {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<String, ValidationError> {
            let has_ctx = context.is_empty();
            let has_parent = parent_data.is_none();
            let has_msg = messages.format_message(path, "dummy", &RuleParams::new(), None);
            let has_val = value.is_null();
            if has_ctx && has_parent && has_val && has_msg.is_empty() {
                Ok("ok".to_string())
            } else {
                Ok("fallback".to_string())
            }
        }
        fn to_json_schema_value(&self) -> serde_json::Value {
            json!({"type": ["string", "number"]})
        }
    }
    let multi_opt = MultiType.nullable();
    let multi_json = multi_opt.to_json_schema();
    assert_eq!(multi_json["type"], json!(["string", "number", "null"]));

    struct NoType;
    impl Schema<String> for NoType {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<String, ValidationError> {
            let has_ctx = context.is_empty();
            let has_parent = parent_data.is_none();
            let has_msg = messages.format_message(path, "dummy", &RuleParams::new(), None);
            let has_val = value.is_null();
            if has_ctx && has_parent && has_val && has_msg.is_empty() {
                Ok("ok".to_string())
            } else {
                Ok("fallback".to_string())
            }
        }
        fn to_json_schema_value(&self) -> serde_json::Value {
            json!({"description": "none"})
        }
    }
    let no_type_opt = NoType.nullable();
    let no_type_json = no_type_opt.to_json_schema();
    assert_eq!(no_type_json["description"], "none");

    // Object schema branches
    let dup_field_obj = object()
        .field("key", string())
        .field("key", string())
        .optional_field("opt_key", string())
        .optional_field("opt_key", string());
    assert!(dup_field_obj.validate(&json!({"key": "val"})).is_ok());

    let obj1 = object().field("a", string());
    let obj2 = object().field("a", string()).field("b", string());
    let merged_obj = obj1.extend(obj2);
    assert!(merged_obj.validate(&json!({"a": "1", "b": "2"})).is_ok());

    let partial_obj = object()
        .field("req", string())
        .optional_field("already_opt", string())
        .partial();
    let part_json = partial_obj.to_json_schema();
    assert_eq!(part_json["type"], "object");
    assert!(
        partial_obj
            .validate_async(&json!({"req": "hello"}))
            .await
            .is_ok()
    );

    let strict_bail_obj = object().field("a", string()).strict().bail(true);
    assert!(
        strict_bail_obj
            .validate_async(&json!({"a": "ok", "extra": 1}))
            .await
            .is_err()
    );

    let opt_only_obj = object().optional_field("opt", string());
    let opt_only_json = opt_only_obj.to_json_schema();
    assert!(opt_only_json.get("required").is_none());

    // String predicates branches
    let phone_schema = string().phone_number();
    assert!(phone_schema.validate(&json!("+14155552671")).is_ok());

    assert!(string().sha512().validate(&json!("bad-sha")).is_err());
    assert!(string().isbn10().validate(&json!("0306406152")).is_ok());
    assert!(string().isbn13().validate(&json!("9780306406157")).is_ok());
    assert!(
        string()
            .safe_integer()
            .validate(&json!("9007199254740991"))
            .is_ok()
    );

    let str_urls = string().url().ipv4().to_json_schema();
    assert_eq!(str_urls["format"], "ipv4");

    assert!(string().base64().validate(&json!("abc===")).is_err());
    assert!(string().jwt().validate(&json!(".b.c")).is_err());
    assert!(string().jwt().validate(&json!("a..c")).is_err());

    assert!(
        string()
            .mac_address()
            .validate(&json!("00:1A:2B:3C:4D:GG"))
            .is_err()
    );
    assert!(
        string()
            .mac_address()
            .validate(&json!("00:1A:2B:3C:4D:5E:6F:GG"))
            .is_err()
    );
    assert!(
        string()
            .mac_address()
            .validate(&json!("00:1A:2B:3C:4D-5E:6F:7A"))
            .is_err()
    );

    assert!(string().mime_type().validate(&json!("")).is_err());
    assert!(string().hostname().validate(&json!("foo_bar.com")).is_err());
    assert!(string().domain().validate(&json!("localhost")).is_err());
    assert!(string().fqdn().validate(&json!("localhost")).is_err());

    assert!(string().port().validate(&json!("70000")).is_err());
    assert!(string().port().validate(&json!("0")).is_err());

    assert!(string().cidr().validate(&json!("192.168.1.1/abc")).is_err());
    assert!(string().cidr().validate(&json!("192.168.1.1/33")).is_err());

    assert!(string().longitude().validate(&json!("abc")).is_err());
    assert!(string().longitude().validate(&json!("181.0")).is_err());

    assert!(
        string()
            .iban()
            .validate(&json!("GB00 ABCD 1234 5678 9012 34!"))
            .is_err()
    );
    assert!(string().isbn10().validate(&json!("03064061a2")).is_err());
    assert!(string().isin().validate(&json!("US03783310!5")).is_err());
    assert!(string().isin().validate(&json!("US03783310?5")).is_err());
}

#[tokio::test]
async fn test_new_rules_and_granular_features_full_coverage() {
    // 1. DateSchema complete coverage
    let dt_custom = date().with_message("custom-date-msg");
    assert_eq!(
        dt_custom
            .validate(&json!("bad-dt"))
            .unwrap_err()
            .first_message(),
        Some("custom-date-msg")
    );
    assert!(dt_custom.validate(&json!(123)).is_err());

    let dt_time = date().time_only();
    assert!(dt_time.validate(&json!("14:30:00")).is_ok());
    assert!(dt_time.validate(&json!("99:99:99")).is_err());
    assert_eq!(dt_time.to_json_schema()["format"], "time");

    let dt_dur = date().duration();
    assert!(dt_dur.validate(&json!("P1Y2M10DT2H30M")).is_ok());
    assert!(dt_dur.validate(&json!("not-duration")).is_err());
    assert_eq!(dt_dur.to_json_schema()["format"], "duration");

    let dt_date_only = date().date_only();
    assert!(dt_date_only.validate(&json!("2026-11-22")).is_ok());
    assert!(dt_date_only.validate(&json!("not-a-date")).is_err());
    assert_eq!(dt_date_only.to_json_schema()["format"], "date");

    let dt_wknd = date().date_only().weekend().bail(true);
    assert!(dt_wknd.validate(&json!("2026-11-21")).is_ok()); // Saturday
    assert!(dt_wknd.validate(&json!("2026-11-20")).is_err()); // Friday

    let dt_wkdy = date().date_only().weekday().bail(true);
    assert!(dt_wkdy.validate(&json!("2026-11-20")).is_ok()); // Friday
    assert!(dt_wkdy.validate(&json!("2026-11-21")).is_err()); // Saturday

    // 2. String to_json_schema for all new predicates
    let color_schema = string().rgb().rgba().hsl().hsla();
    let color_json = color_schema.to_json_schema();
    assert_eq!(color_json["type"], "string");

    let crypto_schema = string().ethereum_address().bitcoin_address();
    let crypto_json = crypto_schema.to_json_schema();
    assert_eq!(crypto_json["type"], "string");

    let biz_schema = string().eu_vat().ein().ssn().issn().imei();
    let biz_json = biz_schema.to_json_schema();
    assert_eq!(biz_json["type"], "string");

    let sys_schema = string()
        .mongodb_object_id()
        .bcp47()
        .cron()
        .duration()
        .ksuid()
        .weekend()
        .weekday();
    let sys_json = sys_schema.to_json_schema();
    assert_eq!(sys_json["type"], "string");

    // 3. String predicates error branch coverage
    assert!(string().rgb().validate(&json!("rgb(300, 0, 0)")).is_err());
    assert!(string().rgb().validate(&json!("rgb(0, 0)")).is_err());
    assert!(string().rgb().validate(&json!("rgb(a, b, c)")).is_err());
    assert!(
        string()
            .rgba()
            .validate(&json!("rgba(0, 0, 0, 2.5)"))
            .is_err()
    );
    assert!(
        string()
            .hsl()
            .validate(&json!("hsl(400, 50%, 50%)"))
            .is_err()
    );
    assert!(
        string()
            .hsl()
            .validate(&json!("hsl(100, 150%, 50%)"))
            .is_err()
    );
    assert!(
        string()
            .hsla()
            .validate(&json!("hsla(0, 10%, 10%, 5.0)"))
            .is_err()
    );

    assert!(
        string()
            .ethereum_address()
            .validate(&json!("0x123"))
            .is_err()
    );
    assert!(
        string()
            .ethereum_address()
            .validate(&json!("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"))
            .is_err()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("not-a-btc-address"))
            .is_err()
    );

    assert!(string().eu_vat().validate(&json!("XX12345678")).is_err());
    assert!(string().eu_vat().validate(&json!("AT")).is_err());
    assert!(
        string()
            .eu_vat()
            .validate(&json!("AT1234567890123456"))
            .is_err()
    );

    assert!(string().ein().validate(&json!("12-34567")).is_err());
    assert!(string().ein().validate(&json!("12-34567890")).is_err());

    assert!(string().ssn().validate(&json!("000-12-3456")).is_err());
    assert!(string().ssn().validate(&json!("666-12-3456")).is_err());
    assert!(string().ssn().validate(&json!("900-12-3456")).is_err());
    assert!(string().ssn().validate(&json!("123-00-4567")).is_err());
    assert!(string().ssn().validate(&json!("123-45-0000")).is_err());
    assert!(string().ssn().validate(&json!("1234567890")).is_err());

    assert!(
        string()
            .mongodb_object_id()
            .validate(&json!("1234"))
            .is_err()
    );
    assert!(
        string()
            .mongodb_object_id()
            .validate(&json!("507f1f77bcf86cd79943901g"))
            .is_err()
    );

    assert!(string().bcp47().validate(&json!("1")).is_err());
    assert!(
        string()
            .bcp47()
            .validate(&json!("toolongsubtag12345"))
            .is_err()
    );

    assert!(string().cron().validate(&json!("* * *")).is_err());
    assert!(string().cron().validate(&json!("* * * * * * *")).is_err());
    assert!(
        string()
            .cron()
            .validate(&json!("@invalid * * * *"))
            .is_err()
    );

    assert!(string().ksuid().validate(&json!("short")).is_err());
    assert!(
        string()
            .ksuid()
            .validate(&json!("0ujXoFu748723498723948723489723?"))
            .is_err()
    );

    assert!(string().issn().validate(&json!("1234-5678")).is_err());
    assert!(string().issn().validate(&json!("1234567890")).is_err());

    assert!(string().imei().validate(&json!("123456789012345")).is_err());
    assert!(string().imei().validate(&json!("123456")).is_err());

    assert!(string().weekend().validate(&json!("2026-11-20")).is_err());
    assert!(string().weekend().validate(&json!("not-date")).is_err());
    assert!(string().weekday().validate(&json!("2026-11-21")).is_err());
    assert!(string().weekday().validate(&json!("not-date")).is_err());

    assert!(
        string()
            .duration()
            .validate(&json!("not-duration"))
            .is_err()
    );

    // 4. Sanitizers edge branches
    let str_san = string()
        .trim_start()
        .trim_end()
        .escape_html()
        .normalize_email();
    let res = str_san
        .validate(&json!("   <user>+tag@EXAMPLE.COM   "))
        .unwrap();
    assert_eq!(res, "&lt;user&gt;+tag@example.com");

    // 5. Cross-field confirmed rule
    let conf_schema = object()
        .field("pwd", string())
        .field("pwd_confirmation", string())
        .confirmed("pwd");
    assert!(
        conf_schema
            .validate(&json!({"pwd": "123", "pwd_confirmation": "123"}))
            .is_ok()
    );
    assert!(
        conf_schema
            .validate(&json!({"pwd": "123", "pwd_confirmation": "456"}))
            .is_err()
    );
    assert!(conf_schema.validate(&json!({"pwd": "123"})).is_err());

    // 6. DateSchema non-bail and format branches
    let dt_wknd_no_bail = date().date_only().weekend();
    assert!(dt_wknd_no_bail.validate(&json!("2026-11-20")).is_err());
    let dt_wkdy_no_bail = date().date_only().weekday();
    assert!(dt_wkdy_no_bail.validate(&json!("2026-11-21")).is_err());
    assert!(date().validate(&json!("bad-dt")).is_err());
    assert!(date().date_only().validate(&json!("bad-d")).is_err());
    assert!(date().time_only().validate(&json!("bad-t")).is_err());
    assert!(date().duration().validate(&json!("bad-dur")).is_err());

    // 7. Array async non-bail and edge cases
    let arr_non_bail = array(string())
        .not_empty()
        .min_items(2)
        .max_items(3)
        .exact_items(2)
        .distinct()
        .contains(json!("item1"))
        .contains_all([json!("item1"), json!("item2")])
        .contains_any([json!("item1")])
        .bail(false);
    assert!(arr_non_bail.validate_async(&json!([])).await.is_err());
    assert!(
        arr_non_bail
            .validate_async(&json!(["other", "other"]))
            .await
            .is_err()
    );

    let arr_subset_non_str = array(number()).subset_of(&["tag"]).bail(false);
    assert!(
        arr_subset_non_str
            .validate_async(&json!([123]))
            .await
            .is_err()
    );
    assert!(arr_subset_non_str.validate(&json!([123])).is_err());

    let arr_contains_bail = array(string()).contains(json!("need")).bail(true);
    assert!(
        arr_contains_bail
            .validate_async(&json!(["nope"]))
            .await
            .is_err()
    );
    assert!(arr_contains_bail.validate(&json!(["nope"])).is_err());

    let arr_contains_all_bail = array(string())
        .contains_all([json!("a"), json!("b")])
        .bail(true);
    assert!(
        arr_contains_all_bail
            .validate_async(&json!(["a"]))
            .await
            .is_err()
    );
    assert!(arr_contains_all_bail.validate(&json!(["a"])).is_err());

    let arr_contains_any_bail = array(string()).contains_any([json!("x")]).bail(true);
    assert!(
        arr_contains_any_bail
            .validate_async(&json!(["y"]))
            .await
            .is_err()
    );
    assert!(arr_contains_any_bail.validate(&json!(["y"])).is_err());

    // 8. Tuple async non-bail and bail coverage
    let t2_async = tuple2(string().email(), number().min(0.0)).bail(false);
    assert!(
        t2_async
            .validate_async(&json!(["bad-email", -10.0]))
            .await
            .is_err()
    );
    assert!(
        tuple2(string(), number())
            .bail(true)
            .validate_async(&json!(["ok", "bad"]))
            .await
            .is_err()
    );

    let t3_async = tuple3(string().email(), number().min(0.0), boolean()).bail(false);
    assert!(
        t3_async
            .validate_async(&json!(["bad-email", -10.0, "not-bool"]))
            .await
            .is_err()
    );
    assert!(
        tuple3(string(), number(), boolean())
            .bail(true)
            .validate_async(&json!(["ok", "bad", true]))
            .await
            .is_err()
    );
    assert!(
        tuple3(string(), number(), boolean())
            .bail(true)
            .validate_async(&json!(["ok", 10.0, "bad"]))
            .await
            .is_err()
    );
    assert!(
        tuple3(string(), number(), boolean())
            .bail(true)
            .validate(&json!(["ok", "bad", true]))
            .is_err()
    );
    assert!(
        tuple3(string(), number(), boolean())
            .bail(true)
            .validate(&json!(["ok", 10.0, "bad"]))
            .is_err()
    );

    let t4_async = tuple4(string().email(), number().min(0.0), boolean(), integer()).bail(false);
    assert!(
        t4_async
            .validate_async(&json!(["bad-email", -10.0, "not-bool", "not-int"]))
            .await
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate_async(&json!(["ok", "bad", true, 1]))
            .await
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate_async(&json!(["ok", 10.0, "bad", 1]))
            .await
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate_async(&json!(["ok", 10.0, true, "bad"]))
            .await
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate(&json!(["ok", "bad", true, 1]))
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate(&json!(["ok", 10.0, "bad", 1]))
            .is_err()
    );
    assert!(
        tuple4(string(), number(), boolean(), integer())
            .bail(true)
            .validate(&json!(["ok", 10.0, true, "bad"]))
            .is_err()
    );

    // 9. Tuple alias, with_message, and non-array mismatch
    let t_alias = tuple(string(), integer());
    assert!(t_alias.validate(&json!(["a", 1])).is_ok());

    let t2_msg = tuple2(string(), integer()).with_message("custom-t2");
    assert_eq!(
        t2_msg
            .validate(&json!("not-arr"))
            .unwrap_err()
            .first_message(),
        Some("custom-t2")
    );
    assert!(t2_msg.validate_async(&json!("not-arr")).await.is_err());

    let t3_msg = tuple3(string(), integer(), boolean()).with_message("custom-t3");
    assert_eq!(
        t3_msg
            .validate(&json!("not-arr"))
            .unwrap_err()
            .first_message(),
        Some("custom-t3")
    );
    assert!(t3_msg.validate_async(&json!("not-arr")).await.is_err());

    let t4_msg = tuple4(string(), integer(), boolean(), number()).with_message("custom-t4");
    assert_eq!(
        t4_msg
            .validate(&json!("not-arr"))
            .unwrap_err()
            .first_message(),
        Some("custom-t4")
    );
    assert!(t4_msg.validate_async(&json!("not-arr")).await.is_err());

    // 10. String Schema configuration methods & granular branches
    let str_default = <jachai::schema::StringSchema as std::default::Default>::default();
    assert!(str_default.validate(&json!("hello")).is_ok());

    let str_no_trim = string().trim(false);
    assert_eq!(str_no_trim.validate(&json!("  hi  ")).unwrap(), "  hi  ");

    let str_empty_keep = string().empty_string_as_null(false);
    assert_eq!(str_empty_keep.validate(&json!("")).unwrap(), "");

    let str_cases = string().to_lowercase().to_uppercase();
    assert_eq!(str_cases.validate(&json!("mixed")).unwrap(), "MIXED");

    // Bitcoin address branches (Bech32 & Base58Check)
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"))
            .is_ok()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2"))
            .is_ok()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"))
            .is_ok()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("10OIl0000000000000000000000000000"))
            .is_err()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("bc1q111111111111111111111111111111"))
            .is_err()
    );
    assert!(
        string()
            .bitcoin_address()
            .validate(&json!("2invalidbtcaddress1234567890123"))
            .is_err()
    );
    assert!(string().bitcoin_address().validate(&json!("")).is_err());

    // EU VAT branches
    assert!(string().eu_vat().validate(&json!("DE123456789")).is_ok());
    assert!(string().eu_vat().validate(&json!("DE12")).is_err());

    // EIN branches (no hyphen)
    assert!(string().ein().validate(&json!("123456789")).is_ok());
    assert!(string().ein().validate(&json!("12345678A")).is_err());

    // SSN branches (no hyphen & invalid prefixes)
    assert!(string().ssn().validate(&json!("123456789")).is_ok());
    assert!(string().ssn().validate(&json!("12345678A")).is_err());
    assert!(string().ssn().validate(&json!("901234567")).is_err());
    assert!(string().ssn().validate(&json!("000123456")).is_err());
    assert!(string().ssn().validate(&json!("666123456")).is_err());
    assert!(string().ssn().validate(&json!("123004567")).is_err());
    assert!(string().ssn().validate(&json!("123450000")).is_err());

    // ISSN branches
    assert!(string().issn().validate(&json!("0378-5955")).is_ok());
    assert!(string().issn().validate(&json!("123A-5678")).is_err());
    assert!(string().issn().validate(&json!("1234-5670")).is_err());

    // HSL/HSLA percentage branches
    assert!(string().hsl().validate(&json!("hsl(180, 50, 50)")).is_err());
    assert!(
        string()
            .hsl()
            .validate(&json!("hsl(180, bad%, 50%)"))
            .is_err()
    );
    assert!(
        string()
            .hsl()
            .validate(&json!("hsl(180, 150%, 50%)"))
            .is_err()
    );
    assert!(
        string()
            .hsla()
            .validate(&json!("hsla(180, 50, 50, 0.5)"))
            .is_err()
    );
    assert!(
        string()
            .hsla()
            .validate(&json!("hsla(180, 50%, bad%, 0.5)"))
            .is_err()
    );

    // 11. Record, Enum, and Nullable edge coverage
    let rec_msg = record(string()).with_message("custom-rec");
    assert_eq!(
        rec_msg.validate(&json!(null)).unwrap_err().first_message(),
        Some("custom-rec")
    );
    assert_eq!(
        rec_msg.validate(&json!(123)).unwrap_err().first_message(),
        Some("custom-rec")
    );
    assert_eq!(
        rec_msg
            .validate_async(&json!(null))
            .await
            .unwrap_err()
            .first_message(),
        Some("custom-rec")
    );
    assert_eq!(
        rec_msg
            .validate_async(&json!(123))
            .await
            .unwrap_err()
            .first_message(),
        Some("custom-rec")
    );

    let enum_default = <jachai::schema::EnumSchema<String> as std::default::Default>::default();
    assert!(enum_default.validate(&json!(123)).is_err());

    let double_null_json = string().nullable().nullable().to_json_schema();
    assert!(double_null_json["type"].is_array());

    // 12. Default trait implementations across scalar schemas
    let num_default = <jachai::schema::number::NumberSchema as std::default::Default>::default();
    assert!(num_default.validate(&json!(42.0)).is_ok());

    let int_default = <jachai::schema::number::IntegerSchema as std::default::Default>::default();
    assert!(int_default.validate(&json!(42)).is_ok());

    let bool_default = <jachai::schema::boolean::BooleanSchema as std::default::Default>::default();
    assert!(bool_default.validate(&json!(true)).is_ok());

    let dt_default = <jachai::DateSchema as std::default::Default>::default();
    assert!(dt_default.validate(&json!("2026-11-22T10:00:00Z")).is_ok());

    // 13. Duration and Time deep branch coverage
    let dur_schema = date().duration();
    assert!(dur_schema.validate(&json!("P4W")).is_ok());
    assert!(dur_schema.validate(&json!("PT1.5S")).is_ok());
    assert!(dur_schema.validate(&json!("PW")).is_err());
    assert!(dur_schema.validate(&json!("P1YT")).is_err());
    assert!(dur_schema.validate(&json!("P1")).is_err());
    assert!(dur_schema.validate(&json!("P1X")).is_err());
    assert!(dur_schema.validate(&json!("PT1")).is_err());
    assert!(dur_schema.validate(&json!("PT1X")).is_err());
    assert!(dur_schema.validate(&json!("P")).is_err());

    let time_schema = date().time_only();
    assert!(time_schema.validate(&json!("12:30:00.")).is_err());
    assert!(time_schema.validate(&json!("12:30:00.123")).is_ok());
    assert!(time_schema.validate(&json!("12:30:00z")).is_ok());
    assert!(time_schema.validate(&json!("12:30:00-05:00")).is_ok());
    assert!(time_schema.validate(&json!("12:30:00+25:00")).is_err());
    assert!(time_schema.validate(&json!("12:30:00+05:65")).is_err());
}

#[tokio::test]
async fn test_coverage_zero_alloc_check_and_in_place_paths() {
    // 1. Error & RuleParams serialization, deserialization, iter, insert overwrite, remove missing
    let mut params = RuleParams::new();
    params.insert("field", "username");
    params.insert("field", "user_id"); // overwrite
    params.insert("min", "5");
    assert_eq!(params.get("field"), Some("user_id"));
    assert_eq!(params.remove("nonexistent"), None);
    assert_eq!(params.remove("min"), Some("5".to_string()));
    assert_eq!(params.iter().count(), 1);

    let serialized = serde_json::to_string(&params).unwrap();
    assert!(serialized.contains("user_id"));
    let deserialized: RuleParams = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.get("field"), Some("user_id"));
    assert!(serde_json::from_str::<RuleParams>("123").is_err());

    // 2. Boolean check, check_bool, validate_bool, check_path
    let bool_s = boolean();
    assert!(bool_s.check_bool(true).is_ok());
    assert!(bool_s.check_bool(false).is_ok());
    assert!(bool_s.validate_bool(true).unwrap());
    assert!(!bool_s.validate_bool(false).unwrap());
    assert!(bool_s.check(&json!(true)).is_ok());
    assert!(bool_s.check(&json!(false)).is_ok());
    assert!(bool_s.check(&json!(null)).is_err());
    assert!(bool_s.check(&json!("invalid")).is_err());

    let accepted_s = boolean().accepted();
    assert!(accepted_s.check_bool(true).is_ok());
    assert!(accepted_s.check_bool(false).is_err());
    assert!(accepted_s.validate_bool(true).is_ok());
    assert!(accepted_s.validate_bool(false).is_err());
    assert!(accepted_s.check(&json!(true)).is_ok());
    assert!(accepted_s.check(&json!(false)).is_err());

    // 3. Integer check, check_i64, validate_i64
    let int_s = integer().between(10, 100).positive();
    assert!(int_s.check_i64(50).is_ok());
    assert!(int_s.check_i64(5).is_err());
    assert!(int_s.check_i64(150).is_err());
    assert_eq!(int_s.validate_i64(50).unwrap(), 50);
    assert!(int_s.validate_i64(5).is_err());
    assert!(int_s.check(&json!(50)).is_ok());
    assert!(int_s.check(&json!(5)).is_err());
    assert!(int_s.check(&json!(null)).is_err());
    assert!(int_s.check(&json!("not-num")).is_err());

    let neg_int = integer().negative().non_positive();
    assert!(neg_int.check_i64(-5).is_ok());
    assert!(neg_int.check_i64(0).is_err());
    let non_neg = integer().non_negative();
    assert!(non_neg.check_i64(0).is_ok());

    // 4. Number check, check_f64, validate_f64, step, integer, finite
    let num_s = number().between(10.0, 100.0).multiple_of(2.5);
    assert!(num_s.check_f64(15.0).is_ok());
    assert!(num_s.check_f64(16.0).is_err()); // not a multiple of 2.5
    assert!(num_s.check_f64(5.0).is_err()); // below min
    assert!(num_s.check_f64(105.0).is_err()); // above max
    assert_eq!(num_s.validate_f64(15.0).unwrap(), 15.0);
    assert!(num_s.validate_f64(16.0).is_err());
    assert!(num_s.check(&json!(15.0)).is_ok());
    assert!(num_s.check(&json!(16.0)).is_err());
    assert!(num_s.check(&json!(null)).is_err());
    assert!(num_s.check(&json!("str")).is_err());

    let step_zero = number().multiple_of(0.0);
    assert!(step_zero.check_f64(10.0).is_err());
    let num_int = number().int().positive();
    assert!(num_int.check_f64(10.0).is_ok());
    assert!(num_int.check_f64(10.5).is_err());
    let num_neg = number().negative().non_positive();
    assert!(num_neg.check_f64(-1.0).is_ok());
    assert!(num_neg.check_f64(0.0).is_err());
    let num_nonneg = number().non_negative();
    assert!(num_nonneg.check_f64(0.0).is_ok());

    // 5. String check, check_str, validate_str, transforms, empty_as_null
    let str_s = string().min_len(3).max_len(10).trim(true).to_lowercase();
    assert!(str_s.check_str("  HELLO  ").is_ok());
    assert!(str_s.check_str("HI").is_err());
    assert_eq!(str_s.validate_str("  HELLO  ").unwrap(), "hello");
    assert!(str_s.validate_str("HI").is_err());
    assert!(str_s.check(&json!("  HELLO  ")).is_ok());
    assert!(str_s.check(&json!("HI")).is_err());
    assert!(str_s.check(&json!(null)).is_err());
    assert!(str_s.check(&json!(123)).is_err());

    let trim_cases = string()
        .trim_start()
        .trim_end()
        .to_uppercase()
        .normalize_email()
        .escape_html();
    assert_eq!(
        trim_cases
            .validate_str("  user.name+tag@gmail.com  ")
            .unwrap(),
        "USER.NAME+TAG@gmail.com"
    );
    assert_eq!(
        trim_cases.validate_str("  <script>  ").unwrap(),
        "&lt;SCRIPT&gt;"
    );

    let empty_null_s = string().empty_to_null();
    assert!(empty_null_s.check_str("").is_err());
    assert!(empty_null_s.check_str("valid").is_ok());
    assert!(empty_null_s.validate_str("").is_err());
    assert!(empty_null_s.check(&json!("")).is_err());

    let alpha_s = string().alpha();
    assert!(alpha_s.check_str("abc").is_ok());
    assert!(alpha_s.check_str("123").is_err());
    assert!(alpha_s.check_str("").is_err());
    let alnum_s = string().alphanumeric();
    assert!(alnum_s.check_str("abc123").is_ok());
    assert!(alnum_s.check_str("abc!").is_err());
    assert!(alnum_s.check_str("").is_err());
    let ascii_s = string().ascii();
    assert!(ascii_s.check_str("hello").is_ok());
    assert!(ascii_s.check_str("🦀").is_err());

    // String fast predicates error branches
    assert!(string().ipv4().check_str("").is_err());
    assert!(string().ipv4().check_str("1.2.3").is_err());
    assert!(string().ipv4().check_str("1.2.3.4.5").is_err());
    assert!(string().ipv4().check_str("1.2.3.256").is_err());
    assert!(string().ipv4().check_str("1.2.3.01").is_err());
    assert!(string().ipv4().check_str("1.2.3.a").is_err());
    assert!(string().ipv4().check_str("999.999.999.999").is_err());

    assert!(string().uuid().check_str("short").is_err());
    assert!(
        string()
            .uuid()
            .check_str("123456789012345678901234567890123456")
            .is_err()
    );
    assert!(
        string()
            .uuid()
            .check_str("123e4567-e89b-02d3-a456-426614174000")
            .is_err()
    ); // version 0
    assert!(
        string()
            .uuid()
            .check_str("123e4567-e89b-12d3-0456-426614174000")
            .is_err()
    ); // variant 0
    assert!(
        string()
            .uuid()
            .check_str("123e4567-e89b-12d3-a456-42661417400z")
            .is_err()
    ); // invalid hex

    assert!(string().email().check_str("").is_err());
    assert!(string().email().check_str("no-at").is_err());
    assert!(string().email().check_str("@domain.com").is_err());
    assert!(string().email().check_str("a@").is_err());
    assert!(string().email().check_str("a@b").is_err());
    assert!(string().email().check_str("a@b..c").is_err());
    assert!(string().email().check_str("a@-b.com").is_err());
    assert!(string().email().check_str("a@b-.com").is_err());

    #[cfg(feature = "schema-color")]
    {
        assert!(string().rgb().check_str("rgb(255, 0)").is_err());
        assert!(string().rgb().check_str("rgb(256, 0, 0)").is_err());
        assert!(string().rgb().check_str("rgb(-1, 0, 0)").is_err());
        assert!(string().rgb().check_str("rgb(a, 0, 0)").is_err());
        assert!(string().rgba().check_str("rgba(255, 0, 0, 1.5)").is_err());
        assert!(string().rgba().check_str("rgba(255, 0, 0, -0.5)").is_err());
        assert!(string().rgba().check_str("rgba(255, 0, 0, abc)").is_err());
        assert!(string().rgba().check_str("rgba(255, 0, 0)").is_err());
        assert!(string().hsl().check_str("hsl(400, 50%, 50%)").is_err());
        assert!(string().hsl().check_str("hsl(200, 150%, 50%)").is_err());
        assert!(string().hsl().check_str("hsl(200, 50%, 150%)").is_err());
        assert!(
            string()
                .hsla()
                .check_str("hsla(200, 50%, 50%, 2.0)")
                .is_err()
        );
        assert!(string().hsla().check_str("hsla(200, 50%, 50%)").is_err());
    }

    // 6. Array check, not_empty, bounds, distinct, subset, contains
    let arr_s = array(integer())
        .not_empty()
        .min_items(2)
        .max_items(4)
        .exact_items(3)
        .distinct();
    assert!(arr_s.check(&json!([1, 2, 3])).is_ok());
    assert!(arr_s.check(&json!([])).is_err());
    assert!(arr_s.check(&json!([1])).is_err());
    assert!(arr_s.check(&json!([1, 2, 3, 4])).is_err());
    assert!(arr_s.check(&json!([1, 1, 3])).is_err());
    assert!(arr_s.check(&json!([1, "not-num", 3])).is_err());
    assert!(arr_s.check(&json!(null)).is_err());
    assert!(arr_s.check(&json!(123)).is_err());

    // Array distinct > 16 items
    let large_distinct: Vec<i64> = (0..20).collect();
    assert!(
        array(integer())
            .distinct()
            .check(&json!(large_distinct))
            .is_ok()
    );
    let mut large_dup = large_distinct.clone();
    large_dup[19] = 0;
    assert!(
        array(integer())
            .distinct()
            .check(&json!(large_dup))
            .is_err()
    );

    // Array subset and contains in check
    let arr_subset = array(string())
        .subset_of(&["a", "b", "c"])
        .contains("a")
        .contains_all(vec![json!("a"), json!("b")])
        .contains_any(vec![json!("a"), json!("b")]);
    assert!(arr_subset.check(&json!(["a", "b"])).is_ok());
    assert!(arr_subset.check(&json!(["a", "d"])).is_err());
    assert!(arr_subset.check(&json!(["b", "c"])).is_err()); // missing "a"
    assert!(arr_subset.check(&json!(["a", "c"])).is_err()); // missing "b"

    // 7. Tuple check: native tuples and macro tuples
    let native_tuple = (string(), integer());
    assert!(native_tuple.check(&json!(["hello", 42])).is_ok());
    assert!(native_tuple.check(&json!(["hello"])).is_err());
    assert!(native_tuple.check(&json!(123)).is_err());
    assert!(native_tuple.check(&json!(["hello", "not-int"])).is_err());

    let obj_with_native = object().field("tup", (string(), integer()));
    assert!(
        obj_with_native
            .check(&json!({ "tup": ["hello", 42] }))
            .is_ok()
    );
    assert!(
        obj_with_native
            .check(&json!({ "tup": ["hello", "bad"] }))
            .is_err()
    );

    let t2 = tuple2(string(), integer());
    assert!(t2.check(&json!(["a", 1])).is_ok());
    assert!(t2.check(&json!(["a"])).is_err());
    assert!(t2.check(&json!(123)).is_err());
    assert!(t2.check(&json!(["a", "bad"])).is_err());

    let t3 = tuple3(string(), integer(), boolean());
    assert!(t3.check(&json!(["a", 1, true])).is_ok());
    assert!(t3.check(&json!(["a", 1])).is_err());
    assert!(t3.check(&json!(["a", 1, "not-bool"])).is_err());

    let t4 = tuple4(string(), integer(), boolean(), number());
    assert!(t4.check(&json!(["a", 1, true, 3.15])).is_ok());
    assert!(t4.check(&json!(["a", 1, true])).is_err());

    // 8. Union check: union2, union3, union4
    let u2 = union2(string().min_len(5), string().email());
    assert!(u2.check(&json!("hello")).is_ok());
    assert!(u2.check(&json!("a@b.com")).is_ok());
    assert!(u2.check(&json!("hi")).is_err());

    let u3 = union3(
        string().min_len(5),
        string().email(),
        string().starts_with("prefix_"),
    );
    assert!(u3.check(&json!("prefix_test")).is_ok());
    assert!(u3.check(&json!("bad")).is_err());

    let u4 = union4(
        string().min_len(5),
        string().email(),
        string().starts_with("p_"),
        string().ends_with("_s"),
    );
    assert!(u4.check(&json!("test_s")).is_ok());
    assert!(u4.check(&json!("bad")).is_err());

    // 9. Record check
    let rec_s = record(integer());
    assert!(rec_s.check(&json!({ "x": 1, "y": 2 })).is_ok());
    assert!(rec_s.check(&json!({ "x": 1, "y": "bad" })).is_err());
    assert!(rec_s.check(&json!(null)).is_err());
    assert!(rec_s.check(&json!(123)).is_err());

    // 10. Enum check
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    enum RoleTest {
        Admin,
        User,
    }
    impl std::str::FromStr for RoleTest {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "admin" => Ok(RoleTest::Admin),
                "user" => Ok(RoleTest::User),
                _ => Err("invalid".to_string()),
            }
        }
    }
    let role_s = enum_of::<RoleTest>();
    assert!(role_s.check(&json!("admin")).is_ok());
    assert!(role_s.check(&json!("invalid")).is_err());
    assert!(role_s.check(&json!(null)).is_err());
    assert!(role_s.check(&json!(123)).is_err());

    // 11. Date check
    let dt_s = date();
    assert!(dt_s.check(&json!("2026-11-23T08:00:00Z")).is_ok());
    assert!(dt_s.check(&json!("invalid")).is_err());
    assert!(dt_s.check(&json!(null)).is_err());
    assert!(dt_s.check(&json!(123)).is_err());

    let d_s = date().date_only();
    assert!(d_s.check(&json!("2026-11-23")).is_ok());
    assert!(d_s.check(&json!("invalid")).is_err());

    let tm_s = date().time_only();
    assert!(tm_s.check(&json!("12:30:00Z")).is_ok());
    assert!(tm_s.check(&json!("invalid")).is_err());

    // 12. Optional, Nullable, Default check
    let opt_s = string().optional();
    assert!(opt_s.check(&json!(null)).is_ok());
    assert!(opt_s.check(&json!("val")).is_ok());
    assert!(opt_s.check(&json!(123)).is_err());

    let null_s = string().nullable();
    assert!(null_s.check(&json!(null)).is_ok());
    assert!(null_s.check(&json!("val")).is_ok());
    assert!(null_s.check(&json!(123)).is_err());

    let def_s = string().default_value("def".to_string());
    assert!(def_s.check(&json!(null)).is_ok());
    assert!(def_s.check(&json!("val")).is_ok());
    assert!(def_s.check(&json!(123)).is_err());

    // 13. Object check
    let obj_s = object()
        .field("name", string().min_len(2))
        .field("score", integer().positive())
        .strict()
        .cross_refine(&["name"], |_| true, "rule", "msg");
    assert!(
        obj_s
            .check(&json!({ "name": "alice", "score": 10 }))
            .is_ok()
    );
    assert!(
        obj_s
            .check(&json!({ "name": "alice", "score": 10, "extra": 1 }))
            .is_err()
    );
    assert!(obj_s.check(&json!({ "name": "a", "score": 10 })).is_err());
    assert!(obj_s.check(&json!(null)).is_err());
    assert!(obj_s.check(&json!("str")).is_err());

    // Object typed_to_value numbers
    let obj_num = object().field("f", number()).field("i", integer());
    let val_num = obj_num.validate(&json!({ "f": 1.25, "i": 42 })).unwrap();
    assert_eq!(val_num["f"], 1.25);
    assert_eq!(val_num["i"], 42);

    // 14. Request check_request, check_request_with_context, and coercer
    let req_s = request()
        .params(object().field("id", integer()))
        .query(object().field("q", string()))
        .coercer(jachai::coercion::Coercer::new().empty_strings_to_null(true));

    let parts = RequestParts::new()
        .with_params(json!({ "id": 1 }))
        .with_query(json!({ "q": "search" }))
        .with_headers(json!({}))
        .with_cookies(json!({}))
        .with_body(json!({}));
    assert!(req_s.check_request(&parts).is_ok());

    let ctx = ValidationContext::new();
    assert!(
        req_s
            .check_request_with_context(&parts, &ctx, jachai::messages::default_messages_provider())
            .is_ok()
    );

    let bad_parts = RequestParts::new().with_params(json!({ "id": "not-int" }));
    assert!(req_s.check_request(&bad_parts).is_err());

    // Schema trait check on RequestSchema
    let req_as_schema: &dyn Schema<ValidatedRequest> = &req_s;
    assert!(
        req_as_schema
            .check(&json!({ "params": { "id": 1 }, "query": { "q": "test" } }))
            .is_ok()
    );
    assert!(
        req_as_schema
            .check(&json!({ "params": { "id": "bad" } }))
            .is_err()
    );

    // RequestParts conversions
    let null_parts = RequestParts::from(serde_json::Value::Null);
    assert_eq!(null_parts.params, serde_json::Value::Null);
    let mut parts_map = serde_json::Map::new();
    parts_map.insert("params".to_string(), json!({ "id": 99 }));
    let from_map = RequestParts::from(serde_json::Value::Object(parts_map));
    assert_eq!(from_map.params["id"], 99);

    // RequestSchema validate_async failure with path
    let async_req = request().params(object().field(
        "id",
        string().refine_async(|id| {
            let v = id.to_string();
            Box::pin(async move {
                if v == "valid" {
                    Ok(())
                } else {
                    Err("must be valid".to_string())
                }
            })
        }),
    ));
    let async_bad = RequestParts::new().with_params(json!({ "id": "invalid" }));
    let async_err = async_req.validate_async(&async_bad).await.unwrap_err();
    assert!(async_err.field_errors().contains_key("params.id"));
    let async_bad = RequestParts::new().with_params(json!({ "id": -10 }));
    let async_err = async_req.validate_async(&async_bad).await.unwrap_err();
    assert!(async_err.field_errors().contains_key("params.id"));

    // 15. Messages edge cases
    let custom_msgs = SimpleMessagesProvider::new();
    let empty_rendered = custom_msgs.format_message("", "unknown", &RuleParams::new(), None);
    assert!(empty_rendered.contains("value"));
}

#[tokio::test]
async fn test_coverage_maximum_sweep() {
    use jachai::messages::{default_messages_provider, humanize_field_name};

    // 1. messages.rs
    assert_eq!(humanize_field_name(""), "value");
    let provider = default_messages_provider();
    let rendered_unknown = provider.format_message(
        "user_email",
        "custom_rule",
        &RuleParams::new(),
        Some("Invalid :placeholder for :field"),
    );
    assert!(rendered_unknown.contains(":placeholder"));
    assert!(rendered_unknown.contains("user email"));

    // 2. request.rs
    let parts_full = RequestParts::new()
        .with_params(json!({ "id": 1 }))
        .with_query(json!({ "page": 2 }))
        .with_headers(json!({ "authorization": "Bearer token" }))
        .with_cookies(json!({ "session": "xyz" }))
        .with_body(json!({ "data": "payload" }));
    let validated_full = request()
        .params(object().field("id", integer()))
        .query(object().field("page", integer()))
        .headers(object().field("authorization", string()))
        .cookies(object().field("session", string()))
        .body(object().field("data", string()))
        .validate(&parts_full)
        .unwrap();
    let parts_val = validated_full.to_value();
    assert!(parts_val.get("headers").is_some());
    assert!(parts_val.get("cookies").is_some());
    assert!(parts_val.get("body").is_some());

    let parts_non_obj = RequestParts::from(&json!("not_an_object"));
    assert_eq!(parts_non_obj.params, serde_json::Value::Null);

    let req_with_coercer_cleared = request().sanitize(true).sanitize(false);
    assert!(
        req_with_coercer_cleared
            .check_request(&RequestParts::default())
            .is_ok()
    );

    let req_bail = request()
        .params(object().field("id", integer().min(10)))
        .bail(true);
    let bad_parts = RequestParts::new().with_params(json!({ "id": 2 }));
    assert!(req_bail.check_request(&bad_parts).is_err());

    // Request parts downcast to String and serde_json::to_value fallback
    let req_typed = request()
        .params(string().min_len(2))
        .query(integer().min(1));
    let typed_parts = RequestParts::new()
        .with_params(json!("hello"))
        .with_query(json!(5));
    let validated_typed = req_typed.validate(&typed_parts).unwrap();
    assert_eq!(validated_typed.params.unwrap(), json!("hello"));
    assert_eq!(validated_typed.query.unwrap(), json!(5));

    // Request async validation with path and errors
    let ctx = ValidationContext::new();
    let req_schema_nested: &dyn Schema<ValidatedRequest> = &req_typed;
    let nested_req_err = req_schema_nested
        .validate_async_path(
            &json!({ "params": "x", "query": 0 }),
            "envelope",
            &ctx,
            None,
            default_messages_provider(),
        )
        .await
        .unwrap_err();
    assert!(
        nested_req_err
            .issues()
            .iter()
            .any(|i| i.field.starts_with("envelope."))
    );

    let req_bail_schema: &dyn Schema<ValidatedRequest> = &req_bail;
    assert!(
        req_bail_schema
            .check_path(
                &json!({ "params": { "id": 1 } }),
                "root",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_err()
    );

    // 3. schema/array.rs
    let nested_arr_obj = object().field("items", array(integer().min(10)));
    let arr_err = nested_arr_obj
        .validate(&json!({ "items": [2] }))
        .unwrap_err();
    assert_eq!(arr_err.issues()[0].field, "items[0]");

    assert!(
        array(integer())
            .bail(true)
            .not_empty()
            .check(&json!([]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .min_items(3)
            .check(&json!([1, 2]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .max_items(1)
            .check(&json!([1, 2]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .exact_items(2)
            .check(&json!([1]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .distinct()
            .check(&json!([1, 1]))
            .is_err()
    );
    assert!(
        array(string())
            .subset_of(&["a", "b"])
            .bail(true)
            .check(&json!(["a", "c"]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .contains(json!(10))
            .check(&json!([1, 2]))
            .is_err()
    );
    assert!(
        array(integer())
            .bail(true)
            .contains_all(vec![json!(1), json!(2)])
            .check(&json!([1, 3]))
            .is_err()
    );

    let items_17: Vec<serde_json::Value> = (0..16)
        .map(|i| json!(i))
        .chain(std::iter::once(json!(0)))
        .collect();
    assert!(
        array(integer())
            .distinct()
            .bail(true)
            .validate(&serde_json::Value::Array(items_17))
            .is_err()
    );

    // 4. schema/boolean.rs
    assert!(boolean().check(&json!("true")).is_ok());
    assert!(boolean().check(&json!(1)).is_ok());

    // 5. schema/date.rs
    assert!(date().duration().check(&json!("P1Y2M3D")).is_ok());
    assert!(date().duration().check(&json!("P")).is_err());
    assert!(date().check(&json!("2026-03-30T12:00:00+05:00")).is_ok());

    // 6. schema/number.rs
    assert!(number().finite().check(&json!(f64::INFINITY)).is_err());
    assert!(number().check(&json!("987.65")).is_ok());
    assert!(number().check(&json!(true)).is_err());

    let num_all_pred = number()
        .min(5.0)
        .max(10.0)
        .finite()
        .safe_integer()
        .latitude()
        .longitude();
    assert!(num_all_pred.check_f64(7.0).is_ok());

    assert!(integer().min(10).check_i64(5).is_err());
    assert!(integer().max(10).check_i64(15).is_err());
    assert!(integer().range(5, 10).check_i64(15).is_err());
    assert!(integer().positive().check_i64(-5).is_err());
    assert!(integer().negative().check_i64(5).is_err());
    assert!(integer().non_positive().check_i64(5).is_err());
    assert!(integer().non_negative().check_i64(-5).is_err());
    assert!(integer().multiple_of(5).check_i64(7).is_err());

    // 7. schema/object.rs
    let nested_obj = object().field("inner", object().field("tag", string()));
    let nested_res = nested_obj
        .validate(&json!({ "inner": { "tag": "jachai" } }))
        .unwrap();
    assert_eq!(nested_res["inner"]["tag"], "jachai");

    #[derive(Clone)]
    struct CustomU64Schema;
    impl Schema<u64> for CustomU64Schema {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<u64, ValidationError> {
            let _ = (value, path, context, parent_data, messages);
            Ok(999u64)
        }
    }
    let u64_obj = object().field("count", CustomU64Schema);
    let u64_res = u64_obj.validate(&json!({ "count": 999 })).unwrap();
    assert_eq!(u64_res["count"], 999);

    let conf_obj = object()
        .field("pwd", string())
        .field("pwd_confirmation", string())
        .field("other", string())
        .confirmed("pwd");
    let picked_conf = conf_obj.clone().pick(&["pwd", "pwd_confirmation"]);
    let omitted_conf = conf_obj.omit(&["other"]);
    assert!(
        picked_conf
            .validate(&json!({ "pwd": "secret", "pwd_confirmation": "secret" }))
            .is_ok()
    );
    assert!(
        omitted_conf
            .validate(&json!({ "pwd": "secret", "pwd_confirmation": "secret" }))
            .is_ok()
    );

    let cross_obj = object()
        .field("k1", string())
        .field("k2", string())
        .field("extra", string())
        .cross_refine(
            &["k1", "k2"],
            |map| map.get("k1") == map.get("k2"),
            "match",
            "must match",
        );
    let cross_picked = cross_obj.clone().pick(&["k1", "k2"]);
    let cross_omitted = cross_obj.omit(&["extra"]);
    assert!(
        cross_picked
            .validate(&json!({ "k1": "same", "k2": "same" }))
            .is_ok()
    );
    assert!(
        cross_omitted
            .validate(&json!({ "k1": "same", "k2": "same" }))
            .is_ok()
    );

    let coercer_obj = object()
        .field("name", string())
        .coercer(jachai::coercion::Coercer::new().empty_strings_to_null(true));
    assert!(coercer_obj.check(&json!({ "name": "valid" })).is_ok());
    assert!(
        coercer_obj
            .validate_async(&json!({ "name": "valid" }))
            .await
            .is_ok()
    );

    let obj_bail = object()
        .field("a", string().min_len(5))
        .field("b", string().min_len(5))
        .bail(true);
    assert!(obj_bail.check(&json!({ "a": "x", "b": "y" })).is_err());

    // 8. schema/tuple.rs
    assert!(
        (string(), integer())
            .validate_async(&json!(["only_first"]))
            .await
            .is_err()
    );
    let tuple_err = (string().min_len(5), integer())
        .validate_async_path(
            &json!(["a", 10]),
            "coords",
            &ctx,
            None,
            default_messages_provider(),
        )
        .await
        .unwrap_err();
    assert_eq!(tuple_err.issues()[0].field, "coords[0]");

    let t2_bail = tuple2(string().min_len(5), integer().min(10)).bail(true);
    let t2_err = t2_bail
        .check_path(
            &json!(["x", 2]),
            "pair",
            &ctx,
            None,
            default_messages_provider(),
        )
        .unwrap_err();
    eprintln!("T2_ERR: {:?}", t2_err.issues());
    assert_eq!(t2_err.issues().len(), 1);

    // 9. schema/optional.rs
    let nullable_json_schema = object().nullable().to_json_schema();
    assert!(nullable_json_schema.get("type").is_some());

    // 10. schema/record.rs
    let rec_bail = record(string().min_len(5)).bail(true);
    let rec_err = rec_bail
        .check_path(
            &json!({ "f1": "a", "f2": "b" }),
            "data",
            &ctx,
            None,
            default_messages_provider(),
        )
        .unwrap_err();
    assert_eq!(rec_err.issues().len(), 1);

    // 11. schema/union.rs
    assert!(
        union2(string().min_len(5), string().email())
            .check(&json!("hello"))
            .is_ok()
    );
    assert!(
        union2(string().min_len(10), string().email())
            .check(&json!("test@example.com"))
            .is_ok()
    );

    // 12. schema/file.rs
    assert!(
        file()
            .max_size(10)
            .bail(true)
            .validate(&json!({ "size": 100, "mime_type": "text/plain" }))
            .is_err()
    );

    // 13. schema/enum.rs
    #[derive(Debug, PartialEq, Eq)]
    enum StatusChoice {
        Active,
        Inactive,
    }
    impl std::str::FromStr for StatusChoice {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "active" => Ok(StatusChoice::Active),
                "inactive" => Ok(StatusChoice::Inactive),
                _ => Err("invalid status".to_string()),
            }
        }
    }
    let status_enum = enum_of::<StatusChoice>();
    assert!(status_enum.validate(&json!(123)).is_err());
    assert!(status_enum.validate(&json!("unknown")).is_err());

    // 14. schema/mod.rs (default check_path on Schema trait)
    #[derive(Clone)]
    struct CustomDefaultCheckSchema;
    impl Schema<i32> for CustomDefaultCheckSchema {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<i32, ValidationError> {
            let _ = (context, parent_data, messages);
            value
                .as_i64()
                .map(|v| v as i32)
                .ok_or_else(|| ValidationError::single(path, "int", "must be integer"))
        }
    }
    let default_trait_schema = CustomDefaultCheckSchema;
    assert!(default_trait_schema.check(&json!(42)).is_ok());
    assert!(default_trait_schema.check(&json!("not_num")).is_err());

    // 15. schema/string.rs
    // Email edge cases
    assert!(
        string()
            .email()
            .check_str(&format!("user@{}.com", "a".repeat(255)))
            .is_err()
    );
    assert!(string().email().check_str("user name@example.com").is_err());
    assert!(string().email().check_str("user@exam$ple.com").is_err());

    // IPv4 fast check edge cases
    assert!(string().ipv4().check_str("127.0.0.1.1").is_err());
    assert!(string().ipv4().check_str("127.0.0..1").is_err());
    assert!(string().ipv4().check_str("127.0.0.300").is_err());
    assert!(string().ipv4().check_str("127.0.0.01").is_err());
    assert!(string().ipv4().check_str("127.0.0.x").is_err());
    assert!(string().ipv4().check_str("127.0.0").is_err());
    assert!(string().ipv4().check_str("127.0.0.1").is_ok());

    // URL fast check edge cases
    assert!(string().url().check_str("ftp://ftp.example.com").is_ok());
    assert!(string().url().check_str("http://").is_err());
    assert!(string().url().check_str("http:///test").is_err());
    assert!(
        string()
            .url()
            .check_str("http://example.com/ space")
            .is_err()
    );

    // Fallback predicate in check_str
    assert!(
        string()
            .starts_with("prefix")
            .check_str("prefix_val")
            .is_ok()
    );
    assert!(string().regex(r"^\d+$").check_str("12345").is_ok());

    // Sanitization variations
    assert!(string().trim_start().check_str("  hi").is_ok());
    assert!(string().trim_end().check_str("hi  ").is_ok());
    assert!(string().to_uppercase().check_str("hi").is_ok());
    assert!(
        string()
            .normalize_email()
            .check_str("USER+TAG@GMAIL.COM")
            .is_ok()
    );
    assert!(string().escape_html().check_str("<b>text</b>").is_ok());

    assert_eq!(string().trim_start().validate_str("  hi").unwrap(), "hi");
    assert_eq!(string().trim_end().validate_str("hi  ").unwrap(), "hi");
    assert_eq!(string().to_uppercase().validate_str("hi").unwrap(), "HI");
    assert_eq!(
        string()
            .normalize_email()
            .validate_str("USER+TAG@GMAIL.COM")
            .unwrap(),
        "USER+TAG@gmail.com"
    );
    assert_eq!(
        string().escape_html().validate_str("<b>text</b>").unwrap(),
        "&lt;b&gt;text&lt;/b&gt;"
    );

    assert_eq!(
        string().trim_start().validate(&json!("  hi")).unwrap(),
        "hi"
    );
    assert_eq!(string().trim_end().validate(&json!("hi  ")).unwrap(), "hi");
    assert_eq!(
        string().to_uppercase().validate(&json!("hi")).unwrap(),
        "HI"
    );
    assert_eq!(
        string()
            .normalize_email()
            .validate(&json!("USER+TAG@GMAIL.COM"))
            .unwrap(),
        "USER+TAG@gmail.com"
    );
    assert_eq!(
        string()
            .escape_html()
            .validate(&json!("<b>text</b>"))
            .unwrap(),
        "&lt;b&gt;text&lt;/b&gt;"
    );

    assert!(string().min_len(10).bail(true).check_str("abc").is_err());
    assert!(string().min_len(10).bail(true).validate_str("abc").is_err());
    assert!(
        string()
            .min_len(10)
            .bail(true)
            .validate(&json!("abc"))
            .is_err()
    );

    // Other string rules
    assert!(string().domain().check_str("invalid_domain!.com").is_err());
    assert!(string().fqdn().check_str("invalid_fqdn!.com").is_err());
    assert!(
        string()
            .port()
            .check_str("999999999999999999999999")
            .is_err()
    );
    assert!(
        string()
            .cidr()
            .check_str("192.168.1.0/99999999999999")
            .is_err()
    );
    assert!(string().iban().check_str("GB82WEST12345698765430").is_err());
    assert!(string().isin().check_str("US037833100$").is_err());
    assert!(
        string()
            .rgba()
            .check_str("rgba(255, 255, 255, 0.5")
            .is_err()
    );
    assert!(
        string()
            .hsla()
            .check_str("hsla(180, 50%, 50%, 0.5")
            .is_err()
    );
    assert!(
        string()
            .hsla()
            .check_str("hsla(180, 50%, 50%, 0.5)")
            .is_ok()
    );
    assert!(
        string()
            .hsl()
            .check_str("hsl(invalid 50% 50% / 0.5)")
            .is_err()
    );
    assert!(
        string()
            .hsl()
            .check_str("hsl(notanumber, 50%, 50%)")
            .is_err()
    );
    assert!(
        string()
            .ethereum_address()
            .check_str("123456789012345678901234567890123456789012")
            .is_err()
    );
    assert!(string().bcp47().check_str(&"a".repeat(36)).is_err());
    assert!(string().cron().check_str("* * * * $").is_err());
    assert!(string().isbn10().check_str("047195869?").is_err());

    // 16. Fast match string predicates
    assert!(
        string()
            .datetime()
            .check_str("2026-03-30T12:00:00Z")
            .is_ok()
    );
    assert!(string().credit_card().check_str("4532015112830366").is_ok());
    assert!(
        string()
            .ulid()
            .check_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .is_ok()
    );
    assert!(string().hex_color().check_str("#ffffff").is_ok());
    assert!(string().slug().check_str("valid-slug-123").is_ok());
    assert!(string().semver().check_str("1.2.3").is_ok());
    assert!(string().base64().check_str("YQ==").is_ok());
    assert!(string().starts_with("a").check_str("abc").is_ok());
    assert!(string().ends_with("c").check_str("abc").is_ok());
    assert!(string().contains("b").check_str("abc").is_ok());
    assert!(string().alpha().check_str("abc").is_ok());
    assert!(string().alpha().check_str("abc1").is_err());
    assert!(string().alpha().check_str("é").is_ok());
    assert!(string().alphanumeric().check_str("abc1").is_ok());
    assert!(string().alphanumeric().check_str("é1").is_ok());
    assert!(string().ascii().check_str("abc").is_ok());

    // Color branches
    assert!(string().rgb().check_str("rgb(255, 255, 255, 0.5)").is_ok());
    assert!(string().rgb().check_str("rgb(not_num%, 50%, 50%)").is_err());
    assert!(
        string()
            .rgba()
            .check_str("rgba(255, 255, 255, not_num%)")
            .is_err()
    );
    assert!(string().rgba().check_str("rgba(255 255 255)").is_err());
    assert!(
        string()
            .hsla()
            .check_str("hsla(180, 50%, 50%, 50%)")
            .is_ok()
    );
    assert!(string().hsl().check_str("hsl(180, 50%, 50%, 0.5)").is_ok());
    assert!(string().hsla().check_str("hsla(180 50% 50%)").is_err());

    // 17. Additional request branches
    let req_san = request()
        .params(object().field("id", integer()))
        .sanitize(true);
    let parts_san = RequestParts::new().with_params(json!({ "id": 1 }));
    assert!(req_san.check_request(&parts_san).is_ok());
    let req_san_schema: &dyn Schema<ValidatedRequest> = &req_san;
    assert!(
        req_san_schema
            .check_path(
                &json!({ "params": { "id": 1 } }),
                "root",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_ok()
    );
    assert!(
        req_san_schema
            .validate_async_path(
                &json!({ "params": { "id": 1 } }),
                "",
                &ctx,
                None,
                default_messages_provider(),
            )
            .await
            .is_ok()
    );

    let req_async_bail = request()
        .params(object().field("id", integer().min(10)))
        .bail(true);
    let req_async_bail_schema: &dyn Schema<ValidatedRequest> = &req_async_bail;
    assert!(
        req_async_bail_schema
            .validate_async_path(
                &json!({ "params": { "id": 1 } }),
                "",
                &ctx,
                None,
                default_messages_provider(),
            )
            .await
            .is_err()
    );

    let async_typed_val = req_typed.validate_async(&typed_parts).await.unwrap();
    assert_eq!(async_typed_val.params.unwrap(), json!("hello"));
    assert_eq!(async_typed_val.query.unwrap(), json!(5));

    // 18. Additional object branches
    let obj_san = object().sanitize(true).sanitize(false);
    assert!(obj_san.check(&json!({})).is_ok());
    assert!(
        object()
            .strict()
            .bail(true)
            .check(&json!({ "unknown": 1 }))
            .is_err()
    );
    assert!(
        object()
            .field("opt", string().optional())
            .check(&json!({}))
            .is_ok()
    );
    assert!(
        object()
            .optional_field("opt", string())
            .check(&json!({ "opt": "valid" }))
            .is_ok()
    );

    // 19. Additional tuple branches
    let t3_bail = tuple3(string().min_len(5), integer().min(10), boolean()).bail(true);
    assert!(
        t3_bail
            .check_path(
                &json!(["a", 2, true]),
                "t3",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_err()
    );
    let t4_bail = tuple4(string().min_len(5), integer().min(10), boolean(), string()).bail(true);
    assert!(
        t4_bail
            .check_path(
                &json!(["a", 2, true, "x"]),
                "t4",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_err()
    );

    let native_t3 = (string(), integer(), boolean());
    assert!(
        native_t3
            .validate_path(
                &json!(["a", 1, true]),
                "nt3",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_ok()
    );
    let native_t4 = (string(), integer(), boolean(), string());
    assert!(
        native_t4
            .validate_path(
                &json!(["a", 1, true, "b"]),
                "nt4",
                &ctx,
                None,
                default_messages_provider(),
            )
            .is_ok()
    );

    assert!(
        (string().min_len(5), integer())
            .validate_async(&json!(["bad", "not_int"]))
            .await
            .is_err()
    );
    assert!(
        (string().min_len(5), integer(), boolean())
            .validate_async(&json!(["bad", "not_int", true]))
            .await
            .is_err()
    );
    assert!(
        (string().min_len(5), integer(), boolean(), string())
            .validate_async(&json!(["bad", "not_int", true, "ok"]))
            .await
            .is_err()
    );

    // 20. Union 3 and 4 branches
    assert!(
        union3(
            string().min_len(10),
            string().email(),
            string().alphanumeric()
        )
        .check(&json!("abc12345"))
        .is_ok()
    );
    assert!(
        union4(
            string().min_len(10),
            string().email(),
            string().alphanumeric(),
            string().ascii()
        )
        .check(&json!("hello"))
        .is_ok()
    );

    // 21. File branch without bail
    assert!(
        file()
            .max_size(10)
            .validate(&json!({ "size": 100 }))
            .is_err()
    );

    // 22. Optional empty json schema
    use jachai::schema::json_schema::ToJsonSchema;
    #[derive(Clone)]
    struct EmptyJsonSchema;
    impl Schema<String> for EmptyJsonSchema {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<String, ValidationError> {
            let _ = (path, context, parent_data, messages);
            Ok(value.as_str().unwrap_or("").to_string())
        }
    }
    impl ToJsonSchema for EmptyJsonSchema {
        fn to_json_schema(&self) -> serde_json::Value {
            serde_json::Value::Object(serde_json::Map::new())
        }
    }
    assert_eq!(EmptyJsonSchema.nullable().to_json_schema(), json!({}));

    // 23. Number edge branches
    assert!(number().finite().check_f64(f64::INFINITY).is_err());
    assert!(number().decimal_places(2).check_f64(3.15).is_ok());
    assert!(
        number()
            .decimal_places(2)
            .min(10.0)
            .bail(true)
            .check_f64(3.15)
            .is_err()
    );
    assert!(
        number()
            .min(10.0)
            .max(5.0)
            .bail(true)
            .check(&json!(2.0))
            .is_err()
    );

    struct FailingAsyncNumRule;
    impl AsyncCustomRule<f64> for FailingAsyncNumRule {
        fn validate<'a>(
            &'a self,
            value: &'a f64,
            context: &'a RuleContext<'_>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<(), ValidationIssue>> + Send + 'a>,
        > {
            let dummy_num = *value;
            assert!(dummy_num > 0.0);
            Box::pin(async move { Err(context.create_issue("fail", RuleParams::new(), None)) })
        }
    }
    assert!(
        number()
            .rule_async(FailingAsyncNumRule)
            .rule_async(FailingAsyncNumRule)
            .bail(true)
            .validate_async(&json!(5.0))
            .await
            .is_err()
    );
    assert!(
        number()
            .multiple_of(2.5)
            .to_json_schema()
            .get("multipleOf")
            .is_some()
    );
    assert!(integer().min(5).max(10).multiple_of(2).check_i64(8).is_ok());
    assert!(integer().to_json_schema().get("type").is_some());

    // 24. Array edge branches
    assert!(
        array(string().min_len(5))
            .validate_path(
                &json!(["bad"]),
                "items",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(
        array(string().min_len(5))
            .check_path(
                &json!(["bad"]),
                "items",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    let items_17_check: Vec<serde_json::Value> = (0..16)
        .map(|i| json!(i))
        .chain(std::iter::once(json!(0)))
        .collect();
    assert!(
        array(integer())
            .distinct()
            .bail(true)
            .check(&serde_json::Value::Array(items_17_check))
            .is_err()
    );
    assert!(
        array(integer())
            .subset_of(&["a", "b"])
            .check(&json!([123]))
            .is_err()
    );
    assert!(
        array(integer())
            .contains_any(vec![json!(1), json!(2)])
            .bail(true)
            .check(&json!([3, 4]))
            .is_err()
    );
    assert!(
        array(integer().min(10))
            .bail(true)
            .check(&json!([1, 2]))
            .is_err()
    );
    assert!(
        array(integer())
            .max_items(1)
            .bail(true)
            .validate_async(&json!([1, 2]))
            .await
            .is_err()
    );

    // 25. Maximum 100% Sweep - Completing All Remaining Modules
    let err_builder =
        ValidationError::new().with_issue(ValidationIssue::new("field", "rule", "msg"));
    assert_eq!(err_builder.len(), 1);

    #[derive(Debug, PartialEq, Eq)]
    enum UnregisteredEnum {
        VariantA,
    }
    impl std::str::FromStr for UnregisteredEnum {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if s == "a" {
                Ok(UnregisteredEnum::VariantA)
            } else {
                Err("unknown".into())
            }
        }
    }
    let empty_enum = enum_of::<UnregisteredEnum>();
    assert!(empty_enum.validate(&json!(123)).is_err());
    assert!(empty_enum.validate(&json!("unknown")).is_err());

    let union_with_msg = union2(string().min_len(10), string().starts_with("prefix"))
        .with_message("custom union failure");
    assert_eq!(
        union_with_msg
            .validate(&json!("xyz"))
            .unwrap_err()
            .first_message()
            .unwrap(),
        "custom union failure"
    );

    let rec_check = record(string().min_len(5)).bail(true);
    assert!(rec_check.check(&json!(null)).is_err());
    assert!(rec_check.check(&json!(["not an object"])).is_err());
    assert!(
        rec_check
            .check_path(
                &json!({"k": "bad"}),
                "parent",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );

    let tup_check = tuple2(string().min_len(5), integer().min(10)).bail(true);
    assert!(
        tup_check
            .check_path(
                &json!(["bad", 5]),
                "pair",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup_check.check(&json!(["bad", 5])).is_err());
    let tup_nobail = tuple2(string().min_len(5), integer().min(10)).bail(false);
    assert!(
        tup_nobail
            .check_path(
                &json!(["bad", 5]),
                "pair",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup_nobail.check(&json!(["bad", 5])).is_err());

    let tup3_check = tuple3(string().min_len(5), integer().min(10), boolean()).bail(true);
    assert!(
        tup3_check
            .check_path(
                &json!(["bad", 5, true]),
                "trio",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup3_check.check(&json!(["bad", 5, true])).is_err());
    let tup3_nobail = tuple3(string().min_len(5), integer().min(10), boolean()).bail(false);
    assert!(
        tup3_nobail
            .check_path(
                &json!(["bad", 5, true]),
                "trio",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup3_nobail.check(&json!(["bad", 5, true])).is_err());

    let tup4_check =
        tuple4(string().min_len(5), integer().min(10), boolean(), integer()).bail(true);
    assert!(
        tup4_check
            .check_path(
                &json!(["bad", 5, true, 1]),
                "quad",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup4_check.check(&json!(["bad", 5, true, 1])).is_err());
    let tup4_nobail =
        tuple4(string().min_len(5), integer().min(10), boolean(), integer()).bail(false);
    assert!(
        tup4_nobail
            .check_path(
                &json!(["bad", 5, true, 1]),
                "quad",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(tup4_nobail.check(&json!(["bad", 5, true, 1])).is_err());

    assert!(
        string()
            .optional()
            .optional()
            .to_json_schema()
            .get("type")
            .is_some()
    );

    struct CustomNoTypeSchema;
    impl Schema<String> for CustomNoTypeSchema {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<String, ValidationError> {
            let dummy = std::mem::size_of_val(&(value, path, context, parent_data, messages));
            assert!(dummy > 0);
            Ok("ok".to_string())
        }
    }
    assert!(CustomNoTypeSchema.optional().to_json_schema().is_object());

    let obj_opt = object().optional_field("name", string().min_len(5));
    assert!(obj_opt.check(&json!({"name": "hello"})).is_ok());
    assert!(obj_opt.check(&json!({"name": "hi"})).is_err());
    let obj_partial = object().field("name", string().min_len(5)).partial();
    assert!(obj_partial.check(&json!({"name": "hello"})).is_ok());
    assert!(obj_partial.check(&json!({"name": "hi"})).is_err());
    assert!(
        object()
            .strict()
            .bail(true)
            .validate_async(&json!({"unknown": 123}))
            .await
            .is_err()
    );
    assert!(
        object()
            .strict()
            .bail(false)
            .validate_async(&json!({"unknown": 123}))
            .await
            .is_err()
    );
    assert!(
        object()
            .field("opt", string().optional())
            .validate_async(&json!({}))
            .await
            .is_ok()
    );

    let long_path = "a".repeat(70);
    assert!(
        array(string().min_len(10))
            .validate_path(
                &json!(["bad"]),
                &long_path,
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(
        array(string().min_len(10))
            .check_path(
                &json!(["bad"]),
                &long_path,
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(array(integer()).max_items(1).check(&json!([1, 2])).is_err());
    assert!(
        array(integer())
            .contains_any(vec![json!(1), json!(2)])
            .check(&json!([3, 4]))
            .is_err()
    );
    assert!(
        array(integer())
            .max_items(1)
            .validate_async(&json!([1, 2]))
            .await
            .is_err()
    );
    let items_17_val: Vec<serde_json::Value> = (0..16)
        .map(|i| json!(i))
        .chain(std::iter::once(json!(0)))
        .collect();
    assert!(
        array(integer())
            .distinct()
            .validate(&serde_json::Value::Array(items_17_val))
            .is_err()
    );

    let empty_val_json = ValidatedRequest::default().to_value();
    assert!(empty_val_json.as_object().unwrap().is_empty());

    let parts_val = RequestParts::new()
        .with_params(json!({"id": 1}))
        .with_query(json!({"q": "test"}));
    let val_req = request()
        .params(object().field("id", integer()))
        .query(object().field("q", string()))
        .validate(&parts_val)
        .unwrap();
    let val_json = val_req.to_value();
    assert_eq!(val_json.get("params").unwrap().get("id").unwrap(), 1);
    assert_eq!(val_json.get("query").unwrap().get("q").unwrap(), "test");

    struct FailingSerialize;
    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let dummy = std::mem::size_of_val(&serializer);
            if dummy == usize::MAX {
                return serializer.serialize_unit();
            }
            Err(serde::ser::Error::custom("fail serialization"))
        }
    }

    struct FailingSerializeSchema;
    impl Schema<FailingSerialize> for FailingSerializeSchema {
        fn validate_path(
            &self,
            value: &serde_json::Value,
            path: &str,
            context: &ValidationContext,
            parent_data: Option<&serde_json::Value>,
            messages: &dyn MessagesProvider,
        ) -> Result<FailingSerialize, ValidationError> {
            let dummy = std::mem::size_of_val(&(value, path, context, parent_data, messages));
            assert!(dummy > 0);
            Ok(FailingSerialize)
        }

        fn validate_async_path<'a>(
            &'a self,
            value: &'a serde_json::Value,
            path: &'a str,
            context: &'a ValidationContext,
            parent_data: Option<&'a serde_json::Value>,
            messages: &'a dyn MessagesProvider,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<FailingSerialize, ValidationError>>
                    + Send
                    + 'a,
            >,
        > {
            let dummy = std::mem::size_of_val(&(value, path, context, parent_data, messages));
            assert!(dummy > 0);
            Box::pin(async move { Ok(FailingSerialize) })
        }
    }
    assert!(
        request()
            .query(FailingSerializeSchema)
            .validate(&RequestParts::new().with_query(json!(1.0)))
            .is_err()
    );
    assert!(
        request()
            .query(FailingSerializeSchema)
            .validate_async(&RequestParts::new().with_query(json!(1.0)))
            .await
            .is_err()
    );

    assert!(
        number()
            .rule_async(FailingAsyncNumRule)
            .bail(false)
            .validate_async(&json!(5.0))
            .await
            .is_err()
    );
    assert!(number().multiple_of(f64::NAN).to_json_schema().is_object());

    let req_int = request().query(integer());
    let parts_int = RequestParts::new().with_query(json!(42));
    assert_eq!(
        req_int.validate(&parts_int).unwrap().query.unwrap(),
        json!(42)
    );
    assert_eq!(
        req_int
            .validate_async(&parts_int)
            .await
            .unwrap()
            .query
            .unwrap(),
        json!(42)
    );

    let req_san = request()
        .params(object().field("id", integer()))
        .sanitize(true);
    let parts_san = RequestParts::new().with_params(json!({"id": 1}));
    assert_eq!(
        req_san
            .validate_async(&parts_san)
            .await
            .unwrap()
            .params
            .unwrap(),
        json!({"id": 1})
    );

    let req_schema_obj = request()
        .params(object().field("id", integer().min(10)))
        .sanitize(true)
        .bail(true);
    let dyn_req: &dyn Schema<ValidatedRequest> = &req_schema_obj;
    assert!(
        dyn_req
            .validate_path(
                &json!({"params": {"id": 1}}),
                "envelope",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    let req_schema_nobail = request()
        .params(object().field("id", integer().min(10)))
        .sanitize(true);
    let dyn_req_nobail: &dyn Schema<ValidatedRequest> = &req_schema_nobail;
    assert!(
        dyn_req_nobail
            .validate_path(
                &json!({"params": {"id": 1}}),
                "envelope",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );

    // StringSchema fast-path zero-alloc sweep
    let fast = string().trim(false);
    assert!(fast.clone().min_len(2).check_str("hello").is_ok());
    assert!(fast.clone().min_len(5).check_str("hi").is_err());
    assert!(fast.clone().min_len(2).check_str("你好").is_ok());
    assert!(fast.clone().min_len(5).check_str("你好").is_err());

    assert!(fast.clone().max_len(5).check_str("hello").is_ok());
    assert!(fast.clone().max_len(2).check_str("hello").is_err());
    assert!(fast.clone().max_len(2).check_str("你好").is_ok());
    assert!(fast.clone().max_len(1).check_str("你好").is_err());

    assert!(fast.clone().exact_len(2).check_str("你好").is_ok());
    assert!(fast.clone().exact_len(5).check_str("hello").is_ok());
    assert!(fast.clone().exact_len(5).check_str("hi").is_err());
    assert!(fast.clone().exact_len(2).check_str("hello").is_err());
    assert!(fast.clone().exact_len(1).check_str("你好").is_err());

    assert!(fast.clone().email().check_str("user@example.com").is_ok());
    assert!(fast.clone().email().check_str("not_email").is_err());
    let long_domain_email = format!("user@{}.com", "a".repeat(252));
    assert!(fast.clone().email().check_str(&long_domain_email).is_err());

    assert!(fast.clone().url().check_str("https://example.com").is_ok());
    assert!(fast.clone().url().check_str("not_url").is_err());

    assert!(
        fast.clone()
            .uuid()
            .check_str("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11")
            .is_ok()
    );
    assert!(fast.clone().uuid().check_str("not-uuid").is_err());

    assert!(fast.clone().ip().check_str("127.0.0.1").is_ok());
    assert!(fast.clone().ip().check_str("::1").is_ok());
    assert!(fast.clone().ip().check_str("not-ip").is_err());

    assert!(fast.clone().ipv4().check_str("127.0.0.1").is_ok());
    assert!(fast.clone().ipv4().check_str("999.999.999.999").is_err());
    assert!(fast.clone().ipv4().check_str("1.2.3").is_err());
    assert!(fast.clone().ipv4().check_str("1.2.3.4.5").is_err());
    assert!(fast.clone().ipv4().check_str("1.02.3.4").is_err());
    assert!(fast.clone().ipv4().check_str("1.2.3.a").is_err());

    assert!(fast.clone().ipv6().check_str("::1").is_ok());
    assert!(fast.clone().ipv6().check_str("not_ipv6").is_err());

    assert!(fast.clone().date().check_str("2026-03-30").is_ok());
    assert!(fast.clone().date().check_str("bad-date").is_err());

    assert!(fast.clone().time().check_str("12:00:00").is_ok());
    assert!(fast.clone().time().check_str("bad-time").is_err());

    assert!(
        fast.clone()
            .datetime()
            .check_str("2026-03-30T12:00:00Z")
            .is_ok()
    );
    assert!(fast.clone().datetime().check_str("bad-datetime").is_err());

    assert!(
        fast.clone()
            .credit_card()
            .check_str("4532015112830366")
            .is_ok()
    );
    assert!(fast.clone().credit_card().check_str("bad-cc").is_err());

    assert!(
        fast.clone()
            .ulid()
            .check_str("01ARZ3NDEKTSV4RRFFQ69G5FAV")
            .is_ok()
    );
    assert!(fast.clone().ulid().check_str("bad-ulid").is_err());

    assert!(fast.clone().hex_color().check_str("#ffffff").is_ok());
    assert!(fast.clone().hex_color().check_str("bad-color").is_err());

    assert!(fast.clone().slug().check_str("valid-slug-123").is_ok());
    assert!(fast.clone().slug().check_str("Invalid Slug").is_err());

    assert!(fast.clone().semver().check_str("1.2.3").is_ok());
    assert!(fast.clone().semver().check_str("bad-semver").is_err());

    assert!(fast.clone().base64().check_str("YQ==").is_ok());
    assert!(fast.clone().base64().check_str("QQ==").is_ok());
    assert!(fast.clone().base64().check_str("bad-b64!").is_err());

    assert!(fast.clone().starts_with("abc").check_str("abcdef").is_ok());
    assert!(fast.clone().starts_with("abc").check_str("xyz").is_err());

    assert!(fast.clone().ends_with("def").check_str("abcdef").is_ok());
    assert!(fast.clone().ends_with("def").check_str("xyz").is_err());

    assert!(fast.clone().contains("cd").check_str("abcdef").is_ok());
    assert!(fast.clone().contains("cd").check_str("xyz").is_err());

    assert!(fast.clone().alpha().check_str("abc").is_ok());
    assert!(fast.clone().alpha().check_str("é").is_ok());
    assert!(fast.clone().alpha().check_str("").is_err());
    assert!(fast.clone().alpha().check_str("123").is_err());

    assert!(fast.clone().alphanumeric().check_str("abc1").is_ok());
    assert!(fast.clone().alphanumeric().check_str("é1").is_ok());
    assert!(fast.clone().alphanumeric().check_str("").is_err());
    assert!(fast.clone().alphanumeric().check_str("abc!").is_err());

    assert!(fast.clone().ascii().check_str("abc").is_ok());
    assert!(fast.clone().ascii().check_str("é").is_err());
    assert!(fast.clone().ipv4().check_str("1..2.3").is_err());
    assert!(fast.clone().regex("^abc$").check_str("abc").is_ok());
    assert!(
        fast.clone()
            .does_not_contain("xyz")
            .check_str("abc")
            .is_ok()
    );
    assert!(fast.clone().country_code().check_str("US").is_ok());
    assert!(fast.clone().currency_code().check_str("USD").is_ok());
    assert!(
        fast.clone()
            .cuid()
            .check_str("c123456789012345678901234")
            .is_ok()
    );
    assert!(fast.clone().nanoid_len(5).check_str("abcde").is_ok());
    assert!(fast.clone().hex().check_str("deadbeef").is_ok());
    assert!(fast.clone().digits().check_str("12345").is_ok());
    assert!(fast.clone().min_words(2).check_str("hello world").is_ok());

    assert_eq!(string().trim(false).validate_str("hello").unwrap(), "hello");
    assert!(
        string()
            .trim(false)
            .min_len(10)
            .check_path(
                &json!("short"),
                "f",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );
    assert!(
        string()
            .mac_address()
            .check_str("00/11/22/33/44/55")
            .is_err()
    );
    assert!(string().rgb().check_str("cmyk(1, 2, 3)").is_err());
    assert!(string().hsl().check_str("cmyk(1, 2, 3)").is_err());
    assert!(string().hsla().check_str("hsla(not_closed").is_err());

    assert!(
        fast.clone()
            .min_len(2)
            .check_path(
                &json!("hello"),
                "field",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_ok()
    );

    let s_dyn_check: &dyn Schema<String> = &string()
        .trim(false)
        .trim_start()
        .trim_end()
        .to_uppercase()
        .normalize_email()
        .escape_html()
        .min_len(2)
        .bail(true);
    assert!(
        s_dyn_check
            .check_path(
                &json!("  HELLO@TEST.COM <tag>  "),
                "val",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_ok()
    );
    let s_dyn_fail: &dyn Schema<String> =
        &string().trim(false).to_uppercase().min_len(10).bail(true);
    assert!(
        s_dyn_fail
            .check_path(
                &json!("short"),
                "val",
                &ctx,
                None,
                default_messages_provider()
            )
            .is_err()
    );

    assert!(
        string()
            .mac_address()
            .check_str("00:1A:2B:3C:4D:GG")
            .is_err()
    );
    assert!(string().iban().check_str("GB82WEST1234569876543!").is_err());
    assert!(string().isin().check_str("US037833100!").is_err());
    assert!(string().rgb().check_str("rgba(255, 0, 0, 0.5)").is_ok());
    assert!(string().rgb().check_str("rgb(255 0 bad)").is_err());
    assert!(
        string()
            .hsl()
            .check_str("hsla(120, 100%, 50%, 0.8)")
            .is_ok()
    );
    assert!(string().hsl().check_str("hsl(120, 100%)").is_err());
    assert!(string().issn().check_str("1234-567!").is_err());
    assert!(string().exact_len(2).validate(&json!("你好")).is_ok());
    assert!(string().exact_len(2).validate(&json!("你好世界")).is_err());
}
