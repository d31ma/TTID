// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// No dependencies (java.lang.Process only). Requires the `ttid` binary on PATH
// or an explicit path. One long-lived subprocess.
//
//   try (Ttid t = new Ttid()) {
//       String id = t.generate();          // {"...","result":"4VL..."}
//       String up = t.generate("4VL...");  // advance it
//       String times = t.decodeTime("4VL...");
//       String valid = t.isTTID("4VL...");
//       String uuid = t.isUUID("...");
//   }
//
// Each method builds the request, checks it succeeded, and returns the raw JSON
// response line (parse `result` with Jackson/Gson). Method names follow Java's
// camelCase. request(json) is the raw escape hatch.

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.List;

public final class Ttid implements AutoCloseable {
    private final Process proc;
    private final BufferedWriter in;
    private final BufferedReader out;

    public Ttid() throws IOException {
        this("ttid");
    }

    public Ttid(String binary) throws IOException {
        this.proc = new ProcessBuilder(List.of(binary, "exec", "--loop"))
                .redirectError(ProcessBuilder.Redirect.INHERIT)
                .start();
        this.in = new BufferedWriter(
                new OutputStreamWriter(proc.getOutputStream(), StandardCharsets.UTF_8));
        this.out = new BufferedReader(
                new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8));
    }

    /** Send one raw machine-protocol op (JSON string); returns the response line. */
    public synchronized String request(String opJson) throws IOException {
        if (!proc.isAlive()) throw new IOException("ttid process has exited");
        in.write(opJson.stripTrailing());
        in.write('\n');
        in.flush();
        String line = out.readLine();
        if (line == null) throw new IOException("ttid closed the stream");
        return line;
    }

    // Build an op from native fields (skipping null values), send it, and error
    // on a failure response. ponytail: checks the "ok":true field by substring.
    private String op(String name, Object... kv) throws IOException {
        StringBuilder sb = new StringBuilder("{\"op\":").append(toJson(name));
        for (int i = 0; i + 1 < kv.length; i += 2) {
            if (kv[i + 1] == null) continue;
            sb.append(',').append(toJson(kv[i].toString())).append(':').append(toJson(kv[i + 1]));
        }
        String resp = request(sb.append('}').toString());
        if (!resp.contains("\"ok\":true")) throw new IOException(resp.strip());
        return resp;
    }

    /** Generate a new TTID (id == null), or advance it (del == true to tombstone). */
    public String generate() throws IOException {
        return op("generate");
    }

    public String generate(String id) throws IOException {
        return op("generate", "id", id);
    }

    public String generate(String id, boolean del) throws IOException {
        return op("generate", "id", id, "delete", del ? Boolean.TRUE : null);
    }

    public String decodeTime(String id) throws IOException {
        return op("decodeTime", "id", id);
    }

    public String isTTID(String id) throws IOException {
        return op("isTTID", "id", id);
    }

    public String isUUID(String id) throws IOException {
        return op("isUUID", "id", id);
    }

    // Minimal JSON encoder for String / Number / Boolean / null.
    static String toJson(Object v) {
        if (v == null) return "null";
        if (v instanceof String) {
            return "\"" + ((String) v).replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
        }
        if (v instanceof Boolean || v instanceof Number) return v.toString();
        throw new IllegalArgumentException("unsupported JSON value: " + v.getClass());
    }

    @Override
    public void close() throws IOException {
        in.close(); // EOF ends the loop
        try {
            proc.waitFor();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        out.close();
    }
}
