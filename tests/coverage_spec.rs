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
