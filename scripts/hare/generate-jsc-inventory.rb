#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "open3"
require "optparse"

WEBKIT_REVISION = "34c01d13391e00c06862a3d2c5b7fff350ac87e0"
BUN_REVISION = "bbe3f6a2629adf808adbd0da199ae8c94a3c0d47"
SCHEMA_VERSION = 2

REPOSITORY_ROOT = File.expand_path("../..", __dir__)
DEFAULT_WEBKIT_ROOT = File.join(REPOSITORY_ROOT, "tmp", "hare-webkit")
DEFAULT_TSV = File.join(REPOSITORY_ROOT, "OPCODES.tsv")
DEFAULT_MANIFEST = File.join(REPOSITORY_ROOT, "generated", "hare", "jsc-extraction-manifest.json")

SOURCE_PATHS = [
  "Source/JavaScriptCore/bytecode/BytecodeList.rb",
  "Source/JavaScriptCore/bytecode/BytecodeUseDef.cpp",
  "Source/JavaScriptCore/generator/Argument.rb",
  "Source/JavaScriptCore/generator/DSL.rb",
  "Source/JavaScriptCore/generator/Metadata.rb",
  "Source/JavaScriptCore/generator/Opcode.rb",
  "Source/JavaScriptCore/generator/OpcodeGroup.rb",
  "Source/JavaScriptCore/generator/Section.rb",
  "Source/JavaScriptCore/generator/Template.rb",
  "Source/JavaScriptCore/generator/Type.rb",
  "Source/JavaScriptCore/bytecode/Fits.h",
  "Source/JavaScriptCore/bytecode/Instruction.h",
  "Source/JavaScriptCore/bytecode/OpcodeSize.h",
  "Source/JavaScriptCore/bytecode/HandlerInfo.h",
  "Source/JavaScriptCore/bytecode/ExpressionInfo.h",
  "Source/JavaScriptCore/bytecode/UnlinkedCodeBlock.h",
  "Source/JavaScriptCore/bytecode/UnlinkedProgramCodeBlock.h",
  "Source/JavaScriptCore/bytecode/UnlinkedModuleProgramCodeBlock.h",
  "Source/JavaScriptCore/bytecode/UnlinkedEvalCodeBlock.h",
  "Source/JavaScriptCore/bytecode/UnlinkedFunctionCodeBlock.h",
  "Source/JavaScriptCore/bytecode/UnlinkedFunctionExecutable.h",
  "Source/JavaScriptCore/bytecode/UnlinkedCodeBlockGenerator.cpp",
  "Source/JavaScriptCore/runtime/CachedTypes.cpp",
  "Source/JavaScriptCore/runtime/CodeCache.cpp",
  "Source/JavaScriptCore/runtime/JSCJSValue.h",
  "Source/JavaScriptCore/parser/UnlinkedSourceCode.h",
  "Source/JavaScriptCore/parser/SourceCode.h",
  "Source/JavaScriptCore/parser/SourceCodeKey.h",
  "Source/JavaScriptCore/parser/SourceProvider.h",
].freeze

CACHE_ONLY_OPCODES = %w[
  op_debug
  op_log_shadow_chicken_prologue
  op_log_shadow_chicken_tail
  op_loop_hint
  op_nop
  op_profile_control_flow
  op_profile_type
  op_super_sampler_begin
  op_super_sampler_end
  op_wide16
  op_wide32
].freeze

CACHE_ONLY_OPERAND_NAMES = %w[
  arrayProfile
  bottomProfile
  firstFree
  inlineCapacity
  operandTypes
  profileIndex
  recommendedIndexingType
  resultType
  topProfile
  valueProfile
].freeze

CACHE_ONLY_CODE_BLOCK_FIELDS = %w[
  m_age
  m_arrayProfiles
  m_binaryArithProfiles
  m_cachedIdentifierUids
  m_cachedIdentifierUidsLock
  m_exitProfile
  m_liveness
  m_llintExecuteCounter
  m_lock
  m_metadata
  m_quickDFGTierUp
  m_quickFTLTierUp
  m_unaryArithProfiles
  m_unlinkedBaselineCode
  m_valueProfiles
].freeze

CACHE_ONLY_FUNCTION_FIELDS = %w[
  m_cachedCodeBlockForCallOffset
  m_cachedCodeBlockForConstructOffset
  m_decoder
  m_isCached
  m_isGeneratedFromCache
  m_singletonHasBeenInvalidated
].freeze

CACHE_ONLY_SOURCE_PROVIDER_FIELDS = %w[
  m_id
  m_lockingCount
  m_sourceCodeDumpFilePath
  m_sourceCodeDumpLock
  m_sourceCodeDumped
  m_sourceURLStripped
].freeze

SEMANTIC_CODE_BLOCK_FIELDS = %w[
  m_codeGenerationMode
  m_codeType
  m_constantRegisters
  m_constantsSourceCodeRepresentation
  m_constructorKind
  m_derivedContextType
  m_endColumn
  m_evalContextType
  m_expressionInfo
  m_features
  m_functionDecls
  m_functionExprs
  m_hasCapturedVariables
  m_hasCheckpoints
  m_hasTailCalls
  m_identifiers
  m_instructions
  m_isArrowFunctionContext
  m_isBuiltinDefaultClassConstructor
  m_isBuiltinFunction
  m_isClassContext
  m_isConstructor
  m_jumpTargets
  m_lexicallyScopedFeatures
  m_lineCount
  m_numCalleeLocals
  m_numParameters
  m_numVars
  m_outOfLineJumpTargets
  m_parseMode
  m_rareData
  m_scopeRegister
  m_scriptMode
  m_sourceMappingURLDirective
  m_sourceURLDirective
  m_superBinding
  m_thisRegister
].freeze

SEMANTIC_FUNCTION_FIELDS = %w[
  m_constructAbility
  m_constructorKind
  m_derivedContextType
  m_ecmaName
  m_evalContextType
  m_features
  m_firstLineOffset
  m_functionMode
  m_hasCapturedVariables
  m_implementationVisibility
  m_inlineAttribute
  m_isBuiltinDefaultClassConstructor
  m_isBuiltinFunction
  m_lexicallyScopedFeatures
  m_lineCount
  m_name
  m_needsClassFieldInitializer
  m_parameterCount
  m_parametersStartOffset
  m_privateBrandRequirement
  m_rareData
  m_scriptMode
  m_sourceLength
  m_sourceParseMode
  m_startOffset
  m_superBinding
  m_unlinkedBodyEndColumn
  m_unlinkedBodyStartColumn
  m_unlinkedCodeBlockForCall
  m_unlinkedCodeBlockForConstruct
  m_unlinkedFunctionEnd
  m_unlinkedFunctionStart
].freeze

def fail!(message)
  warn "hare JSC inventory: #{message}"
  exit 1
end

def relative_source(path)
  path.sub(%r{\A/+}, "")
end

def sha256(path)
  Digest::SHA256.file(path).hexdigest
end

def line_for(path, pattern)
  File.foreach(path).with_index(1) do |line, number|
    return number if pattern.is_a?(Regexp) ? line.match?(pattern) : line.include?(pattern)
  end
  fail!("could not find #{pattern.inspect} in #{path}")
end

def source_record(path, line:, symbol:, type:, generated_definition: "not_generated:pinned_declaration")
  {
    "path" => relative_source(path),
    "line" => line,
    "symbol" => symbol,
    "type" => type,
    "generated_definition" => generated_definition,
  }
end

def semantic_config(role:, representation: "owned_scalar", encoding: "native_typed_value",
                    presence: "required", condition: "always", ownership: "copied_scalar",
                    extraction: nil, destination: nil, validation: "type and cross-record invariants hold",
                    applicability: ["program", "module", "eval", "function_call", "function_construct"])
  {
    "classification" => "semantic",
    "semantic_role" => role,
    "representation" => representation,
    "encoding" => encoding,
    "presence" => presence,
    "condition" => condition,
    "ownership" => ownership,
    "extraction" => extraction,
    "destination" => destination,
    "validation" => validation,
    "exclusion_reason" => nil,
    "applicability" => applicability,
  }
end

def cache_config(reason:, representation: "jsc_process_state", encoding: "excluded",
                 presence: "conditional", condition: "present only when JSC creates it",
                 applicability: ["build_vm_only"])
  {
    "classification" => "cache_only",
    "semantic_role" => "cache_only",
    "representation" => representation,
    "encoding" => encoding,
    "presence" => presence,
    "condition" => condition,
    "ownership" => "excluded",
    "extraction" => nil,
    "destination" => nil,
    "validation" => nil,
    "exclusion_reason" => reason,
    "applicability" => applicability,
  }
end

def record(id:, kind:, source:, config:, width_source: "native field declaration", field_role: nil, extras: {})
  {
    "id" => id,
    "kind" => kind,
    "bun_revision" => BUN_REVISION,
    "webkit_revision" => WEBKIT_REVISION,
    "classification" => config.fetch("classification"),
    "semantic_role" => config.fetch("semantic_role"),
    "presence" => config.fetch("presence"),
    "condition" => config.fetch("condition"),
    "representation" => config.fetch("representation"),
    "encoding" => config.fetch("encoding"),
    "width_source" => width_source,
    "field_role" => field_role || config.fetch("semantic_role"),
    "source" => source,
    "applicability" => config.fetch("applicability"),
    "extraction" => config.fetch("extraction"),
    "ownership" => config.fetch("ownership"),
    "destination" => config.fetch("destination"),
    "validation" => config.fetch("validation"),
    "exclusion_reason" => config.fetch("exclusion_reason"),
  }.merge(extras)
end

def camel_to_snake(name)
  name.gsub(/([a-z0-9])([A-Z])/, '\\1_\\2').downcase
end

def member_declarations(path, start_marker, end_marker)
  lines = File.readlines(path)
  start_index = lines.index { |line| line.include?(start_marker) }
  fail!("missing member-range start #{start_marker.inspect} in #{path}") unless start_index
  end_index = (start_index...lines.length).find { |index| lines[index].include?(end_marker) }
  fail!("missing member-range end #{end_marker.inspect} in #{path}") unless end_index

  members = []
  lines[start_index..end_index].each_with_index do |line, offset|
    match = line.match(/^\s*((?:const\s+)?[A-Za-z_:][A-Za-z0-9_:<>, *&.]*)\s+(m_[A-Za-z0-9_]+)\b/)
    next unless match
    members << {
      "name" => match[2],
      "type" => match[1].strip.gsub(/\s+/, " "),
      "line" => start_index + offset + 1,
    }
  end
  members.uniq { |member| member.fetch("name") }
end

def referenced_members(path, start_marker, end_marker, receiver)
  lines = File.readlines(path)
  start_index = lines.index { |line| line.include?(start_marker) }
  fail!("missing reference-range start #{start_marker.inspect} in #{path}") unless start_index
  end_index = (start_index + 1...lines.length).find { |index| lines[index].include?(end_marker) }
  fail!("missing reference-range end #{end_marker.inspect} in #{path}") unless end_index
  lines[start_index...end_index].join.scan(/#{Regexp.escape(receiver)}\.(m_[A-Za-z0-9_]+)/).flatten.uniq.sort
end

def assert_referenced_members(path, start_marker, end_marker, receiver, expected)
  actual = referenced_members(path, start_marker, end_marker, receiver)
  return if actual == expected.sort

  fail!("cached boundary drift in #{path}:#{start_marker}: new=#{(actual - expected).inspect} stale=#{(expected - actual).inspect}")
end

def member_config(member, owner:, cache_fields:, role: "execution", applicability: nil)
  name = member.fetch("name")
  if cache_fields.include?(name)
    return cache_config(reason: "JSC cache, profiling, lock, decoder, or execution-tier state has no application meaning")
  end

  bare_name = name.delete_prefix("m_")
  aggregate = member.fetch("type").match?(/Vector|Map|Set|unique_ptr|WriteBarrier|RefPtr|Identifier|String|Environment|Table|Info/)
  presence = "required"
  condition = "the containing pinned object exists"
  case owner
  when "code_block_rare"
    presence = "conditional"
    condition = "UnlinkedCodeBlock::hasRareData()"
  when "function_rare"
    presence = "conditional"
    condition = "UnlinkedFunctionExecutable::m_rareData is non-null"
  end
  if %w[m_sourceURLDirective m_sourceMappingURLDirective].include?(name)
    presence = "optional"
    condition = "the corresponding String/StringImpl is non-null"
  elsif name == "m_rareData"
    presence = "conditional"
    condition = "at least one pinned rare-data value is present"
  elsif name == "m_instructions"
    presence = "required"
    condition = "bytecode generation and finalization succeeded"
  elsif name == "m_expressionInfo"
    presence = "required"
    condition = "bytecode generation and finalization succeeded; the decoded table may be empty"
  elsif name == "m_unlinkedCodeBlockForCall"
    presence = "conditional"
    condition = "nested executable is non-constructor, or the Function specialization plan requests call"
  elsif name == "m_unlinkedCodeBlockForConstruct"
    presence = "conditional"
    condition = "nested executable is constructor with a permitted non-async construct specialization, or the Function specialization plan requests construct"
  elsif name == "m_classSource"
    presence = "optional"
    condition = "the executable represents a class and classSource() is non-null"
  elsif name == "m_parentScopeTDZVariables"
    presence = "optional"
    condition = "the function was generated under a parent TDZ environment"
  elsif name == "m_generatorOrAsyncWrapperFunctionParameterNames"
    presence = "conditional"
    condition = "the pinned generator/async wrapper parse mode supplies wrapper parameter names"
  elsif name == "m_classElementDefinitions"
    presence = "conditional"
    condition = "the function contains class fields or a static initialization block"
  elsif name == "m_parentPrivateNameEnvironment"
    presence = "optional"
    condition = "the function closes over a parent private-name environment"
  elsif owner == "unlinked_source" && name == "m_provider"
    presence = "required"
    condition = "the admitted source is non-null"
  end
  ownership = if member.fetch("type").match?(/WriteBarrier|JSCell/)
    "rooted_cell"
  elsif aggregate
    "callback_borrow"
  else
    "copied_scalar"
  end
  representation = aggregate ? "owned_declarative_record" : "owned_scalar"
  extraction = "HareJscVisitor::emit_#{owner}_#{camel_to_snake(bare_name)}"
  destination = "visitor.#{owner}.#{camel_to_snake(bare_name)}"
  config = semantic_config(
    role: role,
    representation: representation,
    presence: presence,
    condition: condition,
    ownership: ownership,
    extraction: extraction,
    destination: destination,
    validation: "copied value is type-valid and contains no JSC address after the callback",
    applicability: applicability || ["program", "module", "eval", "function_call", "function_construct"],
  )
  config
end

def flatten_metadata(fields, prefix = [])
  fields.flat_map do |name, type|
    if type.is_a?(Hash)
      flatten_metadata(type, prefix + [name])
    else
      [[(prefix + [name]).join("."), name.to_s, type.to_s]]
    end
  end
end

def argument_width(type)
  case type
  when "VirtualRegister"
    ["signed logical register or constant reference", "Fits<VirtualRegister, OpcodeSize>; narrow/wide constant-index remapping"]
  when "BoundLabel"
    ["signed bytecode target offset", "Fits<GenericBoundLabel, OpcodeSize> via saved/committed target"]
  when "GetPutInfo"
    ["packed resolve/initialization/strictness flags", "Fits<GetPutInfo, OpcodeSize>"]
  when "PutByIdFlags"
    ["packed direct/strict flags", "Fits<PutByIdFlags, OpcodeSize>"]
  when "OperandTypes"
    ["two packed ResultType hints", "Fits<OperandTypes, OpcodeSize>"]
  when "ResultType"
    ["packed result-type hint", "Fits<ResultType, OpcodeSize>"]
  when "SymbolTableOrScopeDepth"
    ["tagged symbol-table index or scope depth", "Fits<SymbolTableOrScopeDepth, OpcodeSize>"]
  when "ECMAMode"
    ["strict/sloppy mode byte", "Fits<ECMAMode, OpcodeSize>"]
  when "PrivateFieldPutKind"
    ["private-field put mode byte", "Fits<PrivateFieldPutKind, OpcodeSize>"]
  when "bool"
    ["boolean", "Fits<bool, OpcodeSize> through uint8_t"]
  when "int"
    ["signed 32-bit integer", "integral Fits<int, OpcodeSize>"]
  when "unsigned"
    ["unsigned 32-bit integer", "integral Fits<unsigned, OpcodeSize>"]
  else
    ["typed enum or scalar", "Fits<#{type}, OpcodeSize>"]
  end
end

def cache_operand?(name)
  CACHE_ONLY_OPERAND_NAMES.include?(name) || name.end_with?("ValueProfile") || name.end_with?("Profile")
end

def parse_value_flow(path, generated_type_to_opcode)
  flow = Hash.new { |hash, key| hash[key] = { "uses_at" => [], "defines_at" => [], "source_lines" => [] } }
  File.foreach(path).with_index(1) do |line, line_number|
    match = line.match(/^\s*(USES|DEFS)\((Op[A-Za-z0-9_]+),\s*([^)]*)\)/)
    next unless match
    opcode = generated_type_to_opcode[match[2]]
    fail!("BytecodeUseDef.cpp names unknown generated type #{match[2]}") unless opcode
    key = match[1] == "USES" ? "uses_at" : "defines_at"
    match[3].split(",").map(&:strip).reject(&:empty?).each do |operand|
      entry = flow[[opcode, operand]]
      entry[key] << "entry"
      entry["source_lines"] << line_number
    end
  end
  flow
end

def add_value_flow(flow, opcode, operand, kind, point, source_line)
  entry = flow[[opcode, operand]]
  entry.fetch(kind) << point
  entry.fetch("source_lines") << source_line
end

def scalar_operand_role(name, type)
  return "control_target" if type == "BoundLabel"
  return "argument_count" if name == "argc"
  return "argument_range_base" if name == "argv"
  return "frame_slot_base" if name == "stackOffset"
  return "argument_index" if %w[firstVarArg numParametersToSkip].include?(name)
  return "switch_table_index" if name == "tableIndex"
  return "function_table_index" if name == "functionDecl"
  return "bit_vector_index" if name == "bitVector"
  return "identifier_index" if %w[property var message].include?(name) && type == "unsigned"
  return "resume_point" if name == "yieldPoint"
  return "element_or_field_index" if name == "index"
  return "count" if name.match?(/count|argc/i)
  return "mode_or_flags" if type.match?(/Mode|Flags|Kind|Type|Info/)
  return "table_or_slot_index" if type == "unsigned"
  return "signed_offset_or_count" if type == "int"
  return "boolean_control" if type == "bool"

  "typed_scalar"
end

def opcode_owner(name)
  return ["H017", "module_eval"] if %w[op_call_direct_eval op_resolve_scope_for_hoisting_func_decl_in_eval].include?(name)
  return ["H016", "exception_abrupt"] if name.match?(/\Aop_(catch|throw|throw_static_error|check_traps|unreachable)\z/)
  return ["H012", "control"] if name.match?(/\Aop_(j|switch_)/) || %w[op_enter op_ret].include?(name)
  return ["H013", "numeric_coercion"] if name.match?(/\Aop_(is_|typeof|has_structure)/)
  return ["H015", "function_async_scope"] if name.match?(/(call|construct|iterator|generator|promise|func|argument|scope|lexical_environment|yield|create_rest|check_tdz|to_this)/) || %w[op_create_this op_get_parent_scope op_push_with_scope].include?(name)
  return ["H014", "object_property"] if name.match?(/(array|object|property|private|prototype|internal_field|enumerator|spread|reg_exp|by_id|by_val|in_by|getter|setter|instanceof|strcat)/) || %w[op_get_length op_to_object op_to_property_key op_to_property_key_or_number].include?(name)
  return ["H013", "numeric_coercion"] if name.match?(/\Aop_(eq|neq|stricteq|nstricteq|less|lesseq|greater|greatereq|below|beloweq|mod|pow|urshift|add|mul|div|sub|bitand|bitor|bitxor|lshift|rshift|eq_null|neq_null|to_string|to_primitive|is_|typeof|inc|dec|negate|not|identity_with_profile|has_structure|to_number|to_numeric|bitnot|unsigned)\z/)
  return ["H012", "locals"] if %w[op_mov].include?(name)

  nil
end

def opcode_effects(name, family, cache_only, value_flow)
  return [] if cache_only

  effects = ["reads_frame"]
  has_def = value_flow.any? { |(opcode, _operand), flow| opcode == name && flow.fetch("defines_at").any? }
  effects << "writes_frame" if has_def || name == "op_enter"

  if family == "control" || name == "op_ret"
    effects.concat(%w[control_flow heap_read])
    coercive_branch = name.match?(/\Aop_j(?:eq|neq|less|lesseq|greater|greatereq|nless|nlesseq|ngreater|ngreatereq)\z/)
    effects.concat(%w[may_call_user may_throw safepoint]) if coercive_branch
  end

  case family
  when "object_property"
    effects.concat(%w[heap_read heap_write may_allocate may_call_user may_throw safepoint])
  when "function_async_scope"
    effects.concat(%w[heap_read heap_write may_allocate may_call_user may_throw safepoint])
    effects << "suspend" if name.match?(/yield|async_iterator/)
  when "exception_abrupt"
    if name == "op_catch"
      effects << "exception_state_read"
    elsif name == "op_check_traps"
      effects.concat(%w[interruption_check may_throw safepoint])
    elsif name == "op_unreachable"
      effects << "trap"
    else
      effects << "throw"
    end
  when "module_eval"
    effects.concat(%w[heap_read heap_write may_allocate may_call_user may_throw realm_access safepoint scope_access])
  when "numeric_coercion"
    pure = name.match?(/\Aop_(?:identity_with_profile|is_|typeof(?:_|$)|has_structure|stricteq|nstricteq|eq_null|neq_null|not|below|beloweq)/)
    if pure
      effects << "heap_read"
    else
      effects.concat(%w[heap_read may_allocate may_call_user may_throw safepoint])
    end
  end

  effects.uniq.sort
end

def capture_opcode_locations(bytecode_path)
  original_op = DSL.method(:op)
  original_group = DSL.method(:op_group)

  DSL.define_singleton_method(:op) do |name, config = {}|
    line = caller_locations.find { |location| File.expand_path(location.path) == File.expand_path(bytecode_path) }&.lineno
    before = instance_variable_get(:@current_section).opcodes.length
    original_op.call(name, config)
    instance_variable_get(:@current_section).opcodes[before..].each { |opcode| opcode.instance_variable_set(:@hare_source_line, line) }
  end

  DSL.define_singleton_method(:op_group) do |name, opcodes, config|
    line = caller_locations.find { |location| File.expand_path(location.path) == File.expand_path(bytecode_path) }&.lineno
    before = instance_variable_get(:@current_section).opcodes.length
    original_group.call(name, opcodes, config)
    instance_variable_get(:@current_section).opcodes[before..].each { |opcode| opcode.instance_variable_set(:@hare_source_line, line) }
  end
end

def add_member_range(records, webkit_root, source_path, range_start, range_end, owner:, cache_fields:,
                     semantic_fields:, kind:, role: "execution", applicability: nil)
  absolute = File.join(webkit_root, source_path)
  members = member_declarations(absolute, range_start, range_end)
  fail!("member range #{source_path}:#{range_start} was empty") if members.empty?
  actual_fields = members.map { |member| member.fetch("name") }.sort
  classified_fields = (semantic_fields + cache_fields).uniq.sort
  unless actual_fields == classified_fields
    missing = actual_fields - classified_fields
    stale = classified_fields - actual_fields
    fail!("unclassified member drift in #{source_path}: new=#{missing.inspect} stale=#{stale.inspect}")
  end

  members.each do |member|
    config = member_config(member, owner: owner, cache_fields: cache_fields, role: role, applicability: applicability)
    records << record(
      id: "#{owner}.#{member.fetch('name').delete_prefix('m_')}",
      kind: kind,
      source: source_record(
        source_path,
        line: member.fetch("line"),
        symbol: member.fetch("name"),
        type: member.fetch("type"),
      ),
      config: config,
      field_role: role,
    )
  end
  members.map { |member| member.fetch("name") }
end

def add_synthetic_record(records, id:, kind:, source_path:, source_line:, symbol:, type:, config:,
                         width_source: "pinned semantic accessor", field_role: nil, generated_definition: "not_generated:pinned_accessor", extras: {})
  records << record(
    id: id,
    kind: kind,
    source: source_record(
      source_path,
      line: source_line,
      symbol: symbol,
      type: type,
      generated_definition: generated_definition,
    ),
    config: config,
    width_source: width_source,
    field_role: field_role,
    extras: extras,
  )
end

options = {
  webkit_root: DEFAULT_WEBKIT_ROOT,
  tsv: DEFAULT_TSV,
  manifest: DEFAULT_MANIFEST,
  check: false,
}

OptionParser.new do |parser|
  parser.banner = "Usage: generate-jsc-inventory.rb [options]"
  parser.on("--webkit-root PATH", "Pinned WebKit checkout (default: tmp/hare-webkit)") { |value| options[:webkit_root] = File.expand_path(value) }
  parser.on("--tsv PATH", "OPCODES.tsv output") { |value| options[:tsv] = File.expand_path(value) }
  parser.on("--manifest PATH", "JSON manifest output") { |value| options[:manifest] = File.expand_path(value) }
  parser.on("--check", "Fail instead of rewriting when generated files differ") { options[:check] = true }
end.parse!

webkit_root = options.fetch(:webkit_root)
fail!("missing WebKit checkout #{webkit_root}") unless File.directory?(webkit_root)

git_revision, git_status = Open3.capture2("git", "-C", webkit_root, "rev-parse", "HEAD")
fail!("cannot read WebKit revision") unless git_status.success?
fail!("expected WebKit #{WEBKIT_REVISION}, found #{git_revision.strip}") unless git_revision.strip == WEBKIT_REVISION

_source_diff, source_diff_status = Open3.capture2(
  "git", "-C", webkit_root, "diff", "--quiet", "HEAD", "--", *SOURCE_PATHS,
)
fail!("pinned WebKit source files have local modifications") unless source_diff_status.success?

SOURCE_PATHS.each do |path|
  fail!("missing pinned source #{path}") unless File.file?(File.join(webkit_root, path))
end

cached_types_check_path = File.join(webkit_root, "Source/JavaScriptCore/runtime/CachedTypes.cpp")
assert_referenced_members(
  cached_types_check_path,
  "ALWAYS_INLINE void CachedCodeBlock<CodeBlockType>::encode", "class CachedSourceCodeKey", "codeBlock",
  %w[
    m_arrayProfiles m_binaryArithProfiles m_codeGenerationMode m_codeType
    m_constantRegisters m_constantsSourceCodeRepresentation m_constructorKind
    m_derivedContextType m_endColumn m_evalContextType m_expressionInfo m_features
    m_functionDecls m_functionExprs m_hasCapturedVariables m_hasCheckpoints
    m_hasTailCalls m_identifiers m_instructions m_isArrowFunctionContext
    m_isBuiltinDefaultClassConstructor m_isBuiltinFunction m_isClassContext
    m_isConstructor m_jumpTargets m_lexicallyScopedFeatures m_lineCount m_metadata
    m_numCalleeLocals m_numParameters m_numVars m_outOfLineJumpTargets m_parseMode
    m_rareData m_scopeRegister m_scriptMode m_sourceMappingURLDirective
    m_sourceURLDirective m_superBinding m_thisRegister m_unaryArithProfiles
    m_valueProfiles
  ],
)
assert_referenced_members(
  cached_types_check_path,
  "ALWAYS_INLINE void CachedFunctionExecutable::encode", "ALWAYS_INLINE UnlinkedFunctionExecutable* CachedFunctionExecutable::decode", "executable",
  %w[
    m_constructAbility m_constructorKind m_derivedContextType m_evalContextType
    m_features m_firstLineOffset m_functionMode m_hasCapturedVariables
    m_implementationVisibility m_inlineAttribute m_isBuiltinDefaultClassConstructor
    m_isBuiltinFunction m_lexicallyScopedFeatures m_lineCount
    m_needsClassFieldInitializer m_parameterCount m_parametersStartOffset
    m_privateBrandRequirement m_rareData m_scriptMode m_sourceLength
    m_sourceParseMode m_startOffset m_superBinding m_unlinkedBodyEndColumn
    m_unlinkedBodyStartColumn m_unlinkedCodeBlockForCall
    m_unlinkedCodeBlockForConstruct m_unlinkedFunctionEnd m_unlinkedFunctionStart
  ],
)
assert_referenced_members(
  cached_types_check_path,
  "class CachedCodeBlockRareData", "UnlinkedCodeBlock::RareData* decode", "rareData",
  %w[
    m_bitVectors m_constantIdentifierSets m_exceptionHandlers
    m_needsClassFieldInitializer m_opProfileControlFlowBytecodeOffsets
    m_privateBrandRequirement m_typeProfilerInfoMap m_unlinkedStringSwitchJumpTables
    m_unlinkedSwitchJumpTables
  ],
)
assert_referenced_members(
  cached_types_check_path,
  "class CachedFunctionExecutableRareData", "UnlinkedFunctionExecutable::RareData* decode", "rareData",
  %w[
    m_classElementDefinitions m_classSource
    m_generatorOrAsyncWrapperFunctionParameterNames m_parentPrivateNameEnvironment
    m_parentScopeTDZVariables
  ],
)

generator_root = File.join(webkit_root, "Source", "JavaScriptCore", "generator")
$LOAD_PATH.unshift(generator_root)
require "DSL"
DSL.types %i[bool int unsigned uintptr_t uint8_t]

bytecode_relative = "Source/JavaScriptCore/bytecode/BytecodeList.rb"
bytecode_path = File.join(webkit_root, bytecode_relative)
capture_opcode_locations(bytecode_path)
DSL.instance_variable_get(:@context).eval(File.read(bytecode_path), bytecode_path)
fail!("BytecodeList.rb left an open section") unless DSL.instance_variable_get(:@current_section).nil?

sections = DSL.instance_variable_get(:@sections)
fail!("unexpected section set #{sections.map(&:name).inspect}") unless sections.map(&:name) == %i[Bytecode CLoopHelpers NativeHelpers CLoopReturnHelpers]

bytecode_section = sections.find { |section| section.name == :Bytecode }
generated_type_to_opcode = bytecode_section.opcodes.to_h { |opcode| [opcode.capitalized_name, opcode.name.to_s] }
use_def_relative = "Source/JavaScriptCore/bytecode/BytecodeUseDef.cpp"
use_def_path = File.join(webkit_root, use_def_relative)
value_flow = parse_value_flow(use_def_path, generated_type_to_opcode)

varargs_line = line_for(use_def_path, "case op_call_varargs")
%w[op_call_varargs op_tail_call_varargs op_construct_varargs op_super_construct_varargs].each do |opcode|
  %w[callee thisValue arguments].each { |operand| add_value_flow(value_flow, opcode, operand, "uses_at", "all_checkpoints", varargs_line) }
  add_value_flow(value_flow, opcode, "dst", "defines_at", "makeCall", varargs_line)
end

iterator_open_line = line_for(use_def_path, "case op_iterator_open")
%w[op_iterator_open op_async_iterator_open].each do |opcode|
  %w[symbolIterator iterable].each { |operand| add_value_flow(value_flow, opcode, operand, "uses_at", "symbolCall_and_later", iterator_open_line) }
  add_value_flow(value_flow, opcode, "iterator", "uses_at", "getNext_and_later", iterator_open_line)
  add_value_flow(value_flow, opcode, "iterator", "defines_at", "symbolCall", iterator_open_line)
  add_value_flow(value_flow, opcode, "next", "defines_at", "getNext", iterator_open_line)
end

iterator_next_line = line_for(use_def_path, "case op_iterator_next")
%w[iterator next].each { |operand| add_value_flow(value_flow, "op_iterator_next", operand, "uses_at", "all_checkpoints", iterator_next_line) }
add_value_flow(value_flow, "op_iterator_next", "iterable", "uses_at", "computeNext_and_later", iterator_next_line)
add_value_flow(value_flow, "op_iterator_next", "done", "defines_at", "getDone", iterator_next_line)
add_value_flow(value_flow, "op_iterator_next", "value", "defines_at", "getDone", iterator_next_line)
add_value_flow(value_flow, "op_iterator_next", "value", "defines_at", "getValue", iterator_next_line)

async_next_line = line_for(use_def_path, "case op_async_iterator_next")
%w[next iterator driver].each { |operand| add_value_flow(value_flow, "op_async_iterator_next", operand, "uses_at", "entry", async_next_line) }

call_like_line = line_for(use_def_path, "auto handleOpCallLike")
%w[op_call op_tail_call op_construct op_super_construct op_call_ignore_result op_call_direct_eval].each do |opcode|
  add_value_flow(value_flow, opcode, "callee", "uses_at", "entry", call_like_line)
end
%w[thisValue scope].each { |operand| add_value_flow(value_flow, "op_call_direct_eval", operand, "uses_at", "entry", call_like_line) }

array_range_line = line_for(use_def_path, "auto handleNewArrayLike")
%w[op_new_array op_new_array_with_spread].each do |opcode|
  add_value_flow(value_flow, opcode, "argv", "uses_at", "register_range", array_range_line)
end
strcat_line = line_for(use_def_path, "case op_strcat")
add_value_flow(value_flow, "op_strcat", "src", "uses_at", "register_range", strcat_line)

instanceof_line = line_for(use_def_path, "case op_instanceof")
add_value_flow(value_flow, "op_instanceof", "constructor", "uses_at", "getHasInstance", instanceof_line)
%w[value constructor hasInstanceOrPrototype].each { |operand| add_value_flow(value_flow, "op_instanceof", operand, "uses_at", "getPrototype", instanceof_line) }
%w[value hasInstanceOrPrototype].each { |operand| add_value_flow(value_flow, "op_instanceof", operand, "uses_at", "instanceof", instanceof_line) }
add_value_flow(value_flow, "op_instanceof", "hasInstanceOrPrototype", "defines_at", "getHasInstance", instanceof_line)
add_value_flow(value_flow, "op_instanceof", "hasInstanceOrPrototype", "defines_at", "getPrototype", instanceof_line)
add_value_flow(value_flow, "op_instanceof", "dst", "defines_at", "instanceof", instanceof_line)

records = []
opcode_rows = []

width_path = "Source/JavaScriptCore/bytecode/OpcodeSize.h"
instruction_path = "Source/JavaScriptCore/bytecode/Instruction.h"
fits_path = "Source/JavaScriptCore/bytecode/Fits.h"

[
  ["narrow", 1, "no prefix; uint8 opcode ID; one byte per operand"],
  ["wide16", 2, "op_wide16 prefix; pinned max ID keeps opcode ID at one byte; two bytes per operand"],
  ["wide32", 4, "op_wide32 prefix; pinned max ID keeps opcode ID at one byte; four bytes per operand"],
].each do |name, bytes, encoding|
  add_synthetic_record(
    records,
    id: "instruction_width.#{name}",
    kind: "operand_width_rule",
    source_path: width_path,
    source_line: line_for(File.join(webkit_root, width_path), "#{name == 'narrow' ? 'Narrow' : name == 'wide16' ? 'Wide16' : 'Wide32'} = #{bytes}"),
    symbol: "OpcodeSize::#{name == 'narrow' ? 'Narrow' : name == 'wide16' ? 'Wide16' : 'Wide32'}",
    type: "OpcodeSize",
    config: semantic_config(
      role: "validation",
      representation: "instruction_encoding_width",
      encoding: encoding,
      ownership: "copied_scalar",
      extraction: "JSInstruction::width()",
      destination: "visitor.instruction.width",
      validation: "prefix, opcode-ID width, operand width, padding, and JSInstruction::size() agree",
    ),
    width_source: "OpcodeSize.h and Instruction.h::BaseInstruction::size",
    field_role: "validation",
  )
end

sections.each do |section|
  section.opcodes.each do |opcode|
    name = opcode.name.to_s
    source_line = opcode.instance_variable_get(:@hare_source_line)
    fail!("missing source line for #{name}") unless source_line
    bytecode = section.name == :Bytecode
    cache_only = !bytecode || CACHE_ONLY_OPCODES.include?(name)
    owner_family = cache_only ? ["H006", "encoding_or_dispatch"] : opcode_owner(name)
    fail!("semantic opcode #{name} has no downstream owner/family") unless owner_family
    owner_task, family = owner_family
    effects = opcode_effects(name, family, cache_only, value_flow)
    generated_type = bytecode ? opcode.capitalized_name : nil
    generated_location = if bytecode
      "Source/JavaScriptCore/bytecode/BytecodeStructs.h::#{generated_type}"
    else
      "Source/JavaScriptCore/bytecode/Bytecodes.h::FOR_EACH_#{section.config.fetch(:macro_name_component)}_ID"
    end
    opcode_config = if cache_only
      cache_config(
        reason: bytecode ? "encoding, profiling, sampling, or debugger-only instruction has no Hare program meaning" : "interpreter/native dispatch helper is never present as a Hare application instruction",
        representation: bytecode ? "bytecode_encoding_or_tooling" : "llint_dispatch_address",
      )
    else
      semantic_config(
        role: family == "control" ? "control_flow" : "execution",
        representation: "opcode_identity",
        encoding: "pinned OpcodeID #{opcode.id}",
        ownership: "copied_scalar",
        extraction: "JSInstruction::as<#{generated_type}>()",
        destination: "visitor.instruction.opcode",
        validation: "opcode ID, decoded width, byte length, and generated struct type agree",
      )
    end

    records << record(
      id: "opcode.#{section.name}.#{name}",
      kind: bytecode ? "opcode" : "helper_opcode",
      source: source_record(
        bytecode_relative,
        line: source_line,
        symbol: name,
        type: bytecode ? "Opcode" : "HelperOpcode",
        generated_definition: generated_location,
      ),
      config: opcode_config,
      width_source: bytecode ? "Opcode::length plus JSInstruction::width()/size()" : "Bytecodes.h generated helper enumeration",
      extras: {
        "section" => section.name.to_s,
        "opcode_id" => opcode.id,
        "generated_type" => generated_type,
        "operand_word_count" => opcode.length,
        "owner_task" => owner_task,
        "family" => family,
        "effects" => effects,
      },
    )

    semantic_operands = 0
    cache_operands = 0
    operand_names = []
    (opcode.args || []).each do |argument|
      argument_name = argument.name.to_s
      operand_names << argument_name
      type = argument.instance_variable_get(:@type).to_s
      optional = argument.instance_variable_get(:@optional)
      operand_cache = cache_only || cache_operand?(argument_name)
      representation, width_rule = argument_width(type)
      flow = value_flow[[name, argument_name]]
      uses_at = flow.fetch("uses_at").uniq
      defines_at = flow.fetch("defines_at").uniq
      operand_role = if operand_cache
        "cache_hint_or_layout"
      elsif type == "VirtualRegister" && uses_at.any? && defines_at.any?
        uses_at.include?("register_range") ? "register_range_use_def" : "value_use_def"
      elsif type == "VirtualRegister" && uses_at.any?
        uses_at.include?("register_range") ? "register_range_use" : "value_use"
      elsif type == "VirtualRegister" && defines_at.any?
        "value_def"
      elsif type == "VirtualRegister"
        "encoded_constant_or_register_reference"
      else
        scalar_operand_role(argument_name, type)
      end
      if operand_cache
        cache_operands += 1
        argument_config = cache_config(
          reason: cache_only ? "the containing opcode is excluded" : "profile, representation hint, or JSC frame-layout operand does not change generic JavaScript behavior",
          representation: representation,
          encoding: "decoded through #{width_rule}",
          presence: optional ? "conditional" : "required",
          condition: optional ? "VirtualRegister::isValid()" : "the containing instruction is present",
        )
      else
        semantic_operands += 1
        argument_config = semantic_config(
          role: type == "BoundLabel" ? "control_flow" : "execution",
          representation: representation,
          encoding: "decoded through #{width_rule}",
          presence: optional ? "conditional" : "required",
          condition: optional ? "VirtualRegister::isValid()" : "the containing instruction is present",
          ownership: "copied_scalar",
          extraction: "JSInstruction::as<#{generated_type}>().m_#{argument_name}",
          destination: "visitor.instruction.operands.#{camel_to_snake(argument_name)}",
          validation: type == "VirtualRegister" ? "register is valid when required and resolves to a local, argument, or in-range constant" : "decoded value is valid for #{type} and every referenced table index/target is in range",
        )
      end
      records << record(
        id: "operand.#{name}.#{argument_name}",
        kind: "operand",
        source: source_record(
          bytecode_relative,
          line: source_line,
          symbol: "#{name}.#{argument_name}",
          type: type,
          generated_definition: bytecode ? "#{generated_location}::m_#{argument_name}" : generated_location,
        ),
        config: argument_config,
        width_source: width_rule,
        field_role: operand_role,
        extras: {
          "section" => section.name.to_s,
          "opcode" => name,
          "operand_index" => argument.index,
          "optional_in_dsl" => optional,
          "owner_task" => owner_task,
          "value_flow" => {
            "uses_at" => uses_at,
            "defines_at" => defines_at,
            "source" => use_def_relative,
            "source_lines" => flow.fetch("source_lines").uniq.sort,
          },
        },
      )
    end

    metadata_names = []
    unless opcode.metadata.empty?
      initializer_map = opcode.metadata.instance_variable_get(:@initializers) || {}
      flatten_metadata(opcode.metadata.instance_variable_get(:@fields)).each do |metadata_path, leaf_name, type|
        metadata_names << metadata_path
        initialized_from = initializer_map[leaf_name.to_sym]
        reason = if initialized_from
          "runtime metadata copy is excluded; original semantic value is imported from operand #{initialized_from}"
        else
          "mutable inline-cache, profile, watchpoint, shape, or linked-cell state is not generic program meaning"
        end
        metadata_config = cache_config(
          reason: reason,
          representation: "jsc_unlinked_metadata",
          encoding: "native #{type} field behind metadata ID",
          condition: "the #{name} metadata table entry exists",
        )
        records << record(
          id: "metadata.#{name}.#{metadata_path}",
          kind: "metadata_field",
          source: source_record(
            bytecode_relative,
            line: source_line,
            symbol: "#{name}.Metadata.#{metadata_path}",
            type: type,
            generated_definition: "#{generated_location}::Metadata::m_#{leaf_name}",
          ),
          config: metadata_config,
          width_source: "native metadata field; instruction stores a width-sized unsigned metadata ID",
          field_role: "metadata",
          extras: {
            "section" => section.name.to_s,
            "opcode" => name,
            "metadata_path" => metadata_path,
            "initialized_from_operand" => initialized_from&.to_s,
          },
        )
      end

      metadata_id_config = cache_config(
        reason: "metadata ID indexes JSC mutable runtime metadata; Hare imports no program meaning from it",
        representation: "unsigned metadata table index",
        encoding: "Fits<unsigned, OpcodeSize>",
        condition: "the opcode has metadata in BytecodeList.rb",
      )
      records << record(
        id: "metadata_reference.#{name}",
        kind: "metadata_reference",
        source: source_record(
          bytecode_relative,
          line: source_line,
          symbol: "#{name}.m_metadataID",
          type: "unsigned",
          generated_definition: "#{generated_location}::m_metadataID",
        ),
        config: metadata_id_config,
        width_source: "Fits<unsigned, OpcodeSize>",
        field_role: "metadata_reference",
        extras: { "opcode" => name },
      )
    end

    tmps = opcode.instance_variable_get(:@tmps) || {}
    tmps.each_with_index do |(tmp_name, tmp_type), index|
      config = semantic_config(
        role: "execution",
        representation: "lowering_temporary",
        encoding: "generated temporary enum index",
        ownership: "stable_id",
        extraction: "#{generated_type}::Tmps::#{tmp_name}",
        destination: "visitor.instruction.temporaries.#{camel_to_snake(tmp_name.to_s)}",
        validation: "temporary index is unique within the opcode and materialized in owned SSA before the JSC borrow ends",
      )
      records << record(
        id: "temporary.#{name}.#{tmp_name}",
        kind: "opcode_temporary",
        source: source_record(
          bytecode_relative,
          line: source_line,
          symbol: "#{name}.#{tmp_name}",
          type: tmp_type.to_s,
          generated_definition: "#{generated_location}::Tmps::#{tmp_name}",
        ),
        config: config,
        width_source: "generated enum; not instruction-stream storage",
        field_role: "temporary",
        extras: { "opcode" => name, "temporary_index" => index },
      )
    end

    (opcode.checkpoints || []).each_with_index do |checkpoint_name, index|
      config = semantic_config(
        role: "control_flow",
        representation: "stable_checkpoint_id",
        encoding: "generated uint8 checkpoint enum",
        ownership: "stable_id",
        extraction: "#{generated_type}::Checkpoints::#{checkpoint_name}",
        destination: "visitor.instruction.checkpoints.#{camel_to_snake(checkpoint_name.to_s)}",
        validation: "checkpoint order and count equal the generated checkpoint table",
      )
      records << record(
        id: "checkpoint.#{name}.#{checkpoint_name}",
        kind: "opcode_checkpoint",
        source: source_record(
          bytecode_relative,
          line: source_line,
          symbol: "#{name}.#{checkpoint_name}",
          type: "uint8_t checkpoint",
          generated_definition: "#{generated_location}::Checkpoints::#{checkpoint_name}",
        ),
        config: config,
        width_source: "generated checkpoint enum; not instruction-stream storage",
        field_role: "checkpoint",
        extras: { "opcode" => name, "checkpoint_index" => index },
      )
    end

    opcode_rows << {
      "section" => section.name.to_s,
      "opcode_id" => opcode.id,
      "opcode" => name,
      "generated_type" => generated_type || "-",
      "source_line" => source_line,
      "operand_words" => opcode.length,
      "widths" => bytecode ? "narrow,wide16,wide32" : "helper-id",
      "classification" => opcode_config.fetch("classification"),
      "semantic_role" => opcode_config.fetch("semantic_role"),
      "owner_task" => owner_task,
      "family" => family,
      "effect_ceiling" => effects.join(","),
      "operands" => operand_names.join(","),
      "semantic_operands" => semantic_operands,
      "cache_only_operands" => cache_operands,
      "metadata_fields" => metadata_names.join(","),
      "checkpoints" => (opcode.checkpoints || []).join(","),
      "temporaries" => tmps.keys.join(","),
      "lowering_status" => cache_only ? "excluded" : "inventoried",
    }
  end
end

derived_operands = [
  [
    "call_argument_range",
    %w[op_call op_tail_call op_construct op_super_construct op_call_ignore_result op_call_direct_eval],
    call_like_line,
    "CallFrame argument registers derived from m_argv and m_argc",
    "the call-like instruction is present",
    "HareJscVisitor::expand_call_argument_range",
    "visitor.instruction.derived_uses.call_arguments",
    "derived registers equal [-m_argv + thisArgumentOffset, +m_argc) and are in frame range",
  ],
  [
    "array_argument_range",
    %w[op_new_array op_new_array_with_spread],
    array_range_line,
    "descending register range derived from m_argv.offset() and m_argc",
    "the array-like instruction is present",
    "HareJscVisitor::expand_array_argument_range",
    "visitor.instruction.derived_uses.array_elements",
    "derived registers equal m_argv.offset() down through argc elements and are in frame range",
  ],
  [
    "strcat_source_range",
    %w[op_strcat],
    strcat_line,
    "descending register range derived from m_src.offset() and m_count",
    "op_strcat is present",
    "HareJscVisitor::expand_strcat_source_range",
    "visitor.instruction.derived_uses.string_parts",
    "derived registers equal m_src.offset() down through count values and are in frame range",
  ],
  [
    "async_iterator_resume_value",
    %w[op_async_iterator_next],
    async_next_line,
    "call argument index 1 derived by resumeValueOperandFor from m_stackOffset",
    "m_hasValue is true",
    "resumeValueOperandFor(OpAsyncIteratorNext)",
    "visitor.instruction.derived_uses.resume_value",
    "derived resume register is in frame range and absent exactly when m_hasValue is false",
  ],
  [
    "enter_local_definitions",
    %w[op_enter],
    line_for(use_def_path, "case op_enter"),
    "all virtualRegisterForLocal(i) for 0 <= i < numVars",
    "op_enter is present",
    "computeDefsForBytecodeIndexImpl(numVars, op_enter)",
    "visitor.instruction.derived_defs.locals",
    "the definition set contains every callee local exactly once",
  ],
]
derived_operands.each do |name, opcodes, source_line, representation, condition, extraction, destination, validation|
  opcodes.each do |opcode|
    add_synthetic_record(
      records,
      id: "derived_operand.#{opcode}.#{name}",
      kind: "derived_operand",
      source_path: use_def_relative,
      source_line: source_line,
      symbol: "#{opcode}.#{name}",
      type: "derived VirtualRegister set",
      config: semantic_config(
        role: "execution",
        representation: representation,
        encoding: "not stored as an independent BytecodeList operand",
        presence: condition == "m_hasValue is true" ? "conditional" : "required",
        condition: condition,
        ownership: "copied_scalar",
        extraction: extraction,
        destination: destination,
        validation: validation,
      ),
      width_source: "BytecodeUseDef.cpp derived-use/definition rule",
      field_role: name.end_with?("definitions") ? "value_def" : "value_use",
      generated_definition: "not_generated:pinned_derived_operand_rule",
      extras: { "opcode" => opcode },
    )
  end
end

# Live UnlinkedCodeBlock fields. Exact member ranges make a pinned-source field
# addition fail until it is classified by the policy above.
code_block_path = "Source/JavaScriptCore/bytecode/UnlinkedCodeBlock.h"
code_block_members = []
code_block_members.concat(add_member_range(
  records, webkit_root, code_block_path,
  "VirtualRegister m_thisRegister;", "FunctionExpressionVector m_functionExprs;",
  owner: "code_block", cache_fields: %w[m_age m_exitProfile m_liveness m_lock m_metadata m_quickDFGTierUp m_quickFTLTierUp m_unlinkedBaselineCode],
  semantic_fields: SEMANTIC_CODE_BLOCK_FIELDS - %w[m_expressionInfo m_outOfLineJumpTargets m_rareData],
  kind: "code_block_field",
))
code_block_members.concat(add_member_range(
  records, webkit_root, code_block_path,
  "OutOfLineJumpTargets m_outOfLineJumpTargets;", "UncheckedKeyHashSet<UniquedStringImpl*> m_cachedIdentifierUids;",
  owner: "code_block", cache_fields: %w[m_arrayProfiles m_binaryArithProfiles m_cachedIdentifierUids m_cachedIdentifierUidsLock m_llintExecuteCounter m_unaryArithProfiles m_valueProfiles],
  semantic_fields: %w[m_expressionInfo m_outOfLineJumpTargets m_rareData],
  kind: "code_block_field",
))
expected_code_block_cache = CACHE_ONLY_CODE_BLOCK_FIELDS.sort
actual_code_block_cache = (code_block_members & CACHE_ONLY_CODE_BLOCK_FIELDS).sort
fail!("code-block cache policy drift: #{actual_code_block_cache.inspect} != #{expected_code_block_cache.inspect}") unless actual_code_block_cache == expected_code_block_cache

rare_cache_fields = %w[m_endDivot m_opProfileControlFlowBytecodeOffsets m_startDivot m_typeProfilerInfoMap]
add_member_range(
  records, webkit_root, code_block_path,
  "FixedVector<UnlinkedHandlerInfo> m_exceptionHandlers;", "unsigned m_privateBrandRequirement : 1;",
  owner: "code_block_rare", cache_fields: rare_cache_fields,
  semantic_fields: %w[m_bitVectors m_constantIdentifierSets m_exceptionHandlers m_needsClassFieldInitializer m_privateBrandRequirement m_unlinkedStringSwitchJumpTables m_unlinkedSwitchJumpTables],
  kind: "rare_data_field", role: "control_flow",
)

function_path = "Source/JavaScriptCore/bytecode/UnlinkedFunctionExecutable.h"
function_members = add_member_range(
  records, webkit_root, function_path,
  "unsigned m_firstLineOffset : 31;", "std::unique_ptr<RareData> m_rareData;",
  owner: "function_executable", cache_fields: CACHE_ONLY_FUNCTION_FIELDS,
  semantic_fields: SEMANTIC_FUNCTION_FIELDS,
  kind: "function_field", role: "execution",
  applicability: ["nested_function", "function_constructor"],
)
actual_function_cache = (function_members & CACHE_ONLY_FUNCTION_FIELDS).sort
fail!("function cache policy drift") unless actual_function_cache == CACHE_ONLY_FUNCTION_FIELDS.sort

add_member_range(
  records, webkit_root, function_path,
  "SourceCode m_classSource;", "PrivateNameEnvironment m_parentPrivateNameEnvironment;",
  owner: "function_rare", cache_fields: [],
  semantic_fields: %w[m_classElementDefinitions m_classSource m_generatorOrAsyncWrapperFunctionParameterNames m_parentPrivateNameEnvironment m_parentScopeTDZVariables m_sourceMappingURLDirective m_sourceURLDirective],
  kind: "function_rare_data_field", role: "execution",
  applicability: ["nested_function", "function_constructor"],
)

[
  ["Source/JavaScriptCore/bytecode/UnlinkedProgramCodeBlock.h", "VariableEnvironment m_varDeclarations;", "VariableEnvironment m_lexicalDeclarations;", "program", ["program"], %w[m_lexicalDeclarations m_varDeclarations]],
  ["Source/JavaScriptCore/bytecode/UnlinkedModuleProgramCodeBlock.h", "VariableEnvironment m_varDeclarations;", "int m_moduleEnvironmentSymbolTableConstantRegisterOffset", "module", ["module"], %w[m_moduleEnvironmentSymbolTableConstantRegisterOffset m_varDeclarations]],
  ["Source/JavaScriptCore/bytecode/UnlinkedEvalCodeBlock.h", "FixedVector<Identifier> m_variables;", "FixedVector<Identifier> m_functionHoistingCandidates;", "eval", ["eval"], %w[m_functionHoistingCandidates m_variables]],
].each do |path, range_start, range_end, owner, applicability, semantic_fields|
  add_member_range(
    records, webkit_root, path, range_start, range_end,
    owner: owner, cache_fields: [], semantic_fields: semantic_fields, kind: "specialized_code_block_field",
    role: "execution", applicability: applicability,
  )
end

source_ranges = [
  ["Source/JavaScriptCore/parser/UnlinkedSourceCode.h", "RefPtr<SourceProvider> m_provider;", "int m_endOffset;", "unlinked_source", [], %w[m_endOffset m_provider m_startOffset], "source_field", "diagnostics"],
  ["Source/JavaScriptCore/parser/SourceCode.h", "OrdinalNumber m_firstLine;", "OrdinalNumber m_startColumn;", "source", [], %w[m_firstLine m_startColumn], "source_field", "diagnostics"],
  ["Source/JavaScriptCore/parser/SourceCodeKey.h", "UnlinkedSourceCode m_sourceCode;", "unsigned m_hash;", "source_key", ["m_hash"], %w[m_flags m_functionConstructorParametersEndPosition m_name m_sourceCode], "source_key_field", "validation"],
  ["Source/JavaScriptCore/parser/SourceProvider.h", "std::atomic<unsigned> m_lockingCount", "CString m_sourceCodeDumpFilePath", "source_provider", CACHE_ONLY_SOURCE_PROVIDER_FIELDS, %w[m_preRedirectURL m_sourceMappingURLDirective m_sourceOrigin m_sourceType m_sourceURL m_sourceURLDirective m_startPosition m_taintedness], "source_provider_field", "validation"],
]
source_ranges.each do |path, range_start, range_end, owner, cache_fields, semantic_fields, kind, role|
  add_member_range(
    records, webkit_root, path, range_start, range_end,
    owner: owner, cache_fields: cache_fields, semantic_fields: semantic_fields, kind: kind,
    role: role, applicability: ["program", "module", "eval", "function_constructor", "nested_function"],
  )
end

source_provider_path = "Source/JavaScriptCore/parser/SourceProvider.h"
add_synthetic_record(
  records,
  id: "source_provider.source_text",
  kind: "source_field",
  source_path: source_provider_path,
  source_line: line_for(File.join(webkit_root, source_provider_path), "virtual StringView source() const"),
  symbol: "SourceProvider::source()",
  type: "StringView",
  config: semantic_config(
    role: "diagnostics",
    representation: "exact_javascript_code_units",
    encoding: "lossless 8-bit or UTF-16 code units",
    ownership: "callback_borrow",
    extraction: "SourceProvider::source()",
    destination: "visitor.source.code_units",
    validation: "length and selected SourceCode range agree; lone surrogates are preserved",
    applicability: ["program", "module", "eval", "function_constructor", "nested_function"],
  ),
  field_role: "source_text",
)

# SourceCodeFlags is packed in SourceCodeKey. Import the components, never the
# cache hash, so Bun's cache-specific equality does not become Hare identity.
source_key_path = "Source/JavaScriptCore/parser/SourceCodeKey.h"
%w[codeType lexicallyScopedFeatures scriptMode derivedContextType evalContextType isArrowFunctionContext codeGenerationMode].each do |component|
  add_synthetic_record(
    records,
    id: "source_key.flags.#{camel_to_snake(component)}",
    kind: "source_key_flag",
    source_path: source_key_path,
    source_line: line_for(File.join(webkit_root, source_key_path), component),
    symbol: "SourceCodeFlags::#{component}",
    type: "packed flag component",
    config: semantic_config(
      role: "validation",
      representation: "owned_enum_or_flags",
      encoding: "decoded from SourceCodeFlags::m_flags",
      ownership: "copied_scalar",
      extraction: "HareJscVisitor::decode_source_code_flags_#{camel_to_snake(component)}",
      destination: "visitor.source_key.flags.#{camel_to_snake(component)}",
      validation: "component is in the pinned enum/bitset domain and agrees with the typed root/code block",
      applicability: ["program", "module", "eval", "function_constructor"],
    ),
    field_role: "source_key",
  )
end

# Function relationships and traversal conditions come from Bun's pinned
# recursive generator, not from pointer or hash-table order.
code_cache_path = "Source/JavaScriptCore/runtime/CodeCache.cpp"
[
  ["function_relationship.declaration", "functionDecls", "declaration index order", "UnlinkedCodeBlock::functionDecls()"],
  ["function_relationship.expression", "functionExprs", "expression index order after declarations", "UnlinkedCodeBlock::functionExprs()"],
].each do |id, symbol, order, extraction|
  add_synthetic_record(
    records,
    id: id,
    kind: "function_relationship",
    source_path: code_cache_path,
    source_line: line_for(File.join(webkit_root, code_cache_path), symbol == "functionDecls" ? "numberOfFunctionDecls" : "numberOfFunctionExprs"),
    symbol: symbol,
    type: "ordered rooted UnlinkedFunctionExecutable edge",
    config: semantic_config(
      role: "execution",
      representation: "stable_function_id_sequence",
      encoding: order,
      presence: "conditional",
      condition: "the parent code block contains the corresponding functions",
      ownership: "rooted_cell",
      extraction: extraction,
      destination: "visitor.function_tree.#{symbol == 'functionDecls' ? 'declarations' : 'expressions'}",
      validation: "dense IDs follow pinned declaration-then-expression index order and never derive from addresses",
    ),
    field_role: "function_tree",
  )
end

[
  ["function_specialization.nested_construct", "CodeForConstruct", "nested executable is a constructor and its parse mode is not AsyncArrowFunctionMode, AsyncMethodMode, or AsyncFunctionMode"],
  ["function_specialization.nested_call", "CodeForCall", "nested executable is not a constructor"],
  ["function_specialization.function_constructor_plan", "HareFunctionSpecializationPlan", "the separate FunctionExecutable adapter explicitly requests the specialization"],
].each do |id, specialization, condition|
  add_synthetic_record(
    records,
    id: id,
    kind: "function_specialization",
    source_path: code_cache_path,
    source_line: line_for(File.join(webkit_root, code_cache_path), specialization == "CodeForConstruct" ? "CodeSpecializationKind::CodeForConstruct" : specialization == "CodeForCall" ? "CodeSpecializationKind::CodeForCall" : "getUnlinkedGlobalFunctionExecutable"),
    symbol: specialization,
    type: "UnlinkedFunctionCodeBlock specialization edge",
    config: semantic_config(
      role: "validation",
      representation: "optional_stable_function_id",
      encoding: specialization,
      presence: "conditional",
      condition: condition,
      ownership: "rooted_cell",
      extraction: "UnlinkedFunctionExecutable::unlinkedCodeBlockFor",
      destination: "visitor.function_tree.specializations",
      validation: "only the pinned/generated specialization is visited; a required unavailable block is an import error",
      applicability: ["nested_function", "function_constructor"],
    ),
    field_role: "function_specialization",
  )
end

# Rare-data nested records needed to turn the aggregate fields above into
# engine-independent control-flow and diagnostics data.
handler_path = "Source/JavaScriptCore/bytecode/HandlerInfo.h"
%w[start end target typeBits].each do |field|
  add_synthetic_record(
    records,
    id: "exception_handler.#{camel_to_snake(field)}",
    kind: "exception_handler_field",
    source_path: handler_path,
    source_line: line_for(File.join(webkit_root, handler_path), /\b#{Regexp.escape(field)}\b.*;/),
    symbol: "UnlinkedHandlerInfo::#{field}",
    type: field == "typeBits" ? "HandlerType" : "uint32_t",
    config: semantic_config(
      role: "control_flow",
      representation: field == "typeBits" ? "handler_kind" : "bytecode_offset",
      encoding: "copied scalar",
      ownership: "copied_scalar",
      extraction: "UnlinkedCodeBlock::exceptionHandler(index).#{field}",
      destination: "visitor.exception_handlers[].#{camel_to_snake(field)}",
      validation: "start < end, range and target are instruction boundaries, handlers remain innermost-first, and type is valid",
    ),
    field_role: "exception_handler",
  )
end

expression_path = "Source/JavaScriptCore/bytecode/ExpressionInfo.h"
%w[instPC divot startOffset endOffset].each do |field|
  add_synthetic_record(
    records,
    id: "expression_info.#{camel_to_snake(field)}",
    kind: "expression_info_field",
    source_path: expression_path,
    source_line: line_for(File.join(webkit_root, expression_path), /\b#{Regexp.escape(field)}\b.*[;{]/),
    symbol: "ExpressionInfo::Entry::#{field}",
    type: "unsigned",
    config: semantic_config(
      role: "diagnostics",
      representation: "source_offset",
      encoding: "decoded ExpressionInfo::Entry scalar",
      ownership: "copied_scalar",
      extraction: "ExpressionInfo::Decoder::#{field}()",
      destination: "visitor.expression_info[].#{camel_to_snake(field)}",
      validation: "entries are monotonic by instruction PC and source offsets are in the selected source range",
    ),
    field_role: "source_location",
  )
end
add_synthetic_record(
  records,
  id: "expression_info.line_column",
  kind: "expression_info_field",
  source_path: expression_path,
  source_line: line_for(File.join(webkit_root, expression_path), "LineColumn lineColumn"),
  symbol: "ExpressionInfo::Entry::lineColumn",
  type: "LineColumn",
  config: semantic_config(
    role: "diagnostics", representation: "zero_based_line_column",
    ownership: "copied_scalar", extraction: "ExpressionInfo::Decoder::lineColumn()",
    destination: "visitor.expression_info[].line_column",
    validation: "line and column agree with source offsets and SourceCode origin",
  ),
  field_role: "source_location",
)

# Constant kinds are exactly the CachedJSValue alternatives plus lossless
# immediate sub-kinds. Nested records state the bytes Hare must own.
cached_types_path = "Source/JavaScriptCore/runtime/CachedTypes.cpp"
cached_types_absolute = File.join(webkit_root, cached_types_path)
encoded_type_body = File.read(cached_types_absolute)[/enum class EncodedType : uint8_t \{(.*?)\n\s*\};/m, 1]
fail!("cannot parse CachedJSValue::EncodedType") unless encoded_type_body
encoded_types = encoded_type_body.scan(/^\s*([A-Za-z0-9_]+),/).flatten
expected_encoded_types = %w[JSValue SymbolTable String ImmutableButterfly RegExp TemplateObjectDescriptor BigInt]
fail!("constant kind drift: #{encoded_types.inspect}") unless encoded_types == expected_encoded_types

constant_kinds = [
  ["immediate_empty", "JSValue", "empty/hole tag", "exact JS immediate tag"],
  ["immediate_undefined", "JSValue", "undefined tag", "exact JS immediate tag"],
  ["immediate_null", "JSValue", "null tag", "exact JS immediate tag"],
  ["immediate_boolean", "JSValue", "boolean payload", "boolean"],
  ["immediate_int32", "JSValue", "signed int32 payload", "i32"],
  ["immediate_double", "JSValue", "IEEE-754 payload", "exact 64-bit floating bits"],
  ["symbol_table", "SymbolTable", "declarative scope layout", "owned symbol-table record"],
  ["string", "String", "exact code units and string/symbol tags", "owned 8-bit or UTF-16 code units"],
  ["immutable_butterfly", "ImmutableButterfly", "indexing kind, length, holes, and elements", "owned element vector"],
  ["regexp", "RegExp", "pattern and flags", "owned regexp descriptor"],
  ["template_object", "TemplateObjectDescriptor", "raw/cooked strings and end offset", "owned template descriptor"],
  ["bigint", "BigInt", "sign and magnitude digits", "owned sign/magnitude"],
]
constant_kinds.each do |name, encoded_type, meaning, representation|
  add_synthetic_record(
    records,
    id: "constant_kind.#{name}",
    kind: "constant_kind",
    source_path: cached_types_path,
    source_line: line_for(cached_types_absolute, encoded_type == "JSValue" ? "m_type = EncodedType::JSValue" : "EncodedType::#{encoded_type}"),
    symbol: "CachedJSValue::EncodedType::#{encoded_type}",
    type: encoded_type,
    config: semantic_config(
      role: "execution",
      representation: representation,
      encoding: meaning,
      ownership: name.start_with?("immediate") ? "copied_scalar" : "copied_bytes",
      extraction: "HareJscVisitor::copy_constant_#{name}",
      destination: "visitor.constants[].#{name}",
      validation: "constant converts losslessly without retaining a JSValue tag, JSC cell, pointer, or provider borrow",
    ),
    field_role: "constant",
  )
end

constant_fields = [
  ["string.code_units", "CachedUniquedStringImpl", "span8() / span16()", "exact code units"],
  ["string.flags", "CachedUniquedStringImpl", "m_isSymbol", "string, symbol, registered, well-known, private, and width tags"],
  ["immutable_butterfly.indexing_type", "CachedImmutableButterfly", "m_indexingType", "IndexingType"],
  ["immutable_butterfly.length", "CachedImmutableButterfly", "m_length", "unsigned"],
  ["immutable_butterfly.elements", "CachedImmutableButterfly", "m_cachedValues", "double bits or recursive constants with holes"],
  ["regexp.pattern", "CachedRegExp", "m_patternString", "exact code units"],
  ["regexp.flags", "CachedRegExp", "m_flags", "Yarr::Flags"],
  ["template.raw_strings", "CachedTemplateObjectDescriptor", "m_rawStrings", "string vector"],
  ["template.cooked_strings", "CachedTemplateObjectDescriptor", "m_cookedStrings", "optional string vector"],
  ["template.end_offset", "CachedTemplateObjectDescriptor", "m_endOffset", "source offset"],
  ["bigint.sign", "CachedBigInt", "m_sign", "boolean sign"],
  ["bigint.digits", "CachedBigInt", "dataStorage()", "magnitude digit bytes"],
  ["symbol_table.bindings", "CachedSymbolTable", "m_map", "ordered-by-stable-key binding records"],
  ["symbol_table.max_scope_offset", "CachedSymbolTable", "m_maxScopeOffset", "ScopeOffset"],
  ["symbol_table.uses_sloppy_eval", "CachedSymbolTable", "m_usesSloppyEval", "boolean"],
  ["symbol_table.nested_lexical_scope", "CachedSymbolTable", "m_nestedLexicalScope", "boolean"],
  ["symbol_table.scope_type", "CachedSymbolTable", "m_scopeType", "scope enum"],
  ["symbol_table.arguments", "CachedSymbolTable", "m_arguments", "optional scoped-argument table"],
  ["symbol_table.private_names", "CachedSymbolTable", "m_rareData", "private-name environment"],
]
constant_fields.each do |name, owner, symbol, representation|
  source_needle = symbol == "span8() / span16()" ? "span8()" : symbol
  add_synthetic_record(
    records,
    id: "constant_field.#{name}",
    kind: "constant_field",
    source_path: cached_types_path,
    source_line: line_for(cached_types_absolute, source_needle),
    symbol: "#{owner}::#{symbol}",
    type: representation,
    config: semantic_config(
      role: "execution", representation: "owned_declarative_record",
      encoding: representation, ownership: "copied_bytes",
      extraction: "HareJscVisitor::copy_#{name.tr('.', '_')}",
      destination: "visitor.constants[].#{name}",
      validation: "lengths, tags, keys, offsets, and nested constant references are in range and deterministic",
    ),
    field_role: "constant_payload",
  )
end

add_synthetic_record(
  records,
  id: "constant_field.string.atomic_interning",
  kind: "constant_field",
  source_path: cached_types_path,
  source_line: line_for(cached_types_absolute, "m_isAtomic"),
  symbol: "CachedUniquedStringImpl::m_isAtomic",
  type: "bool",
  config: cache_config(
    reason: "JSC atom-table interning is object/cache identity; Hare preserves code units and semantic symbol identity instead",
    representation: "jsc_string_interning_state",
  ),
  field_role: "constant_payload",
)

jsc_value_path = "Source/JavaScriptCore/runtime/JSCJSValue.h"
%w[Other Integer Double LinkTimeConstant].each do |representation|
  add_synthetic_record(
    records,
    id: "constant_source_representation.#{camel_to_snake(representation)}",
    kind: "constant_source_representation",
    source_path: jsc_value_path,
    source_line: line_for(File.join(webkit_root, jsc_value_path), /^\s*#{Regexp.escape(representation)},?$/),
    symbol: "SourceCodeRepresentation::#{representation}",
    type: "SourceCodeRepresentation",
    config: semantic_config(
      role: "validation",
      representation: "constant_source_spelling_kind",
      encoding: representation,
      ownership: "copied_scalar",
      extraction: "UnlinkedCodeBlock::constantSourceCodeRepresentation(index)",
      destination: "visitor.constants[].source_representation",
      validation: "entry count is no greater than the constant pool and missing entries normalize to Other",
    ),
    field_role: "constant",
  )
end

# Function rare data contains declarative class elements. These nested fields
# are included separately so the aggregate cannot hide optional source
# positions or property-key kinds.
class_element_fields = [
  ["ident", "Identifier", "required", "always"],
  ["position", "JSTextPosition", "required", "always"],
  ["initializerPosition", "optional<JSTextPosition>", "optional", "the element has an initializer source position"],
  ["kind", "ClassElementDefinition::Kind", "required", "always"],
]
class_element_fields.each do |name, type, presence, condition|
  add_synthetic_record(
    records,
    id: "class_element.#{camel_to_snake(name)}",
    kind: "class_element_field",
    source_path: function_path,
    source_line: line_for(File.join(webkit_root, function_path), /\b#{Regexp.escape(name)}\b.*[;{]/),
    symbol: "UnlinkedFunctionExecutable::ClassElementDefinition::#{name}",
    type: type,
    config: semantic_config(
      role: name == "kind" ? "execution" : "diagnostics",
      representation: "owned_class_element_field",
      encoding: type,
      presence: presence,
      condition: condition,
      ownership: name == "ident" ? "copied_bytes" : "copied_scalar",
      extraction: "ClassElementDefinition::#{name}",
      destination: "visitor.functions[].class_elements[].#{camel_to_snake(name)}",
      validation: "identifier, element kind, and source positions agree with the selected class source",
      applicability: ["nested_function", "function_constructor"],
    ),
    field_role: "class_element",
  )
end

%w[line offset lineStartOffset].each do |name|
  add_synthetic_record(
    records,
    id: "js_text_position.#{camel_to_snake(name)}",
    kind: "source_position_field",
    source_path: cached_types_path,
    source_line: line_for(cached_types_absolute, /position\.#{Regexp.escape(name)}/),
    symbol: "JSTextPosition::#{name}",
    type: "int",
    config: semantic_config(
      role: "diagnostics", representation: "source_position_scalar",
      ownership: "copied_scalar", extraction: "JSTextPosition::#{name}",
      destination: "visitor.source_positions[].#{camel_to_snake(name)}",
      validation: "line, absolute offset, and line-start offset are mutually consistent and in range",
      applicability: ["nested_function", "function_constructor"],
    ),
    field_role: "source_location",
  )
end

environment_fields = [
  ["variable.everything_captured", "m_isEverythingCaptured", "m_isEverythingCaptured = env.m_isEverythingCaptured", "bool", "execution", "VariableEnvironment::isEverythingCaptured"],
  ["variable.has_await_using", "m_hasAwaitUsingDeclaration", "m_hasAwaitUsingDeclaration = env.m_hasAwaitUsingDeclaration", "bool", "execution", "VariableEnvironment::hasAwaitUsingDeclaration"],
  ["variable.bindings", "m_map", "m_map.encode(encoder, env.m_map)", "VariableEnvironment map", "execution", "HareJscVisitor::copy_variable_environment_bindings"],
  ["variable.private_names", "m_privateNames", "m_privateNames.encode(encoder, rareData.m_privateNames)", "PrivateNameEnvironment", "execution", "HareJscVisitor::copy_private_name_environment"],
  ["tdz.variables", "m_variables", "m_variables.encode(encoder, std::get", "CompactTDZEnvironment variables", "execution", "HareJscVisitor::copy_tdz_variables"],
  ["tdz.link_handle", "m_handle", "m_handle.encode(encoder, environment.m_handle)", "CompactTDZEnvironmentMap::Handle", "execution", "HareJscVisitor::copy_tdz_handle"],
  ["tdz.parent", "m_parent", "m_parent.encode(encoder, environment.m_parent)", "optional TDZEnvironmentLink", "execution", "HareJscVisitor::copy_tdz_parent"],
  ["scoped_arguments.length", "m_length", "m_length = scopedArgumentsTable.m_arguments.size()", "uint32_t", "execution", "ScopedArgumentsTable::length"],
  ["scoped_arguments.slots", "m_arguments", "m_arguments.encode(encoder, scopedArgumentsTable.m_arguments", "ScopeOffset vector", "execution", "HareJscVisitor::copy_scoped_argument_slots"],
]
environment_fields.each do |name, symbol, source_needle, type, role, extraction|
  add_synthetic_record(
    records,
    id: "environment.#{name}",
    kind: "environment_field",
    source_path: cached_types_path,
    source_line: line_for(cached_types_absolute, source_needle),
    symbol: symbol,
    type: type,
    config: semantic_config(
      role: role, representation: "owned_environment_record",
      encoding: type,
      presence: name == "tdz.parent" || name == "variable.private_names" ? "optional" : "required",
      condition: name == "tdz.parent" ? "the TDZ link has a parent" : name == "variable.private_names" ? "the environment contains private names" : "the containing environment exists",
      ownership: type.match?(/vector|map|Environment/i) ? "copied_bytes" : "copied_scalar",
      extraction: extraction,
      destination: "visitor.environments.#{name}",
      validation: "binding keys, scope offsets, TDZ links, private-name IDs, and argument slots are deterministic and in range",
    ),
    field_role: "environment",
  )
end
add_synthetic_record(
  records,
  id: "environment.tdz.cached_hash",
  kind: "environment_field",
  source_path: cached_types_path,
  source_line: line_for(cached_types_absolute, "m_hash = env.m_hash"),
  symbol: "CompactTDZEnvironment::m_hash",
  type: "unsigned",
  config: cache_config(
    reason: "container hash is derived cache state; Hare recomputes deterministic IDs from the copied variable sequence",
    representation: "container_hash",
  ),
  field_role: "environment",
)

# Switch table fields are private to the pinned structs but are semantic after
# conversion to stable target IDs.
switch_fields = [
  ["simple.min", "UnlinkedSimpleJumpTable::m_min", "int32_t"],
  ["simple.default_offset", "UnlinkedSimpleJumpTable::m_defaultOffset", "int32_t"],
  ["simple.is_list", "UnlinkedSimpleJumpTable::m_isList", "bool"],
  ["simple.branch_offsets", "UnlinkedSimpleJumpTable::m_branchOffsets", "FixedVector<int32_t>"],
  ["string.offset_table", "UnlinkedStringJumpTable::m_offsetTable", "StringOffsetTable"],
  ["string.min_length", "UnlinkedStringJumpTable::m_minLength", "unsigned"],
  ["string.max_length", "UnlinkedStringJumpTable::m_maxLength", "unsigned"],
  ["string.default_offset", "UnlinkedStringJumpTable::m_defaultOffset", "int32_t"],
  ["string.branch_offset", "UnlinkedStringJumpTable::OffsetLocation::m_branchOffset", "int32_t"],
  ["string.index_in_table", "UnlinkedStringJumpTable::OffsetLocation::m_indexInTable", "unsigned"],
]
switch_fields.each do |name, symbol, type|
  field = symbol.split("::").last
  add_synthetic_record(
    records,
    id: "switch_table.#{name}",
    kind: "switch_table_field",
    source_path: code_block_path,
    source_line: line_for(File.join(webkit_root, code_block_path), /\b#{Regexp.escape(field)}\b.*;/),
    symbol: symbol,
    type: type,
    config: semantic_config(
      role: "control_flow", representation: "owned_switch_table",
      encoding: "stable keys and bytecode target offsets", ownership: type.include?("Vector") || type.include?("Table") ? "callback_borrow" : "copied_scalar",
      extraction: "HareJscVisitor::copy_switch_#{name.tr('.', '_')}",
      destination: "visitor.switch_tables[].#{name}",
      validation: "keys are unique, targets are instruction boundaries, defaults are present, and min/max/list metadata agrees",
    ),
    field_role: "switch_table",
  )
end

records.each do |entry|
  if entry.fetch("classification") == "semantic"
    fail!("semantic row #{entry.fetch('id')} has no extraction") if entry["extraction"].nil? || entry["extraction"].empty?
    fail!("semantic row #{entry.fetch('id')} has no destination") if entry["destination"].nil? || entry["destination"].empty?
    fail!("semantic row #{entry.fetch('id')} has an exclusion reason") if entry["exclusion_reason"]
  else
    fail!("cache-only row #{entry.fetch('id')} is consumed semantically") if entry["destination"] || entry["extraction"]
    fail!("cache-only row #{entry.fetch('id')} has no reason") if entry["exclusion_reason"].nil? || entry["exclusion_reason"].empty?
  end
  if entry.fetch("kind") == "operand" && entry.fetch("classification") == "semantic"
    fail!("semantic operand #{entry.fetch('id')} has no field role") if %w[operand cache_hint_or_layout].include?(entry.fetch("field_role"))
  end
  if %w[opcode helper_opcode].include?(entry.fetch("kind"))
    if entry.fetch("classification") == "semantic"
      fail!("semantic opcode #{entry.fetch('id')} has no conservative effect ceiling") if entry.fetch("effects").empty?
    else
      fail!("cache-only opcode #{entry.fetch('id')} has effects") unless entry.fetch("effects").empty?
    end
  end
end

duplicate_ids = records.group_by { |entry| entry.fetch("id") }.select { |_id, entries| entries.length > 1 }.keys
fail!("duplicate manifest IDs: #{duplicate_ids.join(', ')}") unless duplicate_ids.empty?

counts = {
  "sections" => sections.length,
  "all_opcode_ids" => sections.sum { |section| section.opcodes.length },
  "application_opcodes" => sections.find { |section| section.name == :Bytecode }.opcodes.length,
  "helper_opcode_ids" => sections.reject { |section| section.name == :Bytecode }.sum { |section| section.opcodes.length },
  "operands" => records.count { |entry| entry.fetch("kind") == "operand" },
  "metadata_fields" => records.count { |entry| entry.fetch("kind") == "metadata_field" },
  "metadata_references" => records.count { |entry| entry.fetch("kind") == "metadata_reference" },
  "opcode_checkpoints" => records.count { |entry| entry.fetch("kind") == "opcode_checkpoint" },
  "opcode_temporaries" => records.count { |entry| entry.fetch("kind") == "opcode_temporary" },
  "derived_operands" => records.count { |entry| entry.fetch("kind") == "derived_operand" },
  "semantic_records" => records.count { |entry| entry.fetch("classification") == "semantic" },
  "cache_only_records" => records.count { |entry| entry.fetch("classification") == "cache_only" },
  "total_records" => records.length,
}

expected_counts = {
  "sections" => 4,
  "all_opcode_ids" => 320,
  "application_opcodes" => 194,
  "helper_opcode_ids" => 126,
  "operands" => 609,
  "metadata_fields" => 99,
  "metadata_references" => 50,
  "opcode_checkpoints" => 18,
  "opcode_temporaries" => 5,
  "derived_operands" => 11,
}
expected_counts.each do |key, expected|
  fail!("#{key} drift: expected #{expected}, got #{counts.fetch(key)}") unless counts.fetch(key) == expected
end

manifest = {
  "schema_version" => SCHEMA_VERSION,
  "bun_revision" => BUN_REVISION,
  "webkit_revision" => WEBKIT_REVISION,
  "webkit_repository" => "https://github.com/oven-sh/WebKit.git",
  "contract" => "docs/hare/COMPILER.md#jsc-to-rust-visitor-boundary",
  "generator" => "scripts/hare/generate-jsc-inventory.rb",
  "definitions" => {
    "opcode_dsl" => bytecode_relative,
    "value_flow_rules" => use_def_relative,
    "effect_policy" => "conservative importer ceiling derived from pinned opcode family and value-flow definitions; H009 IR and H010/H018 helper manifests must preserve or refine it with accepted semantic proof",
    "generated_structs" => "Source/JavaScriptCore/bytecode/BytecodeStructs.h",
    "generated_ids" => "Source/JavaScriptCore/bytecode/Bytecodes.h",
    "width_rules" => [width_path, fits_path, instruction_path],
    "boundary_evidence" => [
      code_block_path,
      function_path,
      cached_types_path,
      code_cache_path,
      source_key_path,
      source_provider_path,
    ],
  },
  "sources" => SOURCE_PATHS.map do |path|
    {
      "path" => path,
      "sha256" => sha256(File.join(webkit_root, path)),
    }
  end,
  "coverage" => counts,
  "records" => records.sort_by { |entry| entry.fetch("id") },
}

manifest_text = JSON.pretty_generate(manifest) + "\n"
tsv_headers = %w[
  section opcode_id opcode generated_type source_line operand_words widths
  classification semantic_role owner_task family effect_ceiling operands semantic_operands
  cache_only_operands metadata_fields checkpoints temporaries lowering_status
]
tsv_cells = [tsv_headers] + opcode_rows.map { |row| tsv_headers.map { |header| row.fetch(header).to_s } }
tsv_cells.flatten.each do |cell|
  fail!("TSV cell contains a tab, CR, or LF: #{cell.inspect}") if cell.match?(/[\t\r\n]/)
end
tsv_text = tsv_cells.map { |row| row.join("\t") }.join("\n") + "\n"

{
  options.fetch(:manifest) => manifest_text,
  options.fetch(:tsv) => tsv_text,
}.each do |path, content|
  if options.fetch(:check)
    fail!("generated output differs: #{path}") unless File.file?(path) && File.binread(path) == content
  else
    FileUtils.mkdir_p(File.dirname(path))
    File.binwrite(path, content)
  end
end

puts JSON.generate(
  "webkit_revision" => WEBKIT_REVISION,
  "manifest" => options.fetch(:manifest),
  "opcodes_tsv" => options.fetch(:tsv),
  "coverage" => counts,
  "mode" => options.fetch(:check) ? "check" : "write",
)
