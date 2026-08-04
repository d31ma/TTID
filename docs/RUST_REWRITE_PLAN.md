# TTID Rust Rewrite — Project Plan

- Status: **Complete. Rust is the only engine; `legacy/` was deleted 2026-08-04.**
- Started: **2026-08-03**
- Method: compatibility-first strangler, after
  [FYLO ADR 0002](https://github.com/d31ma/Fylo/blob/main/docs/adr/0002-compatibility-first-strangler-migration.md)
  and TACHYON's `PARITY_LEDGER.md`, scaled to TTID's size.

## 1. Why

Today TTID ships five `bun build --compile` executables and hand-written client
shims. Two consequences:

- **No wasm.** Bun's compiler emits OS/arch executables only, so browsers and
  WASI hosts cannot run the real implementation. `clients/web/ttid.mjs` is a
  second, hand-maintained reimplementation that can silently drift.
- **Two sources of truth** for identical logic, verified only by eye.

One Rust source compiled to both a native binary and a `.wasm` removes the
drift by construction.

## 2. The seamless-swap requirement

**Any client shim must work against either artifact without modification, and
without being able to tell which one it is talking to.**

This is the governing constraint. It is satisfied by *one kernel, two
transports*:

```
              ┌─────────────────────────────────┐
              │  kernel: execute(request) -> response │   pure, no I/O, no clock
              └───────────────┬─────────────────┘
                  ┌───────────┴───────────┐
       native bin │                       │ wasm cdylib
    stdin/stdout  │                       │ linear memory
    NDJSON lines  │                       │ same JSON bytes
                  ▼                       ▼
        11 subprocess shims        web / WASI / plugin hosts
```

The kernel is byte-for-byte identical on both sides because it *is* the same
code. Only the transport differs. A shim writes the same request JSON and reads
back the same response JSON either way.

### 2.1 What the kernel may not do

The kernel takes no I/O and reads no clock. Every non-deterministic input is a
parameter supplied by the transport:

| Input | Native binary | Wasm host |
| --- | --- | --- |
| `now` (high-resolution ms, f64) | `SystemTime::now()` as ms | `performance.timeOrigin + performance.now()` |
| `started_at` (ms, for `durationMs`) | `SystemTime::now()` as ms | `Date.now()` |

`now` **must** carry sub-millisecond precision. Today's JS uses
`performance.now() + performance.timeOrigin`; a host that supplies integer
`Date.now()` instead would collide under rapid generation. This is a contract
requirement, not an implementation detail.

### 2.2 Wasm ABI

Raw C ABI over linear memory, **no `wasm-bindgen`** — same choice as
[FYLO's kernel](https://github.com/d31ma/Fylo/blob/main/src/browser/wasm/src/lib.rs),
for the same reasons: no build-tool dependency, no generated glue to keep in
sync, and a module small enough to inline.

```
ttid_abi_version() -> u32
ttid_allocate(length: usize) -> *mut u8
ttid_deallocate(pointer: *mut u8, length: usize)
ttid_execute(pointer: *const u8, length: usize, now_ms: f64, duration_ms: f64) -> u64
        // returns (pointer << 32) | length of a UTF-8 response the host must
        // deallocate; returns 0 for a blank request, matching the native loop
```

Blocks are boxed slices, so capacity always equals length and the host has one
number to remember rather than two. `duration_ms` is `f64` rather than `i64` so
JavaScript hosts need no `BigInt` to call it.

`unsafe` is confined to the copies across the memory boundary. The workspace
sets `unsafe_code = "deny"`; `src/wasm.rs` is the single module that opts back
in, and the kernel never sees a pointer.

## 3. Non-negotiable invariants

Any of these breaking is a release blocker, not a bug report.

1. **Existing IDs stay valid and decode to the same timestamps.** IDs are in
   production databases; the encoding is a wire format.
2. **The machine protocol is frozen at `protocolVersion: 1`.** Same ops, same
   response field names *and order*, same error `name`/`message` strings.
3. **The 11 subprocess shims are not edited.** They are the regression test:
   if any needs a change, compatibility broke.
4. **Exit codes and the `--help` text are part of the CLI contract.**

## 4. The actual hazard: float64 parity

TTID is float-shaped, not integer-shaped, and both directions sit past
`Number.MAX_SAFE_INTEGER`:

```js
// encode
time = (performance.now() + performance.timeOrigin) * 10_000   // ≈ 1.77e16 > 2^53
time.toString(36)

// decode
Number((parseInt(code, 36) / 10_000).toFixed(0))               // parseInt up to 1.3e17
```

Rust must reproduce this exactly or it mints IDs that decode differently from
every ID already issued. Specifically:

- `parseInt(s, 36)` computes exactly then rounds once to the nearest double.
  Accumulating `acc = acc * 36.0 + d` in f64 rounds at every step and gives
  different answers. Parse to `u128` exactly, then a single `as f64`.
- `toFixed(0)` rounds half away from zero. `f64::round()` also does — but this
  is asserted by the corpus, not assumed.
- `Number.prototype.toString(36)` on a non-integral f64 emits a *fractional*
  base-36 string. Unreachable in range (any timestamp above the 2020-01-01
  floor scales past 2^53, where the f64 ulp is 2, so the value is always
  integral) but recorded in the parity ledger rather than ignored.

This is what the golden corpus exists to prove.

## 5. Layout

One package, not a workspace. TTID is ~90 lines of logic; FYLO's seven crates
would be theatre.

```
Cargo.toml            [lib] crate-type = ["rlib", "cdylib"]
rust-toolchain.toml   pinned 1.97.1 + wasm32-unknown-unknown  (copied from FYLO)
rustfmt.toml
deny.toml
src/ttid.rs           generate / decode_time / is_ttid / is_uuid + Generator
src/machine.rs        execute(request_json, now, duration) -> response_json
src/lib.rs
src/main.rs           native transport: argv + stdin/stdout NDJSON loop
src/wasm.rs           #[cfg(target_arch = "wasm32")] C ABI transport
tests/oracle.rs       replays the golden corpus, and the uniqueness guarantee
tests/invariants.rs   properties over arbitrary input, not just recorded cases
tests/machine.rs      the protocol, asserted field by field
```

Dependencies: `serde`, `serde_json`. No `regex` — the two patterns are simple
enough to hand-check in ~30 lines, and the crate would dominate the wasm
module. Lints, `deny.toml`, and the release profile (`lto`, `codegen-units=1`,
`panic="abort"`, `strip`) are lifted from FYLO.

`scripts/run-rust.mjs` (also from FYLO) pins every invocation to the toolchain
in `rust-toolchain.toml` — necessary here because Homebrew's `rustc` shadows
rustup on this machine.

## 6. Phases

The JavaScript implementation stayed the production default and the behavioral
oracle through Phase 5; nothing reached users before then. Phase 6 handed the
`ttid` name to Rust and kept JavaScript as the oracle and the rollback path.

### Phase 0 — Foundation ✅
Cargo skeleton, pinned toolchain, lint and dependency policy, `run-rust.mjs`,
CI job running fmt + clippy + test. No behavior.

### Phase 1 — Oracle and golden corpus ✅
Freeze today's JS as the oracle. `scripts/compat/generate-corpus.mjs` emits
`test/fixtures/corpus.json`, committed, covering:

- generate: new, update, delete, delete-without-update (`X` placeholder),
  already-deleted rejection, invalid-input rejection — each with its exact
  input `now`, so the result is reproducible rather than time-dependent;
- decodeTime: all three arities, both timestamp bounds, every error path;
- isTTID / isUUID: valid and invalid, including the `length > 36` short-circuit
  and mixed-case input;
- machine protocol: one request/response pair per op plus every error shape,
  with `durationMs` masked.

Gate: the corpus replays green against the current JS.

### Phase 2 — Rust kernel ✅
`src/ttid.rs` + `src/machine.rs`. Gates, both passing:

- `tests/oracle.rs` replays all 79 corpus cases; the 30 machine cases are
  compared as whole response strings, so key order is covered.
- `scripts/compat/differential.mjs` runs randomized requests through the oracle
  and the Rust kernel and requires identical bytes. 220,000 requests across five
  seeds agree; CI runs 100,000 per pull request.

The first real catch: `serde_json::to_value` sorts keys, which broke byte
identity on the `isTTID` result. Fixed with the `preserve_order` feature — the
same reason FYLO enables it.

### Phase 3 — Wasm kernel ✅ (size budget outstanding)
`src/wasm.rs` implements the ABI above. `scripts/wasm/abi-probe.mjs`
instantiates the module and replays the corpus through it: all 32 protocol
cases return the same bytes as the oracle, and the differential harness drives
its Rust side through this same module.

The module has **zero imports** and needs no host glue, so it runs unchanged in
browsers, Node, Deno, Bun, Workers, and WASI hosts.
`scripts/wasm/browser-test.mjs` serves a page that runs it in a real browser —
16 checks pass, including the full corpus over the raw ABI.
`clients/web/ttid-wasm.mjs` is the ergonomic loader.

**Size budget.** `scripts/wasm/size-budget.mjs` holds the module at 128 KiB raw
and 48 KiB brotli — 122.2 and 44.6 today. Brotli is the number that matters,
since that is what a browser downloads. `serde_json` plus `indexmap` are the
bulk; hand-rolled serialization of four ops and six fields would reach roughly
10 KiB if the ceiling ever binds. There is a floor too, so a broken build that
emits almost nothing cannot pass.

**Headless in CI.** `--headless` drives the Chrome already on the runner image.
No Playwright: the page POSTs its verdict to the harness, so the driver only has
to open a URL. Both this and the size budget were checked against a deliberately
broken input to confirm they fail when they should.

### Phase 4 — Native CLI ✅
`src/main.rs` — argument grammar, help text, exit codes, `exec --request` in all
three forms, and the `exec --loop` NDJSON transport. Gates, both passing:

- `scripts/compat/cli-differential.mjs` drives the JavaScript CLI and the Rust
  binary over 32 cases and requires identical stdout and exit codes. Only
  `durationMs` and freshly minted timestamps are normalized.
- `scripts/compat/shim-differential.mjs` runs six client shims **unmodified**
  against both binaries: python, ruby, node, php, dart, go. All identical.

One divergence had to be closed by hand: Bun renders file errors as
`ENOENT: no such file or directory, open '<path>'` where Rust says
`No such file or directory (os error 2)`. `errno_message` maps the kinds a
`--request @path` realistically hits; the rest is in the ledger.

All eleven binary-driven shims are covered. The five compiled ones (rust, java,
kotlin, swift, csharp) are built once during setup, outside the per-run timeout.
A client whose toolchain is missing is reported as a skip and named in the
summary; CI marks the eight that the runner image guarantees as required, so
coverage cannot shrink silently.

### Phase 5 — Candidate and shadow ✅
`publish.yml` gains three jobs that attach the candidate to every release
**alongside** the JavaScript binaries, never in place of them. `install.sh` is
untouched, so nothing a user runs today changes.

| Artifact | Built on | Target |
| --- | --- | --- |
| `ttid-rust-linux-x64` | ubuntu-latest | `x86_64-unknown-linux-musl` |
| `ttid-rust-linux-arm64` | ubuntu-24.04-arm | `aarch64-unknown-linux-musl` |
| `ttid-rust-macos-arm64` | macos-latest | `aarch64-apple-darwin` |
| `ttid-rust-macos-x64` | macos-latest | `x86_64-apple-darwin` |
| `ttid-rust-windows-x64.exe` | windows-latest | `x86_64-pc-windows-msvc` |
| `ttid.wasm` + `ttid-wasm.mjs` | ubuntu-latest | `wasm32-unknown-unknown` |

Plus `SHA256SUMS-rust`, kept separate from the existing `SHA256SUMS` so the two
release lines never collide.

musl for Linux, so the binaries are fully static and carry no glibc floor. Each
slice is smoke-tested on its runner where the architecture allows; the
cross-built macOS x64 slice says so rather than pretending.

**Size.** 419 KB, against 61 MB for the `bun build --compile` binary — 146×
smaller, because Bun's compiler embeds the whole runtime.

### Phase 6 — Cutover ✅

Rust is the shipped engine.

**Releases.** The Rust matrix now produces `ttid-linux-x64`,
`ttid-linux-arm64`, `ttid-macos-arm64`, `ttid-macos-x64`, and
`ttid-windows-x64.exe` — the exact names `install.sh` and `install.ps1` already
resolve — plus `ttid.wasm` and `ttid-wasm.mjs`, and the `SHA256SUMS` the
installers verify against. Neither installer needed a change.

The JavaScript build still runs and still ships, as `ttid-js-*` with its own
`SHA256SUMS-js`. That is the rollback path: reverting a release needs no
rebuild.

**A window that had to be closed.** The release was previously created public
and populated afterwards, so between `gh release create` and the last upload a
user running `install.sh` would find the binary or the checksums missing. It is
now created `--draft` and published with `gh release edit --draft=false` only
after every asset is attached.

**Source layout.** The JavaScript engine has been deleted. `src/` holds Rust and
nothing else; the only JavaScript left in the repo is the shipped web client,
the client shims, and the verification harnesses.

Its three jobs were handed off before it went:

- *behavioral oracle* → `test/fixtures/corpus.json`, frozen and replayed by
  `tests/oracle.rs` via `include_str!`;
- *differential counterparty* → committed recordings in
  `test/fixtures/cli-expectations.json` and `shim-expectations.json`, produced
  while it was still there to check them against;
- *test suite* → `tests/invariants.rs` and `tests/machine.rs`. Every test is
  accounted for in `docs/TEST_MIGRATION.md`.

The rollback line went with it: releases no longer carry `ttid-js-*`. Nothing
was lost operationally, because no release had shipped them.

**The web client keeps both implementations, deliberately.**
`clients/web/ttid.mjs` stays the default: 4 KB, synchronous, no fetch.
`clients/web/ttid-wasm.mjs` + `ttid.wasm` is 122 KB and asynchronous, and buys
a browser nothing it does not already have. What made the hand-written shim
risky was drift, and that risk is now gone by other means — both are gated
against the same corpus and the same uniqueness tests, in a real browser, on
every change. Recorded in the ledger rather than resolved by deleting one.

Reach for the wasm client when the JavaScript one cannot serve: a WASI runtime,
a sandboxed plugin host, or a caller that wants the identical compiled engine
the binary runs.

## 7. What is deliberately not copied from FYLO

FYLO's ceremony exists because it writes bytes to disk that must survive
forever, across crash and rollback. TTID computes a string. Skipped: the crash
matrix, the 72-hour soak, shard-layout downgrade docs, `xtask`, the
multi-crate boundary, and the storage ADR set. Add any of them the day TTID
grows persistent state.
