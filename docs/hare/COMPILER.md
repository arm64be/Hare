# Hare Compiler Boundary

This document is the W0 contract between Hare's JSC import hook, Hare IR, and
native backend. It fixes ownership and representation boundaries that H006
through H010 share. It does not define the opcode inventory, final Rust type
names, LLVM data layout, command-line plumbing, or language semantics.

The semantic and undefined-behavior contract remains `HARE.md` and the H002
semantic contract. If a defined operation cannot be represented by this
compiler boundary, compilation fails or this contract is escalated; it never
causes execution to fall back to JSC.

## Source of truth

The first implementation targets Bun revision
`bbe3f6a2629adf808adbd0da199ae8c94a3c0d47` and oven-sh/WebKit revision
`34c01d13391e00c06862a3d2c5b7fff350ac87e0`.

Bun has two live handoffs in `src/jsc/bindings/ZigSourceProvider.cpp`:

- `generateCachedModuleByteCodeFromSourceCode` creates an
  `UnlinkedModuleProgramCodeBlock`, constructs its `SourceCodeKey`, and calls
  `JSC::encodeCodeBlock` at lines 218-224.
- `generateCachedCommonJSProgramByteCodeFromSourceCode` does the corresponding
  work for an `UnlinkedProgramCodeBlock` at lines 253-259.

Both functions hold the thread-local small `JSC::VM`, a `JSC::JSLockHolder`,
the `SourceCode`, `SourceCodeKey`, and `ParserError`. JSC has already recursively
generated nested function code blocks. There is no `JSGlobalObject`,
`CallFrame`, or running JavaScript at either handoff. The current Rust entry in
`src/jsc/CachedBytecode.rs` receives serialized bytes, so interception after
`CachedBytecode::generate` is too late.

The pinned JSC definitions that govern this boundary are:

- `Source/JavaScriptCore/runtime/CodeCache.cpp`: root parsing, recursive nested
  function generation, and the separate program, module, eval, and function
  paths;
- `Source/JavaScriptCore/bytecode/UnlinkedCodeBlockGenerator.cpp`: finalization
  of instructions, metadata, constants, identifiers, nested functions,
  expression information, handlers, switches, and rare data;
- `Source/JavaScriptCore/bytecode/UnlinkedCodeBlock.h`: the live code-block
  representation;
- `Source/JavaScriptCore/runtime/CachedTypes.cpp`: the complete set of fields
  consumed by cached-bytecode encoding and the separate function encoder;
- `Source/JavaScriptCore/heap/Strong.h` and `heap/DeferGC.h`: the protection
  mechanisms used by the hook lifetime policy below;
- `Source/JavaScriptCore/parser/SourceCode.h`: source-provider ownership.

Those paths and revisions are evidence, not an external Hare interchange
format. H006 must generate its inventory from the pinned definitions rather
than copying an opcode list out of this document.

## Direct hook

The Hare branch is inserted synchronously immediately before each
`JSC::encodeCodeBlock` call. A shared C++ helper accepts the root kind, `VM&`,
`SourceCode const&`, `SourceCodeKey const&`, and the typed top-level
`UnlinkedCodeBlock&`. In conceptual form:

```cpp
HareImportResult importForHare(
    JSC::VM&,
    HareInputKind,
    const JSC::SourceCode&,
    const JSC::SourceCodeKey&,
    JSC::UnlinkedCodeBlock&);
```

This is a shape contract, not a required symbol name. H008 owns the exact C++
and Rust declarations.

For a Hare artifact, success from this helper replaces bytecode encoding. It
returns an owned Hare compiler result to the bundler path; it does not produce
a `CachedBytecode`, serialize an intermediate block, or re-enter the existing
bytecode decoder. Non-Hare builds retain their existing encode path.

The hook has three synchronous phases:

1. Protect the complete live JSC tree and create the scoped visitor.
2. Copy semantic input through that visitor into owned Hare compiler data and
   construct the initial Hare IR.
3. Run import validation, destroy every borrowed view, and return the owned
   result.

Whole-program analysis, optimization, debug-dump rendering, LLVM emission, and
linking run after the hook releases the JSC lock and borrow scope. This keeps
the protected section bounded and makes it impossible for later phases to
depend on JSC object identity.

### Root and input kinds

`HareInputKind` must distinguish at least:

- module program;
- classic program/CommonJS;
- statically known direct eval;
- statically known Function-constructor source;
- a nested function block's call or construct specialization.

Module and program roots are different JSC types and must remain different
input variants through import. They must not be distinguished by a cast, a
field guess, or the source file extension.

Nested declarations and expressions are part of the recursively generated
tree. Async, generator, method, arrow, and constructor modes are properties of
their function input and code-block specialization, not alternate top-level
cached-bytecode formats. A specialization that JSC legitimately does not
generate is represented as absent; H006 decides from the pinned source which
variants are required for each parse mode. A needed but unavailable block is a
compile error, never permission to generate bytecode for runtime use.

Direct eval and the Function constructor do not flow through Bun's two current
top-level encode calls. H008's protected adapter must be reusable by those
paths. When their source is statically known, H017 adds them to the closed-world
build worklist and invokes the appropriate pinned JSC generation path through
that adapter. Direct eval also carries an explicit lexical-environment input
from its native caller. Function-constructor input carries its
global-environment semantics and parameter/body boundary. Runtime-computed
source remains a build error as specified elsewhere.

## Build-time JSC lifetime

An `UnlinkedCodeBlock` is a garbage-collected `JSCell`. The recursive generator
returns a raw pointer after its local `Strong` has ended, and Bun's current
encode call does not visibly create another root. Hare must not inherit that
implicit lifetime assumption.

The H008 hook must establish this scope, in this order:

1. Enter on the bytecode-cache VM's owning thread with its `JSLockHolder`
   already held.
2. Confirm that parsing succeeded, `ParserError` is invalid, and the typed root
   is non-null.
3. Construct a VM-scoped `JSC::DeferGC` (or the exact pinned-revision
   equivalent) before any operation that could allocate or otherwise reach a
   collection point.
4. Construct a `JSC::Strong` for the top-level block while GC remains deferred.
5. Visit the tree and build owned Hare data synchronously.
6. Destroy all transient views and the `Strong`, then leave the deferred-GC
   scope, lock, and function.

Both the `Strong` and deferred-GC scope are required. The root states the
ownership intent and remains correct if traversal later admits a collection;
the deferred-GC scope protects raw interior and child pointers used by the
adapter today. Removing either protection is a shared contract change that
requires a pinned-JSC lifetime audit.

The hook and visitor obey these invariants:

- The hook remains on the originating VM thread and under the same JSC lock.
- The C++ adapter does not run JavaScript, enter an interpreter/JIT frame, call
  user callbacks, or create a `JSGlobalObject` merely to import bytecode.
- Rust allocation is allowed. JSC heap allocation is forbidden during visitor
  callbacks unless H008 proves it unavoidable and keeps it inside the lock,
  root, and deferred-GC scope. Such a change must have an explicit exception
  and allocation audit.
- No raw `JSCell*`, `JSValue`, `Identifier`, `StringImpl*`, instruction pointer,
  metadata pointer, `SourceProvider*`, `SourceCode*`, `SourceCodeKey*`, VM
  pointer, or C++ container pointer crosses the protected scope.
- No borrowed slice or string outlives the callback that supplied it. Rust
  copies it before returning from that callback.
- `SourceCode` and `SourceCodeKey` keep the provider and its source alive for
  the complete hook. Hare-owned data stores stable content and source IDs, not
  provider pointers or JSC cache hashes.
- The bridge is non-reentrant. It does not call back into bundling or initiate
  compilation of another source until the current JSC borrow has ended.
- The Rust entry catches panics before returning across the C ABI. C++
  exceptions, Rust unwinding, and JSC exceptions never cross the bridge.

### Errors and exception state

Parser failures are handled before the hook and preserve the original
`ParserError` as an owned build diagnostic. The hook starts and ends with no
pending JSC exception. Its permitted JSC accessors are non-throwing and cannot
run user code.

The bridge uses an explicit result discriminant and owned diagnostics. At a
minimum it distinguishes success, unsupported defined input, invalid frontend
data, and internal bridge failure. Allocation failure follows Bun's established
OOM policy. A boolean with a discarded error is not sufficient.

Runtime JavaScript throws are unrelated to build-time JSC exception state.
They become explicit Hare IR abrupt edges and explicit runtime ABI results as
described below.

## Rust-facing visitor

Rust does not receive an `UnlinkedCodeBlock*`. C++ owns traversal and exposes a
sealed, synchronous visitor over semantic records. The safe Rust facade is
conceptually a non-`Clone`, non-`Send`, non-`Sync` borrow tied to one invocation:

```rust
struct BorrowedJscInput<'scope> {
    // private bridge capability; never a public JSC pointer
    _scope: PhantomData<&'scope mut ()>,
    _thread_bound: PhantomData<Rc<()>>,
}
```

The unsafe construction and all pointer interpretation stay in the bridge
module owned by H008. Safe consumers can only register a visitor/builder and
receive typed scalars or callback-scoped byte spans. They cannot retain the
capability, ask for an address, perform an unchecked cast, or call arbitrary
JSC APIs.

The visitor emits semantic categories rather than C++ object layouts:

- compilation identity, input kind, parse/script/code modes, lexical features,
  and stable source identity;
- a deterministic function tree and call/construct specialization edges;
- instruction opcode identity, width, offset, operands, and metadata
  references;
- constants in a lossless engine-independent form, including exact floating
  bits and exact JavaScript string code units;
- identifiers, locals, parameters, scope/`this` registers, and closure data;
- control-flow targets, switches, exception-handler ranges, and abrupt edges;
- expression/source locations and source-map associations;
- metadata, rare data, and flags required to preserve pinned JSC semantics.

The visitor must expose every field required by the generated H006 inventory.
An unexposed required field is an import error and contract-escalation request,
not a default value. Cache-only process state, addresses, profile counters, and
JSC object identity are excluded unless the semantic inventory proves they are
program input.

Bridge-local function and table IDs are dense integers, never pointer-derived.
The deterministic traversal is root first, then JSC's stored declaration order,
then expression order; distinct available specializations use call before
construct. H006 may refine the generated traversal table when pinned JSC data
requires another stable order, but it may not use hash-table order or memory
addresses.

Constants are normalized by meaning, not copied as JSC tagged bits. Strings
preserve lone surrogates through exact code units or an equally lossless tagged
encoding. Numbers preserve `-0` and exact IEEE-754 bits. BigInts preserve sign
and magnitude. Heap templates and executable constants become explicit
declarative records with stable references. If a value cannot be converted
without retaining a JSC cell, import fails and the representation is escalated.

## Hare IR boundary

The importer produces a completely owned, engine-independent compilation
unit. Its externally relevant contents are:

- stable source and function IDs;
- the complete function/control-flow graph;
- typed values and operations;
- constants and declarative runtime data;
- explicit normal, throw, cancellation, suspension, and other abrupt edges;
- explicit heap, region, root, barrier, ownership, lifetime, borrow, move, and
  pin operations where the semantic operation requires them;
- source locations for diagnostics and derived debug data;
- closed-world dependencies discovered while importing static eval,
  Function-constructor source, and statically known imports;
- a set of declared runtime capabilities/helpers that later lowering may need.

The unit contains no JSC pointer, JSC reference count, C++ vtable, `JSValue`
bit-pattern dependency, bytecode-cache record, or source-provider borrow. A
runtime representation may later be deliberately chosen to match a retained
JSC runtime ABI, but that layout choice is explicit in H010's target layout
manifest; it is never inherited accidentally from the build-time visitor.

This contract fixes semantic categories and invariants, not final node names,
enum spellings, crate boundaries, arena choice, or in-memory packing. H009 may
choose those local details. H009 must request a contract change before it:

- drops or combines distinguishable semantic states;
- makes an implicit exception, safepoint, ownership, or failure channel;
- adds a JSC-dependent representation to owned IR;
- changes the meaning of a shared runtime operation.

### Validation gates

Validation occurs at three boundaries:

1. **Import validation, inside the protected hook.** Every instruction and
   metadata family is recognized; IDs and indices are in range; instruction
   widths and block boundaries agree; branches, switches, handlers, constants,
   nested functions, and source references resolve; every transient JSC value
   has been converted to owned data. No unresolved visitor token may remain.
2. **Core IR validation, before analysis or LLVM.** SSA dominance and block
   arguments are valid; types and representations agree; normal and abrupt
   successors are complete; effects, safepoints, roots, barriers, ownership,
   lifetimes, and pins are internally consistent.
3. **Transformation validation.** Any pass that changes control flow, types,
   representations, effects, ownership, or safepoints reruns the affected core
   checks. Invalid IR never reaches a shipping link.

Unsupported defined behavior is a typed compile error from lowering. It is not
"invalid IR." Malformed JSC data, an unknown pinned-revision opcode, or a
missing inventory mapping is a frontend/contract error. This distinction keeps
coverage work separate from compiler correctness failures.

## Runtime ABI

The build-time JSC visitor ABI and the executable's runtime ABI are separate.
No build-time visitor symbol, root token, unlinked block, bytecode offset used
as executable state, parser object, or compiler service is reachable from a
Hare application.

Hare may retain Bun/JSC C++ runtime objects and GC machinery when native code
needs their semantics. It may also call generic native Tier 1 helpers. This is
not JSC execution fallback: all reachable application functions and control
flow remain Hare-compiled native code.

### Required helper contract

Every runtime helper has a generated, versioned manifest entry owned by H010
and consumed by H009 and the H018 helper implementation. The entry states:

- symbol and calling convention;
- fixed-width scalar types plus explicit target-layout pointer and aggregate
  types for every argument and result;
- semantic operation represented by the call;
- explicit realm/runtime-service handles it needs;
- normal and abrupt result variants;
- read, write, allocation, throw, suspend, callback, and synchronization
  effects;
- whether it is a safepoint and which live roots are reported;
- ownership of every pointer/handle argument and result: borrowed, consumed,
  newly owned, GC-traced, pinned, or copied;
- whether input may be retained after return and the explicit owner if so;
- optimizer attributes and invalidation rules.

The Hare IR operation remains the semantic source of truth until late
lowering. A helper body is linked as LTO-visible bitcode when the toolchain can
provide it. A necessarily external platform primitive receives only attributes
proved by its manifest. Neither an opaque call name nor an optimistic LLVM
attribute may stand in for missing semantic information.

A semantic, memory, ownership, or tier guard whose outcome selects observable
control flow is an IR predicate or tagged result followed by an explicit IR
branch. A helper may compute that predicate, but it may not perform the check
and silently choose the successor. This gives Hare and LLVM a real edge to
prove unreachable and remove.

An explicit runtime context or realm handle is allowed, but it contains runtime
services and observationally transparent caches only. Long-lived collector or
realm roots may be registered there with manifest-declared ownership. It may
not hide the current Hare frame, program counter, exception, completion kind,
live root set, ownership ledger, continuation, or deoptimization state. Those
are explicit IR values, frame fields, operands, or results.

Helpers that allocate or may collect are marked safepoints. The compiler emits
the root/frame descriptor and every live runtime reference before the call.
Helpers do not secretly retain borrowed inputs or create unreported roots.
Heap mutations and write barriers are represented by IR effects even when a
helper performs the final store or barrier.

A helper may implement a generic native algorithm such as allocation, string
or BigInt primitives, shape lookup, property storage, GC barriers, scheduler
services, or target intrinsics. It may use retained JSC object/GC internals.
Algorithmic scratch state and semantics-neutral caches are permitted when
declared. Observable checks, calls into user code, ownership transitions,
exceptions, suspension, and re-entry are not permitted to disappear inside an
unmodeled helper.

If a generic operation may invoke a getter, proxy trap, coercion method, or
other application callback, its IR has an explicit call/re-entry edge. A leaf
helper may resolve the next native target and return a tagged continuation
request, or the operation may be expanded into native Tier 1 control flow. It
must not enter the JSC interpreter or JIT to perform that callback.

### Forbidden fallback shapes

The following are contract violations even if they initially appear convenient:

- `execute_opcode(vm, frame, bytecode_pc)` or any equivalent runtime opcode
  dispatcher;
- passing an `UnlinkedCodeBlock`, serialized bytecode, or application source
  to the executable;
- calling JSC's interpreter, baseline JIT, optimizing JIT, eval compiler,
  Function constructor compiler, or bytecode decoder at runtime;
- a Tier 2 miss or failed speculation that transfers to JSC rather than a
  native Tier 1 successor;
- a helper that finds its exception, roots, frame, ownership state, or
  continuation through hidden thread-local/global state;
- a helper that can run application JavaScript without an explicit native call
  edge and declared re-entry effects;
- a generic "slow path" whose manifest does not say which semantic operation,
  checks, effects, and ownership transitions it implements.

Retaining a collector, object layout, string primitive, allocator, or other
reachable runtime utility is not by itself a violation. Reachability from
native code and the helper manifest decide the boundary; the historical fact
that a utility lives in JSC does not.

## Deterministic developer dumps

A dump is an optional diagnostic derived from the owned import model or
validated Hare IR after the JSC borrow ends. The production compiler never
writes a dump in order to read it back, and no cache key or build correctness
depends on parsing one.

For identical source, dependency graph, compiler and pinned WebKit revisions,
target, and profile inputs, a dump is deterministic:

- functions, blocks, values, constants, and metadata use stable dense IDs;
- ordering follows semantic/source order rather than allocation or hash-table
  order;
- pointers, process-specific hashes, timestamps, random seeds, and absolute
  workspace prefixes are absent or normalized;
- floating values and strings use lossless canonical spelling;
- the dump names its Hare schema revision, Bun revision, WebKit revision,
  target, and relevant profile identity.

Dumps may be snapshot-tested, diffed, and attached to fanout reports. They are
not a stable public format, runtime payload, serialization ABI, plugin API, or
promise that old dumps remain readable. There is no production dump parser.
The renderer may change when IR changes, provided determinism and diagnostic
tests are updated together.

## Worker ownership

H003 owns this shared contract. Downstream missions have the following
authority and escalation boundaries:

- **H006 — inventory:** generate the pinned opcode, operand, width, metadata,
  constant, effect, and extraction inventory. It may refine deterministic
  generated ordering and identify missing visitor fields. It may not invent IR
  semantics, pointer lifetimes, or runtime ABI behavior.
- **H007 — porting guide:** document repeatable JSC-to-visitor-to-IR mappings,
  validator patterns, and helper-selection templates keyed to the inventory
  schema. It can proceed from this contract and the pinned source, then consume
  generated H006 rows without changing their semantics. A mapping that changes
  a shared IR or helper meaning is escalated to H003, H009, or H010 rather than
  hidden in a template.
- **H008 — direct hook:** implement the two current call-site branches, a C++
  protection scope and adapter reusable by later static eval/function inputs,
  the sealed Rust visitor, owned errors, and skip-serialization result plumbing.
  It owns unsafe justification. Any pointer escape, JSC execution, mutable
  execution state, or inability to make exception/GC state explicit is an
  immediate escalation.
- **H009 — Hare IR:** choose concrete Rust node and storage spellings, build the
  importer and validators, and keep all semantic effects and ownership state
  explicit. It may not embed JSC layouts or weaken the import/core validation
  gates.
- **H010 — LLVM and ABI:** choose the exact target-compatible value, frame, and
  runtime-service layouts; generate helper declarations/manifests; emit LLVM;
  and verify toolchain compatibility. It may not hide a semantic check, add
  runtime compilation, or turn a native Tier 1 miss into JSC execution.

H006 and H007 may proceed independently once they both treat this document as
normative. Exact opcode rows, Rust type names, C symbol names, LLVM struct
packing, and developer dump spelling remain their owners' implementation
decisions. Changing an invariant above does not.

## Acceptance examples

These examples are contract tests for downstream designs:

1. **Classic program with nested functions.** C++ protects the
   `UnlinkedProgramCodeBlock`, visits its root and recursively generated
   function specializations in deterministic order, and Rust creates owned
   function IDs. Import validation succeeds before the lock is released. LLVM
   never sees a JSC cell pointer.
2. **ES module with async/generator code.** The root remains a module variant;
   lexical/script modes, suspension points, exception handlers, source spans,
   and nested parse modes survive import. Async/generator status is not inferred
   from a function name.
3. **Static direct eval.** The caller adds literal source and its explicit
   lexical environment to the worklist. JSC generates the separate eval block
   at build time under the same protected visitor scope. Runtime-dependent eval
   source produces a build error and ships no compiler.
4. **Static Function constructor.** Parameters, body boundary, global
   environment, parse mode, and generated call/construct blocks become another
   owned compilation unit. The separate JSC function encoding format is not
   serialized or treated as Hare IR.
5. **Generic property read through a proxy.** Hare IR represents the property
   semantics and possible call/throw edges. A native helper may perform shape
   lookup and return the proxy-trap target, but the trap executes through an
   explicit Hare native call edge, never a JSC interpreter entry.
6. **Allocation at a native slow path.** The helper manifest declares a
   safepoint and allocation effect. The caller reports all live runtime roots;
   the result has explicit ownership. A helper that discovers roots from a
   hidden bytecode frame fails review.
7. **Tier 2 guard miss.** The guard branches to an equivalent generic native
   Tier 1 subgraph. There is no deoptimization metadata or JSC destination.
8. **Developer dump.** Two builds with identical declared inputs produce the
   same text despite different addresses or thread scheduling. Deleting the
   dump does not affect the application artifact, and the compiler never reads
   it as input.

The H003 checkpoint is met when H006 can enumerate every required visitor
record, H007 can write mappings without choosing new ownership rules, H008 can
implement the protected synchronous bridge, H009 can build owned validated IR,
and H010 can specify exact LLVM layouts and helpers without adding a fallback
or hidden semantic state.
