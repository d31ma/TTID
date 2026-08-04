// Exercises clients/swift/Ttid.swift against the binary in TTID_BIN.
// Swift requires top-level statements to live in a file named main.swift.
import Foundation

let fixed = "4SQ1NZT5HC0"
let updatedId = "4SQ1NZT5HC0-4SQ1NZT5P1S"
let deletedId = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"

/// Render a result the same way every driver does: compact JSON, keys sorted.
func render(_ value: Any) -> String {
    if let text = value as? String { return "\"\(text)\"" }
    if let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) {
        return String(data: data, encoding: .utf8) ?? "null"
    }
    return "\(value)"
}

let t = try Ttid(binary: ProcessInfo.processInfo.environment["TTID_BIN"]!)
print("generate=" + render(try t.generate()))
print("update=" + render(try t.generate(fixed)))
print("delete=" + render(try t.generate(updatedId, delete: true)))
print("decode=" + render(try t.decodeTime(deletedId)))
print("isTTID=" + render(try t.isTTID(fixed)))
print("isTTID-bad=" + render(try t.isTTID("nope")))
print("isUUID=" + render(try t.isUUID("3f2504e0-4f89-41d3-9a0c-0305e82c3301")))
print("isUUID-bad=" + render(try t.isUUID("nope")))
do {
    _ = try t.generate(deletedId)
    print("error=NO ERROR RAISED")
} catch TtidError.failure(let message) {
    print("error=" + message)
}
t.close()
