//! Shared helpers for the process-level test suites.
//!
//! These replace what `scripts/compat/*.mjs` used to do from Bun. Keeping them
//! in Rust means the whole gate runs under `cargo test`, with no second
//! language in the build.

#![allow(dead_code)]

use std::io::Write;
use std::process::{Command, Stdio};

/// True when `program` resolves on PATH.
#[must_use]
pub fn available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// The result of running a process to completion.
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

/// Run `program` with `args`, optionally writing `stdin`, and wait for it.
pub fn run(program: &str, args: &[&str], stdin: Option<&str>, cwd: Option<&str>) -> Output {
    run_with_env(program, args, stdin, cwd, &[])
}

/// As [`run`], with extra environment variables.
pub fn run_with_env(
    program: &str,
    args: &[&str],
    stdin: Option<&str>,
    cwd: Option<&str>,
    env: &[(&str, &str)],
) -> Output {
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn {program}: {error}"));

    if let Some(input) = stdin {
        let mut handle = child.stdin.take().expect("stdin is piped");
        // A large payload can outrun the child's reads, so write on a thread
        // and let the main thread keep draining stdout.
        let owned = input.to_owned();
        std::thread::spawn(move || {
            let _ = handle.write_all(owned.as_bytes());
        });
    } else {
        drop(child.stdin.take());
    }

    let finished = child.wait_with_output().expect("the child runs");
    Output {
        stdout: String::from_utf8_lossy(&finished.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&finished.stderr).into_owned(),
        code: finished.status.code().unwrap_or(-1),
    }
}

/// Erase what two independent runs cannot agree on: the wall-clock duration,
/// and the timestamps baked into freshly minted identifiers.
///
/// Hand-written rather than a regex — the crate has no regex dependency, and
/// the two patterns are narrow enough to scan directly.
#[must_use]
pub fn normalize(text: &str) -> String {
    mask_identifiers(&mask_duration(text))
}

/// `"durationMs": 3` and `"durationMs":3` both become `"durationMs":<N>`.
fn mask_duration(text: &str) -> String {
    const KEY: &str = "\"durationMs\"";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find(KEY) {
        out.push_str(&rest[..at]);
        out.push_str(KEY);
        let mut tail = &rest[at + KEY.len()..];

        // Skip the colon and any whitespace, then the digits.
        let skipped: String = tail
            .chars()
            .take_while(|c| *c == ':' || c.is_whitespace())
            .collect();
        tail = &tail[skipped.len()..];
        let digits = tail.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            // Not the shape we expected; leave it alone.
            out.push_str(&skipped);
            rest = tail;
            continue;
        }
        out.push_str(skipped.trim_end_matches(char::is_whitespace));
        out.push_str("<N>");
        rest = &tail[digits..];
    }
    out.push_str(rest);
    out
}

/// Replace every TTID-shaped token with `<SEG>` per segment, keeping the `X`
/// placeholder visible so a lost placeholder still shows up as a difference.
fn mask_identifiers(text: &str) -> String {
    let bytes = text.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;

    while index < bytes.len() {
        // A token boundary: the previous byte must not be part of a word.
        let at_boundary = index == 0 || !is_word(bytes[index - 1]);
        if !at_boundary || !is_word(bytes[index]) {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        }

        // Collect segments of uppercase-or-digit separated by '-'.
        let mut end = index;
        let mut segments: Vec<&str> = Vec::new();
        loop {
            let start = end;
            while end < bytes.len()
                && (bytes[end].is_ascii_uppercase() || bytes[end].is_ascii_digit())
            {
                end += 1;
            }
            if end == start {
                break;
            }
            segments.push(&text[start..end]);
            if end < bytes.len() && bytes[end] == b'-' && segments.len() < 3 {
                end += 1;
            } else {
                break;
            }
        }

        let looks_like_ttid = segments.first().is_some_and(|first| first.len() == 11)
            && segments.iter().skip(1).all(|s| (1..=11).contains(&s.len()))
            && (end >= bytes.len() || !is_word(bytes[end]));

        if looks_like_ttid {
            let masked: Vec<&str> = segments
                .iter()
                .map(|s| if *s == "X" { "X" } else { "<SEG>" })
                .collect();
            out.push_str(&masked.join("-"));
            index = end;
        } else {
            out.push(bytes[index] as char);
            index += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_durations_in_both_spellings() {
        assert_eq!(
            mask_duration(r#"{"durationMs": 12}"#),
            r#"{"durationMs":<N>}"#
        );
        assert_eq!(
            mask_duration(r#"{"durationMs":0}"#),
            r#"{"durationMs":<N>}"#
        );
    }

    #[test]
    fn masks_identifiers_but_keeps_the_placeholder() {
        assert_eq!(mask_identifiers("4SQ1NZT5HC0"), "<SEG>");
        assert_eq!(
            mask_identifiers("4SQ1NZT5HC0-X-4SQ1NZT5WRK"),
            "<SEG>-X-<SEG>"
        );
        assert_eq!(mask_identifiers("4SQ1NZT5HC0-4SQ1NZT5P1S"), "<SEG>-<SEG>");
    }

    #[test]
    fn leaves_things_that_are_not_identifiers_alone() {
        assert_eq!(mask_identifiers("hello world"), "hello world");
        assert_eq!(mask_identifiers("ABCDEFGHIJ"), "ABCDEFGHIJ"); // ten characters
        assert_eq!(mask_identifiers("Invalid TTID!"), "Invalid TTID!");
    }
}
