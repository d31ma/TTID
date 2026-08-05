// Exercises clients/dart/ttid.dart against the binary in TTID_BIN.
import 'dart:convert';
import 'dart:io';
import '../../../clients/dart/ttid.dart';

Future<void> main() async {
  const fixed = '4SQ1NZT5HC0';
  const updated = '4SQ1NZT5HC0-4SQ1NZT5P1S';
  const deleted = '4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK';
  final out = <List<Object?>>[];
  final t = await Ttid.open(Platform.environment['TTID_BIN']!);
  out.add(['generate', await t.generate()]);
  out.add(['update', await t.generate(fixed)]);
  out.add(['delete', await t.generate(updated, true)]);
  out.add(['decode', await t.decodeTime(deleted)]);
  out.add(['isTTID', await t.isTtid(fixed)]);
  out.add(['isTTID-bad', await t.isTtid('nope')]);
  out.add(['isUUID', await t.isUuid('3f2504e0-4f89-41d3-9a0c-0305e82c3301')]);
  out.add(['isUUID-bad', await t.isUuid('nope')]);
  out.add(['canonical', await t.canonicalize(fixed.toLowerCase())]);
  try {
    await t.generate(deleted);
    out.add(['error', 'NO ERROR RAISED']);
  } on TtidException catch (e) {
    out.add(['error', e.toString().replaceFirst('TtidException: ', '')]);
  }
  await t.close();
  for (final row in out) {
    var value = row[1];
    if (value is Map) value = Map.fromEntries(value.entries.toList()..sort((a, b) => '${a.key}'.compareTo('${b.key}')));
    print('${row[0]}=${jsonEncode(value)}');
  }
  // No exit() here on purpose: if the client's close() ever regresses and leaks
  // a pipe subscription again, this driver hangs and the harness times out.
}
