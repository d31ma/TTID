// Issue #32: identifiers are matched case-insensitively but only ever emitted
// in uppercase, so string equality is not identity unless a consumer
// normalizes. `canonical` is what they normalize with.
//
// The Rust kernel's version is covered in tests/invariants.rs; this covers the
// hand-written web client, which reimplements the logic and so could drift.

import { expect, test } from 'bun:test';
import TTID, { generate, canonical, isTTID } from '../clients/web/ttid.mjs';

const mixedCase = (id) =>
    [...id].map((c, i) => (i % 2 ? c.toUpperCase() : c.toLowerCase())).join('');

test('every accepted spelling collapses to one canonical form', () => {
    const id = generate();
    for (const spelling of [id, id.toLowerCase(), mixedCase(id)]) {
        expect(canonical(spelling)).toBe(id);
    }
});

test('canonical is idempotent', () => {
    const id = generate();
    expect(canonical(canonical(id.toLowerCase()))).toBe(id);
});

test('canonical covers the whole lifecycle', () => {
    const created = generate();
    const updated = generate(created);
    const deleted = generate(updated, true);
    for (const id of [created, updated, deleted]) {
        expect(canonical(id.toLowerCase())).toBe(id);
    }
});

test('canonical returns null for anything that is not a TTID', () => {
    for (const input of ['', 'not-a-ttid', '3f2504e0-4f89-41d3-9a0c-0305e82c3301', '00000000000']) {
        expect(canonical(input)).toBeNull();
    }
});

test('normalizing does not change what an id means', () => {
    const id = generate();
    expect(TTID.decodeTime(canonical(id.toLowerCase()))).toEqual(TTID.decodeTime(id));
    expect(isTTID(canonical(id.toLowerCase()))).toEqual(isTTID(id));
});

test('canonicalizing restores chronological sort order', () => {
    const ids = Array.from({ length: 200 }, () => generate());
    const mixed = ids.map((id, index) => (index % 2 === 0 ? id.toLowerCase() : id));

    expect([...mixed].sort()).not.toEqual(mixed);
    const normalized = mixed.map(canonical);
    expect([...normalized].sort()).toEqual(normalized);
    expect(normalized).toEqual(ids);
});

test('the class facade exposes it too', () => {
    const id = TTID.generate();
    expect(TTID.canonical(id.toLowerCase())).toBe(id);
});
