//! Replays the golden corpus produced by the JavaScript oracle.
//!
//! The corpus is frozen: the generator that produced it retired with the
//! JavaScript engine, so a diff in `tests/fixtures/corpus.json` is a contract
//! change, not a test update.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use serde_json::Value;
use ttid::ttid::Generator;
use ttid::{machine, ttid as kernel};

fn corpus() -> Value {
    let raw = include_str!("fixtures/corpus.json");
    serde_json::from_str(raw).expect("corpus.json is valid JSON")
}

fn cases(group: &str) -> Vec<Value> {
    corpus()["cases"][group]
        .as_array()
        .unwrap_or_else(|| panic!("corpus group {group} is an array"))
        .clone()
}

fn name(case: &Value) -> &str {
    case["name"].as_str().unwrap()
}

#[test]
fn generate_matches_the_oracle() {
    for case in cases("generate") {
        let id = case["id"].as_str();
        let delete = case["delete"].as_bool().unwrap();
        let now_ms = case["nowMs"].as_f64().unwrap();

        match kernel::generate(id, delete, now_ms) {
            Ok(actual) => assert_eq!(
                Some(actual.as_str()),
                case["expect"].as_str(),
                "generate: {}",
                name(&case)
            ),
            Err(error) => assert_eq!(
                Some(error.0),
                case["error"].as_str(),
                "generate: {}",
                name(&case)
            ),
        }
    }
}

#[test]
fn decode_time_matches_the_oracle() {
    for case in cases("decodeTime") {
        let id = case["id"].as_str().unwrap();
        match kernel::decode_time(id) {
            Ok(times) => {
                assert!(
                    case["error"].is_null(),
                    "decodeTime: {} should have failed",
                    name(&case)
                );
                assert_eq!(
                    serde_json::to_value(&times).unwrap(),
                    case["expect"],
                    "decodeTime: {}",
                    name(&case)
                );
            }
            Err(error) => assert_eq!(
                Some(error.0),
                case["error"].as_str(),
                "decodeTime: {}",
                name(&case)
            ),
        }
    }
}

#[test]
fn is_ttid_matches_the_oracle() {
    for case in cases("isTTID") {
        let id = case["id"].as_str().unwrap();
        let expected = case["expect"].as_i64();
        assert_eq!(kernel::is_ttid(id), expected, "isTTID: {}", name(&case));
    }
}

#[test]
fn is_uuid_matches_the_oracle() {
    for case in cases("isUUID") {
        let id = case["id"].as_str().unwrap();
        let expected = case["expect"].as_bool().unwrap();
        assert_eq!(kernel::is_uuid(id), expected, "isUUID: {}", name(&case));
    }
}

/// The strongest gate: the whole response line, byte for byte, including key
/// order. `durationMs` is zero because the corpus generator pinned `Date.now`.
#[test]
fn machine_responses_are_byte_identical() {
    for case in cases("machine") {
        let now_ms = case["nowMs"].as_f64().unwrap();
        // A fresh generator per case: the corpus pins the stateless contract.
        let actual = machine::execute_value(&case["request"], now_ms, 0, &mut Generator::new());
        assert_eq!(
            actual,
            case["response"].as_str().unwrap(),
            "machine: {}",
            name(&case)
        );
    }
}

#[test]
fn blank_ndjson_lines_produce_no_response() {
    assert!(machine::execute_line("   ", 0.0, 0, &mut Generator::new()).is_none());
    assert!(machine::execute_line("", 0.0, 0, &mut Generator::new()).is_none());
}

#[test]
fn malformed_ndjson_reports_invalid_json() {
    let response = machine::execute_line("{not json", 0.0, 0, &mut Generator::new()).unwrap();
    assert_eq!(
        response,
        r#"{"protocolVersion":1,"ok":false,"op":null,"requestId":null,"durationMs":0,"error":{"name":"Error","message":"Invalid JSON request"}}"#
    );
}

// --- Monotonic uniqueness -------------------------------------------------
// The guarantee the raw clock cannot give. Before `Generator` existed a tight
// loop produced 1746/2000 unique ids in Bun and 135/2000 in a browser, because
// the f64 ulp at this epoch is 2 scaled units — 200ns — and browsers clamp
// `performance.now()` to roughly 100µs.

const BURST: usize = 20_000;

#[test]
fn a_burst_on_a_frozen_clock_never_repeats() {
    let mut generator = Generator::new();
    let frozen = 1_754_179_200_000.0;
    let ids: Vec<String> = (0..BURST)
        .map(|_| generator.generate(None, false, frozen).unwrap())
        .collect();

    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(
        unique.len(),
        BURST,
        "a frozen clock must still yield unique ids"
    );
}

#[test]
fn a_burst_stays_strictly_increasing_and_sortable() {
    let mut generator = Generator::new();
    let frozen = 1_754_179_200_000.0;
    let ids: Vec<String> = (0..BURST)
        .map(|_| generator.generate(None, false, frozen).unwrap())
        .collect();

    for pair in ids.windows(2) {
        assert!(
            pair[0] < pair[1],
            "ids must sort lexicographically by creation"
        );
    }
    // Same length keeps byte order and numeric order in agreement.
    assert!(ids.iter().all(|id| id.len() == ids[0].len()));
}

#[test]
fn a_burst_barely_moves_the_decoded_timestamp() {
    let mut generator = Generator::new();
    let frozen = 1_754_179_200_000.0;
    let first = generator.generate(None, false, frozen).unwrap();
    for _ in 1..BURST {
        generator.generate(None, false, frozen).unwrap();
    }
    let last = generator.generate(None, false, frozen).unwrap();

    let drift = kernel::decode_time(&last).unwrap().created_at
        - kernel::decode_time(&first).unwrap().created_at;
    // One scaled unit is 0.1µs, so 20k ids drift 2ms at most.
    assert!(
        drift <= 2,
        "20k ids drifted the decoded timestamp by {drift}ms"
    );
}

#[test]
fn a_moving_clock_is_byte_identical_to_the_stateless_path() {
    // The bump only replaces what would have been a duplicate. When the clock
    // advances, the monotonic generator must not change the answer.
    let mut generator = Generator::new();
    for step in 0..500 {
        let now = 1_754_179_200_000.0 + f64::from(step);
        let monotonic = generator.generate(None, false, now).unwrap();
        let stateless = kernel::generate(None, false, now).unwrap();
        assert_eq!(monotonic, stateless, "diverged at step {step}");
    }
}

#[test]
fn a_rejected_request_does_not_burn_a_timestamp() {
    let mut generator = Generator::new();
    let frozen = 1_754_179_200_000.0;
    let deleted = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK";

    let before = generator.generate(None, false, frozen).unwrap();
    assert!(generator.generate(Some(deleted), false, frozen).is_err());
    let after = generator.generate(None, false, frozen).unwrap();

    // The failed call must not have advanced the counter past one step.
    let step = |id: &str| i128::from_str_radix(id, 36).unwrap();
    assert_eq!(step(&after) - step(&before), 1);
}
