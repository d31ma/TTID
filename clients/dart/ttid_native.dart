// TTID native client — a self-contained, dependency-free implementation for
// Dart and Flutter (any platform: mobile, web, desktop, server).
//
// Unlike `ttid.dart`, this build does NOT drive the `ttid` binary — it needs no
// `dart:io` and no subprocess, so it runs where the binary can't: Flutter on
// iOS/Android (sandboxed) and Flutter web (compiled to JS). TTID is pure
// computation (base-36 timestamp encoding + validation), reimplemented here in
// Dart core only. IDs are interoperable with every other TTID client.
//
//   import 'ttid_native.dart';
//
//   final id = Ttid.generate();              // new id, e.g. "4VLSK98UX1K"
//   final updated = Ttid.generate(id);       // advance it
//   final deleted = Ttid.generate(updated, true); // mark deleted (final state)
//   Ttid.decodeTime(deleted);                // { createdAt, updatedAt, deletedAt } (ms)
//   Ttid.isTtid(id);                         // DateTime if valid, else null
//   Ttid.isUuid('not-a-uuid');               // RegExpMatch if valid, else null
//
// For a server/desktop app that has the `ttid` binary on PATH and wants to share
// the compiled core, use the binary-driven client in `ttid.dart` instead.

/// Namespace of pure-Dart TTID operations. All methods are static; no state,
/// no process, no I/O.
class Ttid {
  Ttid._();

  static const int _precision = 10000;
  static const String _placeholder = 'X';
  static const int _minTimestampMs = 1577836800000; // 2020-01-01T00:00:00.000Z
  static const int _maxTimestampMs = 7258118400000; // 2200-01-01T00:00:00.000Z

  static final RegExp _ttidPattern =
      RegExp(r'^[A-Z0-9]{11}(-[A-Z0-9]{1,11}){0,2}$', caseSensitive: false);
  static final RegExp _uuidPattern = RegExp(
      r'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$',
      caseSensitive: false);

  /// Current high-resolution timestamp, scaled to preserve sub-ms precision.
  /// `microsecondsSinceEpoch * 10` == `msSinceEpoch * PRECISION`.
  static int _timeNow() => DateTime.now().microsecondsSinceEpoch * 10;

  /// Decode the timestamps embedded in a TTID.
  /// Throws [FormatException] if the format is invalid or a segment is out of range.
  static Map<String, int> decodeTime(String id) {
    if (!_ttidPattern.hasMatch(id)) throw const FormatException('Invalid Format!');

    final parts = id.split('-');
    final updated = parts.length > 1 ? parts[1] : null;
    final deleted = parts.length > 2 ? parts[2] : null;

    int toMs(String code) {
      final ms = (int.parse(code, radix: 36) / _precision).round();
      if (ms < _minTimestampMs || ms > _maxTimestampMs) {
        throw const FormatException('Invalid timestamp encoding');
      }
      return ms;
    }

    final result = <String, int>{'createdAt': toMs(parts[0])};
    if (updated != null && updated != _placeholder) result['updatedAt'] = toMs(updated);
    if (deleted != null) result['deletedAt'] = toMs(deleted);
    return result;
  }

  /// Validate a TTID. Returns the creation [DateTime] if valid, else `null`.
  static DateTime? isTtid(String id) {
    if (id.isEmpty || id.length > 36) return null;
    if (!_ttidPattern.hasMatch(id)) return null;
    try {
      return DateTime.fromMillisecondsSinceEpoch(decodeTime(id)['createdAt']!);
    } catch (_) {
      return null;
    }
  }

  /// Validate a UUID (any version or variant). Returns the match, else `null`.
  static RegExpMatch? isUuid(String id) => _uuidPattern.firstMatch(id);

  /// The canonical (uppercase) spelling of a valid TTID, or `null`.
  ///
  /// Identifiers are matched case-insensitively but only ever emitted in
  /// uppercase, so string equality is not identity unless you normalize.
  /// Feed this any accepted spelling and store what it returns.
  static String? canonical(String id) =>
      isTtid(id) == null ? null : id.toUpperCase();

  /// Generate a new TTID, or advance an existing one through its lifecycle.
  ///
  /// - no args: mints a new single-segment TTID
  /// - [id] only: advances it (a second segment)
  /// - [id] + [delete]: tombstones it (a third segment)
  ///
  /// Throws if [id] is already deleted (three segments) or is not a valid TTID.
  static String generate([String? id, bool delete = false]) {
    if (id != null && isTtid(id) != null && id.split('-').length == 3) {
      throw StateError('This identifier can no longer be modified');
    }

    final time = _timeNow();

    if (id != null && isTtid(id) != null && delete) {
      final parts = id.split('-');
      final updated = parts.length > 1 ? parts[1] : _placeholder;
      final deleted = time.toRadixString(36);
      return '${parts[0]}-$updated-$deleted'.toUpperCase();
    }

    if (id != null && isTtid(id) != null) {
      final created = id.split('-')[0];
      final updated = time.toRadixString(36);
      return '$created-$updated'.toUpperCase();
    }

    if (id != null && isTtid(id) == null) throw ArgumentError('Invalid TTID!');

    return time.toRadixString(36).toUpperCase();
  }
}
