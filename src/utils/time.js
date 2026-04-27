import { PRECISION } from '../constants.js';

/**
 * Retrieves the current high-resolution timestamp, adjusted for precision.
 * @returns {number} The current time adjusted by PRECISION.
 */
export function timeNow() {
    return (performance.now() + performance.timeOrigin) * PRECISION;
}
