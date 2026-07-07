// TTID client — drives the `ttid` binary's persistent NDJSON loop.
//
// No NuGet dependencies (System.Text.Json ships with .NET). Requires the `ttid`
// binary on PATH or an explicit path. One long-lived subprocess.
//
//   using var t = new Ttid();
//   string id = t.Generate().GetString();          // new id
//   JsonElement up = t.Generate(id);               // advance it
//   JsonElement times = t.DecodeTime(up.GetString());
//   JsonElement valid = t.IsTTID(id);
//   JsonElement uuid = t.IsUUID("...");
//
// Each method builds the request and returns the op's `result` as a JsonElement
// (throwing TtidException on failure). Method names follow .NET PascalCase.
// Request(json) is the raw escape hatch returning the full response.

using System;
using System.Diagnostics;
using System.Text.Json;

namespace Ttid
{
    public sealed class TtidException : Exception
    {
        public TtidException(string message) : base(message) { }
    }

    public sealed class Ttid : IDisposable
    {
        private readonly Process _proc;
        private readonly object _lock = new object();

        public Ttid(string binary = "ttid")
        {
            var psi = new ProcessStartInfo
            {
                FileName = binary,
                RedirectStandardInput = true,
                RedirectStandardOutput = true,
                UseShellExecute = false,
            };
            psi.ArgumentList.Add("exec");
            psi.ArgumentList.Add("--loop");
            _proc = Process.Start(psi) ?? throw new InvalidOperationException("failed to start ttid");
        }

        /// <summary>Send one raw machine-protocol op (JSON string); returns the full response.</summary>
        public JsonDocument Request(string opJson)
        {
            lock (_lock) // ponytail: one call in flight; drop the lock only if you pipeline
            {
                if (_proc.HasExited) throw new InvalidOperationException("ttid process has exited");
                _proc.StandardInput.Write(opJson.TrimEnd());
                _proc.StandardInput.Write('\n');
                _proc.StandardInput.Flush();
                string? line = _proc.StandardOutput.ReadLine();
                if (line == null) throw new InvalidOperationException("ttid closed the stream");
                return JsonDocument.Parse(line);
            }
        }

        // Send a fully-formed op JSON and return `result`, throwing on failure.
        private JsonElement Op(string opJson)
        {
            using JsonDocument doc = Request(opJson);
            JsonElement root = doc.RootElement;
            if (!root.GetProperty("ok").GetBoolean())
            {
                string msg = root.TryGetProperty("error", out var e) &&
                             e.TryGetProperty("message", out var m)
                    ? m.GetString() ?? "ttid error"
                    : "ttid error";
                throw new TtidException(msg);
            }
            return root.TryGetProperty("result", out var r) ? r.Clone() : default;
        }

        private static string J(string value) => JsonSerializer.Serialize(value);

        /// <summary>Generate a new TTID, or advance <paramref name="id"/> (del=true to tombstone).</summary>
        public JsonElement Generate(string? id = null, bool del = false)
        {
            string op = "{\"op\":\"generate\"";
            if (id != null) op += $",\"id\":{J(id)}";
            if (del) op += ",\"delete\":true";
            return Op(op + "}");
        }

        public JsonElement DecodeTime(string id) =>
            Op($"{{\"op\":\"decodeTime\",\"id\":{J(id)}}}");

        public JsonElement IsTTID(string id) =>
            Op($"{{\"op\":\"isTTID\",\"id\":{J(id)}}}");

        public JsonElement IsUUID(string id) =>
            Op($"{{\"op\":\"isUUID\",\"id\":{J(id)}}}");

        public void Dispose()
        {
            if (!_proc.HasExited)
            {
                _proc.StandardInput.Close(); // EOF ends the loop
                _proc.WaitForExit(30_000);
            }
            _proc.Dispose();
        }
    }
}
