// Exercises clients/kotlin/Ttid.kt against the binary in TTID_BIN.
private const val FIXED = "4SQ1NZT5HC0"
private const val UPDATED = "4SQ1NZT5HC0-4SQ1NZT5P1S"
private const val DELETED = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"

fun main() {
    Ttid(System.getenv("TTID_BIN")).use { t ->
        println("generate=" + t.generate())
        println("update=" + t.generate(FIXED))
        println("delete=" + t.generate(UPDATED, true))
        println("decode=" + t.decodeTime(DELETED))
        println("isTTID=" + t.isTTID(FIXED))
        println("isTTID-bad=" + t.isTTID("nope"))
        println("isUUID=" + t.isUUID("3f2504e0-4f89-41d3-9a0c-0305e82c3301"))
        println("isUUID-bad=" + t.isUUID("nope"))
        println("canonical=" + t.canonicalize(FIXED.lowercase()))
        try {
            t.generate(DELETED)
            println("error=NO ERROR RAISED")
        } catch (e: Exception) {
            println("error=" + e.message)
        }
    }
}
