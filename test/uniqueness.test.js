// The guarantee the raw clock cannot give.
//
// The encoded timestamp is a double. At the current epoch its ulp is 2 scaled
// units — 200 nanoseconds — so any caller generating faster than that used to
// get duplicates. Measured before the monotonic counter existed: 1746/2000
// unique in Bun, 135/2000 in a browser (browsers clamp `performance.now()` to
// roughly 100µs as a Spectre mitigation).
//
// These tests exist so that never regresses silently again.

import { expect, test } from 'bun:test';
import TTID, { generate } from '../clients/web/ttid.mjs';

const BURST = 20_000;

test('a tight loop produces no duplicates', () => {
        const ids = Array.from({ length: BURST }, () => generate());
        expect(new Set(ids).size).toBe(BURST);
});

test('ids stay strictly increasing, so they stay sortable', () => {
        const ids = Array.from({ length: BURST }, () => generate());
        expect(ids.every((id, index) => index === 0 || ids[index - 1] < id)).toBe(true);
        expect([...ids].sort()).toEqual(ids);
});

test('ids stay 11 characters, so byte order matches numeric order', () => {
        const ids = Array.from({ length: BURST }, () => generate());
        expect(ids.every((id) => id.length === 11)).toBe(true);
});

test('a burst barely moves the decoded timestamp', () => {
        const first = generate();
        for (let index = 2; index < BURST; index++) generate();
        const last = generate();

        const drift = TTID.decodeTime(last).createdAt - TTID.decodeTime(first).createdAt;
        // One scaled unit is 0.1µs. Real elapsed time dominates; the counter's
        // own contribution over 20k ids is about 2ms.
        expect(drift).toBeLessThanOrEqual(50);
});

test('every id in a burst is still valid and decodable', () => {
const ids = Array.from({ length: 1000 }, () => generate());
expect(ids.every((id) => TTID.isTTID(id) instanceof Date)).toBe(true);
});

test('the lifecycle still holds under a burst', () => {
// Updating the same id many times must produce distinct results, not one
// repeated answer.
const base = TTID.generate();
const updates = Array.from({ length: 5000 }, () => TTID.generate(base));
expect(new Set(updates).size).toBe(5000);
expect(updates.every((id) => id.startsWith(`${base}-`))).toBe(true);
});
