//! TTID client — drives the `ttid` binary's persistent NDJSON loop.
//!
//! std only. Requires the `ttid` binary on PATH or an explicit path. One
//! long-lived subprocess. Methods build the request and return the raw response
//! line (also JSON); bring serde if you want typed structs.
//!
//!   let mut t = Ttid::open("ttid")?;
//!   let id = t.generate(None, false)?;                  // {"...","result":"4VL..."}
//!   let up = t.generate(Some("4VL..."), false)?;        // advance it
//!   let times = t.decode_time("4VL...")?;
//!   let valid = t.is_ttid("4VL...")?;
//!   let uuid = t.is_uuid("...")?;
//!
//! Method names follow Rust's snake_case; `request` is the raw escape hatch for
//! ops without a method.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

pub struct Ttid {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Ttid {
    /// Start a warm ttid process. `binary` is usually "ttid".
    pub fn open(binary: &str) -> std::io::Result<Ttid> {
        let mut child = Command::new(binary)
            .args(["exec", "--loop"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        Ok(Ttid { child, stdin: Some(stdin), stdout })
    }

    /// Send one machine-protocol op (a JSON object string) and return the response line.
    pub fn request(&mut self, op_json: &str) -> std::io::Result<String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "ttid closed"))?;
        stdin.write_all(op_json.trim_end().as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        let mut line = String::new();
        if self.stdout.read_line(&mut line)? == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "ttid closed the stream",
            ));
        }
        Ok(line)
    }

    // Send a fully-formed op JSON and error on a failure response.
    // ponytail: checks the always-present "ok":true field by substring.
    fn checked(&mut self, json: String) -> std::io::Result<String> {
        let resp = self.request(&json)?;
        if !resp.contains("\"ok\":true") {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, resp.trim().to_string()));
        }
        Ok(resp)
    }

    /// Generate a new TTID, or advance `id` (`del` to tombstone). `None` for a fresh id.
    pub fn generate(&mut self, id: Option<&str>, del: bool) -> std::io::Result<String> {
        let mut op = String::from(r#"{"op":"generate""#);
        if let Some(id) = id {
            op.push_str(&format!(r#","id":"{}""#, esc(id)));
        }
        if del {
            op.push_str(r#","delete":true"#);
        }
        op.push('}');
        self.checked(op)
    }

    pub fn decode_time(&mut self, id: &str) -> std::io::Result<String> {
        self.checked(format!(r#"{{"op":"decodeTime","id":"{}"}}"#, esc(id)))
    }

    pub fn is_ttid(&mut self, id: &str) -> std::io::Result<String> {
        self.checked(format!(r#"{{"op":"isTTID","id":"{}"}}"#, esc(id)))
    }

    pub fn is_uuid(&mut self, id: &str) -> std::io::Result<String> {
        self.checked(format!(r#"{{"op":"isUUID","id":"{}"}}"#, esc(id)))
    }

    /// End the loop and wait for the process to exit.
    pub fn close(mut self) -> std::io::Result<()> {
        self.stdin.take(); // drop stdin → EOF ends the loop
        self.child.wait().map(|_| ())
    }
}

/// Escape a string for embedding in a JSON string literal.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
