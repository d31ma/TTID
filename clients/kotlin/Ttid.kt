// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// No dependencies (java.lang.ProcessBuilder only). Requires the `ttid` binary on
// PATH or an explicit path. One long-lived subprocess.
//
//   Ttid().use { t ->
//       val id = t.generate()        // {..."result":"4VL..."}
//       t.generate("4VL...")         // advance it
//       t.generate("4VL...", true)   // mark deleted
//       val times = t.decodeTime("4VL...") // methods return the raw JSON
//       val valid = t.isTTID("4VL...")     // response line — parse with kotlinx/Gson
//   }
//
// Each method builds the request, checks it succeeded, and returns the raw JSON
// response line. Method names follow Kotlin's camelCase. request(json) is the
// raw escape hatch.

import java.io.BufferedReader
import java.io.BufferedWriter
import java.io.InputStreamReader
import java.io.OutputStreamWriter

class Ttid(binary: String = "ttid") : AutoCloseable {
    private val proc = ProcessBuilder(binary, "exec", "--loop")
        .redirectError(ProcessBuilder.Redirect.INHERIT)
        .start()
    private val input = BufferedWriter(OutputStreamWriter(proc.outputStream, Charsets.UTF_8))
    private val output = BufferedReader(InputStreamReader(proc.inputStream, Charsets.UTF_8))

    /** Send one raw machine-protocol op (JSON string); returns the response line. */
    @Synchronized
    fun request(opJson: String): String {
        if (!proc.isAlive) throw RuntimeException("ttid process has exited")
        input.write(opJson.trimEnd())
        input.write("\n")
        input.flush()
        return output.readLine() ?: throw RuntimeException("ttid closed the stream")
    }

    // Build an op from native fields (skipping null values), send it, and error
    // on a failure response. ponytail: checks the "ok":true field by substring.
    private fun op(name: String, vararg kv: Pair<String, Any?>): String {
        val sb = StringBuilder("{\"op\":").append(toJson(name))
        for ((key, value) in kv) {
            if (value == null) continue
            sb.append(',').append(toJson(key)).append(':').append(toJson(value))
        }
        val resp = request(sb.append('}').toString())
        if (!resp.contains("\"ok\":true")) throw RuntimeException(resp.trim())
        return resp
    }

    /** Generate a new TTID (id == null), or advance it (delete == true to tombstone). */
    fun generate(id: String? = null, delete: Boolean = false): String =
        op("generate", "id" to id, "delete" to if (delete) true else null)

    fun decodeTime(id: String): String = op("decodeTime", "id" to id)
    fun isTTID(id: String): String = op("isTTID", "id" to id)
    fun isUUID(id: String): String = op("isUUID", "id" to id)

    override fun close() {
        input.close() // EOF ends the loop
        proc.waitFor()
        output.close()
    }

    companion object {
        // Minimal JSON encoder for String / Number / Boolean / null.
        fun toJson(v: Any?): String = when (v) {
            null -> "null"
            is String -> "\"" + v.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
            is Boolean, is Number -> v.toString()
            else -> throw IllegalArgumentException("unsupported JSON value: $v")
        }
    }
}
