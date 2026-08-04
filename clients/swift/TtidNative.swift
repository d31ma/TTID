// TTID native client — a self-contained, dependency-free implementation for
// Swift on any platform, including iOS.
//
// Unlike `Ttid.swift`, this build does NOT drive the `ttid` binary — it spawns
// no subprocess, so it runs on iOS (where launching a separate executable is
// forbidden) as well as macOS and Linux. TTID is pure computation (base-36
// timestamp encoding + validation), reimplemented here with Foundation only.
// IDs are interoperable with every other TTID client.
//
//   let id = try TtidNative.generate()             // new id, e.g. "4VLSK98UX1K"
//   let updated = try TtidNative.generate(id)      // advance it
//   let deleted = try TtidNative.generate(updated, delete: true) // final state
//   try TtidNative.decodeTime(deleted)             // ["createdAt": ..., ...] (ms)
//   TtidNative.isTTID(id)                          // Date if valid, else nil
//   TtidNative.isUUID("not-a-uuid")                // Bool
//
// For a macOS/Linux server or CLI that has the `ttid` binary, use the
// binary-driven client in `Ttid.swift` instead.

import Foundation

enum TtidNativeError: Error, CustomStringConvertible {
    case failure(String)
    var description: String {
        switch self { case .failure(let message): return message }
    }
}

/// Namespace of pure-Swift TTID operations. All methods are static; no state,
/// no process, no I/O.
enum TtidNative {
    private static let precision = 10_000
    private static let placeholder = "X"
    private static let minTimestampMs = 1_577_836_800_000
    private static let maxTimestampMs = 7_258_118_400_000

    private static let ttidRegex = try! NSRegularExpression(
        pattern: "^[A-Z0-9]{11}(-[A-Z0-9]{1,11}){0,2}$", options: [.caseInsensitive])
    private static let uuidRegex = try! NSRegularExpression(
        pattern: "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        options: [.caseInsensitive])

    private static func matches(_ regex: NSRegularExpression, _ s: String) -> Bool {
        regex.firstMatch(in: s, options: [], range: NSRange(s.startIndex..., in: s)) != nil
    }

    /// Current high-resolution timestamp: microseconds * 10 == ms * PRECISION.
    private static func timeNow() -> Int {
        Int(Date().timeIntervalSince1970 * 1_000_000) * 10
    }

    /// Decode the timestamps embedded in a TTID.
    /// Throws if the format is invalid or a segment is out of range.
    static func decodeTime(_ id: String) throws -> [String: Int] {
        guard matches(ttidRegex, id) else { throw TtidNativeError.failure("Invalid Format!") }
        let parts = id.components(separatedBy: "-")

        func toMs(_ code: String) throws -> Int {
            guard let value = Int(code, radix: 36) else {
                throw TtidNativeError.failure("Invalid timestamp encoding")
            }
            let ms = Int((Double(value) / Double(precision)).rounded())
            if ms < minTimestampMs || ms > maxTimestampMs {
                throw TtidNativeError.failure("Invalid timestamp encoding")
            }
            return ms
        }

        var result = ["createdAt": try toMs(parts[0])]
        if parts.count > 1 && parts[1] != placeholder { result["updatedAt"] = try toMs(parts[1]) }
        if parts.count > 2 { result["deletedAt"] = try toMs(parts[2]) }
        return result
    }

    /// Validate a TTID. Returns the creation `Date` if valid, else `nil`.
    static func isTTID(_ id: String) -> Date? {
        if id.isEmpty || id.count > 36 { return nil }
        if !matches(ttidRegex, id) { return nil }
        guard let created = try? decodeTime(id)["createdAt"] else { return nil }
        return Date(timeIntervalSince1970: Double(created) / 1000.0)
    }

    /// The canonical (uppercase) spelling of a valid TTID, or `nil`.
    ///
    /// Identifiers are matched case-insensitively but only ever emitted in
    /// uppercase, so string equality is not identity unless you normalize.
    /// Feed this any accepted spelling and store what it returns.
    static func canonical(_ id: String) -> String? {
        isTTID(id) == nil ? nil : id.uppercased()
    }

    /// Validate a UUID (any version or variant).
    static func isUUID(_ id: String) -> Bool {
        matches(uuidRegex, id)
    }

    /// Generate a new TTID, or advance an existing one through its lifecycle.
    /// Throws if `id` is already deleted (three segments) or is not a valid TTID.
    static func generate(_ id: String? = nil, delete: Bool = false) throws -> String {
        if let id = id, isTTID(id) != nil, id.components(separatedBy: "-").count == 3 {
            throw TtidNativeError.failure("This identifier can no longer be modified")
        }

        let encoded = String(timeNow(), radix: 36, uppercase: true)

        if let id = id, isTTID(id) != nil, delete {
            let parts = id.components(separatedBy: "-")
            let updated = parts.count > 1 ? parts[1] : placeholder
            return "\(parts[0])-\(updated)-\(encoded)".uppercased()
        }

        if let id = id, isTTID(id) != nil {
            let created = id.components(separatedBy: "-")[0]
            return "\(created)-\(encoded)".uppercased()
        }

        if let id = id, isTTID(id) == nil {
            throw TtidNativeError.failure("Invalid TTID!")
        }

        return encoded
    }
}
