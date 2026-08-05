// TTID native client — a self-contained, dependency-free implementation for
// Kotlin on any platform, including Android.
//
// Unlike `Ttid.kt`, this build does NOT drive the `ttid` binary — it spawns no
// subprocess, so it runs on Android (where bundling and exec-ing a native
// binary isn't practical) as well as the JVM. TTID is pure computation (base-36
// timestamp encoding + validation), reimplemented here with the JDK only. IDs
// are interoperable with every other TTID client.
//
//   val id = TtidNative.generate()               // new id, e.g. "4VLSK98UX1K"
//   val updated = TtidNative.generate(id)        // advance it
//   val deleted = TtidNative.generate(updated, true) // final state
//   TtidNative.decodeTime(deleted)               // {createdAt=..., ...} (ms)
//   TtidNative.isTTID(id)                         // Date if valid, else null
//   TtidNative.isUUID("not-a-uuid")              // Boolean
//
// For a JVM server or CLI that has the `ttid` binary, use the binary-driven
// client in `Ttid.kt` instead.

import java.time.Instant
import java.util.Date

/** Namespace of pure-Kotlin TTID operations. No state, no process, no I/O. */
object TtidNative {
    private const val PRECISION = 10_000L
    private const val PLACEHOLDER = "X"
    private const val MIN_TIMESTAMP_MS = 1_577_836_800_000L
    private const val MAX_TIMESTAMP_MS = 7_258_118_400_000L

    private val ttidPattern =
        Regex("^[A-Z0-9]{11}(-[A-Z0-9]{1,11}){0,2}$", RegexOption.IGNORE_CASE)
    private val uuidPattern = Regex(
        "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$",
        RegexOption.IGNORE_CASE
    )

    /** Current high-resolution timestamp: microseconds * 10 == ms * PRECISION. */
    private fun timeNow(): Long {
        val now = Instant.now()
        val micros = now.epochSecond * 1_000_000L + now.nano / 1000L
        return micros * 10L
    }

    /**
     * Decode the timestamps embedded in a TTID.
     * @throws IllegalArgumentException if the format is invalid or out of range.
     */
    fun decodeTime(id: String): Map<String, Long> {
        if (!ttidPattern.matches(id)) throw IllegalArgumentException("Invalid Format!")
        val parts = id.split("-")

        fun toMs(code: String): Long {
            val ms = Math.round(code.toLong(36).toDouble() / PRECISION)
            if (ms < MIN_TIMESTAMP_MS || ms > MAX_TIMESTAMP_MS) {
                throw IllegalArgumentException("Invalid timestamp encoding")
            }
            return ms
        }

        val result = linkedMapOf("createdAt" to toMs(parts[0]))
        if (parts.size > 1 && parts[1] != PLACEHOLDER) result["updatedAt"] = toMs(parts[1])
        if (parts.size > 2) result["deletedAt"] = toMs(parts[2])
        return result
    }

    /** Validate a TTID. Returns the creation [Date] if valid, else `null`. */
    fun isTTID(id: String): Date? {
        if (id.isEmpty() || id.length > 36) return null
        if (!ttidPattern.matches(id)) return null
        return try {
            Date(decodeTime(id)["createdAt"]!!)
        } catch (e: Exception) {
            null
        }
    }

    /**
     * The canonical (uppercase) spelling of a valid TTID, or `null`.
     *
     * Identifiers are matched case-insensitively but only ever emitted in
     * uppercase, so string equality is not identity unless you normalize.
     * Feed this any accepted spelling and store what it returns.
     */
    fun canonical(id: String): String? = if (isTTID(id) == null) null else id.uppercase()

    /** Validate a UUID (any version or variant). */
    fun isUUID(id: String): Boolean = uuidPattern.matches(id)

    /**
     * Generate a new TTID, or advance an existing one through its lifecycle.
     * @throws IllegalStateException if `id` is already deleted (three segments).
     * @throws IllegalArgumentException if `id` is not a valid TTID.
     */
    fun generate(id: String? = null, delete: Boolean = false): String {
        if (id != null && isTTID(id) != null && id.split("-").size == 3) {
            throw IllegalStateException("This identifier can no longer be modified")
        }

        val encoded = timeNow().toString(36).uppercase()

        if (id != null && isTTID(id) != null && delete) {
            val parts = id.split("-")
            val updated = if (parts.size > 1) parts[1] else PLACEHOLDER
            return "${parts[0]}-$updated-$encoded".uppercase()
        }

        if (id != null && isTTID(id) != null) {
            val created = id.split("-")[0]
            return "$created-$encoded".uppercase()
        }

        if (id != null && isTTID(id) == null) throw IllegalArgumentException("Invalid TTID!")

        return encoded
    }
}
