# TTID language clients

Thin, dependency-free shims that let an app in any of these languages use TTID
by driving the compiled `ttid` binary. No npm, no native addon — drop in one
file for your language and call TTID like a library.

| Language      | File                   | Runtime deps    |
| ------------- | ---------------------- | --------------- |
| Python        | `python/ttid.py`       | none (stdlib)   |
| Ruby          | `ruby/ttid.rb`         | none (stdlib)   |
| Node/TS       | `node/ttid.mjs`        | none (stdlib)   |
| PHP           | `php/ttid.php`         | none (ext-json) |
| Go            | `go/ttid.go`           | none (stdlib)   |
| Rust          | `rust/ttid.rs`         | none (std)      |
| C#            | `csharp/Ttid.cs`       | none (BCL)      |
| Java          | `java/Ttid.java`       | none (JDK)      |

## Install the binary

Build it from this repo (`bun run build:exe` → `dist-bin/ttid`) or grab a build
from the [GitHub releases](https://github.com/d31ma/TTID/releases). Put `ttid`
on your PATH, then verify: `ttid --help`. Each shim also accepts an explicit
binary path if you don't want it on PATH.

## The API

Each shim exposes one method per operation. Method names follow **each
language's own paradigm** — `snake_case` in Python/Ruby/Rust, `camelCase` in
Node/PHP/Java, `PascalCase` in Go/C#:

| Op         | Python / Ruby / Rust | Node / PHP / Java | Go / C#      |
| ---------- | -------------------- | ----------------- | ------------ |
| generate   | `generate`           | `generate`        | `Generate`   |
| decodeTime | `decode_time`        | `decodeTime`      | `DecodeTime` |
| isTTID     | `is_ttid`            | `isTTID`          | `IsTTID`     |
| isUUID     | `is_uuid`            | `isUUID`          | `IsUUID`     |

- **`generate(id?, delete?)`** — no args mints a new TTID; passing an existing
  `id` advances it (a second segment); `id` + `delete` tombstones it (a third
  segment). Returns the TTID string.
- **`decodeTime(id)`** — `{ createdAt, updatedAt?, deletedAt? }` (ms since epoch).
- **`isTTID(id)`** — `{ valid, createdAt }` (`createdAt` is `null` when invalid).
- **`isUUID(id)`** — `{ valid }`.

For anything else, use the raw `request(op)` escape hatch — see `ttid --help`
and `src/cli/machine.js`.

## How it works

Each shim spawns **one** long-lived process — `ttid exec --loop` — and talks to
it over stdin/stdout as newline-delimited JSON: one request object per line, one
response object per line, in order. No port, no network, no auth surface; the
child dies with your app.

## Concurrency

The shims send one request at a time and read one response (guarded by a lock
where the language needs it). The protocol carries a `requestId` echoed back in
each response, so if you need pipelining you can send many requests and match
replies by id — but one-in-flight is enough for most apps.

## Example (Python)

```python
from ttid import TTID

with TTID() as t:
    _id = t.generate()                 # "4VLMXG1M1JY"
    updated = t.generate(_id)          # "4VLMXG1M1JY-4VLMXG3P2AB"
    deleted = t.generate(updated, delete=True)
    print(t.decode_time(deleted))      # {"createdAt": ..., "updatedAt": ..., "deletedAt": ...}
    print(t.is_ttid(_id))              # {"valid": True, "createdAt": ...}
```

Construct, call the operation methods, close when done (or use a
`with`/`using`/`try`-with-resources block). Each file's header comment has a
runnable example in that language.
