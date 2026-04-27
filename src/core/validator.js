import { TTID_PATTERN, UUID_PATTERN, BASE, PRECISION, MIN_TIMESTAMP_MS, MAX_TIMESTAMP_MS, PLACEHOLDER } from '../constants.js';

/**
 * Decodes the timestamps embedded in a TTID.
 * @param {string} _id - A valid TTID string.
 * @returns {_timestamps} An object with `createdAt`, and optionally `updatedAt` and `deletedAt` (ms since epoch).
 * @throws {Error} If the format is invalid or any segment encodes an out-of-range timestamp.
 */
export function decodeTime(_id) {
    if (!TTID_PATTERN.test(_id)) throw new Error('Invalid Format!');

    const [created, updated, deleted] = _id.split('-');

    /**
     * @param {string} timeCode
     * @returns {number}
     */
    const convertToMilliseconds = (timeCode) => {
        const ms = Number((parseInt(timeCode, BASE) / PRECISION).toFixed(0));

        if (!isFinite(ms) || ms < MIN_TIMESTAMP_MS || ms > MAX_TIMESTAMP_MS) {
            throw new Error('Invalid timestamp encoding');
        }

        return ms;
    };

    /** @type {_timestamps} */
    const timestamps = {
        createdAt: convertToMilliseconds(created)
    };

    if (updated && updated !== PLACEHOLDER) timestamps.updatedAt = convertToMilliseconds(updated);
    if (deleted) timestamps.deletedAt = convertToMilliseconds(deleted);

    return timestamps;
}

/**
 * Checks whether a string is a valid TTID.
 * @param {string} _id - The string to validate.
 * @returns {Date | null} The creation `Date` if valid, or `null` if not.
 */
export function isTTID(_id) {
    if (!_id || _id.length > 36) return null;

    if (!TTID_PATTERN.test(_id)) return null;

    try {
        const { createdAt, updatedAt, deletedAt } = decodeTime(_id);

        if (updatedAt !== undefined) new Date(updatedAt);
        if (deletedAt !== undefined) new Date(deletedAt);

        return new Date(createdAt);
    } catch {
        return null;
    }
}

/**
 * Checks whether a string is a valid UUID (any version or variant).
 * @param {string} _id - The string to validate.
 * @returns {RegExpMatchArray | null} A `RegExpMatchArray` if valid, or `null` if not.
 */
export function isUUID(_id) {
    return _id.match(UUID_PATTERN);
}
