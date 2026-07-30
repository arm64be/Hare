# Hare JSC-to-native porting guide

This is the H007 mapping contract for turning Hare's pinned JavaScriptCore
frontend data into an owned visitor unit and then Hare IR. It is deliberately
more mechanical than `docs/hare/COMPILER.md`: an opcode owner should be able to
fill in the mapping packet near the end of this document without deciding a
new shared representation, lifetime rule, or runtime ABI.

The guide applies to:

- Bun revision `bbe3f6a2629adf808adbd0da199ae8c94a3c0d47`;
- WebKit revision `34c01d13391e00c06862a3d2c5b7fff350ac87e0`;
- schema version 2 of
  `generated/hare/jsc-extraction-manifest.json`; and
- the corresponding generated `OPCODES.tsv`.

Run `scripts/hare/generate-jsc-inventory.rb --webkit-root tmp/hare-webkit
--check` before relying on any row. A revision, schema, source hash, generated
definition, opcode count, or field classification mismatch is an import
contract error. It is never repaired by accepting the new input, filling a
default, or falling back to JSC execution.

## Authority and scope

The authority order is:

1. `HARE.md`, `docs/hare/SEMANTICS.md`, `docs/hare/MEMORY.md`, and
   `docs/hare/EFFECT.md` define observable behavior and memory rules.
2. `docs/hare/COMPILER.md` defines the protected JSC boundary, owned IR
   invariants, re-entry protocol, and helper contract.
3. The H006 manifest exhaustively classifies the pinned boundary fields and
   gives their extraction, ownership, destination category, and validation.
4. This guide defines repeated mappings and implementation checks.
5. H008 through H018 choose local type names, crate organization, data
   structures, algorithms, and concrete helper symbols within those rules.

`source`, `generated_definition`, and `value_flow.source` in a manifest row
are pinned evidence. `owner_task` and `family` route implementation work; they
do not grant permission to change semantics. `effects` is a conservative
importer ceiling. It prevents a lowering from hiding possible behavior, but it
does not define a concrete helper or ABI.

This guide does not choose:

- final Rust enum, node, block, value, or error names;
- an arena, interner, SSA construction algorithm, or in-memory packing;
- target data layout, calling convention, aggregate layout, or symbol names;
- a one-helper-per-opcode runtime interface; or
- narrower effects without pinned-source and language-semantic proof.

Those choices belong to H009, H010, or H018. If following a recipe requires a
new observable state, implicit failure channel, pointer lifetime, shared IR
meaning, or helper meaning, stop and escalate instead of extending the recipe.

## Required state transition

Every import follows this one-way transition:

```text
protected rooted JSC input
    -> callback-scoped typed scalars and byte spans
    -> completely owned visitor unit
    -> import validation
    -> JSC borrow and build VM released
    -> validated Hare core IR
    -> helper/LLVM lowering
```

No arrow points backward. Analysis and lowering cannot call the visitor, ask
JSC to decode another value, regenerate bytecode, retain a source provider, or
interpret a saved JSC address. The protected bridge remains synchronous and
non-reentrant. A callback-scoped span is copied before that callback returns.
A rooted cell is reachable only while the bridge reads and copies it; rooting
is not address stability and its pointer never enters the visitor unit.

The build-VM owner retains every JSC root until all imports using that VM have
joined. The owner releases roots, drains the accepted teardown protocol, and
destroys thread-local state only after the last visitor result is owned.

## Manifest consumption

Keep two different ledgers:

1. A **definition-coverage ledger**, keyed by manifest `id`, proves that the
   importer has an explicit handler for every row. Its states are `mapped`,
   `conditionally_mapped`, `excluded`, and `not_applicable`.
2. An **occurrence ledger** proves that every value in the current input was
   handled. Instruction data uses `(function_id, byte_offset, manifest_id)`;
   indexed tables add the stable table/element index; singleton root/function
   fields use `(owner_id, manifest_id)`. Its states are:
   - `copied`: a semantic value was extracted, converted to owned data, and
     validated;
   - `validated_absent`: an optional or conditional semantic value was absent
     because its recorded condition was false; or
   - `excluded`: a `cache_only` occurrence was recognized and erased for its
     recorded `exclusion_reason`.

A manifest definition may therefore handle any number of input occurrences;
only an occurrence key must be unique. A shared handler still lists every
manifest ID it covers so definition coverage remains exhaustive.

There is no `ignored`, `unknown`, or `defaulted` state. A required semantic row
cannot be absent. A conditional row cannot be omitted without evaluating its
exact `condition`. A semantic record whose `applicability` contains none of
the current root/function modes is `not_applicable` for that input, not
excluded program data. A `build_vm_only` cache row can still be encountered by
structural decoding, such as an encoding prefix or alignment opcode; record
that occurrence as `excluded`, but never copy it into the application unit.

The binary classification is a firewall:

- `semantic` means execute the recorded extraction while the borrow is valid,
  convert according to `representation` and `ownership`, emit the recorded
  destination category, and run `validation`;
- `cache_only` means recognize enough encoding to advance and validate the
  containing structure, but never use its value for an IR operation, type,
  representation, branch, helper choice, stable identity, or diagnostic.

Metadata IDs, profiles, inline caches, structure guesses, watchpoints,
interpreter return locations, LLInt helpers, sampled counters, locks, hashes,
and cached code blocks therefore cannot become Tier 2 hints accidentally.
Future semantic use requires a deliberate H003/H006 reclassification.

### Common row dimensions

Interpret presence exactly:

| `presence` | Rule |
| --- | --- |
| `required` | The occurrence must exist whenever its owner/applicability exists. Absence is an import error. |
| `optional` | Evaluate the row's accessor/sentinel and copy either `Some(owned)` or explicit absence; never synthesize a value. |
| `conditional` | Evaluate the exact recorded `condition`; copy the value if true and record `validated_absent` if false. |

Interpret ownership exactly:

| `ownership` | Rule |
| --- | --- |
| `copied_scalar` | Copy and validate the typed scalar/discriminant; retain no address or padding. |
| `copied_bytes` | Copy the complete logical byte/code-unit/element payload into owned storage before callback/borrow end. |
| `callback_borrow` | Read only within the callback and complete a lossless owned copy before returning. |
| `stable_id` | Allocate from the prescribed deterministic traversal/domain; never derive it from an address or hash bucket. |
| `rooted_cell` | Keep the JSC cell rooted only during protected extraction, convert it completely, and emit no cell/pointer. |
| `excluded` | Read no program meaning; acknowledge structural presence and erase it for the recorded reason. |

Interpret semantic subroles as routing constraints:

| `semantic_role` | Rule |
| --- | --- |
| `execution` | Contributes owned program data or an operation input. |
| `control_flow` | Contributes graph/checkpoint/handler data and must resolve to explicit successors or stable control IDs. |
| `validation` | Contributes an owned fact used to reject malformed or mismatched input; it does not silently choose semantics. |
| `diagnostics` | Contributes owned source/diagnostic fidelity and cannot become an optimization fact. |
| `cache_only` | Emits no owned semantic value. |

Applicability is a closed dispatch: `program`, `module`, `eval`,
`function_call`, `function_construct`, `function_constructor`, and
`nested_function` select semantic inputs. `build_vm_only` identifies excluded
build/encoding state. A row with multiple modes is processed independently for
each matching owner occurrence.

## Import algorithm

The following ordering is mandatory. Implementations may split it into local
functions, but may not merge the protected and owned phases.

### 1. Open the protected input

Verify root kind, code specialization, selected source range, realm/build-VM
owner, pinned revisions, and manifest schema. Establish the exception scope,
root token, `DeferGC` interval where required, C++ `noexcept` result boundary,
and Rust `catch_unwind` boundary from `docs/hare/COMPILER.md`.

Record the applicable modes from this closed set: `program`, `module`, `eval`,
`function_call`, `function_construct`, `function_constructor`, and
`nested_function`. `build_vm_only` applies only to excluded build state.

### 2. Allocate stable identities

Assign the root function ID first. For each code block, visit stored function
declarations by increasing index, then stored function expressions by
increasing index. Recurse using exactly the specialization selected by pinned
`CodeCache.cpp`:

- a constructor child uses the permitted construct block;
- a non-constructor child uses the call block;
- the pinned async modes with no construct generation remain absent; and
- the separate Function-executable adapter follows its explicit
  specialization plan in that plan's generated stable order.

Dense IDs come from this traversal, not pointers, hashes, allocation order, or
container iteration. Predeclare an ID before visiting a child so recursive
references terminate. A missing required specialization is an import error;
do not invent its counterpart or impose call-before-construct ordering.

### 3. Copy root, source, and declarative tables

Process every applicable code-block, rare-data, function, source, source-key,
constant, environment, switch, expression, class-element, and handler row.
Copy callback borrows immediately. Normalize JSC-specific tags into the
manifest representation. Preserve stable references between copied tables,
but never the source address of a table or element.

### 4. Decode instructions in two passes

First pass:

1. Iterate `JSInstructionStream` with its pinned iterator. The iterator's
   `Ref::offset()` is the byte offset and advances by `JSInstruction::size()`.
2. Before `as<GeneratedType>()`, validate the decoded opcode ID against the
   manifest row and validate `width()`, `size()`, `opcodeIDBytes()`, and
   `operand_word_count` against the pinned width rules.
3. Record every instruction start and the stream-end boundary. Reject zero
   progress, overflow, overlap, trailing bytes, or an instruction ending past
   `instructions().size()`.
4. Decode each operand through the generated accessor. Never reinterpret a
   raw operand byte or manually reproduce `Fits<T, OpcodeSize>`.
5. Copy semantic operands, checkpoint IDs, temporaries, and derived operands;
   acknowledge cache-only operands and metadata without consuming their
   values.

`op_wide16` and `op_wide32` are prefixes inside the encoded instruction. The
pinned `opcodeID()` and `as<T>()` expose the actual opcode and converted
operands. The prefixes have inventory rows so encoding drift is detected, but
they never produce Hare operations or source-visible offsets of their own.

Second pass:

1. Resolve registers/constants, branch targets, switch targets, table indices,
   function IDs, handlers, checkpoints, and source references using the copied
   tables and instruction-boundary set.
2. Materialize exact use/definition sets at each recorded checkpoint.
3. Build normal and abrupt successors without lowering a helper yet.
4. Reject every unresolved token, out-of-range reference, duplicate stable ID,
   malformed interval, invalid optional value, or target that is not an
   instruction boundary.

Two passes are required because forward branches and mutually referring
declarative tables cannot be validated safely from encounter order.

### 5. Seal and release

Run all import validators while the protected input is still available, then
seal the owned visitor unit. Verify that it contains no JSC pointer, `JSValue`
bits, C++ reference count, vtable, source-provider borrow, metadata/cache
record, or unresolved visitor token. Only then release the root/borrow and hand
the unit to H009.

Every early exit uses the same ownership order: destroy or return the partially
built owned result through RAII, end callback spans, keep JSC roots and the
exception scope alive until no decoder can run, then release the root/VM owner
exactly once. Allocation failure, a C++ exception, a Rust panic, and an import
diagnostic cannot leak a partial unit, publish it to another thread, or run a
late callback against released bridge state.

### 6. Lower owned data

H009 maps owned operations to core IR, including explicit normal and abrupt
edges, effects, safepoints, roots, barriers, ownership, re-entry, ambient
state, and closed-world dependencies. H010 supplies a versioned helper entry
before any runtime call is selected. H018 implements that entry without an
interpreter/JIT fallback or hidden callback.

## Instruction and operand mappings

### Widths and offsets

Use the three `operand_width_rule` rows as a set. Narrow instructions use no
prefix and one byte per operand. Wide16 and Wide32 use their prefix and two or
four bytes per operand. At the pinned revision the opcode ID still occupies
the generated width reported by `opcodeIDBytes()`; do not hard-code it from an
example.

Store a byte offset and byte size in the visitor instruction. Convert a byte
offset to a dense instruction ID only after the boundary pass. Source
positions and exception/switch targets that use bytecode offsets retain a
checked mapping back to the original byte offset for diagnostics.

A `BoundLabel` decodes to a signed relative target. Zero denotes an
out-of-line jump entry for the current instruction. Resolve the effective
relative offset through the pinned out-of-line table, add it to the current
instruction byte offset with checked arithmetic, and require the result to be
an instruction boundary. Switch-table branch/default offsets are resolved
relative to the switch instruction by the same checked rule.

### Virtual registers and value flow

Always use the decoded `VirtualRegister`, never its encoded narrow/wide word.
Classify it with the pinned accessors:

- local -> stable local ID within declared `numVars`/callee-local bounds;
- argument/header -> stable frame-slot ID within the function layout;
- constant -> constant-pool ID from `toConstantIndex()`; or
- invalid -> accepted only for a manifest row whose presence/condition permits
  the invalid sentinel.

Narrow and Wide16 encodings remap constant indices through different first
constant values. The generated accessor reverses that mapping. Copying the raw
signed word would silently select the wrong constant and is forbidden.

Use `value_flow.uses_at` and `value_flow.defines_at` as authoritative pinned
flow points. `entry` means before the operation's first observable stage.
Named checkpoints mean the value becomes live or defined at that stage.
`all_checkpoints`, `*_and_later`, and derived range rows must be expanded
literally. A destination defined only after a possibly abrupt stage is not
defined on that stage's abrupt edge.

### Exhaustive `field_role` dispatch

Every current field role maps as follows. A new role makes the dispatch
non-exhaustive and fails the import build until this table and its validator
are updated.

| Manifest `field_role` | Required mapping |
| --- | --- |
| `value_use` | Resolve one value before each recorded use point. |
| `value_def` | Create one definition at each recorded definition point and only on completing paths. |
| `value_use_def` | Read the incoming value and write a distinct outgoing SSA value at the recorded points; do not model an in-place JSC slot mutation. |
| `register_range_use` | Expand the exact pinned derived range, preserving order and duplicates where specified. |
| `register_range_use_def` | Expand both incoming and outgoing range roles; pair elements only when the pinned rule says they correspond. |
| `encoded_constant_or_register_reference` | Preserve a tagged choice between a stable frame value and constant ID after generated decoding. |
| `argument_count` | Checked nonnegative count; validate it with the corresponding base/range rule. |
| `argument_range_base` | Frame base used only through its matching derived range record. |
| `argument_index` | Checked position in the applicable call/parameter layout. |
| `frame_slot_base` | Checked frame-layout coordinate; never a source path, native address, or byte offset. |
| `control_target` | Resolve the effective relative branch target, including the zero/out-of-line case. |
| `switch_table_index` | Resolve the correct copied simple or string switch table for the opcode. |
| `function_table_index` | Resolve a copied declaration/expression entry to its traversal-assigned function ID. |
| `identifier_index` | Resolve exact identifier code units and stable semantic identity. |
| `bit_vector_index` | Resolve the copied rare-data bit vector and validate its consumer-specific length. |
| `scope_depth` | Checked lexical-scope depth interpreted with the opcode's resolve/get mode. |
| `scope_slot_index` | Checked binding slot in the resolved scope layout; pair it with the copied identifier and `GetPutInfo`. |
| `symbol_table_or_scope_depth` | Preserve the pinned discriminated `SymbolTableOrScopeDepth` meaning selected by the scope operation; never collapse its alternatives to an integer. |
| `lexical_feature_flags` | Decode the complete admitted direct-eval lexical-feature bitset and reject unknown bits. |
| `property_attributes` | Decode the pinned property-descriptor attribute bitset and preserve every accepted flag. |
| `structure_flags` | Decode the pinned structure-test flag mask as a semantic predicate input, not profile data. |
| `table_or_slot_index` | Reserved generic scalar-table dispatch for an older/newer generated row; it is invalid until the row names its exact domain. |
| `element_or_field_index` | Validate against the operation's object/internal-field domain before lowering. |
| `count` | Checked cardinality paired with its manifest-described collection. |
| `signed_offset_or_count` | Preserve sign, use checked arithmetic, and select offset versus count from the row's source type and destination. |
| `typed_scalar` | Preserve the pinned enum/scalar domain and reject unknown discriminants. |
| `mode_or_flags` | Decode each accepted bit/discriminant; reject reserved combinations rather than truncating them. |
| `boolean_control` | Preserve a boolean control input and enforce every conditional-row implication it controls. |
| `resume_point` | Stable continuation/resume ID, not a native label or JSC program counter. |
| `checkpoint` | Stable ordered sub-operation ID with exact checkpoint use/def and abrupt edges. |
| `temporary` | Owned lowering temporary, unique within the opcode and materialized before the JSC borrow ends. |
| `control_flow` | Declarative control-flow state such as jump targets or block-level control flags; resolve to owned graph data. |
| `exception_handler` | Copy `[start, end)`, target, and handler kind; preserve innermost-first order. |
| `switch_table` | Copy keys and relative targets into deterministic owned cases plus a required default. |
| `function_tree` | Ordered declaration/expression relationship using stable function IDs. |
| `function_specialization` | Conditional call/construct edge selected exactly by the generated specialization plan. |
| `constant` | Lossless engine-independent constant or source-representation tag. |
| `constant_payload` | Owned nested bytes/records needed to reconstruct the constant's language meaning. |
| `environment` | Owned declarative binding/scope data with stable binding and parent IDs. |
| `class_element` | Owned key, kind, and exact required/optional source positions. |
| `source_text` | Exact selected 8-bit or UTF-16 JavaScript code units, including lone surrogates. |
| `source_location` | Checked source offset or line/column tied to the selected source range. |
| `source_key` | Owned parse/code flags and source identity components; never the cached hash. |
| `diagnostics` | Owned diagnostic/source data; it cannot influence execution except through an explicitly semantic source flag. |
| `validation` | Cross-record invariant input; retain only the owned fact needed by a validator. |
| `execution` | Semantic code-block/function field routed by its destination and representation. |
| `metadata` | Acknowledge and erase the mutable metadata field unless a separate semantic row names the original operand it copied. |
| `metadata_reference` | Validate encoded layout/table bounds as required for safe decoding, then erase. |
| `cache_hint_or_layout` | Decode only as needed to validate/advance the instruction, then erase. |
| `cache_only` | Recognize the excluded object/opcode and record coverage; emit nothing. |

The role is interpreted together with `kind`, `representation`, `condition`,
and `destination`. If that combination still permits two observable meanings,
the mapping is incomplete and must be escalated; opcode spelling is not a
tie-breaker.

### Representation and encoding dispatch

`encoding` describes how the pinned accessor obtains a value. `representation`
describes the owned logical result. Never memcpy a source object merely because
its owned representation has a similar name.

- `owned_scalar`, `boolean`, `i32`, `signed 32-bit integer`, `unsigned 32-bit
  integer`, and `typed enum or scalar` become checked language-neutral scalars
  or discriminants with no source padding.
- `strict/sloppy mode byte`, `private-field put mode byte`, `packed
  direct/strict flags`, `packed resolve/initialization/strictness flags`,
  `tagged symbol-table index or scope depth`, and `owned_enum_or_flags` use an
  exhaustive tagged/bitset conversion. Reserved values fail import.
- `signed logical register or constant reference` uses the decoded
  `VirtualRegister` mapping; `signed bytecode target offset` uses the checked
  relative-target mapping.
- `owned_declarative_record`, `owned_environment_record`,
  `owned_class_element_field`, and `owned_switch_table` are recursively built
  from their nested manifest rows. They are never C++ aggregate copies.
- `stable_function_id_sequence`, `optional_stable_function_id`,
  `stable_checkpoint_id`, and `lowering_temporary` use the deterministic ID
  domains specified above.
- `bytecode_offset`, `source_offset`, `zero_based_line_column`, and
  `source_position_scalar` remain checked coordinates in their named domain;
  they are not mutually interchangeable integers.
- `instruction_encoding_width` and `opcode_identity` configure/validate the
  decoder and dispatch but do not import native layout.
- `exact_javascript_code_units` and `owned 8-bit or UTF-16 code units` preserve
  exact code units. The other constant representations—`exact JS immediate
  tag`, `exact 64-bit floating bits`, `owned element vector`, `owned regexp
  descriptor`, `owned sign/magnitude`, `owned symbol-table record`, and `owned
  template descriptor`—follow the constant rules below.
- `constant_source_spelling_kind` is validation/spelling data, not a runtime
  representation choice.
- Every prose `derived_operand` representation is executed only through its
  row's pinned extraction and validation formula. Similar-looking call, array,
  string, local, and resume ranges are not interchangeable.

An unrecognized representation is a non-exhaustive schema error even if the
source type would fit an existing integer or byte container.

## Non-instruction record kinds

The importer has an exhaustive kind switch in addition to the field-role
switch:

| Manifest `kind` | Import action |
| --- | --- |
| `opcode`, `operand`, `derived_operand`, `opcode_checkpoint`, `opcode_temporary` | Build the owned instruction record and exact staged flow. |
| `helper_opcode` | Validate the pinned ID namespace and erase; it is never application code or a Hare helper request. |
| `operand_width_rule` | Configure and validate generated decoding; emit no program operation. |
| `metadata_field`, `metadata_reference` | Enforce the cache firewall and safe structural decoding. |
| `code_block_field`, `rare_data_field`, `specialized_code_block_field` | Copy the applicable root/function header, tables, and declarative control data. |
| `function_field`, `function_rare_data_field`, `function_relationship`, `function_specialization` | Build owned function descriptors and deterministic traversal edges. |
| `source_field`, `source_provider_field`, `source_key_field`, `source_key_flag`, `source_position_field`, `expression_info_field` | Copy exact source identity/text/locations and validate selected ranges. |
| `constant_kind`, `constant_field`, `constant_source_representation` | Build lossless constants and their spelling categories. |
| `environment_field` | Build owned binding, TDZ, private-name, parent, and scoped-argument records. |
| `exception_handler_field`, `switch_table_field` | Build checked graph-side abrupt and multiway control data. |
| `class_element_field` | Build declarative class element records with exact optionality. |

No generic reflection fallback handles an unknown kind. Schema evolution must
add an explicit case and test.

## Constants, names, sources, and environments

### Constants

Normalize by language meaning, never by `JSValue` bits:

- empty/hole, `undefined`, `null`, and booleans retain distinct tags;
- int32 retains its signed value;
- number retains the exact IEEE-754 bits, including NaN payload as imported and
  the distinction between `0` and `-0`;
- strings retain exact code units and width/tag facts required by the manifest;
- symbols retain their semantic category and stable identity, not a cell
  address or atom-table address;
- BigInt retains sign and magnitude digits;
- RegExp retains exact pattern code units and flags;
- immutable butterflies retain indexing kind, length, holes, and recursively
  copied elements;
- template descriptors retain ordered raw/cooked strings and end offset; and
- symbol tables retain declarative binding/scope data through stable IDs.

`constantSourceCodeRepresentation` is validation/spelling input. A missing
entry normalizes to the pinned `Other` case only because the pinned accessor
defines that behavior; it is not a general missing-data default.

JSC atomic interning, cached hashes, map buckets, write barriers, and cell
identity are excluded. When semantic identity is needed, allocate a stable ID
from deterministic traversal or exact copied key data. Never sort an
observably ordered collection. An unordered declarative map may be serialized
in a deterministic semantic-key order only when its manifest validation and
language contract establish that iteration order is not observable.

### Names and source

Identifiers, private names, URLs, directives, and source text retain exact
code units. Do not normalize Unicode, replace lone surrogates, resolve a URL,
or canonicalize a path unless the semantic contract for that particular field
requires it. Source offsets are relative to the selected `SourceCode` range;
line/column pairs must agree with its origin and code units.

Source-provider locks, IDs, stripped-URL caches, dump paths, and cached hashes
are excluded. Diagnostics may quote owned source text after JSC teardown; they
may not reopen the provider.

### Environments

Copy variable bindings, capture flags, `await using` flags, TDZ variables and
parent links, private names, scope kinds/offsets, and scoped-argument slots as
declarative records. Use stable IDs for identity-bearing environments and
bindings. Direct eval later receives explicit live environment handles from
`DirectEvalContext`; the copied unlinked environment describes layout and
resolution, not a snapshot of runtime values.

## Control flow, checkpoints, and abrupt completion

Create the instruction-boundary set before resolving any edge. A conditional
branch has target and fallthrough successors. An unconditional branch has only
its resolved target. A return has an explicit function-completion successor.
A switch has one edge per copied case plus its default. Duplicate case keys,
missing defaults, overflow, or non-boundary targets are import errors.

Exception handler ranges are half-open `[start, end)`. Preserve pinned
innermost-first order when ranges overlap. Every possibly throwing operation
has an abrupt edge to the selected handler or function escape. Do not recover
the handler later from a bytecode program counter hidden in a runtime context.

A checkpoint is an ordered semantic sub-operation, not just liveness
metadata. Split the operation so that:

- uses required at a checkpoint dominate that checkpoint;
- definitions appear only at their recorded checkpoint and completing paths;
- each callback/throw/suspension point has the correct normal and abrupt edge;
- live roots are published before any checkpoint that may allocate, collect,
  call user code, or suspend; and
- resumption continues from an explicit stable continuation, never a JSC
  instruction address.

An operation without generated checkpoints can still have explicit Hare
substeps when its language semantics require visible calls or abrupt edges.
Generated checkpoints are a lower bound on staged fidelity, not permission to
hide other effects.

## Effect ceilings

Each semantic opcode row has a nonempty `effects` array. Treat it as a
conservative upper approximation during initial mapping. It is not permission
to perform an effect. The accepted language operation and pinned opcode
semantics determine which paths actually exist; the ceiling says which
categories may not be declared impossible without proof.

| Effect | Required consequence |
| --- | --- |
| `reads_frame`, `writes_frame` | Declare exact input/output values; a frame mutation cannot remain implicit. |
| `control_flow` | Emit all explicit successors and reject hidden dispatch. |
| `heap_read`, `heap_write` | Declare alias/region effects; a write also carries the required barrier/ownership operation. |
| `may_allocate` | Model allocation failure policy and collection possibility. |
| `may_throw`, `throw` | Emit possible or unconditional abrupt completion with the original thrown value. |
| `may_call_user` | Use the H003 native re-entry protocol with explicit target, arguments, roots, ambient state, and completion. |
| `safepoint` | Publish every live managed root and end/reload invalid raw-pointer access. |
| `suspend` | Materialize continuation ownership, live state, cancellation/interruption edges, and resume point. |
| `exception_state_read` | Consume explicit exception/completion state; never query a hidden pending JSC exception. |
| `interruption_check` | Emit the explicit interruption/termination decision and successor. |
| `realm_access`, `scope_access` | Pass explicit identity-bearing realm/scope inputs with declared ownership. |
| `trap` | Emit an explicit unreachable/internal-contract trap, not undefined native control flow. |

An implementation may conservatively retain non-observable alias/scheduling
annotations from the whole ceiling. It must not invent a callback, throw,
allocation, write, suspension, interruption, or realm/scope access that the
semantic operation cannot perform. Graph-shaped effects therefore require the
mapping packet to cite the real semantic path; without that audit the opcode
mapping remains incomplete. An effect may be narrowed only with a recorded
proof from pinned opcode semantics and the accepted language contract. The
proof is reviewed with the mapping and updates the generated policy if it
applies to the inventory family. Profiles and JSC metadata are never proofs.

Removing `may_call_user`, `may_throw`, `safepoint`, or `suspend` changes graph
shape and requires especially direct evidence. Adding a concrete helper does
not discharge an effect: the helper manifest and call site must represent the
same or a more conservative behavior.

## Helper selection

Select by semantic operation, not JSC slow-path function name, metadata ID,
profile, or current cached shape.

1. Emit a direct core operation when H009 can represent the semantics and its
   effects explicitly.
2. Inline a pure calculation only when all types, representations, ownership,
   and abrupt cases are proved; otherwise retain the exact generic Tier 1
   path.
3. Request a generic native helper only when H010 has a versioned manifest
   entry for the semantic operation and H018 can implement it without
   interpreter/JIT execution.
4. Keep observable predicates and tagged results in IR. A helper may compute a
   predicate or return a continuation request, but may not silently choose an
   observable successor.
5. For getters, setters, proxies, coercion methods, constructors, custom
   iteration, host callbacks, or other application calls, publish the frame,
   roots, realm/worker/Effect ambient state, and pending writes before native
   re-entry. Restore and revalidate after every completion.

Every helper request records:

- the source manifest opcode/field IDs;
- semantic operation, normal result, and every abrupt result;
- exact arguments, realm/runtime services, ownership, and retention rules;
- reads, writes, allocation, callback, throw, suspension, synchronization,
  scheduling, interruption, termination, and ambient-state effects;
- safepoint/root-frame requirements;
- visible guards and continuation edges; and
- the H010 manifest entry or an explicit `blocked_on_H010` marker.

`blocked_on_H010` is allowed in an intentionally incomplete fanout shard. It
is not allowed at a W3 convergence gate and is never replaced by an untyped
foreign call.

## Family recipes

### H012: control flow and locals

Map `locals` operations to explicit value movement/definitions. Map `control`
operations to checked target IDs and fallthroughs. Abstract equality and
relational branches retain coercion, callback, and throw edges; strict/tag
branches may narrow those edges only with direct proof. Phi/block parameters
are H009 structure derived from predecessor values, not imported JSC slots.

Switch cases use copied semantic keys and relative targets. Loops have no
special runtime fallback: backedges are ordinary graph edges, while
interruption checks appear only where the accepted semantics/inventory require
them.

### H013: numeric, BigInt, comparison, and coercion

Map each opcode to the corresponding accepted ECMAScript abstract operation,
including `ToPrimitive`, `ToNumeric`, string concatenation for addition,
BigInt/Number separation, mixed-domain throws, division/remainder edge cases,
shift masking, comparison ordering, NaN, infinities, and signed zero. Keep
coercion calls and abrupt edges explicit. A proven unboxed fast operation must
retain the generic Tier 1 branch until the proof or guard removes it.

Do not use result/profile metadata as a type fact. Constant folding preserves
exact number bits and the specified BigInt result or failure.

### H014: objects, properties, arrays, strings, symbols, and shapes

Represent property-key conversion, prototype traversal, private-name/brand
checks, descriptor attributes, holes, array length/index rules, string and
RegExp semantics, and barriers explicitly. Generic `Get`, `Set`, `Has`,
`Delete`, `Define`, `Instanceof`, and enumeration operations may encounter
proxies, accessors, custom `@@hasInstance`, or coercion; use native re-entry
instead of invoking JSC application execution.

Inline shapes and offsets are permitted only as proved/guarded Tier 2 data.
H006 metadata and profile fields remain erased. Heap writes declare alias,
ownership, and barrier effects even if a helper performs the physical store.

### H015: calls, constructors, functions, scopes, generators, and async

Expand call argument ranges from their `derived_operand` rows, preserving the
pinned order and receiver position. Carry call versus construct, receiver,
`new.target`, callee, arguments, realm, scope, and closure environment as
explicit values. Resolve closed-world native targets where proved; otherwise
use a generic callable/constructable dispatch that returns an explicit native
continuation request, never an interpreter entry.

A tail-call opcode has no fallthrough in the current function. Its recorded
destination is the normal call result at the pinned `makeCall`/entry flow
point and is forwarded to function completion; it is not a live local after
the terminator. `call_ignore_result` has no destination. Varargs opcodes use
their recorded `arguments`, receiver, `firstVarArg`, checkpoint flow, and
argument-count temporary rather than the fixed-call `argc`/`argv` expansion.
Sharing dispatch machinery must not erase those graph and flow differences.

Create closures and lexical environments from stable function/environment
IDs. Generators, async functions, iterators, promises, yield, and resume points
own explicit continuation state. Every suspension ends stack/scoped borrows,
publishes roots and ambient state, and has cancellation/interruption/throw
paths required by its operation.

### H016: exceptions and abrupt completion

Represent throw values and completion kinds explicitly. `catch` consumes the
selected abrupt predecessor's owned exception state and defines its manifest
destinations. Handler selection uses the copied half-open ranges and stable
targets. `check_traps` exposes interruption/termination; `unreachable` becomes
an internal contract trap. No pending JSC exception, `CallFrame`, or host
longjmp carries application control.

Promise rejection, iterator close, finally, cancellation, and termination
retain their distinct ordering and completion rules when the owning operation
requires them; they are not normalized into a generic throw.

### H017: modules, literal eval, and literal Function

Use the accepted closed-world graph and H002 semantics. Root/module parse and
build failures are build diagnostics. An admitted literal eval or Function
body whose parse fails at runtime lowers to its recorded runtime
`SyntaxError` recipe at the call site. Unknown/effectful source construction,
resolution-only calls without a virtual-path contract, and out-of-graph
targets are compile errors, never runtime compilation.

Direct eval consumes the full `DirectEvalContext`: live identity-bearing
environment, strictness and lexical features, `this`, `new.target`, `super`,
private names, realm, referrer, conditions, and ambient runtime state as
applicable. Function construction uses the separate Function-executable
adapter and explicit specialization plan. Modules preserve resolution,
evaluation order, namespace/live-binding identity, rejected-Promise behavior,
and worker/realm ownership from the semantic contract.

## Worked mappings

These examples show packet completeness. Abstract operation names are
descriptive, not final H009 enum names.

### `op_mov`

- Consume `opcode.Bytecode.op_mov`, `operand.op_mov.src`, and
  `operand.op_mov.dst`.
- Decode `src` as `value_use` at `entry`; decode `dst` as `value_def` at
  `entry`.
- Emit an SSA copy/alias-preserving value definition. Do not copy a JSC slot or
  inherit its physical address.
- Preserve `reads_frame,writes_frame`; there is one fallthrough and no helper.
- Validate both decoded registers and define `dst` exactly once.

### `op_jless`

- Read `lhs` and `rhs` at `entry` and resolve `targetLabel`, including the
  zero/out-of-line representation.
- Emit the accepted abstract relational comparison followed by target and
  fallthrough successors.
- Preserve `control_flow,heap_read,may_call_user,may_throw,reads_frame,safepoint`.
  Object-to-primitive coercion can re-enter application code and throw; its
  abrupt edge does not choose either branch.
- A later numeric/string proof may remove coercion edges, but the generic
  mapping may not assume primitive operands from a profile.

### `op_get_by_val`

- Read `base` and `property` at `entry`; define `dst` only on normal completion.
- Emit semantic property-key conversion and `Get(base, key)` with prototype,
  proxy, and accessor behavior.
- A getter/proxy/coercion call uses explicit re-entry; a throw reaches the
  selected handler/escape. Heap reads/writes, allocation, callback, throw,
  safepoint, and frame effects begin at the manifest ceiling and may be narrowed
  only by proof.
- Acknowledge and erase `operand.op_get_by_val.valueProfile`; it cannot choose
  representation, shape, or helper.

### `op_call`

- Read `callee` at `entry`, decode `argc` and `argv`, and consume
  `derived_operand.op_call.call_argument_range`.
- Expand exactly the pinned range
  `[-m_argv + thisArgumentOffset, +m_argc)` and validate every frame register.
  Preserve its order; do not reconstruct arguments from neighboring slots.
- Emit callable validation, explicit receiver/arguments, native dispatch or
  continuation request, normal result to `dst`, and throw/termination or other
  accepted abrupt completions.
- Publish roots and ambient state before re-entry. Erase `valueProfile` and
  all call-link/array-profile metadata.

Construct and super-construct opcodes use their own receiver/`new.target` and
completion rules; sharing the range-expansion utility does not make their
semantic operation a call.

### `op_async_iterator_next`

- Read `next`, `iterator`, and `driver` at entry; define `dst` only on the
  completing path.
- Treat `stackOffset` as `frame_slot_base`, not an address. When `hasValue` is
  true, consume the conditional derived row and compute the resume value with
  pinned `resumeValueOperandFor`; when false, record `validated_absent`.
- Preserve explicit call, throw, suspension, continuation ownership, roots,
  cancellation/interruption, and resume behavior. Erase iteration/call-link
  metadata and `valueProfile`.

### `op_iterator_next`

- Create ordered `computeNext`, `getDone`, and `getValue` checkpoint IDs.
- `iterator` and `next` are live at all checkpoints; `iterable` is live from
  `computeNext` onward. Apply the manifest definitions for `done` and `value`
  at their named checkpoints, including the later replacement of `value`.
- Each user-call/property-read stage has its own normal and abrupt edge.
  Values defined after a stage do not exist on that stage's abrupt edge.
- Preserve the generated `nextResult` temporary as an owned staged value and
  erase every value profile and mutable metadata field.

### `op_switch_string`

- Read `scrutinee`, resolve `tableIndex` to the copied string jump table, and
  use exact string code units as case keys.
- Add each table-relative branch offset to the switch instruction byte offset,
  validate the boundary, and emit the required default edge.
- Reject duplicate semantic keys, invalid min/max metadata, missing default,
  overflow, or a non-boundary target. Do not preserve hash-table bucket order
  or JSC string addresses.

### `op_catch`

- Consume explicit exception state from the selected abrupt predecessor.
- Define both `exception` and `thrownValue` at `entry` as the pinned opcode
  requires, then continue normally.
- Preserve `exception_state_read,reads_frame,writes_frame`. Do not query or
  clear a hidden JSC pending exception; the core-IR completion operation owns
  that state transition.
- Erase the mutable metadata buffer.

### `op_call_direct_eval`

- Read `callee`, `thisValue`, and `scope`; validate/expand `argc` and `argv`;
  preserve `lexicallyScopedFeatures`; and define `dst` on normal completion.
- Attach the accepted `DirectEvalContext`, selected static source/graph target,
  realm/referrer/conditions, and runtime `SyntaxError` recipe where applicable.
- Preserve heap, callback, throw, realm, scope, safepoint, and frame effects.
  A non-admitted target is a typed compile error, not a helper call that parses
  source at runtime.
- Erase call-link metadata and `valueProfile`.

### Encoding and metadata rows

When a raw stream starts with `op_wide16` or `op_wide32`, the generated
instruction decoder returns the underlying semantic opcode and converted
operands. Mark the prefix row `excluded` after width validation and emit only
the underlying operation. For an opcode metadata reference, validate enough
structure to read the instruction safely, acknowledge every metadata field,
and erase all of them. If an initializer says metadata copied an original
operand, consume the semantic operand row—not the mutable metadata copy.

## Mapping packet

Every opcode implementation or generated family entry must fill all fields
below. `none` is permitted only with a one-line invariant proving why the
category cannot occur.

```text
opcode:
owner_task / family:
applicable root and specialization modes:
manifest opcode ID:
semantic operand IDs:
conditional/optional rows and exact predicates:
cache-only operand/metadata IDs and exclusion reasons:
decoded byte offset / width / byte size validation:
value uses by entry/checkpoint:
value definitions by entry/checkpoint:
derived ranges or operands:
stable table/function/source references:
normal successors:
abrupt successors and handler selection:
checkpoint order and staged completions:
abstract semantic operation:
manifest effect ceiling:
retained effects:
narrowed effects and pinned proof:
roots / barriers / ownership / borrows / pins:
native re-entry publication and completion:
suspension / continuation / cancellation / interruption:
helper manifest entry or blocked_on_H010:
source/diagnostic association:
import validators:
core-IR validators:
closed-world dependency discovered:
unsupported-defined compile-error case:
contract escalation, if any:
```

The implementation is incomplete until every applicable manifest ID appears
in exactly one packet or shared declarative-table packet and the coverage
ledger has no open entries.

## Validators

Run these validators independently; passing an earlier layer does not waive a
later one.

1. **Pin/schema:** exact Bun/WebKit revisions, schema version, source hashes,
   generated locations, opcode/record counts, and exhaustive kind/role
   dispatch.
2. **Coverage:** every definition ID has one explicit mapping; every occurrence
   key is copied, conditionally absent, or excluded exactly once; repeated
   occurrences of the same definition remain valid and independently checked.
3. **Instruction structure:** monotonic byte offsets, exact widths/sizes,
   complete stream consumption, known opcode/type, and valid operand domains.
4. **References:** registers, constants, identifiers, tables, functions,
   environments, source ranges, branches, switches, handlers, and checkpoints
   resolve to the right copied domain.
5. **Flow:** exact entry/checkpoint uses and definitions, dominance, no use
   before definition, and no definition leaking onto an abrupt edge.
6. **Control:** complete normal/abrupt successors, half-open handler coverage,
   innermost selection, explicit returns/throws/traps, and boundary targets.
7. **Ownership:** no JSC pointer/tag/borrow in owned output; callback spans are
   copied; rooted cells do not escape; no raw pointer crosses re-entry,
   safepoint, suspension, or borrow end.
8. **Cache firewall:** excluded values have no dataflow to operations, types,
   layouts, guards, helper choice, identity, or diagnostics.
9. **Effects:** retained behavior is at least the manifest ceiling until each
   narrowing proof passes; callbacks, throws, safepoints, barriers, roots,
   suspension, realm/scope, and ambient state are explicit.
10. **Helper/ABI:** every runtime call has an accepted H010 entry and explicit
    tagged results; no opaque call, hidden check, or interpreter/JIT entry.
11. **Determinism:** stable IDs and serialization do not depend on addresses,
    hashes, threads, allocation order, or unspecified container order.
12. **Closed world:** all imported dependencies are graph members and all
    dynamic-code cases follow H002 diagnostics/runtime-error classification.

Malformed frontend data, an unknown manifest mapping, or a failed import
validator is a frontend/contract error. Unsupported defined behavior is a
typed compile error from lowering. Invalid core IR is an internal compiler
error. Do not blur these categories to make a test pass.

## Escalation matrix

| Finding | Owner/action |
| --- | --- |
| Pinned field/opcode lacks a row or classification | H006: update generator and inventory from pinned source. |
| Observable JavaScript/Bun/Web/Node behavior is unspecified or contradictory | H002/H003 contract decision before lowering. |
| Visitor needs a new semantic category or JSC-dependent owned representation | H003; do not add it locally. |
| Safe bridge cannot express a root, exception scope, lifetime, or protected extraction | H008 plus H003/H004 as applicable. |
| Core IR cannot represent an explicit state, edge, effect, ownership fact, or re-entry | H009 and contract escalation. |
| Concrete layout, calling convention, helper result, or target rule is missing | H010; leave `blocked_on_H010`. |
| Analysis cannot express a proof/guard/hint/unknown fact | H011; retain generic Tier 1 behavior. |
| Opcode family needs a semantic behavior not in its accepted recipe | H012-H017 report to H002/H003/H009, not a private node. |
| Generic helper would hide a check, callback, state transition, or fallback | H018/H010 redesign. |
| Ownership, pin, root, transfer, or lifetime cannot be represented | H004/H019 escalation. |
| Effect runtime behavior cannot use the shared ABI/re-entry/lifetime contract | H005/H020 escalation; no Effect whitelist. |

## H007 acceptance reviews

Two passes are required after any mapping-rule change.

### Review A: semantic, flow, and ownership attack

Assume every value aliases, every property/coercion calls user code, every
runtime operation allocates, every call throws, every suspension outlives its
source frame, and every overlapping handler matters. For each family and
worked mapping, trace operands, derived ranges, checkpoints, normal/abrupt
edges, roots, barriers, borrows, stable identities, sources/constants, and
cache erasure. Reject any recipe that could retain a JSC value or normalize a
distinguishable language state.

### Review B: implementer ambiguity and authority attack

Pretend to implement H008, H009, H010, and one opcode from every H012-H017
family using only the manifest and this guide. Enumerate every manifest kind,
field role, ownership mode, presence mode, effect, and applicability value.
Reject any case with two plausible observable mappings, any helper selection
without a complete contract, any instruction that requires guessing an IR or
ABI choice, and any missing escalation route.

Record both verdicts and accepted fixes in this section before changing H007
to `done`. A pass is valid only against regenerated schema-version-2 artifacts
and a clean `git diff --check` result.

### Recorded verdicts — 2026-07-30

- **Review A: PASS.** The final pass covered all six semantic opcode families,
  representative locals/branch/property/call/iterator/switch/catch/eval
  mappings, every checkpoint flow point, branch and handler resolution,
  constants/source fidelity, cache erasure, root/borrow escape, re-entry,
  suspension, and early failure. Accepted fixes separated definition coverage
  from repeated input occurrences, specified failure cleanup order, made
  tail-call and varargs flow explicit, and clarified that an effect ceiling
  cannot authorize behavior.
- **Review B: PASS.** A clean-room implementation pass enumerated every current
  manifest kind, field role, ownership and presence mode, semantic role,
  applicability value, effect, and H012-H017 family. Accepted fixes added
  literal ownership/presence/applicability and representation dispatch, and
  replaced ambiguous unsigned scalar roles with generated lexical-feature,
  property-attribute, structure-flag, scope-depth, scope-slot, and
  symbol-table-or-scope-depth roles. No remaining recipe requires an H007
  choice of shared IR or ABI semantics.

Both verdicts use manifest schema version 2 generated from WebKit
`34c01d13391e00c06862a3d2c5b7fff350ac87e0` after the scalar-role correction.
The generator check, independent regeneration comparison, manifest-vocabulary
coverage, CommonMark parse, and whitespace checks pass. There is no open H007
mapping blocker.
