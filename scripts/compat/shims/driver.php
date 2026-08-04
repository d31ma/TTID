<?php
// Exercises clients/php/ttid.php against the binary in TTID_BIN.
require __DIR__ . '/../../../clients/php/ttid.php';

$FIXED = '4SQ1NZT5HC0';
$UPDATED = '4SQ1NZT5HC0-4SQ1NZT5P1S';
$DELETED = '4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK';
$out = [];
$t = new TTID(getenv('TTID_BIN'));
$out[] = ['generate', $t->generate()];
$out[] = ['update', $t->generate($FIXED)];
$out[] = ['delete', $t->generate($UPDATED, true)];
$out[] = ['decode', $t->decodeTime($DELETED)];
$out[] = ['isTTID', $t->isTTID($FIXED)];
$out[] = ['isTTID-bad', $t->isTTID('nope')];
$out[] = ['isUUID', $t->isUUID('3f2504e0-4f89-41d3-9a0c-0305e82c3301')];
$out[] = ['isUUID-bad', $t->isUUID('nope')];
try {
    $t->generate($DELETED);
    $out[] = ['error', 'NO ERROR RAISED'];
} catch (TTIDException $e) {
    $out[] = ['error', $e->getMessage()];
}
$t->close();
foreach ($out as [$name, $value]) {
    if (is_array($value)) ksort($value);
    echo $name . '=' . json_encode($value) . "\n";
}
