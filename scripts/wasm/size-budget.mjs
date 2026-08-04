// Asserts the wasm module stays within budget.
//
//   bun run build:wasm
//   bun scripts/wasm/size-budget.mjs [--update]
//
// The budget is a ceiling, not a target. It exists so a dependency added for
// convenience cannot quietly triple what a browser downloads. When a change
// legitimately needs more room, raise BUDGET deliberately and say why — that
// edit is the review signal.

import { readFile, stat } from 'node:fs/promises'
import { gzipSync, brotliCompressSync, constants } from 'node:zlib'

const WASM = new URL('../../target/wasm32-unknown-unknown/release/ttid.wasm', import.meta.url)

// 128 KiB raw. The kernel itself is a few KB; serde_json and indexmap are the
// bulk. Hand-rolled serialization would reach roughly 10 KiB if this ever
// becomes a real constraint — see docs/PARITY_LEDGER.md.
const BUDGET = 128 * 1024

// What a browser actually downloads. Every static host worth using serves one
// of these, so it is the honest number to hold a line on.
const BROTLI_BUDGET = 48 * 1024

const bytes = await readFile(WASM).catch(async () => {
    console.error(`Missing ${WASM.pathname}. Run \`bun run build:wasm\` first.`)
    process.exit(1)
})

const gzip = gzipSync(bytes, { level: 9 }).length
const brotli = brotliCompressSync(bytes, {
    params: { [constants.BROTLI_PARAM_QUALITY]: 11 }
}).length

const kib = (value) => `${(value / 1024).toFixed(1)} KiB`
const report = [
    ['raw', bytes.length, BUDGET],
    ['brotli', brotli, BROTLI_BUDGET]
]

let over = false
for (const [label, actual, budget] of report) {
    const headroom = budget - actual
    const status = headroom >= 0 ? '✓' : '✗'
    if (headroom < 0) over = true
    console.log(
        `${status} ${label.padEnd(7)} ${kib(actual).padStart(10)} / ${kib(budget).padStart(10)}` +
            `  (${headroom >= 0 ? '' : '+'}${kib(Math.abs(headroom))} ${headroom >= 0 ? 'headroom' : 'over'})`
    )
}
console.log(`  gzip    ${kib(gzip).padStart(10)}`)

if (over) {
    console.error(
        '\nThe wasm module is over budget. Either shrink it, or raise the budget in' +
            '\nscripts/wasm/size-budget.mjs with a note explaining what earned the space.'
    )
    process.exit(1)
}

// A module that shrinks to nothing is a broken build, not a win.
const FLOOR = 8 * 1024
if (bytes.length < FLOOR) {
    console.error(`\nThe module is only ${kib(bytes.length)} — that is too small to be a real build.`)
    process.exit(1)
}
