/** Branded string type representing a TTID in one of its three lifecycle states. */
type _ttid = string | `${string}-${string}` | `${string}-${string}-${string}`;

/** Decoded timestamps extracted from a TTID. */
interface _timestamps {
    createdAt: number;
    updatedAt?: number;
    deletedAt?: number;
}
