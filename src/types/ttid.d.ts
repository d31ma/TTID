declare module "@d31ma/ttid" {

    /** Branded string type representing a TTID in one of its three lifecycle states. */
    export type _ttid = string | `${string}-${string}` | `${string}-${string}-${string}`

    /** Decoded timestamps extracted from a TTID. */
    export interface _timestamps {
        createdAt: number
        updatedAt?: number
        deletedAt?: number
    }

    export default class TTID {

        /**
         * Checks whether a string is a valid TTID.
         * @param _id - The string to validate.
         * @returns The creation `Date` if valid, or `null` if not.
         */
        static isTTID(_id: string): Date | null

        /**
         * Checks whether a string is a valid UUID (any version or variant).
         * @param _id - The string to validate.
         * @returns A `RegExpMatchArray` if valid, or `null` if not.
         */
        static isUUID(_id: string): RegExpMatchArray | null

        /**
         * Generates a new TTID or advances an existing one through its lifecycle.
         * @param _id - An existing TTID to update or delete. Omit to create a new one.
         * @param del - When `true`, marks the TTID as deleted.
         * @returns The new or advanced TTID.
         * @throws {Error} If `_id` is already deleted or is not a valid TTID.
         */
        static generate(_id?: string, del?: boolean): _ttid

        /**
         * Decodes the timestamps embedded in a TTID.
         * @param _id - A valid TTID string.
         * @returns An object with `createdAt`, and optionally `updatedAt` and `deletedAt` (ms since epoch).
         * @throws {Error} If the format is invalid or any segment encodes an out-of-range timestamp.
         */
        static decodeTime(_id: string): _timestamps
    }
}

// Global ambient aliases so src/index.ts can reference these types without an import.
type _ttid = import("@d31ma/ttid")._ttid
type _timestamps = import("@d31ma/ttid")._timestamps
