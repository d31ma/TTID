/**
 * Multiplier applied to high-resolution timestamps to preserve sub-millisecond
 * precision when encoding to base-36. A value of 10,000 gives ~0.1ms resolution.
 * @constant {number}
 */
export const PRECISION = 10_000;

/**
 * Encoding base for timestamp segments. Base-36 uses digits 0–9 and letters A–Z,
 * yielding compact 11-character timestamps for current Unix millisecond values.
 * @constant {number}
 */
export const BASE = 36;

/**
 * Segment placeholder used when an ID is deleted without a prior update,
 * preserving the three-segment TTID structure.
 * @constant {string}
 */
export const PLACEHOLDER = 'X';

/**
 * Minimum accepted timestamp (ms since epoch): 2020-01-01T00:00:00.000Z
 * @constant {number}
 */
export const MIN_TIMESTAMP_MS = 1_577_836_800_000;

/**
 * Maximum accepted timestamp (ms since epoch): 2200-01-01T00:00:00.000Z
 * @constant {number}
 */
export const MAX_TIMESTAMP_MS = 7_258_118_400_000;

/**
 * Cached compiled regex for TTID format validation, avoiding repeated recompilation.
 * @constant {RegExp}
 */
export const TTID_PATTERN = /^[A-Z0-9]{11}(-[A-Z0-9]{1,11}){0,2}$/i;

/**
 * Cached compiled regex for UUID format validation.
 * @constant {RegExp}
 */
export const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
