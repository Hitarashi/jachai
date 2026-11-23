use jachai::prelude::*;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct UserProfile {
    name: String,
    age: i64,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct QueryFilter {
    page: i64,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct HeaderMeta {
    auth_token: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct CookieData {
    session: String,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
struct RouteParam {
    id: i64,
}

#[test]
fn test_array_schema_full_coverage() {
    // min_items
    let s_min = array(integer()).min_items(3).bail(true);
    let err = s_min.validate(&json!([1, 2])).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "min_items");

    // max_items
    let s_max = array(integer()).max_items(2);
    let err = s_max.validate(&json!([1, 2, 3])).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "max_items");

    // exact_items
    let s_exact = array(integer()).exact_items(2);
    let err = s_exact.validate(&json!([1])).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "exact_items");

    // not_empty
    let s_not_empty = array(integer())
        .not_empty()
        .with_message("Array cannot be empty");
    let err = s_not_empty.validate(&json!([])).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "not_empty");
    assert_eq!(err.first_message(), Some("Array cannot be empty"));

    // distinct
    let s_distinct = array(integer()).distinct().bail(true);
    let err = s_distinct.validate(&json!([1, 2, 1])).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "distinct");

    // null input & non-array input
    let err_null = array(integer())
        .validate(&serde_json::Value::Null)
        .unwrap_err();
    assert_eq!(err_null.first_issue().unwrap().rule, "required");

    let err_non_arr = array(integer())
        .validate(&json!("not an array"))
        .unwrap_err();
    assert_eq!(err_non_arr.first_issue().unwrap().rule, "required");

    // Element failure with bail
    let s_elem_bail = array(integer().min(10)).bail(true);
    let err_elem = s_elem_bail.validate(&json!([5, 4, 3])).unwrap_err();
    assert_eq!(err_elem.len(), 1);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_array_schema_async_full_coverage() {
    let s_async = array(integer().min(5))
        .min_items(2)
        .max_items(4)
        .exact_items(3)
        .not_empty()
        .distinct()
        .bail(true);

    // valid
    let valid = json!([6, 7, 8]);
    assert_eq!(s_async.validate_async(&valid).await.unwrap(), vec![6, 7, 8]);

    // null
    let err_null = s_async
        .validate_async(&serde_json::Value::Null)
        .await
        .unwrap_err();
    assert_eq!(err_null.first_issue().unwrap().rule, "required");

    // non array
    let err_non = s_async.validate_async(&json!(123)).await.unwrap_err();
    assert_eq!(err_non.first_issue().unwrap().rule, "required");

    // not empty
    let s_empty = array(integer()).not_empty().bail(true);
    let err_empty = s_empty.validate_async(&json!([])).await.unwrap_err();
    assert_eq!(err_empty.first_issue().unwrap().rule, "not_empty");

    // min items
    let s_min = array(integer()).min_items(3).bail(true);
    let err_min = s_min.validate_async(&json!([1, 2])).await.unwrap_err();
    assert_eq!(err_min.first_issue().unwrap().rule, "min_items");

    // max items
    let s_max = array(integer()).max_items(2).bail(true);
    let err_max = s_max.validate_async(&json!([1, 2, 3])).await.unwrap_err();
    assert_eq!(err_max.first_issue().unwrap().rule, "max_items");

    // exact items
    let s_exact = array(integer()).exact_items(2).bail(true);
    let err_exact = s_exact.validate_async(&json!([1])).await.unwrap_err();
    assert_eq!(err_exact.first_issue().unwrap().rule, "exact_items");

    // distinct
    let s_dist = array(integer()).distinct().bail(true);
    let err_dist = s_dist.validate_async(&json!([1, 1])).await.unwrap_err();
    assert_eq!(err_dist.first_issue().unwrap().rule, "distinct");

    // subset_of async
    let s_sub = array(string()).subset_of(&["alpha", "beta"]).bail(true);
    assert!(s_sub.validate_async(&json!(["alpha"])).await.is_ok());
    let err_sub = s_sub.validate_async(&json!(["gamma"])).await.unwrap_err();
    assert_eq!(err_sub.first_issue().unwrap().rule, "subset_of");
    let err_sub_non_str = s_sub.validate_async(&json!([123])).await.unwrap_err();
    assert_eq!(err_sub_non_str.first_issue().unwrap().rule, "subset_of");

    // contains, contains_all, contains_any async with bail
    let s_cnt = array(string())
        .contains(json!("required_val"))
        .contains_all([json!("all_1"), json!("all_2")])
        .contains_any([json!("any_1"), json!("any_2")])
        .bail(true);
    let err_cnt = s_cnt.validate_async(&json!(["missing"])).await.unwrap_err();
    assert_eq!(err_cnt.first_issue().unwrap().rule, "contains");

    let s_all = array(string())
        .contains_all([json!("all_1"), json!("all_2")])
        .bail(true);
    let err_all = s_all.validate_async(&json!(["all_1"])).await.unwrap_err();
    assert_eq!(err_all.first_issue().unwrap().rule, "contains_all");

    let s_any = array(string())
        .contains_any([json!("any_1"), json!("any_2")])
        .bail(true);
    let err_any = s_any.validate_async(&json!(["other"])).await.unwrap_err();
    assert_eq!(err_any.first_issue().unwrap().rule, "contains_any");

    // without bail: accumulates multiple errors
    let s_no_bail = array(integer())
        .min_items(5)
        .contains(json!(10))
        .contains_all([json!(20)])
        .contains_any([json!(30)]);
    let err_accum = s_no_bail.validate_async(&json!([1])).await.unwrap_err();
    assert!(err_accum.len() >= 3);

    // element failure bail
    let s_elem = array(integer().min(10)).bail(true);
    let err_elem = s_elem.validate_async(&json!([1, 2])).await.unwrap_err();
    assert_eq!(err_elem.len(), 1);
}

#[test]
fn test_boolean_schema_full_coverage() {
    let s_acc = boolean()
        .accepted()
        .bail(true)
        .with_message("Must accept terms");
    assert!(s_acc.validate(&json!(true)).is_ok());
    assert!(s_acc.validate(&json!("yes")).is_ok());
    assert!(s_acc.validate(&json!("true")).is_ok());
    assert!(s_acc.validate(&json!("on")).is_ok());
    assert!(s_acc.validate(&json!(1)).is_ok());
    assert!(s_acc.validate(&json!("1")).is_ok());

    let err_false = s_acc.validate(&json!(false)).unwrap_err();
    assert_eq!(err_false.first_issue().unwrap().rule, "accepted");
    assert_eq!(err_false.first_message(), Some("Must accept terms"));

    assert!(s_acc.validate(&json!(0)).is_err());
    assert!(s_acc.validate(&json!("no")).is_err());
    assert!(s_acc.validate(&json!("off")).is_err());

    let err_null = boolean().validate(&serde_json::Value::Null).unwrap_err();
    assert_eq!(err_null.first_issue().unwrap().rule, "required");

    let err_invalid = boolean().validate(&json!("not_a_bool")).unwrap_err();
    assert_eq!(err_invalid.first_issue().unwrap().rule, "boolean");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_boolean_schema_async_full_coverage() {
    let s = boolean().accepted();
    assert!(s.validate_async(&json!(true)).await.is_ok());
    assert!(s.validate_async(&json!(false)).await.is_err());
    assert!(s.validate_async(&serde_json::Value::Null).await.is_err());
    assert!(s.validate_async(&json!("invalid")).await.is_err());
}

#[test]
fn test_number_and_integer_schema_full_coverage() {
    // positive & negative & multiple_of & range
    let s_pos = integer().positive().bail(true);
    assert!(s_pos.validate(&json!(10)).is_ok());
    assert!(s_pos.validate(&json!(0)).is_err());
    assert!(s_pos.validate(&json!(-5)).is_err());

    let s_neg = integer().negative().bail(true);
    assert!(s_neg.validate(&json!(-10)).is_ok());
    assert!(s_neg.validate(&json!(0)).is_err());
    assert!(s_neg.validate(&json!(5)).is_err());

    let s_mul = integer().multiple_of(5);
    assert!(s_mul.validate(&json!(15)).is_ok());
    assert!(s_mul.validate(&json!(14)).is_err());

    let s_range = integer().range(10, 20);
    assert!(s_range.validate(&json!(15)).is_ok());
    assert!(s_range.validate(&json!(5)).is_err());
    assert!(s_range.validate(&json!(25)).is_err());

    // Float validation
    let s_num = number().positive().multiple_of(0.5);
    assert!(s_num.validate(&json!(2.5)).is_ok());
    assert!(s_num.validate(&json!(-2.5)).is_err());

    // Integer rejection on float
    let s_int = integer();
    assert!(s_int.validate(&json!(3.25)).is_err());

    // Coercion from numeric string
    assert_eq!(integer().validate(&json!("42")).unwrap(), 42);
    assert_eq!(number().validate(&json!("3.25")).unwrap(), 3.25);

    // Errors
    assert_eq!(
        integer()
            .validate(&serde_json::Value::Null)
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "required"
    );
    assert_eq!(
        integer()
            .validate(&json!("not_num"))
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "number"
    );
    assert_eq!(
        number()
            .validate(&serde_json::Value::Null)
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "required"
    );
    assert_eq!(
        number()
            .validate(&json!("not_num"))
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "number"
    );
}

#[test]
fn test_string_schema_full_predicates_and_sanitizations() {
    let s = string()
        .trim(true)
        .without_trim()
        .trim(true)
        .to_lowercase()
        .min_len(2)
        .max_len(20)
        .exact_len(5)
        .ascii()
        .alphanumeric()
        .alpha()
        .starts_with("he")
        .ends_with("lo")
        .contains("ll");

    let res = s.validate(&json!("  HELLO  ")).unwrap();
    assert_eq!(res, "hello");

    let s_upper = string().to_uppercase();
    assert_eq!(s_upper.validate(&json!("world")).unwrap(), "WORLD");

    let s_empty = string().empty_to_null();
    let err_empty = s_empty.validate(&json!("   ")).unwrap_err();
    assert_eq!(err_empty.first_issue().unwrap().rule, "required");

    // Failures
    assert!(string().ascii().validate(&json!("héllo")).is_err());
    assert!(string().alpha().validate(&json!("hello123")).is_err());
    assert!(
        string()
            .alphanumeric()
            .validate(&json!("hello_123!"))
            .is_err()
    );
    assert!(
        string()
            .starts_with("pre")
            .validate(&json!("post"))
            .is_err()
    );
    assert!(string().ends_with("post").validate(&json!("pre")).is_err());
    assert!(
        string()
            .contains("middle")
            .validate(&json!("edge"))
            .is_err()
    );
    assert!(string().ipv4().validate(&json!("256.0.0.1")).is_err());
    assert!(string().ipv6().validate(&json!("127.0.0.1")).is_err());
    assert!(string().ip().validate(&json!("invalid_ip")).is_err());
    assert!(string().url().validate(&json!("not a url")).is_err());
    assert!(string().uuid().validate(&json!("not a uuid")).is_err());

    // Non string & null
    assert_eq!(
        string()
            .validate(&serde_json::Value::Null)
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "required"
    );
    assert_eq!(
        string()
            .validate(&json!(123))
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "string"
    );
}

#[test]
fn test_object_schema_deep_policies_and_cross_rules() {
    // strict policy rejection
    let s_strict = object().field("title", string()).strict().bail(true);

    let err_strict = s_strict
        .validate(&json!({ "title": "Hi", "unknown1": "val1", "unknown2": "val2" }))
        .unwrap_err();
    assert_eq!(err_strict.first_issue().unwrap().rule, "unknown_field");

    // passthrough policy
    let s_pass = object().field("a", integer()).passthrough();
    let res_pass = s_pass
        .validate(&json!({ "a": 1, "b": "preserved" }))
        .unwrap();
    assert_eq!(res_pass["b"], "preserved");

    // cross-rules
    let s_cross = object()
        .field("start_num", integer())
        .field("end_num", integer())
        .field("pass1", string())
        .field("pass2", string())
        .field("user_type", string())
        .optional_field("company", string())
        .optional_field("alt_email", string())
        .optional_field("primary_email", string())
        .after_field("end_num", "start_num")
        .before_field("start_num", "end_num")
        .after_date("end_num", "start_num")
        .before_date("start_num", "end_num")
        .same_as("pass1", "pass2")
        .different_from("pass1", "user_type")
        .required_if("company", "user_type", "business")
        .required_without("alt_email", "primary_email");

    let valid_input = json!({
        "start_num": 10,
        "end_num": 20,
        "pass1": "secret",
        "pass2": "secret",
        "user_type": "business",
        "company": "Acme Inc",
        "primary_email": "a@b.com",
    });
    assert!(s_cross.validate(&valid_input).is_ok());

    // same_as failure
    let err_same = s_cross
        .validate(&json!({
            "start_num": 10,
            "end_num": 20,
            "pass1": "secret",
            "pass2": "different",
            "user_type": "individual",
        }))
        .unwrap_err();
    assert!(err_same.issues().iter().any(|i| i.rule == "same_as"));

    // after_field failure on numbers
    let err_after = object()
        .field("start", integer())
        .field("end", integer())
        .after_field("end", "start")
        .validate(&json!({ "start": 50, "end": 20 }))
        .unwrap_err();
    assert_eq!(err_after.first_issue().unwrap().rule, "after");

    // before_field failure on numbers
    let err_before = object()
        .field("start", integer())
        .field("end", integer())
        .before_field("start", "end")
        .validate(&json!({ "start": 50, "end": 20 }))
        .unwrap_err();
    assert_eq!(err_before.first_issue().unwrap().rule, "before");

    // decode & validate_and_decode
    let s_user = object().field("name", string()).field("age", integer());
    let dto: UserProfile = s_user
        .validate_and_decode(&json!({ "name": "Bob", "age": 28 }))
        .unwrap();
    assert_eq!(dto.name, "Bob");
    assert_eq!(dto.age, 28);

    // decode failure
    assert!(
        s_user
            .decode::<UserProfile>(&json!({ "name": "Bob", "age": "not an int" }))
            .is_err()
    );

    // decode_into
    let s_into = object()
        .field("name", string())
        .field("age", integer())
        .decode_into::<UserProfile>();
    let decoded_into = s_into
        .validate(&json!({ "name": "Charlie", "age": 32 }))
        .unwrap();
    assert_eq!(decoded_into.name, "Charlie");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_object_schema_async_full_coverage() {
    let s_async = object()
        .field("name", string().min_len(2))
        .field("score", integer().min(0))
        .strict()
        .bail(true);

    let valid = json!({ "name": "Diana", "score": 100 });
    let res = s_async.validate_async(&valid).await.unwrap();
    assert_eq!(res["name"], "Diana");

    let err_strict = s_async
        .validate_async(&json!({ "name": "Diana", "score": 100, "extra": true }))
        .await
        .unwrap_err();
    assert_eq!(err_strict.first_issue().unwrap().rule, "unknown_field");

    let err_null = s_async
        .validate_async(&serde_json::Value::Null)
        .await
        .unwrap_err();
    assert_eq!(err_null.first_issue().unwrap().rule, "required");

    let err_non = s_async
        .validate_async(&json!("not an obj"))
        .await
        .unwrap_err();
    assert_eq!(err_non.first_issue().unwrap().rule, "required");

    // passthrough async
    let s_pass = object().field("name", string()).passthrough();
    let res_pass = s_pass
        .validate_async(&json!({ "name": "Eve", "extra": 123 }))
        .await
        .unwrap();
    assert_eq!(res_pass["extra"], 123);

    // decode_async
    let dto: UserProfile = object()
        .field("name", string())
        .field("age", integer())
        .decode_async(&json!({ "name": "Eve", "age": 22 }))
        .await
        .unwrap();
    assert_eq!(dto.name, "Eve");
}

#[test]
fn test_record_tuple_union_full_coverage() {
    // RecordSchema
    let s_rec = record(integer().min(1))
        .bail(true)
        .with_message("Record fail");
    assert!(s_rec.validate(&json!({ "a": 1, "b": 2 })).is_ok());
    assert!(s_rec.validate(&json!({ "a": 0 })).is_err());
    assert!(s_rec.validate(&serde_json::Value::Null).is_err());
    assert!(s_rec.validate(&json!(123)).is_err());

    // Tuple2Schema
    let s_tup = tuple2(string(), integer())
        .bail(true)
        .with_message("Tuple fail");
    assert!(s_tup.validate(&json!(["one", 1])).is_ok());
    assert!(s_tup.validate(&json!(["one"])).is_err());
    assert!(s_tup.validate(&json!(["one", 1, "extra"])).is_err());
    assert!(s_tup.validate(&json!([123, "not_int"])).is_err());
    assert!(s_tup.validate(&serde_json::Value::Null).is_err());

    // Union2Schema
    let s_union =
        union2(integer().map(|i| i.to_string()), string().min_len(5)).with_message("Union fail");
    assert_eq!(s_union.validate(&json!(42)).unwrap(), "42");
    assert_eq!(s_union.validate(&json!("hello")).unwrap(), "hello");
    let err_union = s_union.validate(&json!("hi")).unwrap_err();
    assert_eq!(err_union.first_issue().unwrap().rule, "union");
    assert_eq!(err_union.first_message(), Some("Union fail"));

    let s_union_no_msg = union2(integer().map(|i| i.to_string()), string().min_len(5));
    let err_combined = s_union_no_msg.validate(&json!(false)).unwrap_err();
    assert!(err_combined.len() >= 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_record_tuple_union_async_full_coverage() {
    // Record async
    let s_rec = record(integer().min(1));
    assert!(s_rec.validate_async(&json!({ "x": 10 })).await.is_ok());
    assert!(
        s_rec
            .validate_async(&serde_json::Value::Null)
            .await
            .is_err()
    );
    assert!(s_rec.validate_async(&json!("not_obj")).await.is_err());

    // Tuple async
    let s_tup = tuple2(string(), integer());
    assert!(s_tup.validate_async(&json!(["hi", 1])).await.is_ok());
    assert!(s_tup.validate_async(&json!(["hi"])).await.is_err());
    assert!(
        s_tup
            .validate_async(&serde_json::Value::Null)
            .await
            .is_err()
    );

    // Union async
    let s_union = union2(integer().map(|i| i.to_string()), string());
    assert_eq!(s_union.validate_async(&json!(99)).await.unwrap(), "99");
    assert_eq!(
        s_union.validate_async(&json!("test")).await.unwrap(),
        "test"
    );
    assert!(s_union.validate_async(&json!(false)).await.is_err());
}

#[test]
fn test_request_parts_and_schema_coverage() {
    let parts = RequestParts::new()
        .with_params(json!({ "id": 100 }))
        .with_query(json!({ "page": 1 }))
        .with_headers(json!({ "auth_token": "bearer-xyz" }))
        .with_cookies(json!({ "session": "sess-123" }))
        .with_body(json!({ "name": "Frank", "age": 40 }));

    let req_schema = request_schema()
        .params(object().field("id", integer()))
        .query(object().field("page", integer()))
        .headers(object().field("auth_token", string()))
        .cookies(object().field("session", string()))
        .body(object().field("name", string()).field("age", integer()))
        .bail(true);

    let validated = req_schema.validate(&parts).unwrap();
    let route: RouteParam = validated.decode_params().unwrap();
    assert_eq!(route.id, 100);

    let query: QueryFilter = validated.decode_query().unwrap();
    assert_eq!(query.page, 1);

    let headers: HeaderMeta = validated.decode_headers().unwrap();
    assert_eq!(headers.auth_token, "bearer-xyz");

    let cookies: CookieData = validated.decode_cookies().unwrap();
    assert_eq!(cookies.session, "sess-123");

    let body: UserProfile = validated.decode_body().unwrap();
    assert_eq!(body.name, "Frank");

    // Failures with bail
    let bad_parts = RequestParts::new()
        .with_params(json!({ "id": "invalid" }))
        .with_query(json!({ "page": "invalid" }));

    let err = req_schema.validate(&bad_parts).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(err.first_issue().unwrap().field.starts_with("params"));
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_request_schema_async_coverage() {
    let parts = RequestParts::new()
        .with_params(json!({ "id": 200 }))
        .with_query(json!({ "page": 2 }))
        .with_headers(json!({ "auth_token": "token-abc" }))
        .with_cookies(json!({ "session": "sess-456" }))
        .with_body(json!({ "name": "Grace", "age": 35 }));

    let req_schema = request_schema()
        .params(object().field("id", integer()))
        .query(object().field("page", integer()))
        .headers(object().field("auth_token", string()))
        .cookies(object().field("session", string()))
        .body(object().field("name", string()).field("age", integer()));

    let ctx = ValidationContext::new();
    let validated = req_schema
        .validate_async_with_context(&parts, &ctx)
        .await
        .unwrap();
    let body: UserProfile = validated.decode_body().unwrap();
    assert_eq!(body.name, "Grace");

    // async error
    let bad_parts = RequestParts::new().with_body(json!({ "name": 123 }));
    assert!(req_schema.validate_async(&bad_parts).await.is_err());
}

#[test]
fn test_error_and_issue_edge_cases() {
    let mut params = RuleParams::new().with("min", "5").with("field", "count");
    assert!(params.contains("min"));
    assert_eq!(params.remove("min"), Some("5".to_string()));
    assert!(!params.contains("min"));
    assert!(!params.is_empty());
    assert_eq!(params.len(), 1);

    let issue_no_field = ValidationIssue::new("", "required", "Missing");
    assert_eq!(issue_no_field.to_string(), "required: Missing");

    let mut err = ValidationError::new();
    assert_eq!(err.to_string(), "Validation failed with 0 issues");

    err.push(ValidationIssue::new("[0]", "min", "Too low"));
    err.push(ValidationIssue::new("page", "integer", "Invalid"));
    err.prefix_paths("query");

    assert_eq!(err.issues()[0].field, "query[0]");
    assert_eq!(err.issues()[1].field, "query.page");

    assert!(!err.to_json_string().is_empty());

    let problem_doc = err.to_problem_details(Some("/api/v1/resource"));
    assert_eq!(problem_doc["instance"], "/api/v1/resource");

    let issues_vec = err.into_issues();
    assert_eq!(issues_vec.len(), 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_modifiers_and_combinators_extra() {
    // NullableSchema async
    let s_null = integer().nullable();
    assert_eq!(
        s_null
            .validate_async(&serde_json::Value::Null)
            .await
            .unwrap(),
        None
    );
    assert_eq!(s_null.validate_async(&json!(42)).await.unwrap(), Some(42));
    assert!(s_null.validate_async(&json!("not_num")).await.is_err());

    // OptionalSchema async
    let s_opt = integer().optional();
    assert_eq!(
        s_opt
            .validate_async(&serde_json::Value::Null)
            .await
            .unwrap(),
        None
    );
    assert_eq!(s_opt.validate_async(&json!(42)).await.unwrap(), Some(42));
    assert!(s_opt.validate_async(&json!("not_num")).await.is_err());

    // DefaultSchema async & default() alias
    let s_def = integer().default(100);
    assert_eq!(
        s_def
            .validate_async(&serde_json::Value::Null)
            .await
            .unwrap(),
        100
    );
    assert_eq!(s_def.validate_async(&json!(50)).await.unwrap(), 50);

    // TransformSchema async
    let s_trans = integer().transform(|n| {
        if n > 0 {
            Ok(n * 2)
        } else {
            Err("Must be positive".to_string())
        }
    });
    assert_eq!(s_trans.validate_async(&json!(5)).await.unwrap(), 10);
    assert!(s_trans.validate_async(&json!(-5)).await.is_err());

    // MapSchema async
    let s_map = integer().map(|n| n + 10);
    assert_eq!(s_map.validate_async(&json!(5)).await.unwrap(), 15);

    // RefineSchema async
    let s_ref = integer().refine_with(|n| *n % 2 == 0, "even", "Must be even");
    assert_eq!(s_ref.validate_async(&json!(4)).await.unwrap(), 4);
    assert!(s_ref.validate_async(&json!(5)).await.is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_cross_field_rules_and_bail() {
    let s_cross = object()
        .field("start", integer())
        .field("end", integer())
        .field("pass", string())
        .field("confirm", string())
        .field("kind", string())
        .optional_field("detail", string())
        .optional_field("optional_note", string())
        .optional_field("fallback_note", string())
        .bail(true)
        .same_as("confirm", "pass")
        .different_from("pass", "kind")
        .required_if("detail", "kind", "special")
        .required_without("fallback_note", "optional_note")
        .after_field("end", "start")
        .before_field("start", "end");

    // All valid
    let valid = json!({
        "start": 1,
        "end": 10,
        "pass": "secret",
        "confirm": "secret",
        "kind": "special",
        "detail": "info",
        "optional_note": "present"
    });
    assert!(s_cross.validate_async(&valid).await.is_ok());

    // SameAs failure async
    let bad_same = json!({
        "start": 1,
        "end": 10,
        "pass": "secret",
        "confirm": "wrong",
        "kind": "other"
    });
    assert_eq!(
        s_cross
            .validate_async(&bad_same)
            .await
            .unwrap_err()
            .first_issue()
            .unwrap()
            .rule,
        "same_as"
    );

    // DifferentFrom failure async
    let bad_diff = json!({
        "start": 1,
        "end": 10,
        "pass": "same",
        "confirm": "same",
        "kind": "same"
    });
    let err_diff = s_cross.validate_async(&bad_diff).await.unwrap_err();
    assert!(err_diff.issues().iter().any(|i| i.rule == "different_from"));

    // RequiredIf failure async
    let bad_req_if = json!({
        "start": 1,
        "end": 10,
        "pass": "secret",
        "confirm": "secret",
        "kind": "special"
    });
    let err_req_if = s_cross.validate_async(&bad_req_if).await.unwrap_err();
    assert!(err_req_if.issues().iter().any(|i| i.rule == "required_if"));

    // RequiredWithout failure async
    let bad_req_wo = json!({
        "start": 1,
        "end": 10,
        "pass": "secret",
        "confirm": "secret",
        "kind": "normal"
    });
    let err_req_wo = s_cross.validate_async(&bad_req_wo).await.unwrap_err();
    assert!(
        err_req_wo
            .issues()
            .iter()
            .any(|i| i.rule == "required_without")
    );

    // AfterField failure async
    let bad_after = json!({
        "start": 20,
        "end": 10,
        "pass": "secret",
        "confirm": "secret",
        "kind": "normal",
        "optional_note": "yes"
    });
    let err_after = s_cross.validate_async(&bad_after).await.unwrap_err();
    assert!(err_after.issues().iter().any(|i| i.rule == "after"));

    // Date strings after & before
    let s_date = object()
        .field("start_date", string())
        .field("end_date", string())
        .after_date("end_date", "start_date")
        .before_date("start_date", "end_date");

    let bad_dates = json!({
        "start_date": "2026-12-01",
        "end_date": "2026-01-01"
    });
    assert!(s_date.validate_async(&bad_dates).await.is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_request_parts_all_failures_and_bail() {
    let req = request_schema()
        .params(object().field("id", integer().min(10)))
        .query(object().field("page", integer().min(1)))
        .headers(object().field("x-api-key", string().min_len(5)))
        .cookies(object().field("token", string().min_len(5)))
        .body(object().field("email", string().email()))
        .bail(true);

    // Fail params with bail
    let parts_bad_params = RequestParts::new().with_params(json!({ "id": 1 }));
    let err_params = req.validate(&parts_bad_params).unwrap_err();
    assert_eq!(err_params.len(), 1);
    assert_eq!(err_params.first_issue().unwrap().field, "params.id");

    // Fail query with bail
    let parts_bad_query = RequestParts::new()
        .with_params(json!({ "id": 20 }))
        .with_query(json!({ "page": 0 }));
    let err_query = req.validate(&parts_bad_query).unwrap_err();
    assert_eq!(err_query.len(), 1);
    assert_eq!(err_query.first_issue().unwrap().field, "query.page");

    // Fail headers with bail
    let parts_bad_headers = RequestParts::new()
        .with_params(json!({ "id": 20 }))
        .with_query(json!({ "page": 1 }))
        .with_headers(json!({ "x-api-key": "bad" }));
    let err_headers = req.validate(&parts_bad_headers).unwrap_err();
    assert_eq!(err_headers.len(), 1);
    assert_eq!(
        err_headers.first_issue().unwrap().field,
        "headers.x-api-key"
    );

    // Fail cookies with bail
    let parts_bad_cookies = RequestParts::new()
        .with_params(json!({ "id": 20 }))
        .with_query(json!({ "page": 1 }))
        .with_headers(json!({ "x-api-key": "validkey" }))
        .with_cookies(json!({ "token": "bad" }));
    let err_cookies = req.validate(&parts_bad_cookies).unwrap_err();
    assert_eq!(err_cookies.len(), 1);
    assert_eq!(err_cookies.first_issue().unwrap().field, "cookies.token");

    // Fail body with bail
    let parts_bad_body = RequestParts::new()
        .with_params(json!({ "id": 20 }))
        .with_query(json!({ "page": 1 }))
        .with_headers(json!({ "x-api-key": "validkey" }))
        .with_cookies(json!({ "token": "validtoken" }))
        .with_body(json!({ "email": "not-email" }));
    let err_body = req.validate(&parts_bad_body).unwrap_err();
    assert_eq!(err_body.len(), 1);
    assert_eq!(err_body.first_issue().unwrap().field, "body.email");

    // Async all failures with bail
    assert!(req.validate_async(&parts_bad_params).await.is_err());
    assert!(req.validate_async(&parts_bad_query).await.is_err());
    assert!(req.validate_async(&parts_bad_headers).await.is_err());
    assert!(req.validate_async(&parts_bad_cookies).await.is_err());
    assert!(req.validate_async(&parts_bad_body).await.is_err());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_tuple_async_and_bail_coverage() {
    let s_tup = tuple2(string().min_len(3), integer().min(10)).bail(true);
    let bad_first = json!(["ab", 20]);
    let err1 = s_tup.validate_async(&bad_first).await.unwrap_err();
    assert_eq!(err1.len(), 1);

    let bad_second = json!(["abc", 5]);
    let err2 = s_tup.validate_async(&bad_second).await.unwrap_err();
    assert_eq!(err2.len(), 1);

    let bad_len = json!(["abc"]);
    assert!(s_tup.validate_async(&bad_len).await.is_err());
}

#[test]
fn test_readme_examples_verification() {
    let user_schema = object()
        .field("username", string().min_len(3).max_len(30).trim(true))
        .field("email", string().email().to_lowercase())
        .optional_field("age", integer().min(18).max(120));

    let input = json!({
        "username": "  alice  ",
        "email": "Alice@Example.COM",
        "age": 25,
    });

    let validated = user_schema.validate(&input).unwrap();
    assert_eq!(validated["username"], "alice");
    assert_eq!(validated["email"], "alice@example.com");

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct CreateUserDto {
        username: String,
        email: String,
        age: Option<i64>,
    }

    let user_schema2 = object()
        .field("username", string().min_len(3).trim(true))
        .field("email", string().email())
        .optional_field("age", integer().min(18));

    let input2 = json!({
        "username": "  bob  ",
        "email": "bob@example.com",
        "age": 30,
    });

    let dto: CreateUserDto = user_schema2.validate_and_decode(&input2).unwrap();
    assert_eq!(dto.username, "bob");
    assert_eq!(dto.email, "bob@example.com");
    assert_eq!(dto.age, Some(30));

    let password_schema = object()
        .field("password", string().min_len(8))
        .field("password_confirmation", string().min_len(8))
        .same_as("password_confirmation", "password");

    let input3 = json!({
        "password": "secretpassword",
        "password_confirmation": "mismatched",
    });

    let err = password_schema.validate(&input3).unwrap_err();
    assert_eq!(err.first_issue().unwrap().rule, "same_as");
    assert_eq!(
        err.first_message(),
        Some("The password confirmation and password fields must match.")
    );

    let req_schema = request_schema()
        .query(object().field("page", integer().min(1).default(1)))
        .body(object().field("title", string().min_len(1)));

    let parts = RequestParts::new()
        .with_query(json!({ "page": 2 }))
        .with_body(json!({ "title": "My Post" }));

    let validated_req = req_schema.validate(&parts).unwrap();
    assert_eq!(validated_req.query.unwrap()["page"], 2);
    assert_eq!(validated_req.body.unwrap()["title"], "My Post");

    let s = object().field("email", string().email());
    let err_rfc = s
        .validate(&json!({ "email": "invalid-email" }))
        .unwrap_err();
    let problem_details = err_rfc.to_problem_details(Some("/api/v1/users"));
    assert_eq!(problem_details["status"], 422);
    assert_eq!(
        problem_details["type"],
        "https://tools.ietf.org/html/rfc9457"
    );
    assert_eq!(problem_details["instance"], "/api/v1/users");
}

#[derive(Debug, PartialEq, Eq)]
enum UserRole {
    Admin,
    Member,
    Guest,
}

impl std::str::FromStr for UserRole {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "admin" => Ok(UserRole::Admin),
            "member" => Ok(UserRole::Member),
            "guest" => Ok(UserRole::Guest),
            other => Err(format!("Unknown role: {other}")),
        }
    }
}

#[test]
fn test_coercer_comprehensive() {
    let coercer = Coercer::new();
    assert_eq!(coercer, Coercer::default());

    // In-place sanitization with default settings
    let mut payload = json!({
        "name": "   Alice   ",
        "empty": "   ",
        "nested": {
            "sub_empty": "",
            "bio": "   Rust developer   "
        },
        "tags": ["   tech   ", "   ", "rust"]
    });

    coercer.sanitize_in_place(&mut payload);
    assert_eq!(payload["name"], "Alice");
    assert_eq!(payload["empty"], serde_json::Value::Null);
    assert_eq!(payload["nested"]["sub_empty"], serde_json::Value::Null);
    assert_eq!(payload["nested"]["bio"], "Rust developer");
    assert_eq!(payload["tags"][0], "tech");
    assert_eq!(payload["tags"][1], serde_json::Value::Null);
    assert_eq!(payload["tags"][2], "rust");

    // Disabled empty_strings_to_null
    let coercer_no_null = Coercer::new()
        .empty_strings_to_null(false)
        .trim_strings(true);
    let mut payload2 = json!({ "empty": "   " });
    coercer_no_null.sanitize_in_place(&mut payload2);
    assert_eq!(payload2["empty"], "");

    // Disabled trim_strings
    let coercer_no_trim = Coercer::new()
        .empty_strings_to_null(false)
        .trim_strings(false);
    let payload3 = json!({ "whitespace": "   hello   " });
    let sanitized3 = coercer_no_trim.sanitize(payload3.clone());
    assert_eq!(sanitized3["whitespace"], "   hello   ");

    // normalize_in_place
    let mut payload4 = json!({ "k": "   ", "v": 123 });
    jachai::coercion::normalize_in_place(&mut payload4);
    assert_eq!(payload4["k"], serde_json::Value::Null);
    assert_eq!(payload4["v"], 123);

    // Coercer helper methods
    assert_eq!(Coercer::to_bool(&json!(true)), Some(true));
    assert_eq!(Coercer::to_bool(&json!(false)), Some(false));
    assert_eq!(Coercer::to_bool(&json!("true")), Some(true));
    assert_eq!(Coercer::to_bool(&json!("yes")), Some(true));
    assert_eq!(Coercer::to_bool(&json!("1")), Some(true));
    assert_eq!(Coercer::to_bool(&json!("on")), Some(true));
    assert_eq!(Coercer::to_bool(&json!(1)), Some(true));
    assert_eq!(Coercer::to_bool(&json!("false")), Some(false));
    assert_eq!(Coercer::to_bool(&json!("no")), Some(false));
    assert_eq!(Coercer::to_bool(&json!("0")), Some(false));
    assert_eq!(Coercer::to_bool(&json!("off")), Some(false));
    assert_eq!(Coercer::to_bool(&json!(0)), Some(false));
    assert_eq!(Coercer::to_bool(&json!(42)), None);
    assert_eq!(Coercer::to_bool(&json!("unrelated")), None);
    assert_eq!(Coercer::to_bool(&json!(null)), None);

    assert_eq!(
        Coercer::to_number(&json!(42)),
        Some(serde_json::Number::from(42))
    );
    assert_eq!(
        Coercer::to_number(&json!("42")),
        Some(serde_json::Number::from(42))
    );
    assert_eq!(
        Coercer::to_number(&json!("18446744073709551615")),
        Some(serde_json::Number::from(18446744073709551615u64))
    );
    assert_eq!(
        Coercer::to_number(&json!("3.25")),
        serde_json::Number::from_f64(3.25)
    );
    assert_eq!(Coercer::to_number(&json!("invalid")), None);
    assert_eq!(Coercer::to_number(&json!(true)), None);

    assert_eq!(Coercer::to_i64(&json!(42)), Some(42));
    assert_eq!(Coercer::to_i64(&json!("42")), Some(42));
    assert_eq!(Coercer::to_i64(&json!("invalid")), None);

    assert_eq!(Coercer::to_u64(&json!(42)), Some(42));
    assert_eq!(Coercer::to_u64(&json!("42")), Some(42));
    assert_eq!(Coercer::to_u64(&json!("-5")), None);

    assert_eq!(Coercer::to_f64(&json!(42.5)), Some(42.5));
    assert_eq!(Coercer::to_f64(&json!("42.5")), Some(42.5));
    assert_eq!(Coercer::to_f64(&json!("invalid")), None);

    assert_eq!(Coercer::to_string(&json!("test")), Some("test".to_string()));
    assert_eq!(Coercer::to_string(&json!(123)), Some("123".to_string()));
    assert_eq!(Coercer::to_string(&json!(true)), Some("true".to_string()));
    assert_eq!(Coercer::to_string(&json!(null)), None);

    assert_eq!(
        Coercer::to_array(&json!([1, 2])),
        Some(vec![json!(1), json!(2)])
    );
    assert_eq!(Coercer::to_array(&json!("solo")), Some(vec![json!("solo")]));
    assert_eq!(Coercer::to_array(&json!(null)), None);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_tuple_combinators_3_and_4() {
    // tuple alias
    let t2 = tuple(string(), integer());
    assert_eq!(
        t2.validate(&json!(["hello", 123])).unwrap(),
        ("hello".to_string(), 123)
    );

    // Tuple3Schema sync
    let t3 = tuple3(string(), integer(), boolean())
        .bail(true)
        .with_message("Tuple3 failure");

    let val3 = json!(["Alice", 30, true]);
    assert_eq!(t3.validate(&val3).unwrap(), ("Alice".to_string(), 30, true));

    // Tuple3 invalid length
    let err_len3 = t3.validate(&json!(["Alice", 30])).unwrap_err();
    assert_eq!(err_len3.first_issue().unwrap().rule, "exact_items");
    assert_eq!(err_len3.first_message(), Some("Tuple3 failure"));

    // Tuple3 item errors
    let err_items3 = tuple3(string(), integer(), boolean())
        .validate(&json!(["Alice", "bad_int", "bad_bool"]))
        .unwrap_err();
    assert!(err_items3.issues().len() >= 2);

    // Tuple3Schema async
    let res_async3 = t3.validate_async(&val3).await.unwrap();
    assert_eq!(res_async3, ("Alice".to_string(), 30, true));

    let err_async_len3 = t3.validate_async(&json!(["Alice"])).await.unwrap_err();
    assert_eq!(err_async_len3.first_issue().unwrap().rule, "exact_items");

    // Tuple4Schema sync
    let t4 = tuple4(string(), integer(), boolean(), number())
        .bail(true)
        .with_message("Tuple4 failure");

    let val4 = json!(["Bob", 40, false, 99.5]);
    assert_eq!(
        t4.validate(&val4).unwrap(),
        ("Bob".to_string(), 40, false, 99.5)
    );

    // Tuple4 invalid length
    let err_len4 = t4.validate(&json!(["Bob", 40, false])).unwrap_err();
    assert_eq!(err_len4.first_issue().unwrap().rule, "exact_items");
    assert_eq!(err_len4.first_message(), Some("Tuple4 failure"));

    // Tuple4 item errors
    let err_items4 = tuple4(string(), integer(), boolean(), number())
        .validate(&json!(["Bob", "not_an_int", "not_a_bool", "not_a_number"]))
        .unwrap_err();
    assert!(err_items4.issues().len() >= 3);

    // Tuple4Schema async
    let res_async4 = t4.validate_async(&val4).await.unwrap();
    assert_eq!(res_async4, ("Bob".to_string(), 40, false, 99.5));

    let err_async_len4 = t4.validate_async(&json!(["Bob"])).await.unwrap_err();
    assert_eq!(err_async_len4.first_issue().unwrap().rule, "exact_items");
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_union_combinators_3_and_4() {
    // union alias
    let u2 = union(integer().map(|i| i.to_string()), string());
    assert_eq!(u2.validate(&json!(99)).unwrap(), "99");
    assert_eq!(
        u2.validate(&json!("already_string")).unwrap(),
        "already_string"
    );

    // Union3Schema sync
    let u3 = union3(
        integer().min(100).map(|i| format!("int:{i}")),
        boolean().map(|b| format!("bool:{b}")),
        string().min_len(5).map(|s| format!("str:{s}")),
    )
    .with_message("Union3 failure");

    assert_eq!(u3.validate(&json!(150)).unwrap(), "int:150");
    assert_eq!(u3.validate(&json!(true)).unwrap(), "bool:true");
    assert_eq!(u3.validate(&json!("superhero")).unwrap(), "str:superhero");

    let err_u3 = u3.validate(&json!("tiny")).unwrap_err();
    assert_eq!(err_u3.first_issue().unwrap().rule, "union");
    assert_eq!(err_u3.first_message(), Some("Union3 failure"));

    // Union3Schema without inline message (merged errors)
    let u3_no_msg = union3(
        integer().min(100).map(|i| format!("int:{i}")),
        boolean().map(|b| format!("bool:{b}")),
        string().min_len(5).map(|s| format!("str:{s}")),
    );
    let err_merged3 = u3_no_msg.validate(&json!("tiny")).unwrap_err();
    assert!(err_merged3.issues().len() >= 2);

    // Union3Schema async
    assert_eq!(u3.validate_async(&json!(150)).await.unwrap(), "int:150");
    assert_eq!(
        u3.validate_async(&json!(false)).await.unwrap(),
        "bool:false"
    );
    assert_eq!(
        u3.validate_async(&json!("superhero")).await.unwrap(),
        "str:superhero"
    );
    let err_async3 = u3.validate_async(&json!("tiny")).await.unwrap_err();
    assert_eq!(err_async3.first_issue().unwrap().rule, "union");

    let err_async_merged3 = u3_no_msg.validate_async(&json!("tiny")).await.unwrap_err();
    assert!(err_async_merged3.issues().len() >= 2);

    // Union4Schema sync
    let u4 = union4(
        integer().min(100).map(|i| format!("int:{i}")),
        boolean().map(|b| format!("bool:{b}")),
        string().min_len(5).map(|s| format!("str:{s}")),
        number().map(|n| format!("num:{n}")),
    )
    .with_message("Union4 failure");

    assert_eq!(u4.validate(&json!(200)).unwrap(), "int:200");
    assert_eq!(u4.validate(&json!(false)).unwrap(), "bool:false");
    assert_eq!(u4.validate(&json!("wonderland")).unwrap(), "str:wonderland");
    assert_eq!(u4.validate(&json!(12.5)).unwrap(), "num:12.5");

    let err_u4 = u4.validate(&json!("xyz")).unwrap_err();
    assert_eq!(err_u4.first_issue().unwrap().rule, "union");
    assert_eq!(err_u4.first_message(), Some("Union4 failure"));

    // Union4Schema without inline message (merged errors)
    let u4_no_msg = union4(
        integer().min(100).map(|i| format!("int:{i}")),
        boolean().map(|b| format!("bool:{b}")),
        string().min_len(5).map(|s| format!("str:{s}")),
        number().map(|n| format!("num:{n}")),
    );
    let err_merged4 = u4_no_msg.validate(&json!("xyz")).unwrap_err();
    assert!(err_merged4.issues().len() >= 2);

    // Union4Schema async
    assert_eq!(u4.validate_async(&json!(200)).await.unwrap(), "int:200");
    assert_eq!(u4.validate_async(&json!(true)).await.unwrap(), "bool:true");
    assert_eq!(
        u4.validate_async(&json!("wonderland")).await.unwrap(),
        "str:wonderland"
    );
    assert_eq!(u4.validate_async(&json!(12.5)).await.unwrap(), "num:12.5");
    let err_async4 = u4.validate_async(&json!("xyz")).await.unwrap_err();
    assert_eq!(err_async4.first_issue().unwrap().rule, "union");

    let err_async_merged4 = u4_no_msg.validate_async(&json!("xyz")).await.unwrap_err();
    assert!(err_async_merged4.issues().len() >= 2);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_max_coverage_edge_branches() {
    // RuleContext context() and get_ambient()
    let mut v_ctx = ValidationContext::new();
    v_ctx.insert("ambient_tenant_id".to_string());
    let provider = SimpleMessagesProvider::default();
    let parent = json!({ "age": 25 });
    let r_ctx = RuleContext::new("user.age", &v_ctx, Some(&parent), &provider);
    assert_eq!(r_ctx.path(), "user.age");
    assert_eq!(r_ctx.get_ambient::<String>().unwrap(), "ambient_tenant_id");
    assert_eq!(
        r_ctx.context().get::<String>().unwrap(),
        "ambient_tenant_id"
    );

    // Tuple2Schema edge cases: bail at first vs second
    let t2_bail = tuple2(integer().min(10), integer().min(10)).bail(true);
    let err2_first = t2_bail.validate(&json!([1, 1])).unwrap_err();
    assert_eq!(err2_first.issues().len(), 1);
    assert_eq!(err2_first.issues()[0].field, "[0]");

    let err2_second = t2_bail.validate(&json!([20, 1])).unwrap_err();
    assert_eq!(err2_second.issues().len(), 1);
    assert_eq!(err2_second.issues()[0].field, "[1]");

    // Async Tuple2 bail
    let err2_first_async = t2_bail.validate_async(&json!([1, 1])).await.unwrap_err();
    assert_eq!(err2_first_async.issues().len(), 1);
    let err2_second_async = t2_bail.validate_async(&json!([20, 1])).await.unwrap_err();
    assert_eq!(err2_second_async.issues().len(), 1);

    // Non-array inputs for Tuple2, Tuple3, Tuple4
    assert!(
        tuple2(integer(), integer())
            .validate(&json!("not_array"))
            .is_err()
    );
    assert!(
        tuple2(integer(), integer())
            .validate_async(&json!("not_array"))
            .await
            .is_err()
    );
    assert!(
        tuple3(integer(), integer(), integer())
            .validate(&json!("not_array"))
            .is_err()
    );
    assert!(
        tuple3(integer(), integer(), integer())
            .validate_async(&json!("not_array"))
            .await
            .is_err()
    );
    assert!(
        tuple4(integer(), integer(), integer(), integer())
            .validate(&json!("not_array"))
            .is_err()
    );
    assert!(
        tuple4(integer(), integer(), integer(), integer())
            .validate_async(&json!("not_array"))
            .await
            .is_err()
    );

    // Tuple3 bail edge cases (sync and async)
    let t3_bail = tuple3(integer().min(10), integer().min(10), integer().min(10)).bail(true);
    // element 0 fails
    assert_eq!(
        t3_bail
            .validate(&json!([1, 20, 20]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t3_bail
            .validate_async(&json!([1, 20, 20]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    // element 1 fails
    assert_eq!(
        t3_bail
            .validate(&json!([20, 1, 20]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t3_bail
            .validate_async(&json!([20, 1, 20]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    // element 2 fails
    assert_eq!(
        t3_bail
            .validate(&json!([20, 20, 1]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t3_bail
            .validate_async(&json!([20, 20, 1]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );

    // Tuple3 length mismatch async
    assert!(
        tuple3(integer(), integer(), integer())
            .validate_async(&json!([1, 2]))
            .await
            .is_err()
    );
    assert!(
        tuple3(integer(), integer(), integer())
            .validate_async(&json!([1, 2, 3, 4]))
            .await
            .is_err()
    );

    // Tuple4 bail edge cases (sync and async)
    let t4_bail = tuple4(
        integer().min(10),
        integer().min(10),
        integer().min(10),
        integer().min(10),
    )
    .bail(true);
    // element 0 fails
    assert_eq!(
        t4_bail
            .validate(&json!([1, 20, 20, 20]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t4_bail
            .validate_async(&json!([1, 20, 20, 20]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    // element 1 fails
    assert_eq!(
        t4_bail
            .validate(&json!([20, 1, 20, 20]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t4_bail
            .validate_async(&json!([20, 1, 20, 20]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    // element 2 fails
    assert_eq!(
        t4_bail
            .validate(&json!([20, 20, 1, 20]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t4_bail
            .validate_async(&json!([20, 20, 1, 20]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    // element 3 fails
    assert_eq!(
        t4_bail
            .validate(&json!([20, 20, 20, 1]))
            .unwrap_err()
            .issues()
            .len(),
        1
    );
    assert_eq!(
        t4_bail
            .validate_async(&json!([20, 20, 20, 1]))
            .await
            .unwrap_err()
            .issues()
            .len(),
        1
    );

    // Tuple4 length mismatch async
    assert!(
        tuple4(integer(), integer(), integer(), integer())
            .validate_async(&json!([1, 2, 3]))
            .await
            .is_err()
    );
    assert!(
        tuple4(integer(), integer(), integer(), integer())
            .validate_async(&json!([1, 2, 3, 4, 5]))
            .await
            .is_err()
    );

    // Nested tuples inside objects (testing non-empty path prefix formatting)
    let s_nested_tup = object()
        .field("point3", tuple3(integer(), integer(), integer()))
        .field("quad", tuple4(integer(), integer(), integer(), integer()));

    let val_nested = json!({
        "point3": [10, "bad", 30],
        "quad": [1, 2, "bad", 4]
    });
    let err_nested = s_nested_tup.validate(&val_nested).unwrap_err();
    let fields: Vec<String> = err_nested
        .issues()
        .iter()
        .map(|i| i.field.clone())
        .collect();
    assert!(fields.contains(&"point3[1]".to_string()));
    assert!(fields.contains(&"quad[2]".to_string()));

    let err_nested_async = s_nested_tup.validate_async(&val_nested).await.unwrap_err();
    let fields_async: Vec<String> = err_nested_async
        .issues()
        .iter()
        .map(|i| i.field.clone())
        .collect();
    assert!(fields_async.contains(&"point3[1]".to_string()));
    assert!(fields_async.contains(&"quad[2]".to_string()));

    // Nested tuples with invalid length inside object
    let val_nested_lens = json!({
        "point3": [10],
        "quad": [1, 2]
    });
    let err_nested_lens = s_nested_tup.validate(&val_nested_lens).unwrap_err();
    assert_eq!(err_nested_lens.issues()[0].field, "point3");

    let err_nested_lens_async = s_nested_tup
        .validate_async(&val_nested_lens)
        .await
        .unwrap_err();
    assert_eq!(err_nested_lens_async.issues()[0].field, "point3");

    // Array async predicates coverage: not_empty, min_items, max_items, distinct
    let arr_not_empty_async = array(integer()).not_empty().bail(true);
    assert!(
        arr_not_empty_async
            .validate_async(&json!([]))
            .await
            .is_err()
    );

    let arr_min_async = array(integer()).min_items(3).bail(true);
    assert!(arr_min_async.validate_async(&json!([1, 2])).await.is_err());

    let arr_max_async = array(integer()).max_items(2).bail(true);
    assert!(
        arr_max_async
            .validate_async(&json!([1, 2, 3]))
            .await
            .is_err()
    );

    let arr_distinct_async = array(integer()).distinct().bail(true);
    assert!(
        arr_distinct_async
            .validate_async(&json!([1, 2, 1]))
            .await
            .is_err()
    );

    let arr_distinct_no_bail = array(integer()).distinct();
    assert!(
        arr_distinct_no_bail
            .validate_async(&json!([1, 2, 1]))
            .await
            .is_err()
    );

    // String async multiple rules and bail
    struct AsyncPrefixRule;
    impl AsyncCustomRule<String> for AsyncPrefixRule {
        fn validate<'a>(
            &'a self,
            value: &'a String,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            Box::pin(async move {
                if !value.starts_with("prefix_") {
                    Err(ValidationIssue::new(
                        ctx.path(),
                        "prefix",
                        "Must start with prefix_",
                    ))
                } else {
                    Ok(())
                }
            })
        }
    }

    struct AsyncSuffixRule;
    impl AsyncCustomRule<String> for AsyncSuffixRule {
        fn validate<'a>(
            &'a self,
            value: &'a String,
            ctx: &'a RuleContext<'a>,
        ) -> futures_util::future::BoxFuture<'a, Result<(), ValidationIssue>> {
            Box::pin(async move {
                if !value.ends_with("_suffix") {
                    Err(ValidationIssue::new(
                        ctx.path(),
                        "suffix",
                        "Must end with _suffix",
                    ))
                } else {
                    Ok(())
                }
            })
        }
    }

    let s_str_async_multi = string()
        .bail(true)
        .rule_async(AsyncPrefixRule)
        .rule_async(AsyncSuffixRule);

    assert_eq!(
        s_str_async_multi
            .validate_async(&json!("prefix_test_suffix"))
            .await
            .unwrap(),
        "prefix_test_suffix"
    );
    let err_str_prefix = s_str_async_multi
        .validate_async(&json!("bad_test_suffix"))
        .await
        .unwrap_err();
    assert_eq!(err_str_prefix.issues()[0].rule, "prefix");
    let err_str_suffix = s_str_async_multi
        .validate_async(&json!("prefix_test_bad"))
        .await
        .unwrap_err();
    assert_eq!(err_str_suffix.issues()[0].rule, "suffix");

    // String sync failure bypasses async rules
    let err_str_sync_fail = s_str_async_multi
        .validate_async(&json!(12345))
        .await
        .unwrap_err();
    assert_eq!(err_str_sync_fail.issues()[0].rule, "string");

    // Number predicates extra: multiple_of negative/edge cases, int predicate on floats
    let s_num_step = number().multiple_of(2.5);
    assert!(s_num_step.validate(&json!(7.5)).is_ok());
    assert!(s_num_step.validate(&json!(7.0)).is_err());

    let s_num_int = number().int();
    assert!(s_num_int.validate(&json!(42.0)).is_ok());
    assert!(s_num_int.validate(&json!(42.7)).is_err());

    let s_num_pos = number().positive();
    assert!(s_num_pos.validate(&json!(1.0)).is_ok());
    assert!(s_num_pos.validate(&json!(0.0)).is_err());
    assert!(s_num_pos.validate(&json!(-1.0)).is_err());

    let s_num_neg = number().negative();
    assert!(s_num_neg.validate(&json!(-5.0)).is_ok());
    assert!(s_num_neg.validate(&json!(0.0)).is_err());
    assert!(s_num_neg.validate(&json!(5.0)).is_err());
}

#[test]
fn test_enum_of_alias_coverage() {
    let role_schema = enum_of::<UserRole>();
    assert_eq!(
        role_schema.validate(&json!("admin")).unwrap(),
        UserRole::Admin
    );
    assert_eq!(
        role_schema.validate(&json!("member")).unwrap(),
        UserRole::Member
    );
    assert_eq!(
        role_schema.validate(&json!("guest")).unwrap(),
        UserRole::Guest
    );
    assert!(role_schema.validate(&json!("superadmin")).is_err());

    // validate_with_context on various schema types
    let ctx = ValidationContext::new();
    assert_eq!(
        string()
            .validate_with_context(&json!("test"), &ctx)
            .unwrap(),
        "test"
    );
    assert_eq!(
        integer().validate_with_context(&json!(42), &ctx).unwrap(),
        42
    );
    assert!(boolean().validate_with_context(&json!(true), &ctx).unwrap());
    assert_eq!(
        number().validate_with_context(&json!(3.25), &ctx).unwrap(),
        3.25
    );
    assert_eq!(
        array(integer())
            .validate_with_context(&json!([1, 2]), &ctx)
            .unwrap(),
        vec![1, 2]
    );
}
