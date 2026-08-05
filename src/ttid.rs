//! The TTID kernel: pure computation, no I/O and no clock.
//!
//! Every function here is a direct port of the retired JavaScript engine, and
//! the float64 arithmetic is deliberately reproduced rather
//! than "cleaned up" — see section 4 of `docs/RUST_REWRITE_PLAN.md`. Existing
//! identifiers are a wire format.

/// Multiplier applied to high-resolution timestamps to preserve sub-millisecond
/// precision when encoding to base-36.
pub const PRECISION: f64 = 10_000.0;

/// Encoding base for timestamp segments.
const BASE: u128 = 36;

/// Segment placeholder used when an ID is deleted without a prior update.
const PLACEHOLDER: &str = "X";

/// Minimum accepted timestamp (ms since epoch): 2020-01-01T00:00:00.000Z.
const MIN_TIMESTAMP_MS: f64 = 1_577_836_800_000.0;

/// Maximum accepted timestamp (ms since epoch): 2200-01-01T00:00:00.000Z.
const MAX_TIMESTAMP_MS: f64 = 7_258_118_400_000.0;

const DIGITS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// The timestamps encoded in a TTID, in milliseconds since the epoch.
///
/// Field order matches the JavaScript oracle's object literal, which the
/// machine protocol serializes verbatim.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timestamps {
    /// When the identifier was created.
    pub created_at: i64,
    /// When the identifier was last updated, if ever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    /// When the identifier was deleted, if ever.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<i64>,
}

/// A failure that the JavaScript oracle raises as an `Error`.
///
/// The message is part of the machine protocol contract and must match the
/// oracle's string exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtidError(pub &'static str);

impl std::fmt::Display for TtidError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for TtidError {}

/// Result alias for kernel operations.
pub type Result<T> = std::result::Result<T, TtidError>;

/// True when every byte is ASCII alphanumeric — the `[A-Z0-9]` character class
/// under the oracle's case-insensitive flag.
fn is_alphanumeric_run(segment: &str, min: usize, max: usize) -> bool {
    let length = segment.len();
    length >= min && length <= max && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Equivalent of the oracle's `TTID_PATTERN`:
/// `^[A-Z0-9]{11}(-[A-Z0-9]{1,11}){0,2}$` with the case-insensitive flag.
fn matches_ttid_pattern(id: &str) -> bool {
    let mut segments = id.split('-');
    let Some(created) = segments.next() else {
        return false;
    };
    if !is_alphanumeric_run(created, 11, 11) {
        return false;
    }
    let mut extra = 0;
    for segment in segments {
        extra += 1;
        if extra > 2 || !is_alphanumeric_run(segment, 1, 11) {
            return false;
        }
    }
    true
}

/// Equivalent of the oracle's `UUID_PATTERN`, case-insensitive.
fn matches_uuid_pattern(id: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut groups = id.split('-');
    for width in GROUPS {
        let Some(group) = groups.next() else {
            return false;
        };
        if group.len() != width || !group.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return false;
        }
    }
    groups.next().is_none()
}

/// Encode a scaled timestamp the way `Number.prototype.toString(36)` does.
///
/// Only reachable for integral values: any timestamp at or above the
/// 2020-01-01 floor scales past 2^53, where the f64 ulp is 2. Fractional and
/// negative inputs are unreachable in range and are truncated toward zero
/// rather than emitting the oracle's fractional base-36 form — recorded as a
/// deliberate divergence in `docs/PARITY_LEDGER.md`.
fn scale(now_ms: f64) -> u128 {
    let scaled = now_ms * PRECISION;
    debug_assert!(
        scaled.fract() == 0.0 && scaled >= 0.0,
        "scaled timestamps are integral and non-negative in the supported range"
    );
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is integral and non-negative in the supported range; \
                  saturating casts match the reachable behavior of the oracle"
    )]
    let value = scaled as u128;
    value
}

fn to_base36(mut value: u128) -> String {
    if value == 0 {
        return "0".to_owned();
    }
    let mut bytes = Vec::with_capacity(11);
    while value > 0 {
        // A remainder of a division by 36 always fits a usize.
        let digit = usize::try_from(value % BASE).unwrap_or(0);
        bytes.push(DIGITS[digit]);
        value /= BASE;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap_or_default()
}

/// Decode a base-36 segment the way `parseInt(segment, 36)` does.
///
/// The oracle computes the value exactly and rounds once to the nearest double.
/// Accumulating in f64 would round at every step and give different answers, so
/// the exact value is built in `u128` and converted once.
fn parse_base36(segment: &str) -> f64 {
    let mut value: u128 = 0;
    for byte in segment.bytes() {
        let digit = match byte {
            b'0'..=b'9' => u128::from(byte - b'0'),
            b'a'..=b'z' => u128::from(byte - b'a') + 10,
            b'A'..=b'Z' => u128::from(byte - b'A') + 10,
            _ => return f64::NAN,
        };
        value = value.saturating_mul(BASE).saturating_add(digit);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "matching parseInt: build the value exactly, then round once"
    )]
    let converted = value as f64;
    converted
}

/// Convert an encoded segment to milliseconds, reproducing the oracle's
/// `Number((parseInt(code, 36) / PRECISION).toFixed(0))`.
fn segment_to_milliseconds(segment: &str) -> Result<i64> {
    let milliseconds = parse_base36(segment) / PRECISION;
    // `toFixed(0)` picks the nearest integer and breaks ties upward; `round`
    // breaks ties away from zero, which agrees for the non-negative range.
    let rounded = milliseconds.round();
    if !rounded.is_finite() || !(MIN_TIMESTAMP_MS..=MAX_TIMESTAMP_MS).contains(&rounded) {
        return Err(TtidError("Invalid timestamp encoding"));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is integral and bounded by MAX_TIMESTAMP_MS"
    )]
    let as_integer = rounded as i64;
    Ok(as_integer)
}

/// Decode the timestamps embedded in a TTID.
///
/// # Errors
///
/// Returns an error if the format is invalid or any segment encodes an
/// out-of-range timestamp.
pub fn decode_time(id: &str) -> Result<Timestamps> {
    if !matches_ttid_pattern(id) {
        return Err(TtidError("Invalid Format!"));
    }
    let mut segments = id.split('-');
    let created = segments.next().unwrap_or_default();
    let updated = segments.next();
    let deleted = segments.next();

    Ok(Timestamps {
        created_at: segment_to_milliseconds(created)?,
        updated_at: match updated {
            Some(segment) if segment != PLACEHOLDER => Some(segment_to_milliseconds(segment)?),
            _ => None,
        },
        deleted_at: match deleted {
            Some(segment) => Some(segment_to_milliseconds(segment)?),
            None => None,
        },
    })
}

/// Check whether a string is a valid TTID, returning its creation time in
/// milliseconds.
///
/// Mirrors the oracle's `isTTID`, including the length short-circuit that runs
/// before the pattern test.
#[must_use]
pub fn is_ttid(id: &str) -> Option<i64> {
    if id.is_empty() || id.chars().count() > 36 {
        return None;
    }
    decode_time(id).ok().map(|times| times.created_at)
}

/// Return the canonical spelling of a valid TTID.
///
/// Identifiers are matched case-insensitively but only ever *emitted* in
/// uppercase, so a consumer that stores or compares whatever spelling it was
/// handed can end up treating one identifier as several. This is the function
/// that settles it: feed it any accepted spelling, store what it returns.
///
/// Deliberately lenient in what it accepts — normalizing already-canonical
/// input is a no-op, and rejecting non-canonical input here would leave a
/// consumer that already has some no way to repair it.
///
/// Returns `None` for anything that is not a valid TTID.
#[must_use]
pub fn canonical(id: &str) -> Option<String> {
    is_ttid(id).map(|_| id.to_uppercase())
}

/// Check whether a string is a valid UUID of any version or variant.
#[must_use]
pub fn is_uuid(id: &str) -> bool {
    matches_uuid_pattern(id)
}

/// Generate a new TTID or advance an existing one through its lifecycle.
///
/// `now_ms` is the high-resolution current time in milliseconds, supplied by
/// the transport. It must carry sub-millisecond precision or rapid generation
/// will collide.
///
/// # Errors
///
/// Returns an error if `id` is already deleted (three segments) or is not a
/// valid TTID.
pub fn generate(id: Option<&str>, delete: bool, now_ms: f64) -> Result<String> {
    build(id, delete, scale(now_ms))
}

/// Build an id from an already-scaled timestamp.
fn build(id: Option<&str>, delete: bool, scaled: u128) -> Result<String> {
    let existing = id.filter(|value| !value.is_empty());
    let valid = existing.filter(|value| is_ttid(value).is_some());

    if let Some(value) = valid
        && value.split('-').count() == 3
    {
        return Err(TtidError("This identifier can no longer be modified"));
    }

    let encoded = to_base36(scaled);

    if let Some(value) = valid {
        let mut segments = value.split('-');
        let created = segments.next().unwrap_or_default().to_uppercase();
        if delete {
            let updated = segments.next().unwrap_or(PLACEHOLDER).to_uppercase();
            return Ok(format!("{created}-{updated}-{encoded}"));
        }
        // The oracle discards any existing second segment and re-stamps it.
        return Ok(format!("{created}-{encoded}"));
    }

    if existing.is_some() {
        return Err(TtidError("Invalid TTID!"));
    }

    Ok(encoded)
}

/// A monotonic id source: never returns the same encoded timestamp twice.
///
/// # Why this exists
///
/// The encoded timestamp is a float, and at the current epoch an `f64` has an
/// ulp of 2 scaled units — **200 nanoseconds**. Any caller that generates
/// faster than that gets duplicate ids from the raw clock. Browsers make it far
/// worse: `performance.now()` is clamped to roughly 100 µs as a Spectre
/// mitigation, 500× coarser still.
///
/// Measured, before this existed: 1746/2000 unique in a Bun loop, 135/2000 in a
/// browser.
///
/// # How
///
/// If the clock has not advanced past the last id issued, use `last + 1`
/// instead. One scaled unit is 0.1 µs, so a 2000-id burst drifts the encoded
/// time forward by 200 µs — far below the millisecond that [`decode_time`]
/// rounds to, meaning **timestamps still decode to the same value** while the
/// ids stay distinct and strictly increasing (so they stay sortable).
///
/// Raising `PRECISION` is not an alternative: 11 base-36 characters and f64's
/// mantissa both cap the usable resolution.
///
/// # Limits
///
/// The guarantee is per-generator. Two processes, or two machines, can still
/// collide — the same limit ULID's monotonic factory has. A shim holds one
/// long-lived `ttid` process, so one generator covers one application.
#[derive(Debug, Default)]
pub struct Generator {
    last: u128,
}

impl Generator {
    /// A generator that has issued nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// As [`generate`], but never repeats a timestamp already issued.
    ///
    /// When the clock has advanced, the result is byte-identical to the
    /// stateless [`generate`] — the bump only replaces what would otherwise be
    /// a duplicate.
    ///
    /// # Errors
    ///
    /// Returns an error if `id` is already deleted (three segments) or is not a
    /// valid TTID.
    pub fn generate(&mut self, id: Option<&str>, delete: bool, now_ms: f64) -> Result<String> {
        let scaled = scale(now_ms).max(self.last.saturating_add(1));
        let built = build(id, delete, scaled)?;
        // Only commit once the lifecycle checks have passed, so a rejected
        // request does not burn a timestamp.
        self.last = scaled;
        Ok(built)
    }
}
