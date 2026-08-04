//! TTID — time-tagged identifiers.
//!
//! One kernel, two transports. [`ttid`] is the pure computation; [`machine`] is
//! the frozen JSON protocol that both the native binary and the wasm module
//! speak, so a client shim cannot tell which artifact it is talking to.
//!
//! The kernel reads no clock: the current high-resolution time is a parameter,
//! supplied by whichever transport is in use.

pub mod machine;
pub mod ttid;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
