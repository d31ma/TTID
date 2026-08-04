// TTID web client, backed by the compiled Rust kernel.
//
// Same API as ./ttid.mjs, but the logic is the real `ttid` engine rather than a
// hand-written reimplementation — so it cannot drift from the binary. The
// module is freestanding (zero imports), so this runs unchanged in browsers,
// Node, Deno, Bun, Workers, and WASI hosts.
//
//   import { load } from './ttid-wasm.mjs'
//   const TTID = await load('./ttid.wasm')
//
//   const id = TTID.generate()                   // "4VU6SKZIC3U"
//   const updated = TTID.generate(id)
//   const deleted = TTID.generate(updated, true) // final state
//   TTID.decodeTime(deleted)                     // { createdAt, updatedAt, deletedAt }
//   TTID.isTTID(id)                              // Date if valid, else null
//   TTID.isUUID('...')                           // boolean
//
// Loading is async because instantiating WebAssembly is; ./ttid.mjs stays the
// synchronous, zero-download option. Both are gated against the same corpus.

const EXPECTED_ABI_VERSION = 1

/**
 * The module's exports. `WebAssembly.Instance['exports']` is an index signature
 * of `any`, so naming the shape here is what makes the calls below checkable.
 * @typedef {object} WasmExports
 * @property {WebAssembly.Memory} memory
 * @property {() => number} ttid_abi_version
 * @property {() => void} ttid_reset
 * @property {(length: number) => number} ttid_allocate
 * @property {(pointer: number, length: number) => void} ttid_deallocate
 * @property {(pointer: number, length: number, nowMs: number, durationMs: number) => bigint} ttid_execute
 */

/** Current high-resolution time in ms. Sub-millisecond precision is required:
 *  `Date.now()` alone would collide under rapid generation. */
function nowMs() {
    return typeof performance === 'undefined'
        ? Date.now()
        : performance.timeOrigin + performance.now()
}

/**
 * Instantiate the kernel.
 * @param {string | URL | Response | ArrayBuffer | Uint8Array | WebAssembly.Module} source
 *        A URL to fetch, a streaming `Response`, raw bytes, or a compiled module.
 */
export async function load(source = new URL('./ttid.wasm', import.meta.url)) {
    const instance = await instantiate(source)
    const exports = /** @type {WasmExports} */ (
        /** @type {unknown} */ (instance.exports)
    )
    const { memory, ttid_abi_version, ttid_allocate, ttid_deallocate, ttid_execute } = exports

    const abiVersion = ttid_abi_version()
    if (abiVersion !== EXPECTED_ABI_VERSION) {
        throw new Error(
            `TTID wasm ABI mismatch: module reports ${abiVersion}, this client speaks ${EXPECTED_ABI_VERSION}`
        )
    }

    const encoder = new TextEncoder()
    const decoder = new TextDecoder()

    /**
     * Send one machine-protocol request and return the parsed response.
     * @param {Record<string, unknown>} payload
     * @returns {any}
     */
    function request(payload) {
        const bytes = encoder.encode(JSON.stringify(payload))
        const inPointer = ttid_allocate(bytes.length)
        // `memory.buffer` is re-read after every call: allocating can grow the
        // heap and detach any view taken before it.
        new Uint8Array(memory.buffer, inPointer, bytes.length).set(bytes)

        const packed = ttid_execute(inPointer, bytes.length, nowMs(), 0)
        ttid_deallocate(inPointer, bytes.length)

        if (packed === 0n) throw new Error('TTID kernel returned no response')
        const outPointer = Number(packed >> 32n)
        const outLength = Number(packed & 0xffffffffn)
        // Copy before deallocating — the view aliases guest memory.
        const text = decoder.decode(new Uint8Array(memory.buffer, outPointer, outLength).slice())
        ttid_deallocate(outPointer, outLength)

        const response = JSON.parse(text)
        if (!response.ok) throw new Error(response.error.message)
        return response.result
    }

    /** @type {(id?: string, del?: boolean) => string} */
    const generate = (id, del = false) =>
        request({ op: 'generate', ...(id ? { id } : {}), ...(del ? { delete: true } : {}) })

    /** @type {(id: string) => { createdAt: number, updatedAt?: number, deletedAt?: number }} */
    const decodeTime = (id) => request({ op: 'decodeTime', id })

    /** @type {(id: string) => Date | null} */
    const isTTID = (id) => {
        // The kernel reports validity rather than throwing, matching `isTTID`'s
        // null-on-invalid contract.
        if (typeof id !== 'string' || id.length === 0) return null
        const { valid, createdAt } = request({ op: 'isTTID', id })
        return valid ? new Date(createdAt) : null
    }

    /** @type {(id: string) => boolean} */
    const isUUID = (id) => {
        if (typeof id !== 'string' || id.length === 0) return false
        return request({ op: 'isUUID', id }).valid
    }

    return Object.freeze({
        generate,
        decodeTime,
        isTTID,
        isUUID,
        abiVersion,
        /** Escape hatch: send a raw machine-protocol request. */
        request
    })
}

/**
 * @param {string | URL | Response | ArrayBuffer | ArrayBufferView | WebAssembly.Module} source
 * @returns {Promise<WebAssembly.Instance>}
 */
async function instantiate(source) {
    if (source instanceof WebAssembly.Module) {
        return await WebAssembly.instantiate(source, {})
    }
    if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
        // Compile first rather than calling the two-argument bytes overload:
        // `instantiate` is overloaded on its first parameter and resolves to
        // the `Module` form here, which returns a bare Instance. Compiling
        // makes the intent unambiguous and costs nothing.
        const module = await WebAssembly.compile(/** @type {BufferSource} */ (source))
        return await WebAssembly.instantiate(module, {})
    }
    const response = source instanceof Response ? source : await fetch(source)
    if (!response.ok) {
        throw new Error(`Failed to fetch the TTID kernel: ${response.status} ${response.statusText}`)
    }
    // Streaming compilation needs the right Content-Type; fall back when a
    // static host serves .wasm as octet-stream.
    if (typeof WebAssembly.instantiateStreaming === 'function') {
        try {
            return (await WebAssembly.instantiateStreaming(response.clone(), {})).instance
        } catch {
            /* fall through to the buffered path */
        }
    }
    return (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance
}

export default { load }
