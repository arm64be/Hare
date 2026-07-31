#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "pathname"

ROOT = Pathname.new(__dir__).join("../..").expand_path
OUTPUT = ROOT.join("generated/hare/builtins-inventory.tsv")
HEADER = %w[id kind source symbol line sha256 strategy].join("\t")

Row = Data.define(:kind, :source, :symbol, :line, :sha256, :strategy)

def relative(path)
  Pathname.new(path).relative_path_from(ROOT).to_s.tr("\\", "/")
end

def digest(path)
  Digest::SHA256.file(path).hexdigest
end

def source_files(*patterns)
  patterns.flat_map { |pattern| Dir.glob(ROOT.join(pattern).to_s) }
    .select { |path| File.file?(path) }
    .uniq
    .sort
end

def tracked_files(*paths)
  output = IO.popen(
    ["git", "-C", ROOT.to_s, "ls-files", "-z", "--", *paths],
    err: [:child, :out],
  ) { |io| io.read }
  abort("git ls-files failed") unless $?.success?
  output.split("\0").reject(&:empty?).map { |path| ROOT.join(path).to_s }.sort
end

rows = []
module_files = source_files(
  "src/js/bun/**/*.{js,ts}",
  "src/js/node/**/*.{js,ts}",
  "src/js/thirdparty/**/*.{js,ts}",
  "src/js/internal/**/*.{js,ts}",
  "src/js/internal-for-testing.ts",
).reject { |path| path.end_with?(".d.ts") }

module_files.each do |path|
  rows << Row.new(
    kind: "internal_module",
    source: relative(path),
    symbol: relative(path).delete_prefix("src/js/").sub(/\.(?:js|ts)\z/, ""),
    line: 1,
    sha256: digest(path),
    strategy: "generic_native",
  )
end

native_header = ROOT.join("src/jsc/modules/_NativeModule.h")
native_header.read.scan(/macro\("([^"]+)"_s,\s*([^\)\s]+)\)/).uniq.sort.each do |specifier, enum_name|
  rows << Row.new(
    kind: "native_module",
    source: relative(native_header),
    symbol: "#{specifier}=#{enum_name}",
    line: native_header.each_line.find_index { |line| line.include?("macro(\"#{specifier}\"") } + 1,
    sha256: digest(native_header),
    strategy: "runtime_capability",
  )
end

builtin_files = source_files("src/js/builtins/*.ts")
builtin_files.each do |path|
  File.foreach(path).with_index(1) do |line, line_number|
    match = line.match(/^export\s+(?:async\s+)?function\s+([A-Za-z0-9_]+)/)
    next unless match

    rows << Row.new(
      kind: "builtin_function",
      source: relative(path),
      symbol: match[1],
      line: line_number,
      sha256: digest(path),
      strategy: "generic_native",
    )
  end
end

runtime_files = tracked_files("src/runtime", "src/jsc/modules")
runtime_files.each do |path|
  rows << Row.new(
    kind: "runtime_source",
    source: relative(path),
    symbol: File.basename(path),
    line: 1,
    sha256: digest(path),
    strategy: "native_implementation",
  )
end

(module_files + builtin_files).uniq.sort.each do |path|
  contents = File.binread(path)
  contents.to_enum(:scan, /\$(cpp|bindgenFn|newCppFunction)\b/).each do
    match = Regexp.last_match
    tail = contents.byteslice(match.begin(0), 2_048)
    arguments = tail.scan(/"([^"]+)"/).flatten.first(2)
    symbol = arguments.empty? ? "$#{match[1]}" : "$#{match[1]}:#{arguments.join("::")}"
    rows << Row.new(
      kind: "native_bridge",
      source: relative(path),
      symbol: symbol,
      line: contents.byteslice(0, match.begin(0)).count("\n") + 1,
      sha256: digest(path),
      strategy: "runtime_capability",
    )
  end
end

source_files("src/js/eval/**/*.{js,ts}").each do |path|
  rows << Row.new(
    kind: "build_time_eval",
    source: relative(path),
    symbol: File.basename(path, File.extname(path)),
    line: 1,
    sha256: digest(path),
    strategy: "build_time_only",
  )
end

prefixes = {
  "internal_module" => "M",
  "native_module" => "N",
  "builtin_function" => "F",
  "runtime_source" => "R",
  "native_bridge" => "B",
  "build_time_eval" => "E",
}
counters = Hash.new(0)
body = rows.sort_by { |row| [row.kind, row.source, row.line, row.symbol] }.map do |row|
  id = format("%s%04d", prefixes.fetch(row.kind), counters[row.kind])
  counters[row.kind] += 1
  [id, row.kind, row.source, row.symbol, row.line, row.sha256, row.strategy].join("\t")
end
generated = ([HEADER] + body).join("\n") + "\n"

if ARGV == ["--check"]
  abort("#{relative(OUTPUT)} is stale; regenerate it") unless OUTPUT.exist? && OUTPUT.read == generated
  puts counters.sort.map { |kind, count| "#{kind}=#{count}" }.join(" ")
elsif ARGV.empty?
  OUTPUT.dirname.mkpath
  OUTPUT.write(generated)
  puts "wrote #{relative(OUTPUT)} (#{rows.length} rows)"
else
  abort("usage: #{File.basename($PROGRAM_NAME)} [--check]")
end
