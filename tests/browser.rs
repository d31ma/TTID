//! The kernel running in a real browser.
//!
//! Replaces `scripts/wasm/browser-test.mjs`. The server, the driver and the
//! assertions are Rust; only the page's own `<script>` is JavaScript, which is
//! irreducible — a browser has no other scripting language, so testing "does
//! this work in a browser" means shipping some JavaScript to it, exactly as
//! testing the Python client means running Python.
//!
//! This is not redundant with `tests/wasm.rs`. A real browser is the only place
//! that reproduces the clamped `performance.now()` that once cut a 2000-id
//! burst down to 135 unique identifiers; `wasmi` has no such clamp.
//!
//! Skipped unless a Chrome or Chromium is installed and the module is built.
//! CI has both.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

mod support;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

const MODULE: &str = "target/wasm32-unknown-unknown/release/ttid.wasm";
const CLIENT: &str = "clients/web/ttid-wasm.mjs";

/// The other half of the web surface. `clients/web/ttid.mjs` is the *default*
/// web client — 4 KB, synchronous, no fetch — and it is a hand-written second
/// implementation of the kernel, which is exactly the kind of thing that drifts
/// when nothing checks it. Its suite left with the JavaScript toolchain, so the
/// same page now runs the same assertions against both clients.
const PURE_CLIENT: &str = "clients/web/ttid.mjs";

const BROWSERS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
];

fn find_browser() -> Option<String> {
    BROWSERS
        .iter()
        .find(|candidate| {
            if candidate.starts_with('/') {
                std::path::Path::new(candidate).exists()
            } else {
                support::available(candidate)
            }
        })
        .map(|found| (*found).to_owned())
}

/// The page. Everything it asserts is reported back to the harness as JSON.
const PAGE: &str = r#"<!doctype html>
<meta charset="utf-8"><title>TTID browser parity</title>
<body><pre id="out">running…</pre>
<script type="module">
import { load } from './ttid-wasm.mjs'
import PureTTID from './ttid.mjs'
const results = []
const check = (name, ok, detail) => results.push({ name, ok: !!ok, detail: String(detail ?? '') })

// Every assertion that both web clients must satisfy. `isUUID` is the one
// documented divergence — the wasm client returns a boolean, the hand-written
// one returns the RegExpMatchArray it always did — so it is asserted on
// truthiness, which is the contract callers actually rely on.
const suite = (TTID, label) => {
  const at = (name) => label + ': ' + name

  const id = TTID.generate()
  check(at('generate returns 11 uppercase characters'), /^[A-Z0-9]{11}$/.test(id), id)

  const updated = TTID.generate(id)
  check(at('generate(id) appends a segment'), updated.startsWith(id + '-'), updated)

  const deleted = TTID.generate(updated, true)
  check(at('generate(id, true) yields three segments'), deleted.split('-').length === 3, deleted)

  let locked = false
  try { TTID.generate(deleted) } catch (e) { locked = e.message === 'This identifier can no longer be modified' }
  check(at('a deleted id is immutable'), locked)

  const times = TTID.decodeTime(deleted)
  check(at('decodeTime is chronological'),
        times.createdAt <= times.updatedAt && times.updatedAt <= times.deletedAt,
        JSON.stringify(times))

  check(at('isTTID returns a Date'), TTID.isTTID(id) instanceof Date)
  check(at('isTTID rejects junk'), TTID.isTTID('not-a-ttid') === null)
  check(at('isUUID accepts a uuid'), !!TTID.isUUID('3f2504e0-4f89-41d3-9a0c-0305e82c3301'))
  check(at('isUUID rejects a ttid'), !TTID.isUUID(id))
  check(at('canonical normalizes case'), TTID.canonical(id.toLowerCase()) === id)
  check(at('canonical is idempotent'), TTID.canonical(TTID.canonical(id)) === id)
  check(at('canonical rejects junk'), TTID.canonical('not-a-ttid') === null)

  // The reason this test exists: browsers coarsen performance.now() as a
  // Spectre mitigation, so the raw clock repeats under any burst. Both clients
  // carry the monotonic-step fix; this is what proves it on each of them.
  const burst = new Set()
  for (let i = 0; i < 2000; i++) burst.add(TTID.generate())
  check(at('2000 ids in a tight loop are unique'), burst.size === 2000, burst.size)

  const list = [...burst]
  check(at('ids are strictly increasing'), list.every((v, i) => i === 0 || list[i - 1] < v))
  check(at('ids are all 11 characters'), list.every((v) => v.length === 11))

  const drift = TTID.decodeTime(list[list.length - 1]).createdAt - TTID.decodeTime(list[0]).createdAt
  check(at('a burst barely moves the decoded timestamp'), drift <= 50, drift + 'ms')
}

try {
  const WasmTTID = await load('./ttid.wasm')
  check('abi version is 1', WasmTTID.abiVersion === 1, WasmTTID.abiVersion)

  suite(WasmTTID, 'wasm')
  suite(PureTTID, 'pure')

  // The two are separate implementations of one contract, so an id minted by
  // either has to be readable by the other.
  const fromPure = PureTTID.generate()
  const fromWasm = WasmTTID.generate()
  check('wasm reads an id the pure client minted', WasmTTID.isTTID(fromPure) instanceof Date, fromPure)
  check('the pure client reads an id wasm minted', PureTTID.isTTID(fromWasm) instanceof Date, fromWasm)
  check('both canonicalize identically',
        WasmTTID.canonical(fromPure.toLowerCase()) === PureTTID.canonical(fromPure.toLowerCase()),
        fromPure)
} catch (error) {
  check('the page ran without throwing', false, error && error.stack || error)
}

document.getElementById('out').textContent = JSON.stringify(results, null, 2)
fetch('/result', { method: 'POST', body: JSON.stringify(results) }).catch(() => {})
</script>"#;

fn respond(stream: &mut TcpStream, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

/// Serve the four files the page needs, and capture the verdict it posts back.
fn serve(listener: &TcpListener, verdict: &mpsc::Sender<String>) {
    for incoming in listener.incoming() {
        let Ok(mut stream) = incoming else { continue };
        let mut reader = BufReader::new(stream.try_clone().expect("clones the stream"));

        let mut request_line = String::new();
        if reader.read_line(&mut request_line).is_err() {
            continue;
        }
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or_default().to_owned();
        let path = parts.next().unwrap_or_default().to_owned();

        let mut length = 0_usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }

        match (method.as_str(), path.as_str()) {
            ("POST", "/result") => {
                let mut body = vec![0_u8; length];
                let _ = reader.read_exact(&mut body);
                respond(&mut stream, "text/plain", b"ok");
                let _ = verdict.send(String::from_utf8_lossy(&body).into_owned());
                return;
            }
            (_, "/") => respond(&mut stream, "text/html; charset=utf-8", PAGE.as_bytes()),
            (_, "/ttid.wasm") => {
                let bytes = std::fs::read(MODULE).unwrap_or_default();
                respond(&mut stream, "application/wasm", &bytes);
            }
            (_, "/ttid-wasm.mjs") => {
                let bytes = std::fs::read(CLIENT).unwrap_or_default();
                respond(&mut stream, "text/javascript", &bytes);
            }
            (_, "/ttid.mjs") => {
                let bytes = std::fs::read(PURE_CLIENT).unwrap_or_default();
                respond(&mut stream, "text/javascript", &bytes);
            }
            _ => {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        }
    }
}

#[test]
fn the_kernel_passes_in_a_real_browser() {
    if !std::path::Path::new(MODULE).exists() {
        eprintln!("skipping: {MODULE} not built");
        return;
    }
    let Some(browser) = find_browser() else {
        eprintln!("skipping: no Chrome or Chromium found");
        return;
    };

    let listener = TcpListener::bind("127.0.0.1:0").expect("binds a port");
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || serve(&listener, &sender));

    let profile = std::env::temp_dir().join(format!("ttid-chrome-{port}"));
    let mut child = std::process::Command::new(&browser)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--no-first-run",
            "--disable-dev-shm-usage",
            &format!("--user-data-dir={}", profile.display()),
            &format!("http://127.0.0.1:{port}/"),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("launches the browser");

    let verdict = receiver.recv_timeout(std::time::Duration::from_mins(2));
    let _ = child.kill();
    // Reap it, so the test does not leave a zombie behind.
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);

    let body = verdict.expect("the page reported a verdict within 120s");
    let results: Vec<serde_json::Value> = serde_json::from_str(&body).expect("a verdict list");
    assert!(!results.is_empty(), "the page reported nothing");

    let failed: Vec<String> = results
        .iter()
        .filter(|r| r["ok"] != serde_json::json!(true))
        .map(|r| {
            format!(
                "  {} ({})",
                r["name"].as_str().unwrap_or("?"),
                r["detail"].as_str().unwrap_or("")
            )
        })
        .collect();

    assert!(
        failed.is_empty(),
        "{} of {} browser checks failed:\n{}",
        failed.len(),
        results.len(),
        failed.join("\n")
    );
    println!("  {} browser checks passed in {browser}", results.len());
}
