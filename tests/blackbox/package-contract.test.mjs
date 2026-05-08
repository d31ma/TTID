import { afterAll, beforeAll, test } from 'bun:test'
import { createTtidConsumer } from './helpers.mjs'

let consumer

beforeAll(async () => {
  consumer = await createTtidConsumer()
})

afterAll(async () => {
  await consumer?.cleanup()
})

test('packed package supports the full TTID lifecycle from a clean consumer project', async () => {
  await consumer.runModule(`
    import TTID from '@d31ma/ttid'

    function assert(condition, message) {
      if (!condition) throw new Error(message)
    }

    const created = TTID.generate()
    assert(/^[A-Z0-9]{11}$/.test(created), 'created TTID should be one 11-character uppercase segment')

    const createdDate = TTID.isTTID(created)
    const createdTimes = TTID.decodeTime(created)
    assert(createdDate instanceof Date, 'created TTID should validate to a Date')
    assert(createdDate.getTime() === createdTimes.createdAt, 'validation Date should match decoded createdAt')
    assert(createdTimes.updatedAt === undefined, 'new TTID should not have updatedAt')
    assert(createdTimes.deletedAt === undefined, 'new TTID should not have deletedAt')

    await Bun.sleep(20)
    const updated = TTID.generate(created)
    const [updatedCreated, updatedAt] = updated.split('-')
    const updatedTimes = TTID.decodeTime(updated)
    assert(updatedCreated === created, 'update should preserve the creation segment')
    assert(updatedAt.length === 11, 'update should add an 11-character update segment')
    assert(updatedTimes.updatedAt > updatedTimes.createdAt, 'updatedAt should be after createdAt')
    assert(updatedTimes.deletedAt === undefined, 'updated TTID should not have deletedAt')

    await Bun.sleep(20)
    const deleted = TTID.generate(updated, true)
    const [deletedCreated, deletedUpdated, deletedAt] = deleted.split('-')
    const deletedTimes = TTID.decodeTime(deleted)
    assert(deletedCreated === created, 'delete should preserve the creation segment')
    assert(deletedUpdated === updatedAt, 'delete after update should preserve the update segment')
    assert(deletedAt.length === 11, 'delete should add an 11-character deletion segment')
    assert(deletedTimes.deletedAt > deletedTimes.updatedAt, 'deletedAt should be after updatedAt')

    let locked = false
    try {
      TTID.generate(deleted)
    } catch (error) {
      locked = error.message === 'This identifier can no longer be modified'
    }
    assert(locked, 'deleted TTID should be immutable')
  `)
})

test('packed package preserves validation, placeholder deletion, and UUID contracts', async () => {
  await consumer.runModule(`
    import TTID from '@d31ma/ttid'

    function assert(condition, message) {
      if (!condition) throw new Error(message)
    }

    const created = TTID.generate()
    await Bun.sleep(20)
    const deleted = TTID.generate(created, true)
    const [deletedCreated, placeholder, deletedAt] = deleted.split('-')
    const deletedTimes = TTID.decodeTime(deleted)

    assert(deletedCreated === created, 'delete without update should preserve the creation segment')
    assert(placeholder === 'X', 'delete without update should use the placeholder update segment')
    assert(deletedAt.length === 11, 'placeholder deletion should include an 11-character deletion segment')
    assert(deletedTimes.updatedAt === undefined, 'placeholder update segment should not decode to updatedAt')
    assert(deletedTimes.deletedAt > deletedTimes.createdAt, 'deletedAt should be after createdAt')

    assert(TTID.isTTID(created.toLowerCase()) instanceof Date, 'lowercase TTIDs should validate')
    assert(TTID.isTTID('') === null, 'empty string should not validate as TTID')
    assert(TTID.isTTID('00000000000') === null, 'out-of-range timestamp should not validate as TTID')
    assert(TTID.isTTID('550e8400-e29b-41d4-a716-446655440000') === null, 'UUID should not validate as TTID')
    assert(TTID.isUUID(created) === null, 'TTID should not validate as UUID')
    assert(TTID.isUUID('550e8400-e29b-41d4-a716-446655440000') !== null, 'UUID should validate as UUID')

    let invalidGenerate = false
    try {
      TTID.generate('not-a-valid-ttid')
    } catch (error) {
      invalidGenerate = error.message === 'Invalid TTID!'
    }
    assert(invalidGenerate, 'generate should reject invalid input')

    let invalidFormat = false
    try {
      TTID.decodeTime('not-a-valid-ttid')
    } catch (error) {
      invalidFormat = error.message === 'Invalid Format!'
    }
    assert(invalidFormat, 'decodeTime should reject invalid format')

    let invalidTimestamp = false
    try {
      TTID.decodeTime('00000000000')
    } catch (error) {
      invalidTimestamp = error.message === 'Invalid timestamp encoding'
    }
    assert(invalidTimestamp, 'decodeTime should reject out-of-range timestamp encodings')
  `)
})

test('packed package exposes TypeScript declarations for default and named imports', async () => {
  await consumer.typecheck(`
    import TTID, { type _timestamps, type _ttid } from '@d31ma/ttid'

    const created: _ttid = TTID.generate()
    const updated: _ttid = TTID.generate(created)
    const deleted: _ttid = TTID.generate(updated, true)
    const createdDate: Date | null = TTID.isTTID(deleted)
    const timestamps: _timestamps = TTID.decodeTime(deleted)
    const uuidMatch: RegExpMatchArray | null = TTID.isUUID('550e8400-e29b-41d4-a716-446655440000')

    if (!createdDate || !timestamps.createdAt || !timestamps.deletedAt || !uuidMatch) {
      throw new Error('TTID type contract should support runtime guards')
    }
  `)
})
