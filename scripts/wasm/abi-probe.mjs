// Proves the seamless-swap claim: drive the wasm module over its C ABI with
// the same corpus the native kernel replays, and require identical response
// bytes. If this passes, no client shim can tell the two artifacts apart.
//
//   bun scripts/run-rust.mjs cargo build --lib --release --target wasm32-unknown-unknown
//   bun scripts/wasm/abi-probe.mjs

import { readFile } from 'node:fs/promises'

const WASM = new URL('../../target/wasm32-unknown-unknown/release/ttid.wasm', import.meta.url)
const CORPUS = new URL('../../test/fixtures/corpus.json', import.meta.url)
const EXPECTED_ABI_VERSION = 1

const [bytes, corpus] = await Promise.all([
    readFile(WASM).catch(() => {
        console.error(`Missing ${WASM.pathname}. Build it first:`)
        console.error('  bun scripts/run-rust.mjs cargo build --lib --release --target wasm32-unknown-unknown')
        process.exit(1)
    }),
    readFile(CORPUS, 'utf8').then(JSON.parse)
])

const { instance } = await WebAssembly.instantiate(bytes, {})
const { memory, ttid_abi_version, ttid_allocate, ttid_deallocate, ttid_execute, ttid_reset } =
    instance.exports

const abiVersion = ttid_abi_version()
if (abiVersion !== EXPECTED_ABI_VERSION) {
    console.error(`ABI version mismatch: module reports ${abiVersion}, host expects ${EXPECTED_ABI_VERSION}`)
    process.exit(1)
}

const encoder = new TextEncoder()
const decoder = new TextDecoder()

/** Run one request through the module and return the response string, or null. */
function execute(requestJson, nowMs, durationMs, stateless = true) {
    // The corpus pins the stateless contract and feeds timestamps out of order.
    if (stateless) ttid_reset()
    const payload = encoder.encode(requestJson)
    const inPointer = ttid_allocate(payload.length)
    new Uint8Array(memory.buffer, inPointer, payload.length).set(payload)

    const packed = ttid_execute(inPointer, payload.length, nowMs, durationMs)
    ttid_deallocate(inPointer, payload.length)

    if (packed === 0n) return null
    const outPointer = Number(packed >> 32n)
    const outLength = Number(packed & 0xffffffffn)
    // Copy before deallocating: the view aliases guest memory.
    const response = decoder.decode(new Uint8Array(memory.buffer, outPointer, outLength).slice())
    ttid_deallocate(outPointer, outLength)
    return response
}

let failures = 0

for (const testCase of corpus.cases.machine) {
    const actual = execute(JSON.stringify(testCase.request), testCase.nowMs, 0)
    if (actual !== testCase.response) {
        failures++
        console.error(`✗ ${testCase.name}`)
        console.error(`  expected ${testCase.response}`)
        console.error(`  actual   ${actual}`)
    }
}

// The transport-level behaviors the corpus cannot express.
if (execute('   ', 0, 0) !== null) {
    failures++
    console.error('✗ a blank request should produce no response')
}
const malformed = execute('{not json', 0, 0)
if (!malformed?.includes('Invalid JSON request')) {
    failures++
    console.error(`✗ a malformed request should report Invalid JSON request, got ${malformed}`)
}

// The monotonic guarantee, over the ABI, with the clock frozen — the worst
// case a coarse browser clock can present.
const BURST = 20_000
const frozen = 1_754_179_200_000
ttid_reset()
const burst = []
for (let index = 0; index < BURST; index++) {
    const response = execute(JSON.stringify({ op: 'generate' }), frozen, 0, false)
    burst.push(JSON.parse(response).result)
}
if (new Set(burst).size !== BURST) {
    failures++
    console.error(`✗ frozen-clock burst: only ${new Set(burst).size}/${BURST} unique`)
}
if (!burst.every((id, index) => index === 0 || burst[index - 1] < id)) {
    failures++
    console.error('✗ frozen-clock burst is not strictly increasing')
}
if (!burst.every((id) => id.length === 11)) {
    failures++
    console.error('✗ frozen-clock burst produced ids that are not 11 characters')
}

const kilobytes = (bytes.length / 1024).toFixed(1)
if (failures > 0) {
    console.error(`\n${failures} wasm/native divergence(s).`)
    process.exit(1)
}
console.log(
    `wasm ABI v${abiVersion}: ${corpus.cases.machine.length + 2} cases identical to the oracle, ` +
        `${BURST} unique ids on a frozen clock (${kilobytes} KiB module).`
)
