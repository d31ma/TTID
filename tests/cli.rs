//! The whole `ttid` command surface, against a committed recording.
//!
//! Replaces `scripts/compat/cli-differential.mjs`. Every case runs the real
//! binary and compares stdout and exit code with
//! `tests/fixtures/cli-expectations.json`, which was recorded while the
//! JavaScript engine still existed to check it against.
//!
//! Re-record with `cargo test --test cli -- --ignored record`; the diff is the
//! review.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

mod support;

use serde_json::{Value, json};
use support::{normalize, run};

const EXPECTATIONS: &str = "tests/fixtures/cli-expectations.json";
const BINARY: &str = env!("CARGO_BIN_EXE_ttid");

struct Case {
    name: &'static str,
    argv: &'static [&'static str],
    stdin: Option<&'static str>,
}

const CASES: &[Case] = &[
    Case {
        name: "no arguments prints help and exits 1",
        argv: &[],
        stdin: None,
    },
    Case {
        name: "--help",
        argv: &["--help"],
        stdin: None,
    },
    Case {
        name: "-h",
        argv: &["-h"],
        stdin: None,
    },
    Case {
        name: "generate",
        argv: &["generate"],
        stdin: None,
    },
    Case {
        name: "generate from an existing id",
        argv: &["generate", "4SQ1NZT5HC0"],
        stdin: None,
    },
    Case {
        name: "generate --delete",
        argv: &["generate", "4SQ1NZT5HC0", "--delete"],
        stdin: None,
    },
    Case {
        name: "generate --delete from an updated id",
        argv: &["generate", "4SQ1NZT5HC0-4SQ1NZT5P1S", "--delete"],
        stdin: None,
    },
    Case {
        name: "generate rejects a deleted id",
        argv: &["generate", "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"],
        stdin: None,
    },
    Case {
        name: "generate --delete without an id",
        argv: &["generate", "--delete"],
        stdin: None,
    },
    Case {
        name: "generate rejects junk",
        argv: &["generate", "not-a-ttid"],
        stdin: None,
    },
    Case {
        name: "decode",
        argv: &["decode", "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"],
        stdin: None,
    },
    Case {
        name: "decode one segment",
        argv: &["decode", "4SQ1NZT5HC0"],
        stdin: None,
    },
    Case {
        name: "decode rejects junk",
        argv: &["decode", "nope"],
        stdin: None,
    },
    Case {
        name: "decode without an id",
        argv: &["decode"],
        stdin: None,
    },
    Case {
        name: "validate a good id",
        argv: &["validate", "4SQ1NZT5HC0"],
        stdin: None,
    },
    Case {
        name: "validate junk",
        argv: &["validate", "nope"],
        stdin: None,
    },
    Case {
        name: "validate without an id",
        argv: &["validate"],
        stdin: None,
    },
    Case {
        name: "canonicalize an uppercase id",
        argv: &["canonicalize", "4SQ1NZT5HC0"],
        stdin: None,
    },
    Case {
        name: "canonicalize a lowercase id",
        argv: &["canonicalize", "4sq1nzt5hc0"],
        stdin: None,
    },
    Case {
        name: "canonicalize a mixed-case lifecycle id",
        argv: &["canonicalize", "4sq1nzt5hc0-4SQ1nzT5P1s"],
        stdin: None,
    },
    Case {
        name: "canonicalize rejects junk",
        argv: &["canonicalize", "not-a-ttid"],
        stdin: None,
    },
    Case {
        name: "canonicalize without an id",
        argv: &["canonicalize"],
        stdin: None,
    },
    Case {
        name: "uuid accepts a uuid",
        argv: &["uuid", "3f2504e0-4f89-41d3-9a0c-0305e82c3301"],
        stdin: None,
    },
    Case {
        name: "uuid rejects a ttid",
        argv: &["uuid", "4SQ1NZT5HC0"],
        stdin: None,
    },
    Case {
        name: "uuid without an id",
        argv: &["uuid"],
        stdin: None,
    },
    Case {
        name: "an unknown command",
        argv: &["nope"],
        stdin: None,
    },
    Case {
        name: "an unknown command with an argument",
        argv: &["nope", "arg"],
        stdin: None,
    },
    Case {
        name: "exec without --request",
        argv: &["exec"],
        stdin: None,
    },
    Case {
        name: "--request without a value",
        argv: &["exec", "--request"],
        stdin: None,
    },
    Case {
        name: "exec with a literal payload",
        argv: &[
            "exec",
            "--request",
            "{\"op\":\"isTTID\",\"id\":\"4SQ1NZT5HC0\"}",
        ],
        stdin: None,
    },
    Case {
        name: "exec with a requestId",
        argv: &[
            "exec",
            "--request",
            "{\"op\":\"isUUID\",\"id\":\"nope\",\"requestId\":\"r1\"}",
        ],
        stdin: None,
    },
    Case {
        name: "exec from @file",
        argv: &["exec", "--request", "@REQUEST_FILE"],
        stdin: None,
    },
    Case {
        name: "exec from a missing @file",
        argv: &["exec", "--request", "@/nonexistent/nope.json"],
        stdin: None,
    },
    Case {
        name: "exec rejects an unknown op",
        argv: &["exec", "--request", "{\"op\":\"nope\"}"],
        stdin: None,
    },
    Case {
        name: "exec rejects a non-object payload",
        argv: &["exec", "--request", "\"just a string\""],
        stdin: None,
    },
    Case {
        name: "exec from stdin",
        argv: &["exec", "--request", "-"],
        stdin: Some("{\"op\":\"isTTID\",\"id\":\"4SQ1NZT5HC0\"}"),
    },
    Case {
        name: "exec --loop over several requests",
        argv: &["exec", "--loop"],
        stdin: Some(
            "{\"op\":\"generate\"}\n\n   \n{\"op\":\"decodeTime\",\"id\":\"4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK\"}\n{\"op\":\"isTTID\",\"id\":\"nope\",\"requestId\":\"loop-1\"}\n{not json\n{\"op\":\"nope\"}\n{\"op\":\"isUUID\",\"id\":\"3f2504e0-4f89-41d3-9a0c-0305e82c3301\"}\n",
        ),
    },
];

/// The `@file` case needs a real file; the recording is path-independent
/// because the response never echoes the path on success.
fn request_file() -> std::path::PathBuf {
    let path = std::env::temp_dir().join("ttid-cli-request.json");
    std::fs::write(&path, r#"{"op":"generate","requestId":"from-file"}"#).expect("writes");
    path
}

fn actual(case: &Case, request_path: &str) -> Value {
    let argv: Vec<String> = case
        .argv
        .iter()
        .map(|a| a.replace("REQUEST_FILE", request_path))
        .collect();
    let borrowed: Vec<&str> = argv.iter().map(String::as_str).collect();
    let output = run(BINARY, &borrowed, case.stdin, None);
    json!({ "stdout": normalize(&output.stdout), "code": output.code })
}

fn expectations() -> Value {
    let raw = std::fs::read_to_string(EXPECTATIONS).expect("the recording exists");
    serde_json::from_str(&raw).expect("the recording is valid JSON")
}

#[test]
fn the_cli_surface_matches_the_recording() {
    let expected = expectations();
    let path = request_file();
    let path = path.to_string_lossy().into_owned();

    let mut failures = Vec::new();
    for case in CASES {
        let got = actual(case, &path);
        let want = &expected[case.name];
        if want.is_null() {
            failures.push(format!("{}: no recorded expectation", case.name));
            continue;
        }
        if *want != got {
            failures.push(format!(
                "{}\n  expected code {} stdout:\n{}\n  actual code {} stdout:\n{}",
                case.name,
                want["code"],
                want["stdout"].as_str().unwrap_or_default(),
                got["code"],
                got["stdout"].as_str().unwrap_or_default()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The transport every client shim drives, run far faster than the clock's
/// 200 ns resolution.
#[test]
fn a_burst_down_the_loop_never_repeats() {
    const BURST: usize = 5_000;
    let input = format!("{}\n", r#"{"op":"generate"}"#).repeat(BURST);
    let output = run(BINARY, &["exec", "--loop"], Some(&input), None);

    let ids: Vec<String> = output
        .stdout
        .lines()
        .map(|line| {
            let parsed: Value = serde_json::from_str(line).expect("each line is a response");
            parsed["result"].as_str().expect("an id").to_owned()
        })
        .collect();

    assert_eq!(ids.len(), BURST);
    let unique: std::collections::HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), BURST, "duplicates down one warm process");
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "ids must stay strictly increasing"
    );
}

/// Re-record every case. Ignored by default so a normal run cannot rewrite the
/// thing it is meant to be checked against.
#[test]
#[ignore = "rewrites tests/fixtures/cli-expectations.json"]
fn record() {
    let path = request_file();
    let path = path.to_string_lossy().into_owned();
    let mut recorded = serde_json::Map::new();
    for case in CASES {
        recorded.insert(case.name.to_owned(), actual(case, &path));
    }
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(recorded)).unwrap()
    );
    std::fs::write(EXPECTATIONS, rendered).expect("writes the recording");
    println!("recorded {} CLI cases into {EXPECTATIONS}", CASES.len());
}
