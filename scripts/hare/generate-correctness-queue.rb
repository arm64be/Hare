#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "pathname"

ROOT = Pathname(__dir__).join("../..").expand_path
OPCODES = ROOT.join("OPCODES.tsv")
CASES = ROOT.join("test/hare/instructions/cases.json")
OUTPUT = ROOT.join("test/hare/instructions/OPCODE_QUEUE.tsv")

check = ARGV.delete("--check")
require_complete = ARGV.delete("--require-complete")
abort "usage: #{$PROGRAM_NAME} [--check] [--require-complete]" unless ARGV.empty?

opcode_lines = OPCODES.readlines(chomp: true)
headers = opcode_lines.shift.split("\t", -1)
rows = opcode_lines.map do |line|
  headers.zip(line.split("\t", -1)).to_h
end.select do |row|
  row.fetch("classification") == "semantic"
end
by_name = rows.to_h { |row| [row.fetch("opcode"), row] }
cases = JSON.parse(CASES.read)

case_ids = {}
claims = Hash.new { |hash, key| hash[key] = [] }
verified_claims = Hash.new { |hash, key| hash[key] = [] }
cases.each do |test_case|
  id = test_case.fetch("id")
  abort "duplicate Hare correctness case #{id}" if case_ids.key?(id)
  case_ids[id] = true
  abort "empty Hare correctness source for #{id}" if test_case.fetch("source").strip.empty?
  abort "invalid lowering route for #{id}" unless %w[imported source-transition].include?(test_case.fetch("lowering"))

  test_case.fetch("opcodes").each do |opcode|
    by_name[opcode] or abort "#{id} claims unknown or cache-only opcode #{opcode}"
    claims[opcode] << id
  end
  test_case.fetch("verified_opcodes", []).each do |opcode|
    by_name[opcode] or abort "#{id} verifies unknown or cache-only opcode #{opcode}"
    abort "#{id} verifies #{opcode} without exercising it" unless test_case.fetch("opcodes").include?(opcode)
    verified_claims[opcode] << id
  end
end

queue_rows = [%w[opcode_id opcode family owner status case_ids verified_case_ids]]
rows.sort_by { |row| Integer(row.fetch("opcode_id")) }.each do |row|
  opcode = row.fetch("opcode")
  ids = claims.fetch(opcode, []).sort
  verified_ids = verified_claims.fetch(opcode, []).sort
  queue_rows << [
    row.fetch("opcode_id"),
    opcode,
    row.fetch("family"),
    row.fetch("owner_task"),
    verified_ids.empty? ? (ids.empty? ? "uncovered" : "exercised") : "verified",
    ids.empty? ? "-" : ids.join(","),
    verified_ids.empty? ? "-" : verified_ids.join(","),
  ]
end
output = queue_rows.map { |row| row.join("\t") }.join("\n") << "\n"

if check
  abort "#{OUTPUT.relative_path_from(ROOT)} is missing; run #{$PROGRAM_NAME}" unless OUTPUT.file?
  abort "#{OUTPUT.relative_path_from(ROOT)} is stale; run #{$PROGRAM_NAME}" unless OUTPUT.read == output
else
  OUTPUT.dirname.mkpath
  OUTPUT.write(output)
end

exercised = rows.count { |row| claims.key?(row.fetch("opcode")) }
verified = rows.count { |row| verified_claims.key?(row.fetch("opcode")) }
warn "Hare instruction queue: #{exercised}/#{rows.length} exercised, #{verified}/#{rows.length} verified"
abort "Hare Tier 1 instruction queue is incomplete" if require_complete && verified != rows.length
