# Exercises clients/ruby/ttid.rb against the binary in TTID_BIN.
require "json"
require_relative "../../../clients/ruby/ttid"

FIXED = "4SQ1NZT5HC0"
UPDATED = "4SQ1NZT5HC0-4SQ1NZT5P1S"
DELETED = "4SQ1NZT5HC0-4SQ1NZT5P1S-4SQ1NZT5WRK"
out = []
t = TTID.open(ENV.fetch("TTID_BIN"))
out << ["generate", t.generate]
out << ["update", t.generate(FIXED)]
out << ["delete", t.generate(UPDATED, delete: true)]
out << ["decode", t.decode_time(DELETED)]
out << ["isTTID", t.is_ttid(FIXED)]
out << ["isTTID-bad", t.is_ttid("nope")]
out << ["isUUID", t.is_uuid("3f2504e0-4f89-41d3-9a0c-0305e82c3301")]
out << ["isUUID-bad", t.is_uuid("nope")]
out << ["canonical", t.canonicalize(FIXED.downcase)]
begin
  t.generate(DELETED)
  out << ["error", "NO ERROR RAISED"]
rescue TTID::Error => e
  out << ["error", e.message]
end
t.close
out.each { |name, value| puts "#{name}=#{JSON.generate(value.is_a?(Hash) ? value.sort.to_h : value)}" }
