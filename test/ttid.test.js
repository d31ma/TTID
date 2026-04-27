import TTID from "../src/index.js";
import { test, expect, describe } from 'bun:test'

describe("TTID", () => {

    // ─── Lifecycle integration tests ────────────────────────────────────────────

    test("Generate", () => {

        const _id = TTID.generate()

        console.log("Created", _id, _id.length)

        const { createdAt } = TTID.decodeTime(_id)

        expect(TTID.isTTID(_id)).toBeInstanceOf(Date)
        expect(TTID.isUUID(_id)).toBeNull()
        expect(TTID.isUUID(Bun.randomUUIDv7())).not.toBeNull()
        expect(typeof createdAt).toBe('number')
    })

    test("Update", async () => {

        const _id = TTID.generate()
        await Bun.sleep(1000)
        const _newId = TTID.generate(_id)

        console.log("Updated", _newId, _newId.length)

        const [created, updated] = _newId.split('-')

        const { createdAt, updatedAt } = TTID.decodeTime(_newId)

        expect(updated).not.toBeUndefined()
        expect(created).not.toEqual(updated)
        expect(TTID.isTTID(_newId)).toBeInstanceOf(Date)
        expect(TTID.isUUID(_newId)).toBeNull()
        expect(_id).not.toEqual(_newId)
        expect(typeof createdAt).toBe('number')
        expect(typeof updatedAt).toBe('number')
        expect(createdAt).not.toEqual(updatedAt)
        expect(updatedAt).not.toBeUndefined()
        expect(updatedAt).toBeGreaterThan(createdAt)
    })

    test('Created-Deleted', async () => {

        const _id = TTID.generate()
        await Bun.sleep(1000)
        const _newId = TTID.generate(_id, true)

        console.log("Created-Deleted", _newId, _newId.length)

        const [created, updated, deleted] = _newId.split('-')

        const { createdAt, updatedAt, deletedAt } = TTID.decodeTime(_newId)

        expect(updated).toEqual('X')
        expect(deleted).not.toBeUndefined()
        expect(created).not.toEqual(deleted)
        expect(TTID.isTTID(_newId)).toBeInstanceOf(Date)
        expect(TTID.isUUID(_newId)).toBeNull()
        expect(_id).not.toEqual(_newId)
        expect(typeof createdAt).toBe('number')
        expect(typeof deletedAt).toBe('number')
        expect(createdAt).not.toEqual(updatedAt)
        expect(updatedAt).toBeUndefined()
        expect(deletedAt).not.toBeUndefined()
        expect(deletedAt).toBeGreaterThan(createdAt)
    })

    test('Created-Updated-Deleted', async () => {

        const _id = TTID.generate()
        await Bun.sleep(1000)
        const _newId = TTID.generate(_id)
        await Bun.sleep(1000)
        const _deletedId = TTID.generate(_newId, true)

        console.log("Created-Updated-Deleted", _deletedId, _deletedId.length)

        const [created, updated, deleted] = _deletedId.split('-')

        const { createdAt, updatedAt, deletedAt } = TTID.decodeTime(_deletedId)

        expect(updated).not.toEqual('X')
        expect(deleted).not.toBeUndefined()
        expect(created).not.toEqual(deleted)
        expect(created).not.toEqual(updated)
        expect(updated).not.toEqual(deleted)
        expect(TTID.isTTID(_newId)).toBeInstanceOf(Date)
        expect(TTID.isUUID(_newId)).toBeNull()
        expect(_id).not.toEqual(_newId)
        expect(_newId).not.toEqual(_deletedId)
        expect(typeof createdAt).toBe('number')
        expect(typeof deletedAt).toBe('number')
        expect(typeof updatedAt).toBe("number")
        expect(createdAt).not.toEqual(updatedAt)
        expect(createdAt).not.toEqual(deletedAt)
        expect(updatedAt).not.toEqual(deletedAt)
        expect(updatedAt).not.toBeUndefined()
        expect(deletedAt).not.toBeUndefined()
        expect(deletedAt).toBeGreaterThan(createdAt)
        expect(updatedAt).toBeGreaterThan(createdAt)
    })

    // ─── generate() ─────────────────────────────────────────────────────────────

    describe('generate()', () => {

        test('Updated-Deleted: 2-segment + del:true produces 3-segment without placeholder', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const _updatedId = TTID.generate(_id)
            await Bun.sleep(100)
            const _deletedId = TTID.generate(_updatedId, true)

            const [, updated, deleted] = _deletedId.split('-')

            expect(updated).not.toEqual('X')
            expect(deleted).not.toBeUndefined()
            expect(_deletedId.split('-')).toHaveLength(3)
            expect(TTID.isTTID(_deletedId)).toBeInstanceOf(Date)
        })

        test('del:false explicitly behaves the same as omitting the flag', () => {

            const _id = TTID.generate()
            const _updated = TTID.generate(_id, false)

            expect(_updated.split('-')).toHaveLength(2)
        })

        test('throws when given a UUID string', () => {

            expect(() => TTID.generate(Bun.randomUUIDv7())).toThrow('Invalid TTID!')
        })

        test('throws when attempting to modify a deleted TTID', () => {

            const _id = TTID.generate()
            const _deletedId = TTID.generate(_id, true)

            expect(() => TTID.generate(_deletedId)).toThrow('This identifier can no longer be modified')
            expect(() => TTID.generate(_deletedId, true)).toThrow('This identifier can no longer be modified')
        })

        test('throws when given an invalid TTID string', () => {

            expect(() => TTID.generate('not-a-valid-ttid')).toThrow('Invalid TTID!')
        })

        test('successive calls produce unique IDs', async () => {

            /** @type {string[]} */
            const ids = []
            for (let i = 0; i < 5; i++) {
                await Bun.sleep(1)
                ids.push(TTID.generate())
            }

            expect(new Set(ids).size).toBe(ids.length)
        })
    })

    // ─── isTTID() ────────────────────────────────────────────────────────────────

    describe('isTTID()', () => {

        test('empty string returns null', () => {

            expect(TTID.isTTID('')).toBeNull()
        })

        test('1-segment TTID returns creation Date', () => {

            const _id = TTID.generate()
            expect(TTID.isTTID(_id)).toBeInstanceOf(Date)
        })

        test('2-segment TTID returns creation Date, not the update timestamp', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const _updatedId = TTID.generate(_id)

            const result = TTID.isTTID(_updatedId)
            const { createdAt } = TTID.decodeTime(_updatedId)

            expect(result).toBeInstanceOf(Date)
            expect(result?.getTime()).toBe(createdAt)
        })

        test('3-segment TTID returns creation Date, not the deletion timestamp', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const _deletedId = TTID.generate(_id, true)

            const result = TTID.isTTID(_deletedId)
            const { createdAt } = TTID.decodeTime(_deletedId)

            expect(result).toBeInstanceOf(Date)
            expect(result?.getTime()).toBe(createdAt)
        })

        test('valid-format but out-of-range timestamp returns null', () => {

            expect(TTID.isTTID('00000000000')).toBeNull()
        })

        test('UUID string returns null', () => {

            expect(TTID.isTTID(Bun.randomUUIDv7())).toBeNull()
        })

        test('rejects oversized input without regex evaluation', () => {

            expect(TTID.isTTID('A'.repeat(37))).toBeNull()
        })
    })

    // ─── decodeTime() ────────────────────────────────────────────────────────────

    describe('decodeTime()', () => {

        test('1-segment TTID has only createdAt defined', () => {

            const { createdAt, updatedAt, deletedAt } = TTID.decodeTime(TTID.generate())

            expect(typeof createdAt).toBe('number')
            expect(updatedAt).toBeUndefined()
            expect(deletedAt).toBeUndefined()
        })

        test('2-segment TTID has updatedAt defined and deletedAt undefined', () => {

            const _id = TTID.generate()
            const { updatedAt, deletedAt } = TTID.decodeTime(TTID.generate(_id))

            expect(updatedAt).not.toBeUndefined()
            expect(deletedAt).toBeUndefined()
        })

        test('3-segment TTID with placeholder has updatedAt undefined and deletedAt defined', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const _deletedId = TTID.generate(_id, true)
            const { updatedAt, deletedAt } = TTID.decodeTime(_deletedId)

            expect(updatedAt).toBeUndefined()
            expect(deletedAt).not.toBeUndefined()
        })

        test('timestamps are in chronological order across the full lifecycle', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const _updatedId = TTID.generate(_id)
            await Bun.sleep(100)
            const _deletedId = TTID.generate(_updatedId, true)

            const { createdAt, updatedAt, deletedAt } = TTID.decodeTime(_deletedId)

            if (updatedAt === undefined || deletedAt === undefined) {
                throw new Error('updatedAt and deletedAt should be defined for a deleted TTID')
            }

            expect(updatedAt).toBeGreaterThan(createdAt)
            expect(deletedAt).toBeGreaterThan(updatedAt)
        })

        test('throws on invalid format', () => {

            expect(() => TTID.decodeTime('not-a-valid-ttid')).toThrow('Invalid Format!')
        })

        test('throws on out-of-range timestamp segment', () => {

            expect(() => TTID.decodeTime('00000000000')).toThrow('Invalid timestamp encoding')
        })
    })

    // ─── Output format ───────────────────────────────────────────────────────────

    describe('output format', () => {

        test('generated IDs are always uppercase', () => {

            const _id = TTID.generate()
            expect(_id).toBe(_id.toUpperCase())
        })

        test('single-segment ID is exactly 11 characters', () => {

            expect(TTID.generate()).toHaveLength(11)
        })

        test('two-segment ID has two 11-character segments', () => {

            const _id = TTID.generate()
            const [created, updated] = TTID.generate(_id).split('-')

            expect(created).toHaveLength(11)
            expect(updated).toHaveLength(11)
        })

        test('three-segment ID (with placeholder) has correct segment lengths', async () => {

            const _id = TTID.generate()
            await Bun.sleep(100)
            const [created, placeholder, deleted] = TTID.generate(_id, true).split('-')

            expect(created).toHaveLength(11)
            expect(placeholder).toBe('X')
            expect(deleted).toHaveLength(11)
        })

        test('isTTID Date.getTime() is consistent with decodeTime createdAt', () => {

            const _id = TTID.generate()
            const date = TTID.isTTID(_id)
            const { createdAt } = TTID.decodeTime(_id)

            expect(date?.getTime()).toBe(createdAt)
        })
    })

    // ─── isUUID() ────────────────────────────────────────────────────────────────

    describe('isUUID()', () => {

        test('TTID returns null', () => {

            expect(TTID.isUUID(TTID.generate())).toBeNull()
        })

        test('valid UUID v4 returns a match', () => {

            expect(TTID.isUUID('550e8400-e29b-41d4-a716-446655440000')).not.toBeNull()
        })

        test('valid UUID v7 returns a match', () => {

            expect(TTID.isUUID(Bun.randomUUIDv7())).not.toBeNull()
        })

        test('empty string returns null', () => {

            expect(TTID.isUUID('')).toBeNull()
        })
    })
})
