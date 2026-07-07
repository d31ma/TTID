"""TTID client — drives the `ttid` binary's persistent NDJSON loop.

No pip dependencies. Requires the `ttid` binary on PATH or an explicit path.
One long-lived subprocess.

    from ttid import TTID

    with TTID() as t:
        _id = t.generate()                 # new id
        updated = t.generate(_id)          # advance it
        times = t.decode_time(updated)     # {"createdAt": ..., "updatedAt": ...}
        ttid = t.is_ttid(_id)              # {"valid": True, "createdAt": ...}
        uuid = t.is_uuid("...")            # {"valid": False}

Each method builds the request, sends it, and returns the op's `result` (raising
TTIDError on failure). Method names follow Python's snake_case. `request(op)` is
a raw escape hatch returning the full response dict.
"""

import json
import subprocess
import threading


class TTIDError(RuntimeError):
    pass


class TTID:
    def __init__(self, binary="ttid"):
        self._proc = subprocess.Popen(
            [binary, "exec", "--loop"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._lock = threading.Lock()

    def request(self, op):
        """Send one raw machine-protocol op; return the full response dict."""
        line = json.dumps(op)
        with self._lock:  # ponytail: one call in flight; drop the lock only if you pipeline
            if self._proc.poll() is not None:
                raise TTIDError("ttid process has exited")
            self._proc.stdin.write(line + "\n")
            self._proc.stdin.flush()
            reply = self._proc.stdout.readline()
        if not reply:
            raise TTIDError("ttid closed the stream (stderr may have details)")
        return json.loads(reply)

    def _op(self, op, **fields):
        payload = {"op": op}
        for key, value in fields.items():
            if value is not None:
                payload[key] = value
        response = self.request(payload)
        if not response.get("ok"):
            raise TTIDError((response.get("error") or {}).get("message", "ttid error"))
        return response.get("result")

    def generate(self, id=None, delete=False):
        """Generate a new TTID, or advance an existing one (delete=True to tombstone)."""
        return self._op("generate", id=id, delete=delete or None)

    def decode_time(self, id):
        """Decode embedded timestamps → {createdAt, updatedAt?, deletedAt?}."""
        return self._op("decodeTime", id=id)

    def is_ttid(self, id):
        """Validate a TTID → {valid, createdAt}."""
        return self._op("isTTID", id=id)

    def is_uuid(self, id):
        """Validate a UUID → {valid}."""
        return self._op("isUUID", id=id)

    def close(self):
        if self._proc.poll() is None:
            self._proc.stdin.close()
            self._proc.wait()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
