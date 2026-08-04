// Runs each client shim — unmodified — against the `ttid` binary and checks
// what it gets back. If a shim needs an edit to work, compatibility broke; the
// shims are the regression test, not the subject.
//
//   bun scripts/run-rust.mjs cargo build
//   bun scripts/compat/shim-differential.mjs [--only python,go]
//   bun scripts/compat/shim-differential.mjs --update   # re-record expectations
//
// Every shim's output must match `test/fixtures/shim-expectations.json`, a
// committed recording produced while the JavaScript engine was still present to
// check it against. Re-record with `--update`; the diff is the review.
//
// A recording is a stronger check than the differential it replaced: "both
// engines agree" can be satisfied by both being wrong, while a recording pins
// the actual answer.

import { spawn } from 'node:child_process'
import { mkdtemp, writeFile, readFile, rm, cp, mkdir } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../../', import.meta.url))
const shims = join(root, 'scripts/compat/shims')
const RUST_BIN = join(root, process.env.TTID_RUST_BIN ?? 'target/debug/ttid')

const only = (() => {
    const index = process.argv.indexOf('--only')
    return index === -1 ? null : new Set(process.argv[index + 1].split(','))
})()

// Clients that MUST run. A missing toolchain for one of these is a failure, not
// a skip — otherwise coverage can quietly shrink to nothing and still be green.
const required = (() => {
    const index = process.argv.indexOf('--require')
    return index === -1 ? new Set() : new Set(process.argv[index + 1].split(','))
})()

const update = process.argv.includes('--update')
const scratch = await mkdtemp(join(tmpdir(), 'ttid-shim-'))

const EXPECTATIONS = join(root, 'test/fixtures/shim-expectations.json')
const expectations = JSON.parse(await readFile(EXPECTATIONS, 'utf8').catch(() => '{}'))



/** Run a build step; throws with its output if it fails. */
function build(command, cwd) {
    return new Promise((resolve, reject) => {
        const child = spawn(command[0], command.slice(1), { cwd, stdio: ['ignore', 'pipe', 'pipe'] })
        let output = ''
        child.stdout.on('data', (chunk) => (output += chunk))
        child.stderr.on('data', (chunk) => (output += chunk))
        child.once('error', reject)
        child.once('close', (code) =>
            code === 0 ? resolve() : reject(new Error(`${command.join(' ')} exited ${code}\n${output}`))
        )
    })
}

/** Lay out a scratch directory holding a client and its driver. */
async function stage(name, files) {
    const dir = join(scratch, name)
    await mkdir(dir, { recursive: true })
    for (const [from, to] of files) await cp(join(root, from), join(dir, to))
    return dir
}

// Interpreted clients run straight from the repo. Compiled ones are built once,
// here, so the build cost is not inside the per-run timeout and is not paid
// twice (once per binary under test).
const CLIENTS = [
    { name: 'python', requires: 'python3', command: ['python3', join(shims, 'driver.py')] },
    { name: 'ruby', requires: 'ruby', command: ['ruby', join(shims, 'driver.rb')] },
    { name: 'node', requires: 'node', command: ['node', join(shims, 'driver.mjs')] },
    { name: 'php', requires: 'php', command: ['php', join(shims, 'driver.php')] },
    { name: 'dart', requires: 'dart', command: ['dart', 'run', join(shims, 'driver.dart')] },
    {
        name: 'go',
        requires: 'go',
        async setup() {
            const dir = await stage('go', [['scripts/compat/shims/driver.go', 'main.go']])
            await mkdir(join(dir, 'ttid'), { recursive: true })
            await cp(join(root, 'clients/go/ttid.go'), join(dir, 'ttid/ttid.go'))
            await writeFile(join(dir, 'go.mod'), 'module ttidshim\n\ngo 1.21\n')
            await build(['go', 'build', '-o', 'driver', '.'], dir)
            return { command: [join(dir, 'driver')], cwd: dir }
        }
    },
    {
        name: 'rust',
        requires: 'rustc',
        async setup() {
            const dir = await stage('rust', [
                ['clients/rust/ttid.rs', 'ttid.rs'],
                ['scripts/compat/shims/main.rs', 'main.rs']
            ])
            await build(['rustc', '-O', '--edition', '2021', 'main.rs', '-o', 'driver'], dir)
            return { command: [join(dir, 'driver')], cwd: dir }
        }
    },
    {
        name: 'java',
        requires: 'javac',
        async setup() {
            const dir = await stage('java', [
                ['clients/java/Ttid.java', 'Ttid.java'],
                ['scripts/compat/shims/Driver.java', 'Driver.java']
            ])
            await build(['javac', '-d', 'out', 'Ttid.java', 'Driver.java'], dir)
            return { command: ['java', '-cp', 'out', 'Driver'], cwd: dir }
        }
    },
    {
        name: 'kotlin',
        requires: 'kotlinc',
        async setup() {
            const dir = await stage('kotlin', [
                ['clients/kotlin/Ttid.kt', 'Ttid.kt'],
                ['scripts/compat/shims/driver.kt', 'driver.kt']
            ])
            await build(['kotlinc', 'Ttid.kt', 'driver.kt', '-include-runtime', '-d', 'driver.jar'], dir)
            return { command: ['java', '-jar', 'driver.jar'], cwd: dir }
        }
    },
    {
        name: 'swift',
        requires: 'swiftc',
        async setup() {
            const dir = await stage('swift', [
                ['clients/swift/Ttid.swift', 'Ttid.swift'],
                ['scripts/compat/shims/main.swift', 'main.swift']
            ])
            await build(['swiftc', '-O', 'Ttid.swift', 'main.swift', '-o', 'driver'], dir)
            return { command: [join(dir, 'driver')], cwd: dir }
        }
    },
    {
        name: 'csharp',
        requires: 'dotnet',
        async setup() {
            const dir = await stage('csharp', [
                ['clients/csharp/Ttid.cs', 'Ttid.cs'],
                ['scripts/compat/shims/Program.cs', 'Program.cs']
            ])
            await writeFile(
                join(dir, 'driver.csproj'),
                `<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>net8.0</TargetFramework>
    <Nullable>enable</Nullable>
    <ImplicitUsings>disable</ImplicitUsings>
    <AssemblyName>driver</AssemblyName>
    <RootNamespace>TtidDriver</RootNamespace>
  </PropertyGroup>
</Project>
`
            )
            await build(['dotnet', 'build', '-c', 'Release', '-o', 'out', '--nologo', '-v', 'q'], dir)
            return { command: ['dotnet', join(dir, 'out/driver.dll')], cwd: dir }
        }
    }
]

const TIMEOUT_MS = Number(process.env.TTID_SHIM_TIMEOUT_MS ?? 90_000)

function run(command, binary, cwd) {
    return new Promise((resolve, reject) => {
        const child = spawn(command[0], command.slice(1), {
            cwd: cwd ?? root,
            env: { ...process.env, TTID_BIN: binary },
            stdio: ['ignore', 'pipe', 'pipe']
        })
        let stdout = ''
        let stderr = ''
        child.stdout.on('data', (chunk) => (stdout += chunk))
        child.stderr.on('data', (chunk) => (stderr += chunk))

        const guard = setTimeout(() => {
            child.kill('SIGKILL')
            resolve({ stdout, stderr: `${stderr}\n[timed out after ${TIMEOUT_MS}ms]`, code: 124 })
        }, TIMEOUT_MS)

        child.once('error', (error) => {
            clearTimeout(guard)
            reject(error)
        })
        // Resolve on `exit`, not `close`: some shims leave the `ttid` grandchild
        // alive briefly, and it inherits these pipes — waiting for them to close
        // would wait on the wrong process. Drain first so nothing is truncated.
        child.once('exit', (code) => {
            setTimeout(() => {
                clearTimeout(guard)
                resolve({ stdout, stderr, code })
            }, 250)
        })
    })
}

/** Erase the parts that two independent runs cannot agree on.
 *
 * Some clients (java, kotlin, rust) hand back the whole response line rather
 * than the parsed `result`, so `durationMs` has to go too. That makes their
 * comparison stricter, not weaker: the entire envelope is checked, key order
 * included. */
const normalize = (text) =>
    text
        .replace(/"durationMs"\s*:\s*\d+/g, '"durationMs":<N>')
        .replace(/\b[0-9A-Z]{11}(-(?:[0-9A-Z]{1,11}))*\b/g, (match) =>
            match
                .split('-')
                .map((segment) => (segment === 'X' ? 'X' : '<SEG>'))
                .join('-')
        )

// A `--require` name that matches no client, or that `--only` filters out, would
// silently assert nothing. Catch it before running anything.
const known = new Set(CLIENTS.map((client) => client.name))
for (const name of required) {
    if (!known.has(name)) {
        console.error(`✗ --require names an unknown client: ${name}`)
        console.error(`  known clients: ${[...known].join(', ')}`)
        process.exit(1)
    }
    if (only && !only.has(name)) {
        console.error(`✗ --require ${name} is excluded by --only; it would assert nothing`)
        process.exit(1)
    }
}

let failures = 0
let ran = 0
const skipped = []

for (const client of CLIENTS) {
    if (only && !only.has(client.name)) continue

    if (client.requires && !Bun.which(client.requires)) {
        if (required.has(client.name)) {
            failures++
            console.error(`✗ ${client.name}: required, but \`${client.requires}\` is not installed`)
        } else {
            skipped.push(`${client.name} (no ${client.requires})`)
            console.log(`- ${client.name.padEnd(7)} skipped: \`${client.requires}\` not installed`)
        }
        continue
    }
    ran++

    let command = client.command
    let cwd = client.cwd
    if (client.setup) {
        try {
            ;({ command, cwd } = await client.setup())
        } catch (error) {
            failures++
            console.error(`✗ ${client.name}: build failed`)
            console.error(String(error.message).split('\n').slice(0, 12).join('\n'))
            continue
        }
    }

    // Sequential, not concurrent: `dart run` and the JVM take a lock on their
    // shared caches, so two copies of the same driver deadlock.
    const rust = await run(command, RUST_BIN, cwd)
    if (rust.code !== 0) {
        failures++
        console.error(`✗ ${client.name}: the shim failed against the ttid binary (exit ${rust.code})`)
        console.error(rust.stderr.trim().split('\n').slice(-8).join('\n'))
        continue
    }
    const rustOut = normalize(rust.stdout)

    if (update) {
        expectations[client.name] = rustOut
        console.log(`· ${client.name.padEnd(7)} recorded`)
        continue
    }

    // 1. Against the committed recording. Survives the retirement of legacy/.
    const expected = expectations[client.name]
    if (expected === undefined) {
        failures++
        console.error(
            `✗ ${client.name}: no recorded expectation. Add one with \`--update\` and review the diff.`
        )
        continue
    }
    if (expected !== rustOut) {
        failures++
        console.error(`✗ ${client.name}: output does not match the recorded expectation`)
        report(expected, rustOut, 'expected', 'actual')
        continue
    }


    const lines = rustOut.trim().split('\n').length
    console.log(`✓ ${client.name.padEnd(7)} ${lines} operations as recorded`)
}

/** Print the first differing lines of two outputs. */
function report(left, right, leftLabel, rightLabel) {
    const a = left.split('\n')
    const b = right.split('\n')
    for (let index = 0; index < Math.max(a.length, b.length); index++) {
        if (a[index] !== b[index]) {
            console.error(`  line ${index + 1}`)
            console.error(`    ${leftLabel.padEnd(10)} ${a[index]}`)
            console.error(`    ${rightLabel.padEnd(10)} ${b[index]}`)
        }
    }
}

if (update) {
    // Only rewrite the clients this run actually covered, so `--only` cannot
    // silently drop the rest.
    await writeFile(EXPECTATIONS, `${JSON.stringify(expectations, null, 2)}\n`)
    console.log(`\nRecorded ${ran} client(s) into test/fixtures/shim-expectations.json`)
}

await rm(scratch, { recursive: true, force: true })

if (skipped.length > 0) {
    // Never let reduced coverage read as full coverage.
    console.log(`\nSkipped ${skipped.length}: ${skipped.join(', ')}`)
}

if (failures > 0) {
    console.error(`\n${failures} of ${ran + failures} shims failed.`)
    process.exit(1)
}
if (!update) {
    console.log(
        `\n${ran} of ${CLIENTS.length} client shims drive the ttid binary with no changes` +
            (skipped.length > 0 ? `, ${skipped.length} skipped for missing toolchains.` : '.')
    )
}
