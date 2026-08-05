# Exercises clients/python/ttid.py against the binary in TTID_BIN.
import json, os, sys
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "..", "..", "clients", "python"))
from ttid import TTID, TTIDError

FIXED = "4SQ1NZT5HC0"
UPDATED = "4SQ1NZT5HC0-4SQ1NZT5P1S"
DELETED = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"
out = []
with TTID(os.environ["TTID_BIN"]) as t:
    out.append(("generate", t.generate()))
    out.append(("update", t.generate(FIXED)))
    out.append(("delete", t.generate(UPDATED, True)))
    out.append(("decode", t.decode_time(DELETED)))
    out.append(("isTTID", t.is_ttid(FIXED)))
    out.append(("isTTID-bad", t.is_ttid("nope")))
    out.append(("isUUID", t.is_uuid("3f2504e0-4f89-41d3-9a0c-0305e82c3301")))
    out.append(("isUUID-bad", t.is_uuid("nope")))
    out.append(("canonical", t.canonicalize(FIXED.lower())))
    try:
        t.generate(DELETED)
        out.append(("error", "NO ERROR RAISED"))
    except TTIDError as e:
        out.append(("error", str(e)))
for name, value in out:
    print(f"{name}={json.dumps(value, sort_keys=True)}")
