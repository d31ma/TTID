// Runs the kernel in a real browser: the full corpus over the raw ABI, plus the
// ergonomic client from clients/web/ttid-wasm.mjs, plus the uniqueness
// guarantee against a clock the browser deliberately coarsens.
//
//   bun run build:wasm
//   bun scripts/wasm/browser-test.mjs             # serve on :8787, print the URL
//   bun scripts/wasm/browser-test.mjs --headless  # drive Chrome, exit 0/1
//
// A browser is the only place that reproduces the clamped `performance.now()`
// that once cut a 2000-id burst down to 135 unique ids, so this is not
// redundant with the Bun-hosted probe.
//
// No Playwright: the page POSTs its own verdict to /result, so any browser that
// can load a URL is a sufficient driver.

import { readFile, mkdtemp, rm } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { spawn } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const PORT = Number(process.env.PORT ?? 8787)
const root = new URL('../../', import.meta.url)

const routes = {
    '/ttid.wasm': ['target/wasm32-unknown-unknown/release/ttid.wasm', 'application/wasm'],
    '/ttid-wasm.mjs': ['clients/web/ttid-wasm.mjs', 'text/javascript'],
    '/corpus.json': ['test/fixtures/corpus.json', 'application/json'],
    '/ttid.mjs': ['clients/web/ttid.mjs', 'text/javascript']
}

const PAGE = `<!doctype html>
<meta charset="utf-8">
<title>TTID wasm — browser parity</title>
<style>
  body { font: 14px ui-monospace, monospace; margin: 2rem; }
  #summary { font-size: 1.1rem; font-weight: 600; }
  .pass { color: #1a7f37 } .fail { color: #cf222e }
  li { margin: .2rem 0 }
</style>
<h1>TTID wasm — browser parity</h1>
<p id="summary">running…</p>
<ul id="detail"></ul>
<script type="module">
import { load } from './ttid-wasm.mjs'

const detail = document.getElementById('detail')
const summary = document.getElementById('summary')
const failures = []
const note = (text, ok) => {
  const li = document.createElement('li')
  li.textContent = (ok ? '✓ ' : '✗ ') + text
  li.className = ok ? 'pass' : 'fail'
  detail.appendChild(li)
  if (!ok) failures.push(text)
}

try {
  const corpus = await (await fetch('./corpus.json')).json()

  // 1. Raw ABI: replay every machine case with its pinned clock.
  const bytes = await (await fetch('./ttid.wasm')).arrayBuffer()
  const { instance } = await WebAssembly.instantiate(bytes, {})
  const { memory, ttid_allocate, ttid_deallocate, ttid_execute, ttid_reset } = instance.exports
  const encoder = new TextEncoder(), decoder = new TextDecoder()

  const raw = (json, nowMs, stateless = true) => {
    // The corpus pins the stateless contract and feeds timestamps out of order.
    if (stateless) ttid_reset()
    const payload = encoder.encode(json)
    const inPtr = ttid_allocate(payload.length)
    new Uint8Array(memory.buffer, inPtr, payload.length).set(payload)
    const packed = ttid_execute(inPtr, payload.length, nowMs, 0)
    ttid_deallocate(inPtr, payload.length)
    if (packed === 0n) return null
    const ptr = Number(packed >> 32n), len = Number(packed & 0xffffffffn)
    const text = decoder.decode(new Uint8Array(memory.buffer, ptr, len).slice())
    ttid_deallocate(ptr, len)
    return text
  }

  let mismatches = 0
  for (const c of corpus.cases.machine) {
    if (raw(JSON.stringify(c.request), c.nowMs) !== c.response) {
      mismatches++
      note('corpus: ' + c.name, false)
    }
  }
  note(corpus.cases.machine.length + ' corpus responses byte-identical', mismatches === 0)

  // 2. Ergonomic client, against a live browser clock.
  const TTID = await load('./ttid.wasm')
  note('ABI version ' + TTID.abiVersion, TTID.abiVersion === 1)

  const id = TTID.generate()
  note('generate() -> ' + id, /^[A-Z0-9]{11}$/.test(id))

  const updated = TTID.generate(id)
  note('generate(id) -> ' + updated, updated.startsWith(id + '-'))

  const deleted = TTID.generate(updated, true)
  note('generate(id, true) -> ' + deleted, deleted.split('-').length === 3)

  let locked = false
  try { TTID.generate(deleted) } catch (e) { locked = e.message === 'This identifier can no longer be modified' }
  note('a deleted id is immutable', locked)

  const times = TTID.decodeTime(deleted)
  note('decodeTime -> ' + JSON.stringify(times),
       times.createdAt <= times.updatedAt && times.updatedAt <= times.deletedAt)

  note('isTTID returns a Date', TTID.isTTID(id) instanceof Date)
  note('isTTID rejects junk', TTID.isTTID('not-a-ttid') === null)
  note('isUUID accepts a uuid', TTID.isUUID('3f2504e0-4f89-41d3-9a0c-0305e82c3301') === true)
  note('isUUID rejects a ttid', TTID.isUUID(id) === false)

  // 3. Clock resolution. Browsers coarsen performance.now() as a Spectre
  //    mitigation, so a tight loop collides. The question that matters is
  //    whether wasm is WORSE than the incumbent pure-JS shim — it must not be.
  const legacy = await import('./ttid.mjs')
  const burst = (generate) => {
    const seen = new Set()
    for (let i = 0; i < 2000; i++) seen.add(generate())
    return seen
  }
  const wasmBurst = burst(() => TTID.generate())
  const jsBurst = burst(() => legacy.generate())
  note('wasm: ' + wasmBurst.size + '/2000 unique in a tight loop', wasmBurst.size === 2000)
  note('pure JS shim: ' + jsBurst.size + '/2000 unique in a tight loop', jsBurst.size === 2000)

  // 4. Strictly increasing, so lexicographic sort is creation order.
  const wasmList = [...wasmBurst]
  note('ids are strictly increasing', wasmList.every((v, i, a) => i === 0 || a[i - 1] < v))
  note('ids are all 11 characters', wasmList.every((v) => v.length === 11))

  // 5. The clock is the worst case a browser can present; freeze it entirely.
  const frozenBurst = new Set()
  for (let i = 0; i < 2000; i++) {
    frozenBurst.add(JSON.parse(raw(JSON.stringify({ op: 'generate' }), 1754179200000, false)).result)
  }
  note('frozen clock: ' + frozenBurst.size + '/2000 unique', frozenBurst.size === 2000)

  // 6. A burst must not shift the decoded timestamp meaningfully.
  const drift = TTID.decodeTime(wasmList[wasmList.length - 1]).createdAt -
                TTID.decodeTime(wasmList[0]).createdAt
  note('burst drifts the decoded timestamp by ' + drift + 'ms', drift <= 50)

  summary.textContent = failures.length === 0
    ? 'PASS — ' + detail.children.length + ' checks'
    : 'FAIL — ' + failures.length + ' of ' + detail.children.length
  summary.className = failures.length === 0 ? 'pass' : 'fail'
  window.__ttidResult = { ok: failures.length === 0, checks: detail.children.length, failures }
} catch (error) {
  summary.textContent = 'ERROR — ' + error.message
  summary.className = 'fail'
  window.__ttidResult = { ok: false, error: String(error && error.stack || error) }
}
// Report back so a headless driver needs no CDP session.
fetch('/result', { method: 'POST', body: JSON.stringify(window.__ttidResult) }).catch(() => {})
</script>`

let resolveResult
const verdict = new Promise((resolve) => (resolveResult = resolve))

const server = Bun.serve({
    port: PORT,
    async fetch(request) {
        const { pathname } = new URL(request.url)
        if (pathname === '/result' && request.method === 'POST') {
            resolveResult(await request.json().catch(() => ({ ok: false, error: 'unreadable verdict' })))
            return new Response('ok')
        }
        if (pathname === '/' || pathname === '/index.html') {
            return new Response(PAGE, { headers: { 'content-type': 'text/html; charset=utf-8' } })
        }
        const route = routes[pathname]
        if (!route) return new Response('Not found', { status: 404 })
        const [file, type] = route
        try {
            return new Response(await readFile(new URL(file, root)), {
                headers: { 'content-type': type }
            })
        } catch {
            return new Response(`Missing ${file}. Run \`bun run build:wasm\` first.`, { status: 500 })
        }
    }
})

const BROWSERS = [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    'google-chrome',
    'google-chrome-stable',
    'chromium',
    'chromium-browser'
]

function findBrowser() {
    // GitHub runners export this; honouring it beats guessing at paths.
    if (process.env.CHROME_PATH && existsSync(process.env.CHROME_PATH)) {
        return process.env.CHROME_PATH
    }
    for (const candidate of BROWSERS) {
        // `Bun.file()` reports size 0 for a path that does not exist rather
        // than throwing, so an existence check is the only reliable test --
        // `size >= 0` matched every candidate and always picked the first.
        if (candidate.startsWith('/')) {
            if (existsSync(candidate)) return candidate
        } else if (Bun.which(candidate)) {
            return candidate
        }
    }
    return null
}

const url = `http://localhost:${server.port}/`

if (!process.argv.includes('--headless')) {
    console.log(`TTID wasm browser parity harness: ${url}`)
} else {
    const browser = findBrowser()
    if (!browser) {
        console.error(
            'No Chrome or Chromium found. Install one, or drop --headless and open the page yourself.'
        )
        console.error('Looked for: ' + BROWSERS.join(', '))
        process.exit(1)
    }
    console.log(`Driving ${browser}`)

    const profile = await mkdtemp(join(tmpdir(), 'ttid-chrome-'))
    const child = spawn(
        browser,
        [
            '--headless=new',
            '--disable-gpu',
            '--no-sandbox',
            '--no-first-run',
            '--disable-dev-shm-usage',
            `--user-data-dir=${profile}`,
            url
        ],
        { stdio: ['ignore', 'ignore', 'pipe'] }
    )
    let stderr = ''
    child.stderr.on('data', (chunk) => (stderr += chunk))

    const timeout = new Promise((resolve) =>
        setTimeout(() => resolve({ ok: false, error: 'the page never reported a verdict' }), 120_000)
    )
    const result = await Promise.race([verdict, timeout])

    child.kill('SIGKILL')
    await rm(profile, { recursive: true, force: true })

    if (!result.ok) {
        console.error('Browser parity FAILED')
        if (result.error) console.error(`  ${result.error}`)
        for (const failure of result.failures ?? []) console.error(`  ✗ ${failure}`)
        if (stderr.trim()) console.error(stderr.trim().split('\n').slice(-5).join('\n'))
        process.exit(1)
    }
    console.log(`Browser parity: ${result.checks} checks passed in a real browser.`)
    process.exit(0)
}

