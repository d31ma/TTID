// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// Foundation only. Requires the `ttid` binary on PATH or an explicit path. One
// long-lived subprocess.
//
//   let t = try Ttid()
//   let id = try t.generate() as! String        // new id
//   _ = try t.generate(id)                       // advance it
//   _ = try t.generate(id, delete: true)         // mark deleted
//   let times = try t.decodeTime(id)             // ["createdAt": ..., "updatedAt": ...]
//   let valid = try t.isTTID(id)                 // ["valid": true, "createdAt": ...]
//   let uuid = try t.isUUID("not-a-uuid")        // ["valid": false]
//   t.close()
//
// Each method builds the request and returns the op's `result` (throwing on
// failure). Method names follow Swift's camelCase. request(_:) is the raw escape
// hatch returning the full response dictionary.

import Foundation

enum TtidError: Error { case failure(String) }

final class Ttid {
    private let process = Process()
    private let stdinPipe = Pipe()
    private let stdoutPipe = Pipe()
    private let handle: FileHandle
    private let lock = NSLock()
    private var buffer = Data()

    init(binary: String = "ttid") throws {
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [binary, "exec", "--loop"]
        process.standardInput = stdinPipe
        process.standardOutput = stdoutPipe
        handle = stdoutPipe.fileHandleForReading
        try process.run()
    }

    /// Send one raw machine-protocol op; return the full response dictionary.
    @discardableResult
    func request(_ op: [String: Any]) throws -> [String: Any] {
        lock.lock() // ponytail: one call in flight; drop the lock only if you pipeline
        defer { lock.unlock() }
        let data = try JSONSerialization.data(withJSONObject: op)
        let writer = stdinPipe.fileHandleForWriting
        writer.write(data)
        writer.write(Data([0x0a]))
        let line = try readLine()
        guard let obj = try JSONSerialization.jsonObject(with: line) as? [String: Any] else {
            throw TtidError.failure("ttid returned a non-object response")
        }
        return obj
    }

    private func readLine() throws -> Data {
        while true {
            if let newline = buffer.firstIndex(of: 0x0a) {
                let line = buffer.subdata(in: buffer.startIndex..<newline)
                buffer = buffer.subdata(in: (newline + 1)..<buffer.endIndex)
                return line
            }
            let chunk = handle.availableData
            if chunk.isEmpty { throw TtidError.failure("ttid closed the stream") }
            buffer.append(chunk)
        }
    }

    private func op(_ name: String, _ fields: [String: Any?]) throws -> Any? {
        var payload: [String: Any] = ["op": name]
        for (key, value) in fields { if let value = value { payload[key] = value } }
        let response = try request(payload)
        if response["ok"] as? Bool != true {
            let message = (response["error"] as? [String: Any])?["message"] as? String ?? "ttid error"
            throw TtidError.failure(message)
        }
        return response["result"]
    }

    /// Generate a new TTID, or advance `id` (`delete` to tombstone). Pass nil for a fresh id.
    @discardableResult
    func generate(_ id: String? = nil, delete: Bool = false) throws -> Any? {
        try op("generate", ["id": id, "delete": delete ? true : nil])
    }

    func decodeTime(_ id: String) throws -> Any? { try op("decodeTime", ["id": id]) }
    func isTTID(_ id: String) throws -> Any? { try op("isTTID", ["id": id]) }
    /// Canonical (uppercase) spelling of a valid TTID. Normalize before storing or comparing.
    func canonicalize(_ id: String) throws -> Any? { try op("canonicalize", ["id": id]) }
    func isUUID(_ id: String) throws -> Any? { try op("isUUID", ["id": id]) }

    /// Close stdin so the loop ends, and wait for the process to exit.
    func close() {
        stdinPipe.fileHandleForWriting.closeFile()
        process.waitUntilExit()
    }
}
