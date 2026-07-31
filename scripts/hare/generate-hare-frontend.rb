#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "optparse"

ROOT = File.expand_path("../..", __dir__)
MANIFEST = File.join(ROOT, "generated", "hare", "jsc-extraction-manifest.json")
OPCODES = File.join(ROOT, "OPCODES.tsv")
OUTPUT = File.join(ROOT, "generated", "hare")
SDK_COMPATIBILITY_HEADERS = {
  File.join(ROOT, "src", "jsc", "bindings", "BytecodeGeneratorBase.h") =>
    File.join(ROOT, "tmp", "hare-webkit", "Source", "JavaScriptCore", "bytecompiler", "BytecodeGeneratorBase.h"),
  File.join(ROOT, "src", "jsc", "bindings", "ProfileTypeBytecodeFlag.h") =>
    File.join(ROOT, "tmp", "hare-webkit", "Source", "JavaScriptCore", "bytecompiler", "ProfileTypeBytecodeFlag.h"),
}.freeze

OWNERS = {
  "H012" => ["control", "Control"],
  "H013" => ["numeric", "Numeric"],
  "H014" => ["object", "Object"],
  "H015" => ["function", "Function"],
  "H016" => ["exception", "Exception"],
  "H017" => ["module", "Module"],
}.freeze

ROLE_CODES = {
  "value_use" => [0, "ValueUse"],
  "value_def" => [1, "ValueDefinition"],
  "value_use_def" => [2, "ValueUseDefinition"],
  "register_range_use" => [3, "RegisterRangeUse"],
  "encoded_constant_or_register_reference" => [4, "ConstantOrRegister"],
  "control_target" => [5, "ControlTarget"],
  "argument_count" => [6, "ArgumentCount"],
  "argument_index" => [7, "ArgumentIndex"],
  "argument_range_base" => [8, "ArgumentRangeBase"],
  "identifier_index" => [9, "IdentifierIndex"],
  "function_table_index" => [10, "FunctionIndex"],
  "switch_table_index" => [11, "SwitchTableIndex"],
  "bit_vector_index" => [12, "BitVectorIndex"],
  "frame_slot_base" => [13, "FrameSlotBase"],
  "element_or_field_index" => [14, "ElementOrFieldIndex"],
  "lexical_feature_flags" => [15, "LexicalFeatureFlags"],
  "property_attributes" => [16, "PropertyAttributes"],
  "structure_flags" => [17, "StructureFlags"],
  "scope_depth" => [18, "ScopeDepth"],
  "scope_slot_index" => [19, "ScopeSlotIndex"],
  "symbol_table_or_scope_depth" => [20, "SymbolTableOrScopeDepth"],
  "resume_point" => [21, "ResumePoint"],
  "mode_or_flags" => [22, "ModeOrFlags"],
  "boolean_control" => [23, "BooleanControl"],
  "count" => [24, "Count"],
}.freeze

EFFECT_BITS = {
  "heap_read" => 1 << 0,
  "heap_write" => 1 << 1,
  "may_allocate" => 1 << 2,
  "may_throw" => 1 << 3,
  "throw" => 1 << 3,
  "may_call_user" => 1 << 4,
  "suspend" => 1 << 5,
  "safepoint" => 1 << 8,
  "reads_frame" => 1 << 12,
  "writes_frame" => 1 << 13,
  "realm_access" => 1 << 14,
  "scope_access" => 1 << 15,
  "control_flow" => 1 << 16,
  "trap" => 1 << 17,
  "interruption_check" => 1 << 18,
  "exception_state_read" => 1 << 19,
}.freeze

def fail!(message)
  warn "hare frontend generator: #{message}"
  exit 1
end

def generated_field(record)
  definition = record.dig("source", "generated_definition")
  match = definition&.match(/::(m_[A-Za-z0-9_]+)\z/)
  fail!("cannot derive generated field for #{record.fetch('id')}") unless match
  match[1]
end

def rust_value_kind(type)
  case type
  when "VirtualRegister", "BoundLabel", "int"
    "Signed"
  when "bool"
    "Boolean"
  else
    "Unsigned"
  end
end

def cpp_emit(record, decoded)
  id = record.fetch("id")
  fail!("non-ASCII manifest ID #{id.inspect}") unless id.ascii_only? && !id.include?('"')
  role = ROLE_CODES.fetch(record.fetch("field_role")) { fail!("unknown role for #{id}") }.first
  field = "#{decoded}.#{generated_field(record)}"
  type = record.dig("source", "type")
  call = case type
  when "VirtualRegister"
    "emitSignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, static_cast<int64_t>(#{field}.offset()))"
  when "BoundLabel"
    target = "#{field}.target() ? #{field}.target() : block.outOfLineJumpOffset(instructionRef)"
    "emitSignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, static_cast<int64_t>(#{target}))"
  when "int"
    "emitSignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, static_cast<int64_t>(#{field}))"
  when "bool"
    "emitBooleanOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, #{field})"
  when "ECMAMode", "PrivateFieldPutKind"
    "emitUnsignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, #{field}.value())"
  when "GetPutInfo"
    "emitUnsignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, #{field}.operand())"
  when "SymbolTableOrScopeDepth"
    "emitUnsignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, #{field}.raw())"
  when "PutByIdFlags"
    value = "static_cast<uint64_t>(#{field}.isDirect()) | (static_cast<uint64_t>(#{field}.ecmaMode().value()) << 1)"
    "emitUnsignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, #{value})"
  when "unsigned", "ErrorTypeWithExtension", "JSType", "ResolveType"
    "emitUnsignedOperand(visitorContext, \"#{id}\", #{id.bytesize}, #{role}, static_cast<uint64_t>(#{field}))"
  else
    fail!("unsupported operand type #{type.inspect} for #{id}")
  end
  "        if (!#{call})\n            return false;"
end

def cpp_cases(rows, operands_by_opcode)
  rows.sort_by { |row| Integer(row.fetch("opcode_id")) }.map do |row|
    opcode = row.fetch("opcode")
    records = operands_by_opcode.fetch(opcode, []).sort_by { |record| record.fetch("operand_index") }
    lines = ["    case JSC::#{opcode}: {"]
    unless records.empty?
      lines << "        auto decoded = instruction->as<JSC::#{row.fetch('generated_type')}>();"
      records.each { |record| lines << cpp_emit(record, "decoded") }
    end
    lines << "        return true;"
    lines << "    }"
    lines.join("\n")
  end.join("\n") + "\n"
end

def rust_instruction_records(records)
  records.map do |record|
    kind = {
      "opcode_checkpoint" => "Checkpoint",
      "opcode_temporary" => "Temporary",
      "derived_operand" => "DerivedOperand",
      "metadata_field" => "MetadataField",
      "metadata_reference" => "MetadataReference",
    }.fetch(record.fetch("kind"))
    <<~RUST.chomp
                crate::InstructionRecordDescriptor {
                    manifest_id: #{JSON.generate(record.fetch('id'))},
                    kind: crate::InstructionRecordKind::#{kind},
                    semantic: #{record.fetch('classification') == 'semantic'},
                    representation: #{JSON.generate(record.fetch('representation'))},
                    destination: #{record['destination'] ? "Some(#{JSON.generate(record['destination'])})" : 'None'},
                    exclusion_reason: #{record['exclusion_reason'] ? "Some(#{JSON.generate(record['exclusion_reason'])})" : 'None'},
                }
    RUST
  end.join(",\n")
end

def rust_excluded_operands(records)
  records.map do |record|
    name = record.dig("source", "symbol").split(".").last
    <<~RUST.chomp
                crate::ExcludedOperandDescriptor {
                    manifest_id: #{JSON.generate(record.fetch('id'))},
                    name: #{JSON.generate(name)},
                    source_type: #{JSON.generate(record.dig('source', 'type'))},
                    exclusion_reason: #{JSON.generate(record.fetch('exclusion_reason'))},
                }
    RUST
  end.join(",\n")
end

def rust_inventory(rows, operands_by_opcode, excluded_operands_by_opcode, records_by_opcode, family)
  entries = rows.sort_by { |row| Integer(row.fetch("opcode_id")) }.map do |row|
    operands = operands_by_opcode.fetch(row.fetch("opcode"), []).sort_by { |record| record.fetch("operand_index") }
    excluded_operands = excluded_operands_by_opcode.fetch(row.fetch("opcode"), []).sort_by { |record| record.fetch("operand_index") }
    instruction_records = records_by_opcode.fetch(row.fetch("opcode"), []).sort_by { |record| record.fetch("id") }
    operand_text = operands.map do |record|
      role = ROLE_CODES.fetch(record.fetch("field_role"))[1]
      type = record.dig("source", "type")
      name = record.dig("source", "symbol").split(".").last
      <<~RUST.chomp
                crate::OperandDescriptor {
                    manifest_id: "#{record.fetch('id')}",
                    name: "#{name}",
                    role: hare_ir::OperandRole::#{role},
                    value_kind: crate::ImportedValueKind::#{rust_value_kind(type)},
                    source_type: "#{type}",
                    optional: #{record.fetch('optional_in_dsl')},
                }
      RUST
    end.join(",\n")
    effects = row.fetch("effect_ceiling").split(",").reject(&:empty?).reduce(0) do |bits, effect|
      bits | EFFECT_BITS.fetch(effect) { fail!("unknown effect #{effect} on #{row.fetch('opcode')}") }
    end
    <<~RUST
        crate::OpcodeDescriptor {
            opcode_id: #{row.fetch('opcode_id')},
            opcode: "#{row.fetch('opcode')}",
            generated_type: "#{row.fetch('generated_type')}",
            owner: "#{row.fetch('owner_task')}",
            family: crate::OpcodeFamily::#{family},
            effects: hare_ir::EffectSet::from_bits(0x#{effects.to_s(16)}),
            operands: &[
      #{operand_text}
            ],
            excluded_operands: &[
      #{rust_excluded_operands(excluded_operands)}
            ],
            instruction_records: &[
      #{rust_instruction_records(instruction_records)}
            ],
        }
    RUST
  end.join(",\n")
  <<~RUST
    // @generated by scripts/hare/generate-hare-frontend.rb; do not edit.
    pub const OPCODES: &[crate::OpcodeDescriptor] = &[
    #{entries}
    ];
  RUST
end

def rust_cache_inventory(rows, excluded_operands_by_opcode, records_by_opcode)
  entries = rows.sort_by { |row| Integer(row.fetch("opcode_id")) }.map do |row|
    excluded_operands = excluded_operands_by_opcode.fetch(row.fetch("opcode"), []).sort_by { |record| record.fetch("operand_index") }
    instruction_records = records_by_opcode.fetch(row.fetch("opcode"), []).sort_by { |record| record.fetch("id") }
    <<~RUST.chomp
        crate::CacheOpcodeDescriptor {
            opcode_id: #{row.fetch('opcode_id')},
            opcode: "#{row.fetch('opcode')}",
            generated_type: "#{row.fetch('generated_type')}",
            exclusion: "#{row.fetch('lowering_status')}",
            excluded_operands: &[
      #{rust_excluded_operands(excluded_operands)}
            ],
            instruction_records: &[
      #{rust_instruction_records(instruction_records)}
            ],
        }
    RUST
  end.join(",\n")
  <<~RUST
    // @generated by scripts/hare/generate-hare-frontend.rb; do not edit.
    pub const OPCODES: &[crate::CacheOpcodeDescriptor] = &[
    #{entries}
    ];
  RUST
end

options = { check: false }
OptionParser.new do |parser|
  parser.on("--check", "Fail if generated output differs") { options[:check] = true }
end.parse!

manifest = JSON.parse(File.read(MANIFEST))
fail!("unexpected manifest schema") unless manifest.fetch("schema_version") == 2
SDK_COMPATIBILITY_HEADERS.each do |local, pinned|
  fail!("missing pinned SDK compatibility header: #{pinned}") unless File.file?(pinned)
  fail!("SDK compatibility header drift: #{local}") unless File.binread(local) == File.binread(pinned)
end
opcode_lines = File.readlines(OPCODES, chomp: true)
headers = opcode_lines.shift.split("\t", -1)
rows = opcode_lines.map do |line|
  cells = line.split("\t", -1)
  fail!("malformed OPCODES.tsv row") unless cells.length == headers.length
  headers.zip(cells).to_h
end
application_rows = rows.select { |row| row.fetch("section") == "Bytecode" }
semantic_rows = application_rows.select { |row| row.fetch("classification") == "semantic" }
cache_rows = application_rows.select { |row| row.fetch("classification") == "cache_only" }
fail!("expected 194 application opcodes") unless application_rows.length == 194
fail!("expected 183 semantic application opcodes") unless semantic_rows.length == 183

semantic_operands = manifest.fetch("records").select do |record|
  record["kind"] == "operand" && record["classification"] == "semantic" && record["section"] == "Bytecode"
end
operands_by_opcode = semantic_operands.group_by { |record| record.fetch("opcode") }
excluded_operands = manifest.fetch("records").select do |record|
  record["kind"] == "operand" && record["classification"] == "cache_only" && record["section"] == "Bytecode"
end
excluded_operands_by_opcode = excluded_operands.group_by { |record| record.fetch("opcode") }
instruction_records = manifest.fetch("records").select do |record|
  %w[opcode_checkpoint opcode_temporary derived_operand metadata_field metadata_reference].include?(record["kind"])
end
records_by_opcode = instruction_records.group_by { |record| record.fetch("opcode") }

semantic_rows.each do |row|
  records = operands_by_opcode.fetch(row.fetch("opcode"), [])
  expected = Integer(row.fetch("semantic_operands"))
  fail!("operand count drift for #{row.fetch('opcode')}") unless records.length == expected
  records.each do |record|
    fail!("owner drift for #{record.fetch('id')}") unless record.fetch("owner_task") == row.fetch("owner_task")
  end
end
fail!("semantic operand coverage drift") unless semantic_operands.length == 529
fail!("cache-only operand coverage drift") unless excluded_operands.length == 80
fail!("instruction auxiliary-record coverage drift") unless instruction_records.length == 183

outputs = {}
OWNERS.each do |owner, (directory, family)|
  owned_rows = semantic_rows.select { |row| row.fetch("owner_task") == owner }
  fail!("no rows for #{owner}") if owned_rows.empty?
  owned_operands = owned_rows.sum { |row| operands_by_opcode.fetch(row.fetch("opcode"), []).length }
  output_dir = File.join(OUTPUT, directory)
  outputs[File.join(output_dir, "visitor.inc")] = cpp_cases(owned_rows, operands_by_opcode)
  outputs[File.join(output_dir, "inventory.rs")] = rust_inventory(
    owned_rows,
    operands_by_opcode,
    excluded_operands_by_opcode,
    records_by_opcode,
    family,
  )
  outputs[File.join(output_dir, "coverage.json")] = JSON.pretty_generate(
    "schema_version" => 1,
    "owner_task" => owner,
    "family" => directory,
    "opcode_count" => owned_rows.length,
    "semantic_operand_count" => owned_operands,
    "cache_only_operand_count" => owned_rows.sum { |row| excluded_operands_by_opcode.fetch(row.fetch("opcode"), []).length },
    "instruction_record_count" => owned_rows.sum { |row| records_by_opcode.fetch(row.fetch("opcode"), []).length },
    "opcode_ids" => owned_rows.map { |row| Integer(row.fetch("opcode_id")) }.sort,
    "manifest_sha256" => Digest::SHA256.file(MANIFEST).hexdigest,
    "opcodes_sha256" => Digest::SHA256.file(OPCODES).hexdigest,
  ) + "\n"
end

cache_dir = File.join(OUTPUT, "cache")
outputs[File.join(cache_dir, "visitor.inc")] = cpp_cases(cache_rows, operands_by_opcode)
outputs[File.join(cache_dir, "inventory.rs")] = rust_cache_inventory(
  cache_rows,
  excluded_operands_by_opcode,
  records_by_opcode,
)
outputs[File.join(cache_dir, "coverage.json")] = JSON.pretty_generate(
  "schema_version" => 1,
  "owner_task" => "H006",
  "family" => "encoding_or_dispatch",
  "opcode_count" => cache_rows.length,
  "semantic_operand_count" => 0,
  "cache_only_operand_count" => cache_rows.sum { |row| excluded_operands_by_opcode.fetch(row.fetch("opcode"), []).length },
  "instruction_record_count" => cache_rows.sum { |row| records_by_opcode.fetch(row.fetch("opcode"), []).length },
  "opcode_ids" => cache_rows.map { |row| Integer(row.fetch("opcode_id")) }.sort,
  "manifest_sha256" => Digest::SHA256.file(MANIFEST).hexdigest,
  "opcodes_sha256" => Digest::SHA256.file(OPCODES).hexdigest,
) + "\n"

outputs.each do |path, content|
  if options[:check]
    fail!("generated output differs: #{path}") unless File.file?(path) && File.binread(path) == content
  else
    FileUtils.mkdir_p(File.dirname(path))
    File.binwrite(path, content)
  end
end

puts JSON.generate(
  "mode" => options[:check] ? "check" : "write",
  "application_opcodes" => application_rows.length,
  "semantic_opcodes" => semantic_rows.length,
  "cache_only_opcodes" => cache_rows.length,
  "semantic_operands" => semantic_operands.length,
  "cache_only_operands" => excluded_operands.length,
  "instruction_records" => instruction_records.length,
  "outputs" => outputs.length,
)
