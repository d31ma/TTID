# TTID client — drives the `ttid` binary's persistent NDJSON loop.
#
# Stdlib only (open3, json). Requires the `ttid` binary on PATH or an explicit
# path. One long-lived subprocess.
#
#   require_relative "ttid"
#
#   TTID.open do |t|
#     id      = t.generate            # new id
#     updated = t.generate(id)        # advance it
#     times   = t.decode_time(updated)
#     valid   = t.is_ttid(id)
#     uuid    = t.is_uuid("...")
#   end
#
# Each method builds the request and returns the op's `result` (raising
# TTID::Error on failure). Method names follow Ruby's snake_case. `request(op)`
# is a raw escape hatch returning the full response Hash.

require "open3"
require "json"

class TTID
  class Error < StandardError; end

  def self.open(binary = "ttid")
    t = new(binary)
    return t unless block_given?
    begin
      yield t
    ensure
      t.close
    end
  end

  def initialize(binary = "ttid")
    @stdin, @stdout, @wait = Open3.popen2(binary, "exec", "--loop")
    @mutex = Mutex.new
  end

  # Send one raw machine-protocol op; return the full response Hash.
  def request(op)
    line = JSON.generate(op)
    reply = @mutex.synchronize do # ponytail: one call in flight; drop the lock only if you pipeline
      raise Error, "ttid process has exited" unless @wait.alive?
      @stdin.puts(line)
      @stdout.gets
    end
    raise Error, "ttid closed the stream (stderr may have details)" if reply.nil?
    JSON.parse(reply)
  end

  def generate(id = nil, delete: false)
    op("generate", id: id, delete: (delete || nil))
  end

  def decode_time(id)
    op("decodeTime", id: id)
  end

  def is_ttid(id)
    op("isTTID", id: id)
  end

  def is_uuid(id)
    op("isUUID", id: id)
  end

  def close
    return unless @wait.alive?
    @stdin.close
    @wait.join
  end

  private

  def op(name, **fields)
    payload = { "op" => name }
    fields.each { |k, v| payload[k.to_s] = v unless v.nil? }
    response = request(payload)
    raise Error, (response.dig("error", "message") || "ttid error") unless response["ok"]
    response["result"]
  end
end
