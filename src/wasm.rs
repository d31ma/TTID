//! The wasm transport: a narrow C ABI over linear memory.
//!
//! No `wasm-bindgen`. The host copies a request in, calls [`ttid_execute`], and
//! copies the response out — the same JSON bytes the native binary reads from
//! stdin and writes to stdout. That is the whole of the seamless-swap contract:
//! a shim speaking this ABI and a shim spawning the binary get identical
//! answers because they run identical code.
//!
//! `unsafe` is confined to the copies across the memory boundary; the kernel in
//! [`crate::ttid`] never sees a pointer.

#![allow(unsafe_code)]

use std::cell::RefCell;

use crate::{machine, ttid};

/// Version of this ABI. Bump on any breaking change to the signatures below.
pub const ABI_VERSION: u32 = 1;

thread_local! {
    /// One monotonic generator per module instance, so a burst inside a single
    /// page never repeats an id even when the host clock is coarse.
    static GENERATOR: RefCell<ttid::Generator> = RefCell::new(ttid::Generator::new());
}

/// Report the ABI version so a host can refuse a module it cannot drive.
#[unsafe(no_mangle)]
pub extern "C" fn ttid_abi_version() -> u32 {
    ABI_VERSION
}

/// Forget every timestamp issued so far.
///
/// For test harnesses that pin the clock and need the raw, stateless result —
/// the parity corpus feeds timestamps out of order on purpose. Production hosts
/// have no reason to call this: it deliberately reopens the door to duplicate
/// ids.
#[unsafe(no_mangle)]
pub extern "C" fn ttid_reset() {
    GENERATOR.with_borrow_mut(|generator| *generator = ttid::Generator::new());
}

/// Allocate `length` bytes of guest memory for a host-to-guest copy.
///
/// The host must release the block with [`ttid_deallocate`] using the same
/// length.
#[unsafe(no_mangle)]
pub extern "C" fn ttid_allocate(length: usize) -> *mut u8 {
    // A boxed slice has capacity exactly equal to its length, so the host only
    // ever has to remember one number to give the block back.
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast::<u8>()
}

/// Release a block returned by [`ttid_allocate`] or [`ttid_execute`].
///
/// # Safety
///
/// `pointer` must come from [`ttid_allocate`] or [`ttid_execute`] with exactly
/// this `length`, and must not have been released already.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttid_deallocate(pointer: *mut u8, length: usize) {
    if pointer.is_null() || length == 0 {
        return;
    }
    // SAFETY: the ABI contract requires the original pointer and length, and
    // every block this module hands out is a boxed slice of exactly that size.
    drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) });
}

/// Execute one machine-protocol request.
///
/// `now_ms` is the current high-resolution time in milliseconds — the host
/// should pass `performance.timeOrigin + performance.now()`, not `Date.now()`,
/// or rapid generation will collide. `duration_ms` is what the response reports
/// as `durationMs`; it is `f64` rather than `i64` so hosts need no `BigInt` to
/// call this.
///
/// Returns the response as a packed `(pointer << 32) | length`. The host reads
/// that many UTF-8 bytes and then calls [`ttid_deallocate`] with the same pair.
/// A zero return means the request was blank and carries no response, matching
/// the native transport's handling of empty NDJSON lines.
///
/// # Safety
///
/// `pointer` must identify `length` readable bytes for the duration of the
/// call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ttid_execute(
    pointer: *const u8,
    length: usize,
    now_ms: f64,
    duration_ms: f64,
) -> u64 {
    if pointer.is_null() {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "durationMs is a whole millisecond count supplied by the host"
    )]
    let duration_ms = duration_ms as i64;
    // SAFETY: the ABI contract requires `length` readable bytes at `pointer`.
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    // Invalid UTF-8 cannot be valid JSON, so it takes the same path as any
    // other unparseable line rather than a distinct failure mode.
    let request = std::str::from_utf8(bytes).unwrap_or("\u{fffd}");

    let Some(response) = GENERATOR.with_borrow_mut(|generator| {
        machine::execute_line(request, now_ms, duration_ms, generator)
    }) else {
        return 0;
    };

    let bytes = response.into_bytes().into_boxed_slice();
    let length = bytes.len();
    let pointer = Box::into_raw(bytes).cast::<u8>();

    // wasm32 pointers are 32-bit, so both halves fit.
    (u64::from(pointer as u32) << 32) | length as u64
}
