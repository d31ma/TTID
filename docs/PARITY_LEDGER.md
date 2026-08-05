# Parity Ledger

Where the Rust implementation stands against the JavaScript implementation it
replaced. **The JavaScript engine has been deleted** (2026-08-04); this document
is now a historical record of what was proven before it went, plus the gates
that still run.

Everything marked `identical` was proven by executing a harness while both
engines were present. Those proofs are preserved as committed recordings —
`tests/fixtures/corpus.json`, `cli-expectations.json`, `shim-expectations.json` —
which CI still checks on every run. See [TEST_MIGRATION.md](TEST_MIGRATION.md)
for where each JavaScript test now lives.

Maintained by hand, checked by machine. Every `identical` row below was proven
by executing a harness, not by reading source:

```bash
cargo test
```

## Vocabulary

| Status | Meaning |
| --- | --- |
| `identical` | Both implementations produce the same bytes for the same input. Proven by the golden corpus. |
| `equivalent` | Both support the behavior; the observable result differs only as recorded here. |
| `divergent` | The implementations differ. Each row states whether the difference is reachable. |
| `pending` | Not yet ported. |

## Kernel

| Behavior | Status | Evidence |
| --- | --- | --- |
| `generate` — new id | `identical` | corpus `generate`, 17 cases |
| `generate` — update, delete, `X` placeholder | `identical` | corpus `generate` |
| `generate` — already-deleted rejection | `identical` | corpus `generate` |
| `generate` — sub-millisecond precision | `identical` | corpus `generate`, 0.1 ms apart |
| `generate` — 2020 and 2200 bounds | `identical` | corpus `generate` |
| `decodeTime` — one, two, three segments | `identical` | corpus `decodeTime`, 17 cases |
| `decodeTime` — out-of-range rejection | `identical` | corpus `decodeTime` |
| `decodeTime` — malformed input rejection | `identical` | corpus `decodeTime` |
| `isTTID` — including the 36-character short-circuit | `identical` | corpus `isTTID`, 9 cases |
| `isUUID` | `identical` | corpus `isUUID`, 7 cases |
| Base-36 encode past 2^53 | `identical` | corpus round-trips every generated id |
| `parseInt(s, 36)` rounding | `identical` | exact `u128` accumulation, one final `as f64` |
| `toFixed(0)` tie-breaking | `identical` | `f64::round` agrees on the non-negative range |

## Machine protocol

| Behavior | Status | Evidence |
| --- | --- | --- |
| Response bytes, including key order | `identical` | corpus `machine`, 30 cases compared as strings |
| `protocolVersion`, `ok`, `op`, `requestId`, `durationMs` | `identical` | corpus `machine` |
| Error `name` and `message` strings | `identical` | corpus `machine` |
| `op` defaults to `generate` on success, `null` on error | `identical` | corpus `machine` |
| JavaScript falsiness of `request.id` (`null`, `0`, `false`, `""`) | `identical` | corpus `machine` |
| Non-string truthy `request.id` → `Invalid TTID!` | `identical` | corpus `machine`, 5 cases |
| Blank NDJSON line produces no response | `identical` | `tests/oracle.rs`, `tests/wasm.rs` |
| Malformed JSON → `Invalid JSON request` | `identical` | `tests/oracle.rs`, `tests/wasm.rs` |

## Browser

Run with `cargo test --test browser`. The server, driver and assertions are
Rust; the page's own `<script>` is the only JavaScript, which is irreducible —
a browser has no other scripting language. No Playwright: the page POSTs its
verdict back to the harness, so any browser that can open a URL will do, and CI
uses the Chrome already on the runner image.

A browser is not redundant with the `wasmi`-hosted probe — it is the only place that
reproduces the clamped `performance.now()` that once cut a 2000-id burst to 135
unique ids.

| Behavior | Status | Evidence |
| --- | --- | --- |
| Module runs unmodified in a browser | `identical` | Zero imports; `WebAssembly.instantiate(bytes, {})`. 17 checks pass headless in CI. |
| All 30 machine responses over the raw ABI | `identical` | Browser run, compared as strings |
| `clients/web/ttid-wasm.mjs` ergonomic API | `equivalent` | `isUUID` returns a boolean, not the JS shim's `RegExpMatchArray` |
| Loading is asynchronous | `changed` | `await load(url)` vs the JS shim's synchronous import. Inherent to `WebAssembly.instantiate`. |

**The web client ships both implementations, deliberately.**
`clients/web/ttid.mjs` remains the default — 4 KB, synchronous, no fetch.
`clients/web/ttid-wasm.mjs` is for callers that need the identical compiled
engine: WASI runtimes, sandboxed plugin hosts. The drift risk that made a
hand-written second implementation dangerous is retired by testing, not by
deletion: `tests/browser.rs` runs one assertion suite against both clients in a
real browser on every change — 36 checks, including the 2000-id uniqueness
burst on each and three that mint an id with one client and read it with the
other.

### Duplicate ids under load — found, then fixed on every target

**This was a real defect, not a browser quirk, and it predates the rewrite.**

The encoded timestamp is a double. At the current epoch an `f64` has an ulp of
2 scaled units — **200 nanoseconds** — so any caller generating faster than that
received duplicate ids. Browsers make it far worse: `performance.now()` is
clamped to roughly 100 µs as a Spectre mitigation, 500× coarser again.

Measured before the fix, 2000 ids in a tight loop:

| Target | Unique |
| --- | --- |
| the JavaScript engine (nanosecond clock) | 1746 / 2000 |
| `clients/web/ttid.mjs` in a browser | 30 / 2000 |
| wasm kernel in a browser | 135 / 2000 |

Raising `PRECISION` cannot fix it: 11 base-36 characters cap the value at
`36^11`, and f64's mantissa caps the resolution regardless.

**The fix**, applied to the Rust kernel (`ttid::Generator`) and, while it still
existed, the JavaScript engine and the web shim: if the clock has not advanced
past the last id issued, use the next representable value instead. This is what
ULID's monotonic factory and Snowflake's sequence field do.

In JavaScript the step must be ulp-aware — `last + 1` rounds straight back to
`last` at this magnitude — so `nextRepresentable` doubles the increment until
the sum actually differs.

| Target | Unique after the fix |
| --- | --- |
| `clients/web/ttid.mjs` in a browser | 2000 / 2000 |
| wasm kernel in a browser | 2000 / 2000 |
| wasm kernel, clock **frozen** | 20000 / 20000 |
| Rust kernel, clock **frozen** | 20000 / 20000 |
| `ttid exec --loop` | 5000 / 5000 |

Properties preserved: ids stay 11 characters, stay strictly increasing (so
lexicographic sort is still creation order), and stay decodable. One scaled unit
is 0.1 µs, so a 20000-id burst moves the decoded timestamp by at most 2 ms —
below what `decodeTime` rounds to.

| Behavior | Status | Evidence |
| --- | --- | --- |
| A burst on a frozen clock never repeats | `changed` | `tests/oracle.rs`, `tests/wasm.rs`, browser page |
| A burst stays strictly increasing | `changed` | same |
| A moving clock is byte-identical to the stateless path | `identical` | `tests/oracle.rs` — the bump only replaces a would-be duplicate |
| A rejected request does not burn a timestamp | `changed` | `tests/oracle.rs` |
| The golden corpus | `identical` | Unchanged by the fix; harnesses reset the counter to pin the stateless contract |

**Known limit.** The guarantee is per generator: per process for the binary, per
module instance for wasm, per module for the web client. Two processes
or two machines can still collide — the same limit ULID's monotonic factory has.
A client shim holds one long-lived `ttid` process, so one generator covers one
application.

## Native binary vs wasm module

| Behavior | Status | Evidence |
| --- | --- | --- |
| Every machine-protocol response | `identical` | `tests/wasm.rs` — 32 cases, same bytes |

Both transports call the same `machine::execute_line`, so this is identity by
construction rather than by coincidence. The probe exists to keep it that way.

## Canonical form (issue #32)

Identifiers are matched case-insensitively but only ever emitted in uppercase,
so `2^n` spellings of an `n`-letter id all validate and all decode to the same
instant. String equality was therefore not identity — a portability hazard for
anything that persists or sorts by identifier, and the behaviour is unchanged
all the way back to v26.28.02, so it is not a rewrite regression.

Added in 26.32.03, additive and non-breaking:

| Behavior | Status | Evidence |
| --- | --- | --- |
| `canonical` / `ttid canonicalize` returns the uppercase spelling | `rust-only` | `invariants::canonical_is_uppercase_for_every_accepted_spelling` |
| Idempotent | `rust-only` | `invariants::canonical_is_idempotent` |
| Normalizing preserves the decoded instant | `rust-only` | `invariants::canonical_preserves_the_decoded_instant` |
| Restores chronological byte-order sorting | `rust-only` | `invariants::canonicalizing_restores_chronological_sort_order` |
| Rejects anything that is not a valid TTID | `rust-only` | `invariants::canonical_rejects_what_is_not_a_ttid` |
| Available in all four native clients | `identical` | `tests/invariants.rs`; Swift, Kotlin and Dart compile-checked |

`canonical` is **deliberately lenient** in what it accepts, and stays that way
even after validation tightens. It is the repair path for identifiers already
stored in a non-canonical spelling; a strict `canonicalize` would reject exactly
the input it exists to fix.

**Planned, not yet done.** A future major release will make `isTTID` and
`decodeTime` accept only the canonical form. That is a breaking change and needs
to land together with:

- the four native reimplementations (`clients/web/ttid.mjs`,
  `TtidNative.swift`, `TtidNative.kt`, `ttid_native.dart`), which each carry
  their own case-insensitive pattern;
- three frozen corpus cases (`generate: lowercase input is accepted and
  uppercased`, `decodeTime: lowercase is accepted`, `isTTID: lowercase is
  valid`), re-recorded as a stated divergence from the retired oracle rather
  than silently regenerated;
- `invariants::lowercase_input_is_accepted_and_normalized`, which asserts the
  current leniency.

## Recorded divergences

| Behavior | Status | Reachable? | Notes |
| --- | --- | --- | --- |
| `toString(36)` on a non-integral scaled timestamp | `divergent` | No | The oracle emits a fractional base-36 string; Rust truncates. Any timestamp at or above the 2020-01-01 floor scales past 2^53, where the f64 ulp is 2, so the value is always integral. A `debug_assert` catches it if that ever stops holding. |
| `toString(36)` on a negative scaled timestamp | `divergent` | No | The oracle emits a leading `-`; Rust saturates to zero. Requires a system clock before 1970. |
| `String.prototype.trim` vs Rust `str::trim` | `divergent` | Barely | Rust trims Unicode `White_Space`; JavaScript also trims `U+FEFF`. Only observable for an `id` consisting solely of a byte-order mark, which both reject anyway — with the same message. |
| `String.length` (UTF-16 units) vs `chars().count()` | `divergent` | No | Differs only for astral-plane characters, which fail the ASCII-only pattern in both implementations. Same `null` either way. |

## Native CLI

Verified by `tests/cli.rs` — 32 cases run against both
binaries, comparing stdout and exit code. `durationMs` and the timestamps inside
freshly minted ids are normalized; nothing else is.

| Behavior | Status | Evidence |
| --- | --- | --- |
| `--help` / `-h` text | `identical` | `diff` of both binaries' output |
| No arguments prints help and exits 1 | `identical` | CLI differential |
| `generate`, with id, with `--delete` | `identical` | CLI differential |
| `decode`, `validate`, `uuid` | `identical` | CLI differential |
| Missing-argument errors for every command | `identical` | CLI differential |
| Unknown command message | `identical` | CLI differential |
| `exec --request` literal, `@file`, and `-` | `identical` | CLI differential |
| `exec --loop` NDJSON over stdio | `identical` | CLI differential, 8-line session |
| Two-space indented one-shot output | `identical` | CLI differential |
| Exit codes (0 success, 1 error) | `identical` | CLI differential |

## Client shims

Verified by `tests/shims.rs`. Each shim is run
**unmodified** against the JavaScript binary and the Rust binary; the outputs
must match.

| Shim | Status | Evidence |
| --- | --- | --- |
| `clients/python` | `identical` | 9 operations, both binaries |
| `clients/ruby` | `identical` | 9 operations, both binaries |
| `clients/node` | `identical` | 9 operations, both binaries |
| `clients/php` | `identical` | 9 operations, both binaries |
| `clients/dart` | `identical` | 9 operations, both binaries |
| `clients/go` | `identical` | 9 operations, both binaries |
| `clients/rust` | `identical` | 9 operations, both binaries |
| `clients/java` | `identical` | 9 operations, both binaries |
| `clients/kotlin` | `identical` | 9 operations, both binaries |
| `clients/swift` | `identical` | 9 operations, both binaries |
| `clients/csharp` | `identical` | 9 operations, both binaries |

All eleven binary-driven shims are covered. The compiled ones (rust, java,
kotlin, swift, csharp) are built once during setup, so the compile cost sits
outside the per-run timeout and is not paid twice.

The harness makes **two** checks, and only the second needs `legacy/`:

1. Every shim's output must match `tests/fixtures/shim-expectations.json`, a
   committed recording. Re-record with `cargo test --test shims -- --ignored record` — the
   diff is the review.
2. While `legacy/` exists, each shim also runs against the JavaScript binary and
   the two must agree.

Check 1 is the stronger claim: "both engines agree" can be satisfied by both
being wrong, whereas a committed expectation pins the actual answer. It is also
what lets this gate outlive the oracle.

Three of the eleven — java, kotlin, rust — return the **whole response line**
rather than the parsed `result`, so their comparison covers the entire envelope
including key order, with only `durationMs` masked.

The harness refuses to shrink quietly: a client whose toolchain is absent is
reported as a skip and named in the summary, and CI passes
`--require python,ruby,node,php,go,rust,java,csharp` so a missing toolchain for
any of those is a failure rather than a silent gap. A `--require` name that is
unknown, or that `--only` would exclude, aborts the run instead of asserting
nothing.

**Fixed while building the harness** (pre-existing, unrelated to the rewrite):
`clients/dart/ttid.dart` never cancelled the `StreamIterator` on the child's
stdout, so that subscription kept the Dart event loop alive and a program using
the client — spawned without a TTY — never exited after `main` returned. It
reproduced identically against the JavaScript binary.

`close()` now cancels both pipe subscriptions after awaiting the child's exit.
The cause was isolated by removing each cancel in turn: dropping the stdout one
reproduces the hang, dropping the stderr one does not. Draining stderr is kept
for a separate reason — it forwards diagnostics the way the other clients do,
and stops a chatty child from blocking on a full pipe buffer.

The regression test is that `scripts/compat/shims/driver.dart` deliberately has
**no** `exit()` call, so a reappearance hangs the harness and times out.

## Release

| Behavior | Status | Evidence |
| --- | --- | --- |
| `ttid-{linux,macos,windows}-*` are the Rust binaries | `changed` | `publish.yml` `rust-binaries` matrix |
| `install.sh` / `install.ps1` resolve them unchanged | `identical` | Both build `ttid-${os}-${arch}` and verify `SHA256SUMS` |
| `ttid-js-*` retained for rollback | `rust-only` | `publish.yml` `github-release` job |
| `ttid.wasm` + `ttid-wasm.mjs` attached | `rust-only` | `publish.yml` `rust-wasm` job, gated on the parity probe |
| Release is a draft until every asset is attached | `changed` | Closes the window where an install could fetch a half-populated release |

## Retiring `legacy/` — done

The Rust suite never depended on the oracle at runtime: `tests/oracle.rs`
embeds the corpus with `include_str!`, and `tests/invariants.rs` and
`tests/machine.rs` assert against the kernel directly. Before deletion, the tree
was verified with `legacy/` moved aside; everything below passed without it.

| Gate | Runs without the JavaScript engine |
| --- | --- |
| `cargo test` — 47 tests | yes |
| wasm ABI probe | yes |
| wasm size budget | yes |
| browser parity (headless) | yes |
| CLI surface — 32 cases | yes, against a committed recording |
| client shims — 11 clients | yes, against a committed recording |
| web client suite | yes — folded into `tests/browser.rs`, which runs both web clients |

Two harnesses were retired with it, because a differential needs two things to
differ: `scripts/compat/differential.mjs` (randomized JS-vs-Rust) and
`scripts/compat/generate-corpus.mjs` (regenerated the corpus from the oracle).
The corpus they produced is frozen and still replayed on every run.

## Not yet ported

| Behavior | Status | Blocking |
| --- | --- | --- |
| Wasm module size budget | `identical` | `tests/wasm.rs`: 122.2 KiB raw against a 128 KiB ceiling, 44.6 KiB brotli against 48 KiB. Asserted in CI, with a floor so an empty build cannot pass. |
| `clients/web/ttid.mjs` swap to the wasm kernel | `equivalent` | Resolved: both ship. See the note above. |
