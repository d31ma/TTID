// Exercises clients/node/ttid.mjs against the binary in TTID_BIN.
import { TTID } from '../../../clients/node/ttid.mjs'

const FIXED = '4SQ1NZT5HC0'
const UPDATED = '4SQ1NZT5HC0-4SQ1NZT5P1S'
const DELETED = '4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK'
const sorted = (v) => (v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).sort()) : v)
const out = []
const t = new TTID({ binary: process.env.TTID_BIN })
out.push(['generate', await t.generate()])
out.push(['update', await t.generate(FIXED)])
out.push(['delete', await t.generate(UPDATED, true)])
out.push(['decode', await t.decodeTime(DELETED)])
out.push(['isTTID', await t.isTTID(FIXED)])
out.push(['isTTID-bad', await t.isTTID('nope')])
out.push(['isUUID', await t.isUUID('3f2504e0-4f89-41d3-9a0c-0305e82c3301')])
out.push(['isUUID-bad', await t.isUUID('nope')])
out.push(['canonical', await t.canonicalize(FIXED.toLowerCase())])
try {
    await t.generate(DELETED)
    out.push(['error', 'NO ERROR RAISED'])
} catch (error) {
    out.push(['error', error.message])
}
await t.close()
for (const [name, value] of out) console.log(`${name}=${JSON.stringify(sorted(value))}`)
