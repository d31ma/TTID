import { BASE, PLACEHOLDER } from '../constants.js';
import { timeNow } from '../utils/time.js';
import { isTTID } from './validator.js';

/**
 * Generates a new TTID or advances an existing one through its lifecycle.
 *
 * - No arguments: creates a new single-segment TTID.
 * - `_id` only: updates the TTID, producing a two-segment ID.
 * - `_id` + `del: true`: marks the TTID as deleted, producing a three-segment ID.
 *
 * @param {string} [_id] - An existing TTID to update or delete. Omit to create a new one.
 * @param {boolean} [del=false] - When `true`, marks the TTID as deleted.
 * @returns {_ttid} The new or advanced TTID.
 * @throws {Error} If `_id` is already deleted (three-segment) or is not a valid TTID.
 */
export function generate(_id, del = false) {
    if (_id && isTTID(_id) && _id.split('-').length === 3) {
        throw new Error('This identifier can no longer be modified');
    }

    const time = timeNow();

    if (_id && isTTID(_id) && del) {
        const [created, updated] = _id.split('-');
        const deleted = time.toString(BASE);

        return `${created}-${updated ?? PLACEHOLDER}-${deleted}`.toUpperCase();
    }

    if (_id && isTTID(_id)) {
        const [created] = _id.split('-');
        const updated = time.toString(BASE);

        return `${created}-${updated}`.toUpperCase();
    }

    if (_id && !isTTID(_id)) throw new Error('Invalid TTID!');

    return time.toString(BASE).toUpperCase();
}
