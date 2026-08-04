//! The machine protocol: one JSON request in, one JSON response out.
//!
//! This is the seam that makes the native binary and the wasm module
//! interchangeable. Both transports call [`execute_line`] with the same bytes
//! and hand back the same bytes; neither shim can tell them apart.
//!
//! Response field names *and order* are contract, frozen at
//! `protocolVersion: 1`. See section 2 of `docs/RUST_REWRITE_PLAN.md`.

use serde::Serialize;
use serde_json::Value;

use crate::ttid;

/// The frozen machine protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Every error the oracle raises carries the JavaScript `Error` name.
const ERROR_NAME: &str = "Error";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuccessResponse<'a> {
    protocol_version: u32,
    ok: bool,
    op: &'a str,
    request_id: Option<&'a str>,
    duration_ms: i64,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse<'a> {
    protocol_version: u32,
    ok: bool,
    op: Option<&'a str>,
    request_id: Option<&'a str>,
    duration_ms: i64,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    name: &'static str,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationResult {
    valid: bool,
    created_at: Option<i64>,
}

#[derive(Serialize)]
struct UuidResult {
    valid: bool,
}

/// The oracle reads `op` only when the request is an object and `op` is a
/// string; anything else reports a null op.
fn op_of(request: &Value) -> Option<&str> {
    request.as_object()?.get("op")?.as_str()
}

/// Same rule as [`op_of`], for the optional caller-supplied correlation id.
fn request_id_of(request: &Value) -> Option<&str> {
    request.as_object()?.get("requestId")?.as_str()
}

/// The oracle's `requireString`: present, a string, and non-empty once trimmed.
fn require_string<'a>(request: &'a Value, field: &str) -> Result<&'a str, String> {
    let invalid = || format!(r#"Machine request field "{field}" must be a non-empty string"#);
    let value = request
        .as_object()
        .ok_or_else(|| "Machine request body must be a JSON object".to_owned())?
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if value.trim().is_empty() {
        return Err(invalid());
    }
    Ok(value)
}

/// JavaScript falsiness, for the values JSON can express.
fn is_falsy(value: &Value) -> bool {
    matches!(value, Value::Null | Value::Bool(false))
        || value.as_str() == Some("")
        || value.as_f64() == Some(0.0)
}

/// Resolve `request.id` for `generate`, which the oracle forwards raw.
///
/// JavaScript truthiness decides: falsy values (absent, `null`, `false`, `0`,
/// `""`) mean "create a new id", a string is used as-is, and every other truthy
/// value fails the oracle's `split` call and surfaces as `Invalid TTID!`.
fn generate_id(request: &Value) -> Result<Option<&str>, String> {
    let Some(value) = request.as_object().and_then(|object| object.get("id")) else {
        return Ok(None);
    };
    match value {
        Value::String(text) if !text.is_empty() => Ok(Some(text.as_str())),
        _ if is_falsy(value) => Ok(None),
        _ => Err("Invalid TTID!".to_owned()),
    }
}

/// Every result shape in [`dispatch`] is plain JSON; `to_value` cannot fail.
fn to_json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Dispatch a parsed request to the kernel.
fn dispatch(
    request: &Value,
    now_ms: f64,
    generator: &mut ttid::Generator,
) -> Result<Value, String> {
    let object = request
        .as_object()
        .ok_or_else(|| "Machine request body must be a JSON object".to_owned())?;
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| r#"Machine request field "op" must be a string"#.to_owned())?;

    match op {
        "generate" => {
            let delete = object.get("delete") == Some(&Value::Bool(true));
            let id = generate_id(request)?;
            let generated = generator
                .generate(id, delete, now_ms)
                .map_err(|error| error.0.to_owned())?;
            Ok(Value::String(generated))
        }
        "decodeTime" => {
            let id = require_string(request, "id")?;
            let times = ttid::decode_time(id).map_err(|error| error.0.to_owned())?;
            Ok(to_json(&times))
        }
        "isTTID" => {
            let id = require_string(request, "id")?;
            let created_at = ttid::is_ttid(id);
            Ok(to_json(&ValidationResult {
                valid: created_at.is_some(),
                created_at,
            }))
        }
        "canonicalize" => {
            let id = require_string(request, "id")?;
            let canonical = ttid::canonical(id).ok_or_else(|| "Invalid TTID!".to_owned())?;
            Ok(Value::String(canonical))
        }
        "isUUID" => {
            let id = require_string(request, "id")?;
            Ok(to_json(&UuidResult {
                valid: ttid::is_uuid(id),
            }))
        }
        other => Err(format!(r#"Unsupported machine operation "{other}""#)),
    }
}

/// How a response is serialized.
///
/// The NDJSON loop emits one compact line; the CLI's one-shot path matches the
/// oracle's `JSON.stringify(value, null, 2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One line, no whitespace.
    Compact,
    /// Two-space indentation.
    Indented,
}

impl Format {
    fn render<T: Serialize>(self, value: &T) -> serde_json::Result<String> {
        match self {
            Self::Compact => serde_json::to_string(value),
            Self::Indented => serde_json::to_string_pretty(value),
        }
    }
}

/// Serialize a request/response pair, matching `JSON.stringify` byte for byte.
fn render(
    request: &Value,
    duration_ms: i64,
    outcome: Result<Value, String>,
    format: Format,
) -> String {
    let request_id = request_id_of(request);
    let rendered = match outcome {
        Ok(result) => format.render(&SuccessResponse {
            protocol_version: PROTOCOL_VERSION,
            ok: true,
            op: op_of(request).unwrap_or("generate"),
            request_id,
            duration_ms,
            result,
        }),
        Err(message) => format.render(&ErrorResponse {
            protocol_version: PROTOCOL_VERSION,
            ok: false,
            op: op_of(request),
            request_id,
            duration_ms,
            error: ErrorBody {
                name: ERROR_NAME,
                message,
            },
        }),
    };
    rendered.unwrap_or_else(|error| {
        // Unreachable: every field above is plain JSON. Never panic on a
        // transport boundary.
        format!(
            r#"{{"protocolVersion":{PROTOCOL_VERSION},"ok":false,"op":null,"requestId":null,"durationMs":{duration_ms},"error":{{"name":"{ERROR_NAME}","message":{}}}}}"#,
            serde_json::Value::String(error.to_string())
        )
    })
}

/// Execute one already-parsed request. Exposed for transports that parse the
/// payload themselves, such as the CLI's one-shot `exec --request`.
///
/// `duration_ms` and `now_ms` are supplied by the transport so the kernel stays
/// free of clocks.
#[must_use]
pub fn execute_value(
    request: &Value,
    now_ms: f64,
    duration_ms: i64,
    generator: &mut ttid::Generator,
) -> String {
    render(
        request,
        duration_ms,
        dispatch(request, now_ms, generator),
        Format::Compact,
    )
}

/// As [`execute_value`], with the caller choosing the serialization.
///
/// Returns the rendered response and whether it succeeded, so a transport can
/// set its exit code without parsing its own output back.
#[must_use]
pub fn execute_value_as(
    request: &Value,
    now_ms: f64,
    duration_ms: i64,
    format: Format,
    generator: &mut ttid::Generator,
) -> (String, bool) {
    let outcome = dispatch(request, now_ms, generator);
    let ok = outcome.is_ok();
    (render(request, duration_ms, outcome, format), ok)
}

/// Render a transport-level failure that never reached the kernel — a bad
/// `--request` payload, an unreadable file, an unknown command.
#[must_use]
pub fn transport_error(
    request: &Value,
    duration_ms: i64,
    message: String,
    format: Format,
) -> String {
    render(request, duration_ms, Err(message), format)
}

/// Execute one NDJSON line.
///
/// Returns `None` for a blank line, matching the oracle's loop, which writes
/// nothing rather than an error.
#[must_use]
pub fn execute_line(
    line: &str,
    now_ms: f64,
    duration_ms: i64,
    generator: &mut ttid::Generator,
) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let Ok(request) = serde_json::from_str::<Value>(trimmed) else {
        return Some(render(
            &Value::Null,
            duration_ms,
            Err("Invalid JSON request".to_owned()),
            Format::Compact,
        ));
    };
    Some(execute_value(&request, now_ms, duration_ms, generator))
}
