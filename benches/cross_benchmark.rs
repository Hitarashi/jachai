//! Exhaustive cross-ecosystem validation performance benchmark runner.
//! Touches every single validator rule, schema builder, and domain feature in jachai,
//! comparing against garde, validator, and zod-rs across canonical datasets.

use std::{collections::BTreeMap, fs, hint::black_box, time::Instant};

use garde::Validate as GardeValidate;
use jachai::{
    request::{RequestParts, RequestSchema},
    schema::Schema as JachaiSchema,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::Validate as ValidatorValidate;
use zod_rs::prelude::{
    ArraySchema as ZodArraySchema, Schema as ZodSchema, number as zod_number, object as zod_object,
    string as zod_string,
};

// --- Jachai Typed Structs ---
#[derive(Deserialize, Clone)]
struct JachaiFlat {
    username: String,
    email: String,
    age: i64,
}

impl JachaiFlat {
    #[inline(always)]
    fn validate(
        &self,
        username_s: &jachai::schema::StringSchema,
        email_s: &jachai::schema::StringSchema,
        age_s: &jachai::schema::IntegerSchema,
    ) -> Result<(), jachai::ValidationError> {
        username_s.check_str(&self.username)?;
        email_s.check_str(&self.email)?;
        age_s.check_i64(self.age)?;
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
struct JachaiProfile {
    full_name: String,
    city: String,
}

#[derive(Deserialize, Clone)]
struct JachaiNested {
    username: String,
    email: String,
    profile: JachaiProfile,
}

impl JachaiNested {
    #[inline(always)]
    fn validate(
        &self,
        username_s: &jachai::schema::StringSchema,
        email_s: &jachai::schema::StringSchema,
        full_name_s: &jachai::schema::StringSchema,
        city_s: &jachai::schema::StringSchema,
    ) -> Result<(), jachai::ValidationError> {
        username_s.check_str(&self.username)?;
        email_s.check_str(&self.email)?;
        full_name_s.check_str(&self.profile.full_name)?;
        city_s.check_str(&self.profile.city)?;
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
struct JachaiItem {
    name: String,
    email: String,
    score: i64,
}

#[derive(Deserialize, Clone)]
struct JachaiArrayBatch {
    items: Vec<JachaiItem>,
}

impl JachaiArrayBatch {
    #[inline(always)]
    fn validate(
        &self,
        name_s: &jachai::schema::StringSchema,
        email_s: &jachai::schema::StringSchema,
        score_s: &jachai::schema::IntegerSchema,
    ) -> Result<(), jachai::ValidationError> {
        for item in &self.items {
            name_s.check_str(&item.name)?;
            email_s.check_str(&item.email)?;
            score_s.check_i64(item.score)?;
        }
        Ok(())
    }
}

// --- Garde Structs ---
#[derive(garde::Validate, Deserialize, Clone)]
struct GardeFlat {
    #[garde(length(min = 3, max = 30))]
    username: String,
    #[garde(email)]
    email: String,
    #[garde(range(min = 18, max = 100))]
    age: i64,
}

#[derive(garde::Validate, Deserialize, Clone)]
struct GardeProfile {
    #[garde(length(min = 1, max = 100))]
    full_name: String,
    #[garde(length(min = 1, max = 50))]
    city: String,
}

#[derive(garde::Validate, Deserialize, Clone)]
struct GardeNested {
    #[garde(length(min = 3, max = 30))]
    username: String,
    #[garde(email)]
    email: String,
    #[garde(dive)]
    profile: GardeProfile,
}

#[derive(garde::Validate, Deserialize, Clone)]
struct GardeItem {
    #[garde(length(min = 1, max = 50))]
    name: String,
    #[garde(email)]
    email: String,
    #[garde(range(min = 0, max = 100))]
    score: i64,
}

#[derive(garde::Validate, Deserialize, Clone)]
struct GardeArrayBatch {
    #[garde(dive)]
    items: Vec<GardeItem>,
}

#[derive(garde::Validate, Clone)]
struct GardeEmailPrimitive<'a> {
    #[garde(email)]
    email: &'a str,
}

#[derive(garde::Validate, Clone)]
struct GardeUrlPrimitive<'a> {
    #[garde(url)]
    url: &'a str,
}

#[derive(garde::Validate, Clone)]
struct GardeIpPrimitive<'a> {
    #[garde(ip)]
    ip: &'a str,
}

#[derive(garde::Validate, Clone)]
struct GardeCreditCardPrimitive<'a> {
    #[garde(credit_card)]
    card: &'a str,
}

// --- Validator Structs ---
#[derive(validator::Validate, Deserialize, Clone)]
struct ValidatorFlat {
    #[validate(length(min = 3, max = 30))]
    username: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 18, max = 100))]
    age: i64,
}

#[derive(validator::Validate, Deserialize, Clone)]
struct ValidatorProfile {
    #[validate(length(min = 1, max = 100))]
    full_name: String,
    #[validate(length(min = 1, max = 50))]
    city: String,
}

#[derive(validator::Validate, Deserialize, Clone)]
struct ValidatorNested {
    #[validate(length(min = 3, max = 30))]
    username: String,
    #[validate(email)]
    email: String,
    #[validate(nested)]
    profile: ValidatorProfile,
}

#[derive(validator::Validate, Deserialize, Clone)]
struct ValidatorItem {
    #[validate(length(min = 1, max = 50))]
    name: String,
    #[validate(email)]
    email: String,
    #[validate(range(min = 0, max = 100))]
    score: i64,
}

#[derive(validator::Validate, Deserialize, Clone)]
struct ValidatorArrayBatch {
    #[validate(nested)]
    items: Vec<ValidatorItem>,
}

#[derive(validator::Validate, Clone)]
struct ValidatorEmailPrimitive<'a> {
    #[validate(email)]
    email: &'a str,
}

#[derive(validator::Validate, Clone)]
struct ValidatorUrlPrimitive<'a> {
    #[validate(url)]
    url: &'a str,
}

#[derive(validator::Validate, Clone)]
struct ValidatorIpPrimitive<'a> {
    #[validate(ip)]
    ip: &'a str,
}

#[derive(validator::Validate, Clone)]
struct ValidatorCreditCardPrimitive<'a> {
    #[validate(credit_card)]
    card: &'a str,
}

// --- Comprehensive Corpus Deserialization ---
#[derive(Deserialize)]
struct Corpus {
    flat_valid: Vec<Value>,
    flat_invalid: Vec<Value>,
    nested_valid: Vec<Value>,
    nested_invalid: Vec<Value>,
    array_valid: Vec<Value>,
    array_invalid: Vec<Value>,
    array_unique_valid: Vec<Value>,
    records_valid: Vec<Value>,
    tuples_valid: Vec<Value>,
    enums_valid: Vec<String>,
    datetimes_valid: Vec<String>,
    dates_valid: Vec<String>,
    times_valid: Vec<String>,
    ipv4_valid: Vec<String>,
    ipv6_valid: Vec<String>,
    urls_valid: Vec<String>,
    mac_valid: Vec<String>,
    cidr_valid: Vec<String>,
    domain_valid: Vec<String>,
    primitive_emails_valid: Vec<String>,
    primitive_emails_invalid: Vec<String>,
    primitive_uuids_valid: Vec<String>,
    ulids_valid: Vec<String>,
    semvers_valid: Vec<String>,
    slugs_valid: Vec<String>,
    credit_cards_valid: Vec<String>,
    ibans_valid: Vec<String>,
    currency_codes_valid: Vec<String>,
    latitudes_valid: Vec<String>,
    longitudes_valid: Vec<String>,
    coordinates_valid: Vec<String>,
    country_codes_valid: Vec<String>,
    hex_colors_valid: Vec<String>,
    sha256_valid: Vec<String>,
    base64_valid: Vec<String>,
    jwt_valid: Vec<String>,
    confirmed_objects_valid: Vec<Value>,
    after_date_objects_valid: Vec<Value>,
    required_if_objects_valid: Vec<Value>,
    request_parts_valid: Vec<Value>,
    primitive_integers_valid: Vec<i64>,
    primitive_floats_valid: Vec<f64>,
    primitive_booleans_valid: Vec<bool>,
    string_rules_valid: Vec<String>,
    files_valid: Vec<Value>,
    union_valid: Vec<Value>,
}

#[derive(Serialize)]
struct BenchmarkMetric {
    name: String,
    total_ops: usize,
    duration_ms: f64,
    ns_per_op: f64,
    m_ops_per_sec: f64,
}

fn run_benchmark<T, F>(name: &str, data: &[T], mut op: F, target_ops: usize) -> BenchmarkMetric
where
    F: FnMut(&T),
{
    let iterations = (target_ops / data.len()).max(1);

    // Warmup to stabilize CPU frequency scaling and cache lines
    let warmup_batches = (5_000 / data.len()).max(10);
    for _ in 0..warmup_batches {
        for item in data {
            op(item);
        }
    }

    let start = Instant::now();
    let mut count = 0;
    for _ in 0..iterations {
        for item in data {
            op(item);
            count += 1;
        }
    }
    let elapsed = start.elapsed();
    let duration_ns = elapsed.as_nanos() as f64;
    let duration_ms = elapsed.as_secs_f64() * 1000.0;
    let ns_per_op = duration_ns / count as f64;
    let m_ops_per_sec = (count as f64 / elapsed.as_secs_f64()) / 1_000_000.0;

    BenchmarkMetric {
        name: name.to_string(),
        total_ops: count,
        duration_ms,
        ns_per_op,
        m_ops_per_sec,
    }
}

fn main() {
    let corpus_content = fs::read_to_string("benches/data/corpus.json")
        .expect("Failed to read corpus.json. Please generate it first.");
    let corpus: Corpus =
        serde_json::from_str(&corpus_content).expect("Failed to parse corpus.json");

    let mut results: BTreeMap<String, Vec<BenchmarkMetric>> = BTreeMap::new();

    // 1. FLAT OBJECT (Valid)
    {
        let jachai_schema = jachai::schema::object()
            .field("username", jachai::schema::string().min_len(3).max_len(30))
            .field("email", jachai::schema::string().email())
            .field("age", jachai::schema::integer().between(18, 100));

        let zod_schema = zod_object()
            .field("username", zod_string().min(3).max(30))
            .field("email", zod_string().email())
            .field("age", zod_number().min(18.0).max(100.0));

        let username_s = jachai::schema::string().min_len(3).max_len(30);
        let email_s = jachai::schema::string().email();
        let age_s = jachai::schema::integer().between(18, 100);

        let jachai_flat_data: Vec<JachaiFlat> = corpus
            .flat_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let garde_data: Vec<GardeFlat> = corpus
            .flat_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let validator_data: Vec<ValidatorFlat> = corpus
            .flat_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct)",
                &jachai_flat_data,
                |s| {
                    let res = s.validate(&username_s, &email_s, &age_s);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.flat_valid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.flat_valid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "zod-rs",
                &corpus.flat_valid,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
        ];
        results.insert("flat_valid".to_string(), metrics);
    }

    // 2. FLAT OBJECT (Invalid / Error Path)
    {
        let jachai_schema = jachai::schema::object()
            .field("username", jachai::schema::string().min_len(3).max_len(30))
            .field("email", jachai::schema::string().email())
            .field("age", jachai::schema::integer().between(18, 100));

        let zod_schema = zod_object()
            .field("username", zod_string().min(3).max(30))
            .field("email", zod_string().email())
            .field("age", zod_number().min(18.0).max(100.0));

        let username_s = jachai::schema::string().min_len(3).max_len(30);
        let email_s = jachai::schema::string().email();
        let age_s = jachai::schema::integer().between(18, 100);

        let jachai_flat_invalid: Vec<JachaiFlat> = corpus
            .flat_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(JachaiFlat {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    age: 5,
                })
            })
            .collect();

        let garde_data: Vec<GardeFlat> = corpus
            .flat_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(GardeFlat {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    age: 5,
                })
            })
            .collect();

        let validator_data: Vec<ValidatorFlat> = corpus
            .flat_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(ValidatorFlat {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    age: 5,
                })
            })
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct)",
                &jachai_flat_invalid,
                |s| {
                    let res = s.validate(&username_s, &email_s, &age_s);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.flat_invalid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.flat_invalid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "zod-rs",
                &corpus.flat_invalid,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                100_000,
            ),
        ];
        results.insert("flat_invalid".to_string(), metrics);
    }

    // 3. NESTED OBJECT (Valid)
    {
        let jachai_schema = jachai::schema::object()
            .field("username", jachai::schema::string().min_len(3).max_len(30))
            .field("email", jachai::schema::string().email())
            .field(
                "profile",
                jachai::schema::object()
                    .field(
                        "full_name",
                        jachai::schema::string().min_len(1).max_len(100),
                    )
                    .field("city", jachai::schema::string().min_len(1).max_len(50)),
            );

        let zod_profile = zod_object()
            .field("full_name", zod_string().min(1).max(100))
            .field("city", zod_string().min(1).max(50));
        let zod_schema = zod_object()
            .field("username", zod_string().min(3).max(30))
            .field("email", zod_string().email())
            .field("profile", zod_profile);

        let username_s = jachai::schema::string().min_len(3).max_len(30);
        let email_s = jachai::schema::string().email();
        let full_name_s = jachai::schema::string().min_len(1).max_len(100);
        let city_s = jachai::schema::string().min_len(1).max_len(50);

        let jachai_nested_data: Vec<JachaiNested> = corpus
            .nested_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let garde_data: Vec<GardeNested> = corpus
            .nested_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let validator_data: Vec<ValidatorNested> = corpus
            .nested_valid
            .iter()
            .map(|v| serde_json::from_value(v.clone()).unwrap())
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct)",
                &jachai_nested_data,
                |s| {
                    let res = s.validate(&username_s, &email_s, &full_name_s, &city_s);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.nested_valid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.nested_valid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "zod-rs",
                &corpus.nested_valid,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
        ];
        results.insert("nested_valid".to_string(), metrics);
    }

    // 4. NESTED OBJECT (Invalid)
    {
        let jachai_schema = jachai::schema::object()
            .field("username", jachai::schema::string().min_len(3).max_len(30))
            .field("email", jachai::schema::string().email())
            .field(
                "profile",
                jachai::schema::object()
                    .field(
                        "full_name",
                        jachai::schema::string().min_len(1).max_len(100),
                    )
                    .field("city", jachai::schema::string().min_len(1).max_len(50)),
            );

        let zod_profile = zod_object()
            .field("full_name", zod_string().min(1).max(100))
            .field("city", zod_string().min(1).max(50));
        let zod_schema = zod_object()
            .field("username", zod_string().min(3).max(30))
            .field("email", zod_string().email())
            .field("profile", zod_profile);

        let username_s = jachai::schema::string().min_len(3).max_len(30);
        let email_s = jachai::schema::string().email();
        let full_name_s = jachai::schema::string().min_len(1).max_len(100);
        let city_s = jachai::schema::string().min_len(1).max_len(50);

        let jachai_nested_invalid: Vec<JachaiNested> = corpus
            .nested_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(JachaiNested {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    profile: JachaiProfile {
                        full_name: "".into(),
                        city: "".into(),
                    },
                })
            })
            .collect();

        let garde_data: Vec<GardeNested> = corpus
            .nested_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(GardeNested {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    profile: GardeProfile {
                        full_name: "".into(),
                        city: "".into(),
                    },
                })
            })
            .collect();

        let validator_data: Vec<ValidatorNested> = corpus
            .nested_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(ValidatorNested {
                    username: "x".into(),
                    email: "not-an-email".into(),
                    profile: ValidatorProfile {
                        full_name: "".into(),
                        city: "".into(),
                    },
                })
            })
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct)",
                &jachai_nested_invalid,
                |s| {
                    let res = s.validate(&username_s, &email_s, &full_name_s, &city_s);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.nested_invalid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.nested_invalid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "zod-rs",
                &corpus.nested_invalid,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                100_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                100_000,
            ),
        ];
        results.insert("nested_invalid".to_string(), metrics);
    }

    // 5. ARRAY OF OBJECTS (Valid)
    {
        let jachai_item = jachai::schema::object()
            .field("name", jachai::schema::string().min_len(1).max_len(50))
            .field("email", jachai::schema::string().email())
            .field("score", jachai::schema::integer().between(0, 100));
        let jachai_schema = jachai::schema::array(jachai_item);

        let zod_item = zod_object()
            .field("name", zod_string().min(1).max(50))
            .field("email", zod_string().email())
            .field("score", zod_number().min(0.0).max(100.0));
        let zod_schema = ZodArraySchema::new(zod_item);

        let name_s = jachai::schema::string().min_len(1).max_len(50);
        let email_s = jachai::schema::string().email();
        let score_s = jachai::schema::integer().between(0, 100);

        let jachai_array_data: Vec<JachaiArrayBatch> = corpus
            .array_valid
            .iter()
            .map(|v| JachaiArrayBatch {
                items: serde_json::from_value(v.clone()).unwrap(),
            })
            .collect();

        let garde_data: Vec<GardeArrayBatch> = corpus
            .array_valid
            .iter()
            .map(|v| GardeArrayBatch {
                items: serde_json::from_value(v.clone()).unwrap(),
            })
            .collect();

        let validator_data: Vec<ValidatorArrayBatch> = corpus
            .array_valid
            .iter()
            .map(|v| ValidatorArrayBatch {
                items: serde_json::from_value(v.clone()).unwrap(),
            })
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct batch)",
                &jachai_array_data,
                |batch| {
                    let res = batch.validate(&name_s, &email_s, &score_s);
                    black_box(res).unwrap();
                },
                50_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.array_valid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res).unwrap();
                },
                50_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.array_valid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                50_000,
            ),
            run_benchmark(
                "zod-rs",
                &corpus.array_valid,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                50_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                50_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res).unwrap();
                },
                50_000,
            ),
        ];
        results.insert("array_valid".to_string(), metrics);
    }

    // 6. ARRAY OF OBJECTS (Invalid)
    {
        let jachai_item = jachai::schema::object()
            .field("name", jachai::schema::string().min_len(1).max_len(50))
            .field("email", jachai::schema::string().email())
            .field("score", jachai::schema::integer().between(0, 100));
        let jachai_schema = jachai::schema::array(jachai_item);

        let name_s = jachai::schema::string().min_len(1).max_len(50);
        let email_s = jachai::schema::string().email();
        let score_s = jachai::schema::integer().between(0, 100);

        let jachai_array_invalid: Vec<JachaiArrayBatch> = corpus
            .array_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(JachaiArrayBatch {
                    items: vec![JachaiItem {
                        name: "".into(),
                        email: "bad".into(),
                        score: -1,
                    }],
                })
            })
            .collect();

        let garde_data: Vec<GardeArrayBatch> = corpus
            .array_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(GardeArrayBatch {
                    items: vec![GardeItem {
                        name: "".into(),
                        email: "bad".into(),
                        score: -1,
                    }],
                })
            })
            .collect();

        let validator_data: Vec<ValidatorArrayBatch> = corpus
            .array_invalid
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone()).unwrap_or(ValidatorArrayBatch {
                    items: vec![ValidatorItem {
                        name: "".into(),
                        email: "bad".into(),
                        score: -1,
                    }],
                })
            })
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (typed struct batch)",
                &jachai_array_invalid,
                |batch| {
                    let res = batch.validate(&name_s, &email_s, &score_s);
                    black_box(res.is_err());
                },
                50_000,
            ),
            run_benchmark(
                "jachai (check / in-place)",
                &corpus.array_invalid,
                |v| {
                    let res = jachai_schema.check(v);
                    black_box(res.is_err());
                },
                50_000,
            ),
            run_benchmark(
                "jachai",
                &corpus.array_invalid,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res.is_err());
                },
                50_000,
            ),
            run_benchmark(
                "garde (in-memory struct)",
                &garde_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                50_000,
            ),
            run_benchmark(
                "validator (in-memory struct)",
                &validator_data,
                |s| {
                    let res = s.validate();
                    black_box(res.is_err());
                },
                50_000,
            ),
        ];
        results.insert("array_invalid".to_string(), metrics);
    }

    // 6. ARRAY UNIQUE ELEMENTS
    {
        let jachai_unique_schema = jachai::schema::array(jachai::schema::integer()).distinct();
        let metrics = vec![run_benchmark(
            "jachai (array unique)",
            &corpus.array_unique_valid,
            |v| {
                let res = jachai_unique_schema.validate(v);
                black_box(res).unwrap();
            },
            100_000,
        )];
        results.insert("array_unique".to_string(), metrics);
    }

    // 7. RECORD SCHEMA (Dictionary / Key-Value Map)
    {
        let jachai_record_schema =
            jachai::schema::record(jachai::schema::integer().between(0, 1000));
        let metrics = vec![run_benchmark(
            "jachai",
            &corpus.records_valid,
            |v| {
                let res = jachai_record_schema.validate(v);
                black_box(res).unwrap();
            },
            100_000,
        )];
        results.insert("record_schema".to_string(), metrics);
    }

    // 8. TUPLE SCHEMA (3 Elements: String, Integer, Boolean)
    {
        let jachai_tuple_schema = jachai::schema::tuple3(
            jachai::schema::string().min_len(1),
            jachai::schema::integer(),
            jachai::schema::boolean(),
        );
        let metrics = vec![run_benchmark(
            "jachai",
            &corpus.tuples_valid,
            |v| {
                let res = jachai_tuple_schema.validate(v);
                black_box(res).unwrap();
            },
            100_000,
        )];
        results.insert("tuple_schema".to_string(), metrics);
    }

    // 9. ENUM SCHEMA (Fixed String Variants)
    {
        let jachai_enum_schema = jachai::schema::enum_of::<String>().variants(&[
            "pending",
            "active",
            "suspended",
            "archived",
        ]);
        let enum_values: Vec<Value> = corpus
            .enums_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![run_benchmark(
            "jachai",
            &enum_values,
            |v| {
                let res = jachai_enum_schema.validate(v);
                black_box(res).unwrap();
            },
            200_000,
        )];
        results.insert("enum_schema".to_string(), metrics);
    }

    // 10. TEMPORAL: RFC 3339 DateTime, Date, Time
    {
        let jachai_datetime = jachai::schema::date();
        let jachai_date = jachai::schema::date().date_only();
        let jachai_time = jachai::schema::date().time_only();

        let dt_values: Vec<Value> = corpus
            .datetimes_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let d_values: Vec<Value> = corpus
            .dates_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let t_values: Vec<Value> = corpus
            .times_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (rfc3339 datetime)",
                &dt_values,
                |v| {
                    let res = jachai_datetime.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (rfc3339 date)",
                &d_values,
                |v| {
                    let res = jachai_date.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (rfc3339 time)",
                &t_values,
                |v| {
                    let res = jachai_time.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("temporal_rfc3339".to_string(), metrics);
    }

    // 11. NETWORK: IPv4, IPv6, URL, MAC, CIDR, Domain
    {
        let jachai_ipv4 = jachai::schema::string().ipv4();
        let jachai_ipv6 = jachai::schema::string().ipv6();
        let jachai_url = jachai::schema::string().url();
        let jachai_mac = jachai::schema::string().mac_address();
        let jachai_cidr = jachai::schema::string().cidr();
        let jachai_domain = jachai::schema::string().domain();

        let ipv4_values: Vec<Value> = corpus
            .ipv4_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let ipv6_values: Vec<Value> = corpus
            .ipv6_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let url_values: Vec<Value> = corpus
            .urls_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let mac_values: Vec<Value> = corpus
            .mac_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let cidr_values: Vec<Value> = corpus
            .cidr_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let domain_values: Vec<Value> = corpus
            .domain_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (ipv4 / check_str)",
                &corpus.ipv4_valid,
                |s| {
                    let res = jachai_ipv4.check_str(s);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (ipv4)",
                &ipv4_values,
                |v| {
                    let res = jachai_ipv4.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "garde (ipv4)",
                &corpus.ipv4_valid,
                |s| {
                    let inst = GardeIpPrimitive { ip: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "validator (ipv4)",
                &corpus.ipv4_valid,
                |s| {
                    let inst = ValidatorIpPrimitive { ip: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (ipv6)",
                &ipv6_values,
                |v| {
                    let res = jachai_ipv6.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (url / check_str)",
                &corpus.urls_valid,
                |s| {
                    let res = jachai_url.check_str(s);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (url)",
                &url_values,
                |v| {
                    let res = jachai_url.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "garde (url)",
                &corpus.urls_valid,
                |s| {
                    let inst = GardeUrlPrimitive { url: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "validator (url)",
                &corpus.urls_valid,
                |s| {
                    let inst = ValidatorUrlPrimitive { url: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (mac_address)",
                &mac_values,
                |v| {
                    let res = jachai_mac.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (cidr)",
                &cidr_values,
                |v| {
                    let res = jachai_cidr.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (domain)",
                &domain_values,
                |v| {
                    let res = jachai_domain.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("network_rules".to_string(), metrics);
    }

    // 12. IDENTIFIERS: UUID, ULID, SemVer, Slug
    {
        let jachai_uuid = jachai::schema::string().uuid();
        let jachai_ulid = jachai::schema::string().ulid();
        let jachai_semver = jachai::schema::string().semver();
        let jachai_slug = jachai::schema::string().slug();

        let uuid_values: Vec<Value> = corpus
            .primitive_uuids_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let ulid_values: Vec<Value> = corpus
            .ulids_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let semver_values: Vec<Value> = corpus
            .semvers_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let slug_values: Vec<Value> = corpus
            .slugs_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (uuid)",
                &uuid_values,
                |v| {
                    let res = jachai_uuid.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (ulid)",
                &ulid_values,
                |v| {
                    let res = jachai_ulid.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (semver)",
                &semver_values,
                |v| {
                    let res = jachai_semver.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (slug)",
                &slug_values,
                |v| {
                    let res = jachai_slug.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("identifiers".to_string(), metrics);
    }

    // 13. FINANCIAL: Credit Card (Luhn), IBAN, Currency Code
    {
        let jachai_cc = jachai::schema::string().credit_card();
        let jachai_iban = jachai::schema::string().iban();
        let jachai_currency = jachai::schema::string().currency_code();

        let cc_values: Vec<Value> = corpus
            .credit_cards_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let iban_values: Vec<Value> = corpus
            .ibans_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let currency_values: Vec<Value> = corpus
            .currency_codes_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (credit_card / check_str)",
                &corpus.credit_cards_valid,
                |s| {
                    let res = jachai_cc.check_str(s);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (credit_card)",
                &cc_values,
                |v| {
                    let res = jachai_cc.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "garde (credit_card)",
                &corpus.credit_cards_valid,
                |s| {
                    let inst = GardeCreditCardPrimitive { card: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "validator (credit_card)",
                &corpus.credit_cards_valid,
                |s| {
                    let inst = ValidatorCreditCardPrimitive { card: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (iban)",
                &iban_values,
                |v| {
                    let res = jachai_iban.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (currency_code)",
                &currency_values,
                |v| {
                    let res = jachai_currency.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("financial_rules".to_string(), metrics);
    }

    // 14. GEOGRAPHIC & COLOR
    {
        let jachai_lat = jachai::schema::string().latitude();
        let jachai_lon = jachai::schema::string().longitude();
        let jachai_coords = jachai::schema::string().coordinates();
        let jachai_country = jachai::schema::string().country_code();
        let jachai_hex = jachai::schema::string().hex_color();

        let lat_values: Vec<Value> = corpus
            .latitudes_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let lon_values: Vec<Value> = corpus
            .longitudes_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let coord_values: Vec<Value> = corpus
            .coordinates_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let country_values: Vec<Value> = corpus
            .country_codes_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let hex_values: Vec<Value> = corpus
            .hex_colors_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (latitude)",
                &lat_values,
                |v| {
                    let res = jachai_lat.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (longitude)",
                &lon_values,
                |v| {
                    let res = jachai_lon.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (coordinates)",
                &coord_values,
                |v| {
                    let res = jachai_coords.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (country_code)",
                &country_values,
                |v| {
                    let res = jachai_country.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (hex_color)",
                &hex_values,
                |v| {
                    let res = jachai_hex.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("geographic_and_color".to_string(), metrics);
    }

    // 15. CRYPTOGRAPHIC & SECURITY: SHA-256, Base64, JWT
    {
        let jachai_sha256 = jachai::schema::string().sha256();
        let jachai_base64 = jachai::schema::string().base64();
        let jachai_jwt = jachai::schema::string().jwt();

        let sha_values: Vec<Value> = corpus
            .sha256_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let b64_values: Vec<Value> = corpus
            .base64_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();
        let jwt_values: Vec<Value> = corpus
            .jwt_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (sha256)",
                &sha_values,
                |v| {
                    let res = jachai_sha256.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (base64)",
                &b64_values,
                |v| {
                    let res = jachai_base64.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (jwt)",
                &jwt_values,
                |v| {
                    let res = jachai_jwt.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("crypto_and_security".to_string(), metrics);
    }

    // 16. CROSS-FIELD OBJECT RULES (Confirmed, After Date, Required If)
    {
        let jachai_confirmed = jachai::schema::object()
            .field("password", jachai::schema::string().min_len(6))
            .field("password_confirmation", jachai::schema::string().min_len(6))
            .confirmed("password");

        let jachai_after_date = jachai::schema::object()
            .field("start_date", jachai::schema::date())
            .field("end_date", jachai::schema::date())
            .after_date("end_date", "start_date");

        let jachai_req_if = jachai::schema::object()
            .field("contact_type", jachai::schema::string())
            .optional_field("tax_id", jachai::schema::string().min_len(3))
            .required_if("tax_id", "contact_type", "business");

        let metrics = vec![
            run_benchmark(
                "jachai (confirmed rule)",
                &corpus.confirmed_objects_valid,
                |v| {
                    let res = jachai_confirmed.validate(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai (after_date cross-rule)",
                &corpus.after_date_objects_valid,
                |v| {
                    let res = jachai_after_date.validate(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai (required_if cross-rule)",
                &corpus.required_if_objects_valid,
                |v| {
                    let res = jachai_req_if.validate(v);
                    black_box(res).unwrap();
                },
                100_000,
            ),
        ];
        results.insert("cross_field_rules".to_string(), metrics);
    }

    // 17. MULTI-SOURCE HTTP REQUEST ENVELOPE (jachai Universal Architecture)
    {
        let req_schema = RequestSchema::new()
            .params(jachai::schema::object().field("id", jachai::schema::string().min_len(1)))
            .query(
                jachai::schema::object()
                    .field("page", jachai::schema::string())
                    .field("limit", jachai::schema::string()),
            )
            .headers(
                jachai::schema::object()
                    .field(
                        "authorization",
                        jachai::schema::string().starts_with("Bearer "),
                    )
                    .field("accept", jachai::schema::string().contains("json")),
            )
            .body(
                jachai::schema::object()
                    .field("username", jachai::schema::string().min_len(3))
                    .field("email", jachai::schema::string().email()),
            );

        let prepared_parts: Vec<RequestParts> = corpus
            .request_parts_valid
            .iter()
            .map(|v| {
                let mut parts = RequestParts::new();
                if let Some(params) = v.get("params") {
                    parts = parts.with_params(params.clone());
                }
                if let Some(query) = v.get("query") {
                    parts = parts.with_query(query.clone());
                }
                if let Some(headers) = v.get("headers") {
                    parts = parts.with_headers(headers.clone());
                }
                if let Some(body) = v.get("body") {
                    parts = parts.with_body(body.clone());
                }
                parts
            })
            .collect();

        let metrics = vec![run_benchmark(
            "jachai (multi-source request envelope)",
            &prepared_parts,
            |p| {
                let res = req_schema.validate(p);
                black_box(res).unwrap();
            },
            50_000,
        )];
        results.insert("multi_source_request".to_string(), metrics);
    }

    // 18. PRIMITIVE EMAIL
    {
        let jachai_schema = jachai::schema::string().email();
        let zod_schema = zod_string().email();

        let email_values: Vec<Value> = corpus
            .primitive_emails_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai",
                &email_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (check_str / zero-alloc)",
                &corpus.primitive_emails_valid,
                |s| {
                    let res = jachai_schema.check_str(s);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "zod-rs",
                &email_values,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "garde (primitive)",
                &corpus.primitive_emails_valid,
                |s| {
                    let inst = GardeEmailPrimitive { email: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "validator (primitive)",
                &corpus.primitive_emails_valid,
                |s| {
                    let inst = ValidatorEmailPrimitive { email: s.as_str() };
                    black_box(inst.validate()).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("primitive_email".to_string(), metrics);
    }

    // 19. PRIMITIVE EMAIL INVALID
    {
        let jachai_schema = jachai::schema::string().email();
        let zod_schema = zod_string().email();

        let email_values: Vec<Value> = corpus
            .primitive_emails_invalid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (check_str / zero-alloc)",
                &corpus.primitive_emails_invalid,
                |s| {
                    let res = jachai_schema.check_str(s);
                    black_box(res.is_err());
                },
                200_000,
            ),
            run_benchmark(
                "jachai",
                &email_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res.is_err());
                },
                200_000,
            ),
            run_benchmark(
                "zod-rs",
                &email_values,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res.is_err());
                },
                200_000,
            ),
            run_benchmark(
                "garde (primitive)",
                &corpus.primitive_emails_invalid,
                |s| {
                    let inst = GardeEmailPrimitive { email: s.as_str() };
                    black_box(inst.validate().is_err());
                },
                200_000,
            ),
            run_benchmark(
                "validator (primitive)",
                &corpus.primitive_emails_invalid,
                |s| {
                    let inst = ValidatorEmailPrimitive { email: s.as_str() };
                    black_box(inst.validate().is_err());
                },
                200_000,
            ),
        ];
        results.insert("primitive_email_invalid".to_string(), metrics);
    }

    // 20. PRIMITIVE INTEGER
    {
        let jachai_schema = jachai::schema::integer().between(18, 100);
        let zod_schema = zod_number().min(18.0).max(100.0);

        let int_values: Vec<Value> = corpus
            .primitive_integers_valid
            .iter()
            .map(|&n| Value::Number(serde_json::Number::from(n)))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai",
                &int_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                500_000,
            ),
            run_benchmark(
                "jachai (check_i64 / zero-alloc)",
                &corpus.primitive_integers_valid,
                |&n| {
                    let res = jachai_schema.check_i64(n);
                    black_box(res).unwrap();
                },
                500_000,
            ),
            run_benchmark(
                "zod-rs",
                &int_values,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                500_000,
            ),
        ];
        results.insert("primitive_integer".to_string(), metrics);
    }

    // 21. UNION VALID
    {
        let email_branch = jachai::schema::object()
            .field("channel", jachai::schema::string().exact_len(5))
            .field("address", jachai::schema::string().email());
        let phone_branch = jachai::schema::object()
            .field("channel", jachai::schema::string().exact_len(5))
            .field("phone", jachai::schema::string().min_len(7));
        let jachai_union = jachai::schema::union2(email_branch, phone_branch);

        let metrics = vec![run_benchmark(
            "jachai",
            &corpus.union_valid,
            |v| {
                let res = jachai_union.validate(v);
                black_box(res).unwrap();
            },
            100_000,
        )];
        results.insert("union_valid".to_string(), metrics);
    }

    // 22. END-TO-END INGESTION
    {
        let jachai_schema = jachai::schema::object()
            .field("username", jachai::schema::string().min_len(3).max_len(30))
            .field("email", jachai::schema::string().email())
            .field("age", jachai::schema::integer().between(18, 100));

        let zod_schema = zod_object()
            .field("username", zod_string().min(3).max(30))
            .field("email", zod_string().email())
            .field("age", zod_number().min(18.0).max(100.0));

        let username_s = jachai::schema::string().min_len(3).max_len(30);
        let email_s = jachai::schema::string().email();
        let age_s = jachai::schema::integer().between(18, 100);

        let raw_json_strings: Vec<String> = corpus
            .flat_valid
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (serde deserialize + check)",
                &raw_json_strings,
                |s| {
                    let model: JachaiFlat = serde_json::from_str(s).unwrap();
                    let res = (|| {
                        username_s.check_str(&model.username)?;
                        email_s.check_str(&model.email)?;
                        age_s.check_i64(model.age)?;
                        Ok::<(), jachai::ValidationError>(())
                    })();
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "jachai (parse json + validate)",
                &raw_json_strings,
                |s| {
                    let val: Value = serde_json::from_str(s).unwrap();
                    let res = jachai_schema.validate(&val);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "zod-rs (parse json + validate)",
                &raw_json_strings,
                |s| {
                    let val: Value = serde_json::from_str(s).unwrap();
                    let res = zod_schema.safe_parse(&val);
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "garde (serde deserialize + validate)",
                &raw_json_strings,
                |s| {
                    let model: GardeFlat = serde_json::from_str(s).unwrap();
                    let res = model.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
            run_benchmark(
                "validator (serde deserialize + validate)",
                &raw_json_strings,
                |s| {
                    let model: ValidatorFlat = serde_json::from_str(s).unwrap();
                    let res = model.validate();
                    black_box(res).unwrap();
                },
                100_000,
            ),
        ];
        results.insert("end_to_end_ingestion".to_string(), metrics);
    }

    // 24. PRIMITIVE FLOAT
    {
        let jachai_schema = jachai::schema::number().min(0.0).max(100.0).finite();
        let zod_schema = zod_number().min(0.0).max(100.0);

        let float_values: Vec<Value> = corpus
            .primitive_floats_valid
            .iter()
            .copied()
            .map(|n| {
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            })
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai",
                &float_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                500_000,
            ),
            run_benchmark(
                "jachai (check_f64 / zero-alloc)",
                &corpus.primitive_floats_valid,
                |&n| {
                    let res = jachai_schema.check_f64(n);
                    black_box(res).unwrap();
                },
                500_000,
            ),
            run_benchmark(
                "zod-rs",
                &float_values,
                |v| {
                    let res = zod_schema.safe_parse(v);
                    black_box(res).unwrap();
                },
                500_000,
            ),
        ];
        results.insert("primitive_float".to_string(), metrics);
    }

    // 25. PRIMITIVE BOOLEAN
    {
        let jachai_schema = jachai::schema::boolean().accepted();
        let bool_values: Vec<Value> = corpus
            .primitive_booleans_valid
            .iter()
            .copied()
            .map(Value::Bool)
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (boolean accepted)",
                &bool_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                500_000,
            ),
            run_benchmark(
                "jachai (check_bool / zero-alloc)",
                &corpus.primitive_booleans_valid,
                |&b| {
                    let res = jachai_schema.check_bool(b);
                    black_box(res).unwrap();
                },
                500_000,
            ),
        ];
        results.insert("primitive_boolean".to_string(), metrics);
    }

    // 26. STRING RULES (PREFIX, SUFFIX, CONTAINS)
    {
        let jachai_schema = jachai::schema::string()
            .starts_with("Prefix_")
            .ends_with("_Suffix")
            .contains("User");
        let str_values: Vec<Value> = corpus
            .string_rules_valid
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect();

        let metrics = vec![
            run_benchmark(
                "jachai (prefix/suffix/contains)",
                &str_values,
                |v| {
                    let res = jachai_schema.validate(v);
                    black_box(res).unwrap();
                },
                200_000,
            ),
            run_benchmark(
                "jachai (check_str / zero-alloc)",
                &corpus.string_rules_valid,
                |s| {
                    let res = jachai_schema.check_str(s);
                    black_box(res).unwrap();
                },
                200_000,
            ),
        ];
        results.insert("string_rules".to_string(), metrics);
    }

    // 27. FILE UPLOAD METADATA SCHEMA
    {
        let jachai_schema = jachai::schema::file()
            .size_between(100, 5_000_000)
            .allowed_extensions(&["png", "pdf"])
            .allowed_mime_types(&["image/png", "application/pdf"]);

        let metrics = vec![run_benchmark(
            "jachai (file size/mime/ext)",
            &corpus.files_valid,
            |v| {
                let res = jachai_schema.validate(v);
                black_box(res).unwrap();
            },
            200_000,
        )];
        results.insert("file_schema".to_string(), metrics);
    }

    let json_output = serde_json::to_string_pretty(&results).expect("Failed to serialize results");
    println!("{json_output}");
}
