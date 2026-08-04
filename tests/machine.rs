//! The machine protocol, tested directly.
//!
//! The Rust home of `test/cli.test.js`, which drove `executeMachineOperation`
//! and the response factories in the retired JavaScript engine. Those tests
//! asserted the shape of a response — field by field — rather than comparing it
//! to a recorded string, so they are restated the same way here.
//!
//! `tests/oracle.rs` already pins whole response lines byte for byte. This file
//! covers the same ground from the other direction: it fails with a readable
//! message about *which field* is wrong, where a byte comparison only says the
//! line differs.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use serde_json::{Value, json};
use ttid::machine::{self, Format};
use ttid::ttid::Generator;

const NOW: f64 = 1_754_179_200_000.0;

/// Execute a request and parse the response back into JSON.
fn execute(request: &Value) -> Value {
    let line = machine::execute_value(request, NOW, 0, &mut Generator::new());
    serde_json::from_str(&line).expect("a response is valid JSON")
}

/// The `result` of a successful request.
fn result(request: &Value) -> Value {
    let response = execute(request);
    assert_eq!(response["ok"], json!(true), "expected success: {response}");
    response["result"].clone()
}

#[test]
fn generates_a_ttid_through_the_machine_interface() {
    let response = execute(&json!({ "requestId": "generate-1", "op": "generate" }));

    assert_eq!(response["ok"], json!(true));
    assert_eq!(response["protocolVersion"], json!(1));
    assert_eq!(response["op"], json!("generate"));
    assert_eq!(response["requestId"], json!("generate-1"));
    assert!(response["result"].is_string());
    assert_eq!(response["result"].as_str().unwrap().len(), 11);
}

#[test]
fn updates_and_deletes_an_existing_ttid_through_arguments() {
    let created = result(&json!({ "op": "generate" }));
    let created = created.as_str().unwrap();
    let updated = result(&json!({ "op": "generate", "id": created }));
    let updated = updated.as_str().unwrap();
    let deleted = result(&json!({ "op": "generate", "id": updated, "delete": true }));
    let deleted = deleted.as_str().unwrap();

    assert_eq!(updated.split('-').count(), 2);
    assert_eq!(deleted.split('-').count(), 3);
}

#[test]
fn decodes_timestamps_through_the_machine_interface() {
    let id = result(&json!({ "op": "generate" }));
    let id = id.as_str().unwrap();
    let times = result(&json!({ "requestId": "decode-1", "op": "decodeTime", "id": id }));

    // A one-segment id decodes createdAt and nothing else — the optional keys
    // must be absent, not null.
    assert!(times["createdAt"].is_number());
    assert_eq!(
        times.as_object().unwrap().len(),
        1,
        "unexpected keys: {times}"
    );
}

#[test]
fn validates_ttids_through_the_machine_interface() {
    let id = result(&json!({ "op": "generate" }));
    let id = id.as_str().unwrap();
    let verdict = result(&json!({ "op": "isTTID", "id": id }));

    assert_eq!(verdict["valid"], json!(true));
    assert!(verdict["createdAt"].is_number());

    let rejected = result(&json!({ "op": "isTTID", "id": "not-a-valid-ttid" }));
    assert_eq!(rejected["valid"], json!(false));
    assert_eq!(rejected["createdAt"], Value::Null);
}

#[test]
fn checks_uuids_through_the_machine_interface() {
    let accepted = result(&json!({ "op": "isUUID", "id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301" }));
    assert_eq!(accepted["valid"], json!(true));

    let rejected = result(&json!({ "op": "isUUID", "id": "not-a-uuid" }));
    assert_eq!(rejected["valid"], json!(false));
}

#[test]
fn returns_structured_errors() {
    let response = execute(&json!({
        "requestId": "bad-generate",
        "op": "generate",
        "id": "not-a-valid-ttid"
    }));

    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["requestId"], json!("bad-generate"));
    assert_eq!(response["error"]["message"], json!("Invalid TTID!"));
    assert_eq!(response["error"]["name"], json!("Error"));
    assert_eq!(response["protocolVersion"], json!(1));
}

#[test]
fn rejects_unsupported_operations_through_the_machine_interface() {
    let response = execute(&json!({ "op": "unknownOperation" }));

    assert_eq!(response["ok"], json!(false));
    assert_eq!(
        response["error"]["message"],
        json!(r#"Unsupported machine operation "unknownOperation""#)
    );
    // The op is echoed even when it is not one we support.
    assert_eq!(response["op"], json!("unknownOperation"));
}

#[test]
fn rejects_malformed_envelopes() {
    for (request, message) in [
        (json!({}), r#"Machine request field "op" must be a string"#),
        (
            json!({ "op": 7 }),
            r#"Machine request field "op" must be a string"#,
        ),
        (
            json!("a string"),
            "Machine request body must be a JSON object",
        ),
        (json!(null), "Machine request body must be a JSON object"),
        (json!([]), "Machine request body must be a JSON object"),
    ] {
        let response = execute(&request);
        assert_eq!(response["ok"], json!(false), "request: {request}");
        assert_eq!(
            response["error"]["message"],
            json!(message),
            "request: {request}"
        );
        // A request with no readable op reports a null op.
        if !request.is_object() || request.get("op").and_then(Value::as_str).is_none() {
            assert_eq!(response["op"], Value::Null, "request: {request}");
        }
    }
}

#[test]
fn ops_that_read_an_id_require_a_non_empty_string() {
    for op in ["decodeTime", "isTTID", "isUUID"] {
        for request in [
            json!({ "op": op }),
            json!({ "op": op, "id": "" }),
            json!({ "op": op, "id": "   " }),
            json!({ "op": op, "id": 42 }),
            json!({ "op": op, "id": null }),
        ] {
            let response = execute(&request);
            assert_eq!(response["ok"], json!(false), "request: {request}");
            assert_eq!(
                response["error"]["message"],
                json!(r#"Machine request field "id" must be a non-empty string"#),
                "request: {request}"
            );
        }
    }
}

#[test]
fn request_id_is_echoed_or_null() {
    let with = execute(&json!({ "op": "generate", "requestId": "abc" }));
    assert_eq!(with["requestId"], json!("abc"));

    let without = execute(&json!({ "op": "generate" }));
    assert_eq!(without["requestId"], Value::Null);

    // A non-string requestId is not echoed.
    let wrong_type = execute(&json!({ "op": "generate", "requestId": 7 }));
    assert_eq!(wrong_type["requestId"], Value::Null);
}

#[test]
fn the_ndjson_transport_handles_blank_and_malformed_lines() {
    let mut generator = Generator::new();
    assert!(machine::execute_line("", NOW, 0, &mut generator).is_none());
    assert!(machine::execute_line("   \t ", NOW, 0, &mut generator).is_none());

    let malformed = machine::execute_line("{not json", NOW, 0, &mut generator).unwrap();
    let response: Value = serde_json::from_str(&malformed).unwrap();
    assert_eq!(response["ok"], json!(false));
    assert_eq!(response["error"]["message"], json!("Invalid JSON request"));
    assert_eq!(response["op"], Value::Null);
}

#[test]
fn the_indented_format_matches_json_stringify_with_two_spaces() {
    let (rendered, ok) = machine::execute_value_as(
        &json!({ "op": "isUUID", "id": "not-a-uuid" }),
        NOW,
        0,
        Format::Indented,
        &mut Generator::new(),
    );
    assert!(ok);
    assert_eq!(
        rendered,
        "{\n  \"protocolVersion\": 1,\n  \"ok\": true,\n  \"op\": \"isUUID\",\n  \
         \"requestId\": null,\n  \"durationMs\": 0,\n  \"result\": {\n    \"valid\": false\n  }\n}"
    );
}
