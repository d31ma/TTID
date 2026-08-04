// Drives the whole `ttid` command surface and checks stdout and exit codes.
//
//   bun scripts/run-rust.mjs cargo build
//   bun scripts/compat/cli-differential.mjs
//   bun scripts/compat/cli-differential.mjs --update   # re-record expectations
//
// Every case must match `test/fixtures/cli-expectations.json`, a committed
// recording produced when the JavaScript engine was still present to check it
// against. Re-record with `--update`; the diff is the review.
//
// Values that legitimately vary between processes — durationMs and the
// timestamps inside freshly minted ids — are normalized. Everything else must
// match character for character.

import { spawn } from 'node:child_process'
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const RUST_BIN = process.env.TTID_BIN ?? 'target/debug/ttid'
const update = process.argv.includes('--update')
const EXPECTATIONS = 'test/fixtures/cli-expectations.json'
const expectations = JSON.parse(await readFile(EXPECTATIONS, 'utf8').catch(() => '{}'))

const scratch = await mkdtemp(join(tmpdir(), 'ttid-cli-'))
const requestFile = join(scratch, 'request.json')
await writeFile(requestFile, JSON.stringify({ op: 'generate', requestId: 'from-file' }))

// A stable id to exercise the update/delete/decode paths deterministically.
const FIXED = '4SQ1NZT5HC0'
const FIXED_UPDATED = '4SQ1NZT5HC0-4SQ1NZT5P1S'
const FIXED_DELETED = '4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK'

const CASES = [
    { name: 'no arguments prints help and exits 1', argv: [] },
    { name: '--help', argv: ['--help'] },
    { name: '-h', argv: ['-h'] },
    { name: 'generate', argv: ['generate'] },
    { name: 'generate from an existing id', argv: ['generate', FIXED] },
    { name: 'generate --delete', argv: ['generate', FIXED, '--delete'] },
    { name: 'generate --delete from an updated id', argv: ['generate', FIXED_UPDATED, '--delete'] },
    { name: 'generate rejects a deleted id', argv: ['generate', FIXED_DELETED] },
    { name: 'generate --delete without an id', argv: ['generate', '--delete'] },
    { name: 'generate rejects junk', argv: ['generate', 'not-a-ttid'] },
    { name: 'decode', argv: ['decode', FIXED_DELETED] },
    { name: 'decode one segment', argv: ['decode', FIXED] },
    { name: 'decode rejects junk', argv: ['decode', 'nope'] },
    { name: 'decode without an id', argv: ['decode'] },
    { name: 'validate a good id', argv: ['validate', FIXED] },
    { name: 'validate junk', argv: ['validate', 'nope'] },
    { name: 'validate without an id', argv: ['validate'] },
    { name: 'uuid accepts a uuid', argv: ['uuid', '3f2504e0-4f89-41d3-9a0c-0305e82c3301'] },
    { name: 'uuid rejects a ttid', argv: ['uuid', FIXED] },
    { name: 'uuid without an id', argv: ['uuid'] },
    { name: 'an unknown command', argv: ['nope'] },
    { name: 'an unknown command with an argument', argv: ['nope', 'arg'] },
    { name: 'exec without --request', argv: ['exec'] },
    { name: '--request without a value', argv: ['exec', '--request'] },
    {
        name: 'exec with a literal payload',
        argv: ['exec', '--request', JSON.stringify({ op: 'isTTID', id: FIXED })]
    },
    {
        name: 'exec with a requestId',
        argv: ['exec', '--request', JSON.stringify({ op: 'isUUID', id: 'nope', requestId: 'r1' })]
    },
    { name: 'exec from @file', argv: ['exec', '--request', `@${requestFile}`] },
    { name: 'exec from a missing @file', argv: ['exec', '--request', '@/nonexistent/nope.json'] },
    {
        name: 'exec rejects an unknown op',
        argv: ['exec', '--request', JSON.stringify({ op: 'nope' })]
    },
    {
        name: 'exec rejects a non-object payload',
        argv: ['exec', '--request', '"just a string"']
    },
    {
        name: 'exec from stdin',
        argv: ['exec', '--request', '-'],
        stdin: JSON.stringify({ op: 'isTTID', id: FIXED })
    },
    {
        name: 'exec --loop over several requests',
        argv: ['exec', '--loop'],
        stdin: [
            JSON.stringify({ op: 'generate' }),
            '',
            '   ',
            JSON.stringify({ op: 'decodeTime', id: FIXED_DELETED }),
            JSON.stringify({ op: 'isTTID', id: 'nope', requestId: 'loop-1' }),
            '{not json',
            JSON.stringify({ op: 'nope' }),
            JSON.stringify({ op: 'isUUID', id: '3f2504e0-4f89-41d3-9a0c-0305e82c3301' })
        ].join('\n') + '\n'
    }
]

function run(command, argv, stdin) {
    return new Promise((resolve, reject) => {
        const child = spawn(command[0], [...command.slice(1), ...argv], {
            stdio: ['pipe', 'pipe', 'pipe']
        })
        let stdout = ''
        let stderr = ''
        child.stdout.on('data', (chunk) => (stdout += chunk))
        child.stderr.on('data', (chunk) => (stderr += chunk))
        child.once('error', reject)
        child.once('close', (code) => resolve({ stdout, stderr, code }))
        if (stdin !== undefined) child.stdin.write(stdin)
        child.stdin.end()
    })
}

/** Erase what two independent processes cannot agree on. */
function normalize(text) {
    return (
        text
            // durationMs is wall-clock between two reads.
            .replace(/"durationMs":\s*\d+/g, '"durationMs": <N>')
            // Freshly minted ids carry the moment they were minted.
            .replace(/\b[0-9A-Z]{11}(-(?:[0-9A-Z]{1,11}))*\b/g, (match) =>
                match
                    .split('-')
                    .map((segment) => (segment === 'X' ? 'X' : '<SEG>'))
                    .join('-')
            )
            // decodeTime on a fresh id echoes those same moments back.
            .replace(/"(created|updated|deleted)At":\s*\d+/g, '"$1At": <TS>')
    )
}

let failures = 0

for (const testCase of CASES) {
    const rust = await run([RUST_BIN], testCase.argv, testCase.stdin)
    const actual = { stdout: normalize(rust.stdout), code: rust.code }

    if (update) {
        expectations[testCase.name] = actual
        continue
    }

    // 1. Against the committed recording.
    const expected = expectations[testCase.name]
    if (!expected) {
        failures++
        console.error(`✗ ${testCase.name}: no recorded expectation — add one with \`--update\``)
        continue
    }
    if (expected.code !== actual.code || expected.stdout !== actual.stdout) {
        failures++
        console.error(`✗ ${testCase.name}: does not match the recording`)
        if (expected.code !== actual.code) {
            console.error(`  exit code: expected ${expected.code}, got ${actual.code}`)
        }
        if (expected.stdout !== actual.stdout) {
            console.error(`  --- expected ---\n${expected.stdout}`)
            console.error(`  --- actual ---\n${actual.stdout}`)
        }
        if (rust.stderr.trim()) console.error(`  stderr: ${rust.stderr.trim()}`)
        continue
    }

}

// The `exec --loop` transport is what every client shim drives, and it runs far
// faster than the clock's 200ns resolution. Both binaries must return unique
// ids for a burst down one warm process.
const BURST = 5000
const burstInput = `${JSON.stringify({ op: 'generate' })}\n`.repeat(BURST)

for (const [label, command] of [['ttid', [RUST_BIN]]]) {
    const { stdout } = await run(command, ['exec', '--loop'], burstInput)
    const ids = stdout
        .trim()
        .split('\n')
        .map((line) => JSON.parse(line).result)
    const unique = new Set(ids)
    const increasing = ids.every((id, index) => index === 0 || ids[index - 1] < id)

    if (ids.length !== BURST || unique.size !== BURST || !increasing) {
        failures++
        console.error(
            `✗ ${label} exec --loop burst: ${unique.size}/${ids.length} unique of ${BURST}` +
                (increasing ? '' : ', not strictly increasing')
        )
    } else {
        console.log(`✓ ${label.padEnd(10)} exec --loop: ${BURST}/${BURST} unique, strictly increasing`)
    }
}

if (update) {
    await writeFile(EXPECTATIONS, `${JSON.stringify(expectations, null, 2)}\n`)
    console.log(`Recorded ${CASES.length} CLI cases into ${EXPECTATIONS}`)
}

await rm(scratch, { recursive: true, force: true })

if (failures > 0) {
    console.error(`\n${failures} of ${CASES.length} CLI cases failed.`)
    process.exit(1)
}
if (!update) {
    console.log(
        `${CASES.length} CLI cases match the recording.`
    )
}
