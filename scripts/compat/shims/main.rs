// Exercises clients/rust/ttid.rs against the binary in TTID_BIN.
// The client returns raw response lines; the harness masks durationMs.
mod ttid;

const FIXED: &str = "4SQ1NZT5HC0";
const UPDATED: &str = "4SQ1NZT5HC0-4SQ1NZT5P1S";
const DELETED: &str = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK";

fn main() -> std::io::Result<()> {
    let binary = std::env::var("TTID_BIN").expect("TTID_BIN must be set");
    let mut t = ttid::Ttid::open(&binary)?;

    println!("generate={}", t.generate(None, false)?.trim());
    println!("update={}", t.generate(Some(FIXED), false)?.trim());
    println!("delete={}", t.generate(Some(UPDATED), true)?.trim());
    println!("decode={}", t.decode_time(DELETED)?.trim());
    println!("isTTID={}", t.is_ttid(FIXED)?.trim());
    println!("isTTID-bad={}", t.is_ttid("nope")?.trim());
    println!(
        "isUUID={}",
        t.is_uuid("3f2504e0-4f89-41d3-9a0c-0305e82c3301")?.trim()
    );
    println!("isUUID-bad={}", t.is_uuid("nope")?.trim());
    println!("canonical={}", t.canonicalize(&FIXED.to_lowercase())?.trim());
    // The client surfaces an `ok: false` response as an io::Error carrying the
    // whole response line.
    match t.generate(Some(DELETED), false) {
        Ok(line) => println!("error=NO ERROR RAISED: {}", line.trim()),
        Err(error) => println!("error={error}"),
    }

    t.close()
}
