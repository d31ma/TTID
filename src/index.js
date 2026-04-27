import { isTTID, isUUID, decodeTime } from './core/validator.js';
import { generate } from './core/generator.js';

/**
 * TTID (Time-Tagged Identifier) class.
 */
export default class TTID {
    /**
     * Checks whether a string is a valid TTID.
     * @param {string} _id - The string to validate.
     * @returns {Date | null} The creation `Date` if valid, or `null` if not.
     */
    static isTTID(_id) {
        return isTTID(_id);
    }

    /**
     * Checks whether a string is a valid UUID (any version or variant).
     * @param {string} _id - The string to validate.
     * @returns {RegExpMatchArray | null} A `RegExpMatchArray` if valid, or `null` if not.
     */
    static isUUID(_id) {
        return isUUID(_id);
    }

    /**
     * Generates a new TTID or advances an existing one through its lifecycle.
     * @param {string} [_id] - An existing TTID to update or delete. Omit to create a new one.
     * @param {boolean} [del=false] - When `true`, marks the TTID as deleted.
     * @returns {_ttid} The new or advanced TTID.
     */
    static generate(_id, del = false) {
        return generate(_id, del);
    }

    /**
     * Decodes the timestamps embedded in a TTID.
     * @param {string} _id - A valid TTID string.
     * @returns {_timestamps} An object with `createdAt`, and optionally `updatedAt` and `deletedAt` (ms since epoch).
     */
    static decodeTime(_id) {
        return decodeTime(_id);
    }
}
