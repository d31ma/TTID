// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// Stdlib only (dart:io, dart:convert). Requires the `ttid` binary on PATH or an
// explicit path. One long-lived subprocess.
//
//   final t = await Ttid.open();
//   final id = await t.generate();               // new id
//   final up = await t.generate(id as String);   // advance it
//   await t.generate(up as String, true);        // mark deleted
//   print(await t.decodeTime(up));               // {createdAt: ..., updatedAt: ...}
//   print(await t.isTtid(id));                   // {valid: true, createdAt: ...}
//   print(await t.isUuid('not-a-uuid'));         // {valid: false}
//   await t.close();
//
// Each method builds the request and returns the op's `result` (throwing on
// failure). Method names follow Dart's lowerCamelCase (acronyms as words).
// request(op) is the raw escape hatch returning the full response map.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

class TtidException implements Exception {
  final String message;
  TtidException(this.message);
  @override
  String toString() => 'TtidException: $message';
}

class Ttid {
  final Process _proc;
  final StreamIterator<String> _lines;

  Ttid._(this._proc, this._lines);

  /// Start a warm ttid process. [binary] defaults to "ttid".
  static Future<Ttid> open([String binary = 'ttid']) async {
    final proc = await Process.start(binary, ['exec', '--loop']);
    final lines = StreamIterator(
        proc.stdout.transform(utf8.decoder).transform(const LineSplitter()));
    return Ttid._(proc, lines);
  }

  /// Send one raw machine-protocol op; return the full response map.
  Future<Map<String, dynamic>> request(Map<String, dynamic> op) async {
    _proc.stdin.writeln(jsonEncode(op));
    await _proc.stdin.flush();
    // ponytail: one call in flight; await each call before the next, or guard
    // with a lock if you pipeline.
    if (!await _lines.moveNext()) throw TtidException('ttid closed the stream');
    return jsonDecode(_lines.current) as Map<String, dynamic>;
  }

  Future<dynamic> _op(String op, Map<String, dynamic> fields) async {
    final payload = <String, dynamic>{'op': op};
    fields.forEach((key, value) {
      if (value != null) payload[key] = value;
    });
    final response = await request(payload);
    if (response['ok'] != true) {
      final error = response['error'];
      throw TtidException(
          (error is Map && error['message'] is String) ? error['message'] : 'ttid error');
    }
    return response['result'];
  }

  /// Generate a new TTID, or advance [id] ([delete] to tombstone). Pass null for a fresh id.
  Future<dynamic> generate([String? id, bool delete = false]) =>
      _op('generate', {'id': id, 'delete': delete ? true : null});

  Future<dynamic> decodeTime(String id) => _op('decodeTime', {'id': id});
  Future<dynamic> isTtid(String id) => _op('isTTID', {'id': id});
  Future<dynamic> isUuid(String id) => _op('isUUID', {'id': id});

  /// Close stdin so the loop ends, and wait for the process to exit.
  Future<void> close() async {
    await _proc.stdin.close();
    await _proc.exitCode;
  }
}
