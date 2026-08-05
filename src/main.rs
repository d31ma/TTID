//! The native transport: arguments in, JSON on stdout.
//!
//! A port of the retired JavaScript CLI. Argument grammar, help text, exit codes, and
//! output shape are contract — the 11 client shims drive this binary, and
//! `tests/cli.rs` replays a committed recording of the JavaScript CLI's answers
//! command for command.

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use ttid::machine::{self, Format};
use ttid::ttid::Generator;

const HELP: &str = r#"ttid - time-tagged identifier generator

Usage:
  ttid generate [id] [--delete]
  ttid decode <id>
  ttid validate <id>
  ttid canonicalize <id>
  ttid uuid <id>
  ttid exec --request <json|@path|->
  ttid exec --loop

Options:
  --delete       Mark the TTID as deleted when generating from an existing ID
  --request      Machine request payload, @file path, or - for stdin
  --loop         Persistent NDJSON loop: one request/response per line on stdio
  -h, --help     Show this help and exit

Machine request:
  {"op":"generate","id":"...","delete":true}

All commands write structured JSON to stdout."#;

/// Current time in milliseconds, with sub-millisecond precision preserved.
///
/// The oracle uses `performance.now() + performance.timeOrigin`; the wall clock
/// at nanosecond resolution is the same quantity.
fn now_ms() -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "nanoseconds since 1970 exceed f64's mantissa, and that rounding \
                  is exactly what the oracle's float clock does — matching it is the point"
    )]
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_nanos() as f64 / 1_000_000.0)
}

struct Args {
    positionals: Vec<String>,
    request: Option<String>,
    delete: bool,
    loop_mode: bool,
    help: bool,
}

/// Mirrors `CliArgsParser`: flags are recognized anywhere, everything else is
/// positional, and an unknown `--flag` is simply a positional.
fn parse(arguments: &[String]) -> Result<Args, String> {
    let mut args = Args {
        positionals: Vec::new(),
        request: None,
        delete: false,
        loop_mode: false,
        help: false,
    };
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--request" => {
                // The oracle rejects a missing *or empty* value: `!value`.
                let value = arguments.get(index + 1).filter(|value| !value.is_empty());
                let value = value.ok_or_else(|| "Missing value for --request".to_owned())?;
                args.request = Some(value.clone());
                index += 1;
            }
            "--delete" => args.delete = true,
            "--loop" => args.loop_mode = true,
            "--help" | "-h" => args.help = true,
            other => args.positionals.push(other.to_owned()),
        }
        index += 1;
    }
    Ok(args)
}

/// Mirrors `JsonSourceLoader`: a literal payload, `@path`, or `-` for stdin.
fn load_json_source(source: &str) -> Result<Value, String> {
    let text = if source == "-" {
        if std::io::stdin().is_terminal() {
            return Err("JSON input requires <json|@path|->".to_owned());
        }
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|error| error.to_string())?;
        buffer
    } else if let Some(path) = source.strip_prefix('@') {
        std::fs::read_to_string(path).map_err(|error| errno_message(&error, path))?
    } else {
        source.to_owned()
    };

    serde_json::from_str(&text).map_err(|error| format!("Invalid JSON input: {error}"))
}

/// Reproduce the oracle's file-error text, which comes from Bun and reads
/// `ENOENT: no such file or directory, open '<path>'`.
///
/// Only the kinds a `--request @path` realistically hits are mapped; anything
/// else falls back to Rust's own wording and is recorded in the parity ledger.
fn errno_message(error: &std::io::Error, path: &str) -> String {
    let named = match error.kind() {
        std::io::ErrorKind::NotFound => Some(("ENOENT", "no such file or directory")),
        std::io::ErrorKind::PermissionDenied => Some(("EACCES", "permission denied")),
        _ => None,
    };
    match named {
        Some((code, text)) => format!("{code}: {text}, open '{path}'"),
        None => error.to_string(),
    }
}

/// Mirrors `MachineRequestBuilder`.
fn build_request(args: &Args) -> Result<Value, String> {
    let command = args.positionals.first().map(String::as_str);
    let id = args.positionals.get(1);

    match command {
        Some("exec") => {
            let source = args
                .request
                .as_deref()
                .ok_or_else(|| "Missing --request for exec".to_owned())?;
            load_json_source(source)
        }
        Some("generate") => {
            if args.delete && id.is_none() {
                return Err("Missing id for --delete".to_owned());
            }
            let mut request = json!({ "op": "generate" });
            if let Some(id) = id {
                request["id"] = json!(id);
            }
            if args.delete {
                request["delete"] = json!(true);
            }
            Ok(request)
        }
        Some("decode") => with_id("decodeTime", id, "decode"),
        Some("validate") => with_id("isTTID", id, "validate"),
        Some("canonicalize") => with_id("canonicalize", id, "canonicalize"),
        Some("uuid") => with_id("isUUID", id, "uuid"),
        other => Err(format!(
            r#"Unsupported command "{}""#,
            other.unwrap_or_default()
        )),
    }
}

fn with_id(op: &str, id: Option<&String>, command: &str) -> Result<Value, String> {
    let id = id.ok_or_else(|| format!("Missing id for {command}"))?;
    Ok(json!({ "op": op, "id": id }))
}

/// Mirrors `serveStdioLoop`: one JSON request per line in, one response per
/// line out, in order, with the process kept warm between calls.
fn serve_stdio_loop() -> ExitCode {
    let stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    // One generator for the life of the process: a shim keeps this warm and
    // drives it far faster than the clock's resolution.
    let mut generator = Generator::new();
    for line in stdin.lines() {
        let Ok(line) = line else { break };
        let started_at = now_ms();
        if let Some(response) = machine::execute_line(
            &line,
            started_at,
            duration_since(started_at),
            &mut generator,
        ) {
            // A shim is blocked on this line; flush rather than buffer.
            if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
                break;
            }
        }
    }
    ExitCode::SUCCESS
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "an elapsed millisecond count within one process always fits i64"
)]
fn duration_since(started_at: f64) -> i64 {
    (now_ms() - started_at) as i64
}

fn main() -> ExitCode {
    let started_at = now_ms();
    let arguments: Vec<String> = std::env::args().skip(1).collect();

    let args = match parse(&arguments) {
        Ok(args) => args,
        Err(message) => return fail(&Value::Null, started_at, message),
    };

    if args.help || args.positionals.is_empty() {
        println!("{HELP}");
        return if args.help {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    if args.positionals.first().map(String::as_str) == Some("exec") && args.loop_mode {
        return serve_stdio_loop();
    }

    let request = match build_request(&args) {
        Ok(request) => request,
        // The oracle has not assigned `this.request` yet when the build throws,
        // so the response reports a null op.
        Err(message) => return fail(&Value::Null, started_at, message),
    };

    let (response, ok) = machine::execute_value_as(
        &request,
        now_ms(),
        duration_since(started_at),
        Format::Indented,
        &mut Generator::new(),
    );
    println!("{response}");

    // The oracle exits 1 whenever it renders an error response.
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn fail(request: &Value, started_at: f64, message: String) -> ExitCode {
    println!(
        "{}",
        machine::transport_error(
            request,
            duration_since(started_at),
            message,
            Format::Indented
        )
    );
    ExitCode::FAILURE
}
