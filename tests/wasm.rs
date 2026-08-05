//! The WebAssembly artifact: identical answers, and small enough to ship.
//!
//! Replaces `scripts/wasm/abi-probe.mjs` and `size-budget.mjs`. Driving the
//! module from Rust rather than from a JavaScript host means the whole gate
//! runs under `cargo test`, and it exercises the raw C ABI directly — no
//! bindings layer sitting between the test and the thing under test.
//!
//! The module has to be built first:
//!
//! ```text
//! cargo build --lib --release --target wasm32-unknown-unknown
//! ```
//!
//! Absent that, these tests skip rather than fail: a plain `cargo test` on a
//! machine without the wasm target should not report a red suite. CI builds it,
//! so CI runs them.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use serde_json::Value;
use wasmi::{Engine, Instance, Linker, Module, Store, TypedFunc};

const MODULE: &str = "target/wasm32-unknown-unknown/release/ttid.wasm";
const ABI_VERSION: u32 = 1;

/// 128 KiB raw. The kernel is a few KB; `serde_json` and `indexmap` are the
/// bulk. Hand-rolled serialization would reach roughly 10 KiB if this ceiling
/// ever binds. Raise it deliberately and say what earned the space — that edit
/// is the review signal.
const RAW_BUDGET: usize = 128 * 1024;

/// A build that shrinks to nothing is broken, not a win.
const RAW_FLOOR: usize = 8 * 1024;

#[expect(
    clippy::cast_precision_loss,
    reason = "a byte count that fits in a wasm module never approaches 2^52"
)]
fn kib(n: usize) -> String {
    format!("{:.1} KiB", n as f64 / 1024.0)
}

fn wasm_bytes() -> Option<Vec<u8>> {
    std::fs::read(MODULE).ok()
}

/// The module under test, with its four exported functions bound.
struct Kernel {
    store: Store<()>,
    memory: wasmi::Memory,
    allocate: TypedFunc<i32, i32>,
    deallocate: TypedFunc<(i32, i32), ()>,
    execute: TypedFunc<(i32, i32, f64, f64), i64>,
    reset: TypedFunc<(), ()>,
    abi_version: u32,
}

impl Kernel {
    fn load(bytes: &[u8]) -> Self {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("the module is valid wasm");
        let mut store = Store::new(&engine, ());
        // No imports at all: the module is freestanding, which is exactly why
        // it runs in a browser with `WebAssembly.instantiate(bytes, {})`.
        let instance: Instance = Linker::new(&engine)
            .instantiate_and_start(&mut store, &module)
            .expect("instantiates with no imports");

        let memory = instance
            .get_memory(&store, "memory")
            .expect("exports its memory");
        let version: TypedFunc<(), u32> = instance
            .get_typed_func(&store, "ttid_abi_version")
            .expect("exports ttid_abi_version");
        let abi_version = version.call(&mut store, ()).expect("reports its version");

        Self {
            allocate: instance.get_typed_func(&store, "ttid_allocate").unwrap(),
            deallocate: instance.get_typed_func(&store, "ttid_deallocate").unwrap(),
            execute: instance.get_typed_func(&store, "ttid_execute").unwrap(),
            reset: instance.get_typed_func(&store, "ttid_reset").unwrap(),
            memory,
            store,
            abi_version,
        }
    }

    /// One request in, one response out, over linear memory.
    ///
    /// `stateless` forgets previously issued timestamps first — the corpus pins
    /// the stateless contract and feeds timestamps out of order on purpose.
    fn execute(&mut self, request: &str, now_ms: f64, stateless: bool) -> Option<String> {
        if stateless {
            self.reset.call(&mut self.store, ()).unwrap();
        }

        let bytes = request.as_bytes();
        let length = i32::try_from(bytes.len()).unwrap();
        let pointer = self.allocate.call(&mut self.store, length).unwrap();
        self.memory
            .write(&mut self.store, usize::try_from(pointer).unwrap(), bytes)
            .unwrap();

        let packed = self
            .execute
            .call(&mut self.store, (pointer, length, now_ms, 0.0))
            .unwrap();
        self.deallocate
            .call(&mut self.store, (pointer, length))
            .unwrap();

        if packed == 0 {
            return None;
        }
        // (pointer << 32) | length — wasm32 pointers are 32-bit, so both halves
        // fit in the i64 the ABI returns.
        #[expect(clippy::cast_sign_loss, reason = "the ABI packs two u32 halves")]
        let packed = packed as u64;
        let out_pointer = usize::try_from(packed >> 32).unwrap();
        let out_length = usize::try_from(packed & 0xffff_ffff).unwrap();

        let mut buffer = vec![0_u8; out_length];
        self.memory
            .read(&self.store, out_pointer, &mut buffer)
            .unwrap();
        self.deallocate
            .call(
                &mut self.store,
                (
                    i32::try_from(out_pointer).unwrap(),
                    i32::try_from(out_length).unwrap(),
                ),
            )
            .unwrap();

        Some(String::from_utf8(buffer).expect("responses are UTF-8"))
    }
}

/// Skip rather than fail when the module has not been built.
macro_rules! kernel_or_skip {
    () => {
        match wasm_bytes() {
            Some(bytes) => (Kernel::load(&bytes), bytes),
            None => {
                eprintln!("skipping: {MODULE} not built");
                return;
            }
        }
    };
}

#[test]
fn the_module_is_freestanding_and_reports_its_abi() {
    let (kernel, _) = kernel_or_skip!();
    // Instantiating with an empty linker above is the assertion: a module with
    // imports would have failed there. That is what lets it load in a browser,
    // a Worker, or a WASI host with no glue.
    assert_eq!(kernel.abi_version, ABI_VERSION);
}

#[test]
fn every_corpus_response_is_byte_identical_over_the_abi() {
    let (mut kernel, _) = kernel_or_skip!();
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/corpus.json")).expect("corpus parses");

    let cases = corpus["cases"]["machine"]
        .as_array()
        .expect("machine cases");
    for case in cases {
        let request = serde_json::to_string(&case["request"]).unwrap();
        let now_ms = case["nowMs"].as_f64().unwrap();
        let actual = kernel.execute(&request, now_ms, true);
        assert_eq!(
            actual.as_deref(),
            case["response"].as_str(),
            "case: {}",
            case["name"]
        );
    }
    assert!(cases.len() >= 30, "the corpus should not have shrunk");
}

#[test]
fn the_abi_matches_the_native_transport_on_edge_cases() {
    let (mut kernel, _) = kernel_or_skip!();

    // A blank request carries no response, matching the native NDJSON loop.
    assert_eq!(kernel.execute("   ", 0.0, true), None);
    assert_eq!(kernel.execute("", 0.0, true), None);

    let malformed = kernel.execute("{not json", 0.0, true).expect("a response");
    assert!(malformed.contains("Invalid JSON request"), "{malformed}");
}

#[test]
fn a_frozen_clock_still_yields_unique_ids_over_the_abi() {
    const BURST: usize = 20_000;

    let (mut kernel, _) = kernel_or_skip!();
    let frozen = 1_754_179_200_000.0;

    let mut seen = std::collections::HashSet::with_capacity(BURST);
    let mut previous = String::new();
    for _ in 0..BURST {
        let response = kernel
            .execute(r#"{"op":"generate"}"#, frozen, false)
            .expect("a response");
        let parsed: Value = serde_json::from_str(&response).unwrap();
        let id = parsed["result"].as_str().expect("an id").to_owned();

        assert_eq!(id.len(), 11, "{id}");
        assert!(previous.is_empty() || previous < id, "not increasing: {id}");
        previous = id.clone();
        assert!(seen.insert(id), "duplicate under a frozen clock");
    }
    assert_eq!(seen.len(), BURST);
}

#[test]
fn the_module_stays_within_its_size_budget() {
    let (_, bytes) = kernel_or_skip!();

    assert!(
        bytes.len() >= RAW_FLOOR,
        "only {} — that is too small to be a real build",
        kib(bytes.len())
    );
    assert!(
        bytes.len() <= RAW_BUDGET,
        "wasm module is {} against a {} ceiling. Shrink it, or raise RAW_BUDGET \
         with a note explaining what earned the space.",
        kib(bytes.len()),
        kib(RAW_BUDGET)
    );
    println!(
        "wasm module: {} of {} ({} headroom)",
        kib(bytes.len()),
        kib(RAW_BUDGET),
        kib(RAW_BUDGET - bytes.len())
    );
}
