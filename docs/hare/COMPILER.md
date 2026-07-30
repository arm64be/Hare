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
- `Source/JavaScriptCore/bytecode/UnlinkedFunctionExecutable.{h,cpp}`: the
  GC-managed Function-constructor executable and lazy specialization path;
- `Source/JavaScriptCore/runtime/CachedTypes.cpp`: the complete set of fields
  consumed by cached-bytecode encoding and the separate function encoder;
- `Source/JavaScriptCore/heap/Strong.h` and `heap/DeferGC.h`: the protection
  mechanisms used by the hook lifetime policy below;
- `Source/JavaScriptCore/heap/Heap.h` and `runtime/VM.cpp`: VM access and
  teardown behavior;
- `Source/JavaScriptCore/parser/SourceCode.h`: source-provider ownership.

Those paths and revisions are evidence, not an external Hare interchange
format. H006 must generate its inventory from the pinned definitions rather
than copying an opcode list out of this document.

## Direct hook

The Hare branch is inserted synchronously immediately before each
`JSC::encodeCodeBlock` call. The top-level C++ adapter accepts the root kind,
`VM&`, `SourceCode const&`, `SourceCodeKey const&`, and the typed top-level
`UnlinkedCodeBlock&`. Function-constructor inputs use a separate adapter for
their `UnlinkedFunctionExecutable`. In conceptual form:

```cpp
HareImportResult importTopLevelForHare(
    JSC::VM&,
    HareInputKind,
    const JSC::SourceCode&,
    const JSC::SourceCodeKey&,
    JSC::UnlinkedCodeBlock&) noexcept;

HareImportResult importFunctionExecutableForHare(
    JSC::VM&,
    const JSC::SourceCode&,
    const JSC::SourceCodeKey&,
    JSC::UnlinkedFunctionExecutable&,
    const HareFunctionSpecializationPlan&) noexcept;
```

These are shape contracts, not required symbol names. H008 owns the exact C++
and Rust declarations, but it must preserve the separate object types and
`noexcept` boundary.

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
that adapter. Runtime-computed source remains a build error as specified
elsewhere.

### Direct eval context

An admitted direct eval has a `DirectEvalContext` owned by Hare IR. The name is
conceptual, but the following inputs are mandatory:

- the active realm and its global environment;
- lexical, variable, and private-name environments as distinct identity-bearing
  handles;
- caller strictness and the strictness derived from the eval source;
- the caller's `this` binding status and value;
- present-or-absent `new.target`, `super` binding, home object, and derived
  constructor state;
- caller script/module parse mode, function parse mode, ordinary/arrow/method
  kind, class context, class-field initializer state, and private-brand
  requirements;
- TDZ, declaration-instantiation, Annex B where applicable, and lexical versus
  variable declaration rules;
- the completion-value destination plus normal, return-forbidden, throw, and
  other abrupt-completion behavior;
- native storage locations, root ownership, mutability, and stable environment
  IDs for every captured binding.

The environments are not value snapshots. Their identity and native storage
must make eval reads and writes observable to the caller, including sloppy
`var` declaration effects, while preserving strict eval isolation. `this`,
`new.target`, `super`, and private names reference their caller bindings rather
than copied values where the language requires identity.

H008 supplies the pinned JSC parse/generation context; H009 represents the
owned context and its declaration/completion operations; H010 defines the
runtime environment-handle layout. If any mandatory input or identity rule
cannot be represented, that direct-eval case is rejected at compile time. An
invalid admitted source string is different: it imports a runtime
`SyntaxError` recipe as specified under errors below.

### Function constructor and executable adapter

The Function constructor is not imported through an
`UnlinkedCodeBlock&`. Pinned JSC first creates an
`UnlinkedFunctionExecutable`; its call and construct code blocks are generated
lazily by `UnlinkedFunctionExecutable::unlinkedCodeBlockFor`. The generation
wrapper classifies an initial `ParserError` before calling the adapter; only a
successful non-null executable enters it. The separate adapter must:

1. protect the executable as a GC cell under the build-time VM policy;
2. derive the semantically reachable specialization plan from construct
   ability, call/construct use, parse mode, and the pinned inventory;
3. materialize each required lazy specialization under the JSC lock before
   ordinary read-only visitation;
4. root the executable and every materialized block for the complete visit;
5. recursively materialize and visit their nested function trees;
6. return each lazy-specialization `ParserError` to the wrapper for the
   call-site classification below.

Lazy materialization may allocate JSC cells and mutate executable write
barriers. H008 must keep it outside Rust visitor callbacks and provide a
line-by-line allocation, exception, rooting, and partial-failure cleanup audit.
No executable or generated block pointer escapes the protected scope.

Every admitted construction carries a `FunctionConstructorContext` with:

- runtime `ToString` operations for every parameter/body argument in specified
  order, including explicit native re-entry and throw edges;
- the caller realm, constructor/callee realm, and callee global environment as
  distinct handles;
- an explicit `HostEnsureCanCompileStrings`-equivalent policy operation with
  both realm inputs and its abrupt result;
- call-versus-construct mode, constructor identity, `newTarget`, and the
  `GetPrototypeFromConstructor`-equivalent prototype lookup with re-entry and
  throw behavior;
- function kind and construct ability;
- the exact parameter/body boundary and source identity passed to the pinned
  parser;
- native allocation, prototype, realm, environment, name/length, and root
  ownership inputs for the resulting function object.

The IR preserves the specified ordering of conversion, host policy, parsing,
prototype lookup, allocation, and abrupt completions. Build-time parsing may
precompute the success executable or an error recipe, but it must not skip or
reorder observable runtime operations. If the complete context cannot be
represented, the construction is a compile error rather than a fallback.

## Build-time JSC lifetime

### VM ownership and teardown

The build-context worker owns the bytecode-cache VM. A raw thread-local pointer
is never the owner. The default H008 design is a `HareBuildJscContext` (exact
name unfrozen) that owns each worker thread, its small `JSC::VM` reference, heap
access, and work queue for the duration of one Hare build.

Teardown is ordered:

1. stop accepting compiler work and cancel or complete queued imports;
2. wait until no hook, visitor, `Strong`, or deferred-GC scope is active;
3. request worker shutdown and join every VM-owning thread;
4. on each owning thread, drain required JSC teardown, clear its non-owning TLS
   cache and generation token, release heap access, and release the VM owner;
5. destroy the build context only after every join completes.

A TLS cache may contain only a borrowed `(owner, generation, VM)` lookup. Every
entry validates the live owner and generation before use and is cleared before
the worker or owner ends. A later build cannot observe a prior build's VM.

A deliberate process-lifetime VM owner is an allowed alternative only when its
registry, owning threads, shutdown-at-process-exit order, and permanent heap
access are explicit and the worker threads cannot exit independently. It must
still clear non-owning TLS on thread exit. An intentionally leaked raw `VM*`
without that owner is forbidden.

An `UnlinkedCodeBlock` is a garbage-collected `JSCell`. The recursive generator
returns a raw pointer after its local `Strong` has ended, and Bun's current
encode call does not visibly create another root. Hare must not inherit that
implicit lifetime assumption.

The H008 hook must establish this scope, in this order:

1. Enter on the bytecode-cache VM's owning thread with its `JSLockHolder`
   already held.
2. Classify the input's parser result. A successful top-level or admitted
   dynamic-code input continues with a non-null typed root; an expected parser
   failure follows the error rules below.
3. Construct a VM-scoped `JSC::DeferGC` (or the exact pinned-revision
   equivalent) before any operation that could allocate or otherwise reach a
   collection point.
4. Construct a `JSC::Strong` for the top-level block or function executable
   while GC remains deferred. Root separately materialized function blocks for
   the same scope.
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

Parser outcomes are classified by source role:

- A parser failure for the application root, a statically selected module, or
  generated bundler output is an owned build diagnostic because there is no
  admitted runtime parse operation at which it could be caught.
- A parser failure for admitted direct-eval or Function-constructor source is
  an owned `SyntaxError` construction recipe. Hare emits construction and throw
  edges at the original eval call or Function call/construction site, after all
  earlier observable argument conversion, policy, and re-entry operations.
  `try`/`catch` can therefore observe it at runtime.
- Failure to prove source closed-world or to represent the mandatory eval or
  Function context is a build diagnostic, not a runtime `SyntaxError`.
- Malformed data after a JSC success result is an invalid-frontend/internal
  error, not an unsupported-program diagnostic.

The owned syntax-error recipe preserves the pinned parser category, message
data, source identity, and source position needed to construct the runtime
error. For a finite source set, each recipe is attached only to its matching
runtime-selected source variant. It contains no JSC exception or parser
pointer.

The hook starts and ends with no pending JSC exception. Read-only visitor
accessors are non-throwing and cannot run user code. Lazy Function executable
materialization is a separate audited JSC generation phase and returns its
`ParserError` explicitly.

The C++ entry functions are `noexcept`. They use a catch-all boundary around
JSC adapter work and the Rust call:

- known non-OOM failures map to a discriminated owned `HareImportResult`;
- an unexpected C++ exception maps to `InternalCxxException` without allowing
  the exception to cross C;
- `std::bad_alloc` and equivalent allocation failure invoke Bun's existing OOM
  policy rather than pretending the import is a recoverable program error;
- construction or destruction of an error result must itself be non-throwing.

The Rust `extern "C"` entry wraps its entire implementation in
`catch_unwind(AssertUnwindSafe(...))`. With `panic=unwind`, a panic becomes an
owned `RustPanic` internal-error result. A `panic=abort` build is process-fatal
by policy and must never claim recoverable mapping. No Rust unwind reaches C++.

The result discriminant covers at least success with an owned compiler handle,
root/module build diagnostic, admitted dynamic-code syntax-error recipe,
unsupported defined input, invalid frontend data, Rust panic, C++ exception,
and internal bridge failure. Every payload has one named owner and an explicit
non-throwing destroy function. A boolean with a discarded error is forbidden.

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

H006 owns a generated extraction manifest covering every pinned opcode,
operand, operand-width rule, metadata entry, rare-data field, constant kind,
function relationship, and source/source-key field visible at the boundary.
Each row contains:

- pinned WebKit revision, source path, symbol/type, and generated definition
  location;
- input/root/function specialization applicability;
- binary classification as `semantic` or `cache_only`, with a semantic subrole
  such as execution, control flow, validation, or diagnostics;
- required, optional, or conditionally present status and the exact condition;
- representation, signedness/encoding, width source, and operand/field role;
- C++ extraction accessor or generated method;
- callback borrow, copied scalar/bytes, stable ID, rooted cell during visit, or
  excluded ownership;
- destination visitor record and validation rule, or the reason a cache-only
  field is excluded.

The generator fails if any pinned definition has no row, a semantic row has no
extraction method, or a cache-only row is consumed as program meaning. H003
does not contain the generated rows; H006 remains their source and owner.

The visitor must expose every semantic field required by that manifest. An
unexposed required field is an import error and contract-escalation request,
not a default value. Cache-only process state, addresses, profile counters, and
JSC object identity are not imported unless a later pinned-source review
reclassifies them through the shared contract.

Bridge-local function and table IDs are dense integers, never pointer-derived.
Top-level recursive traversal follows pinned JSC generation: visit the root,
then each block's stored declarations by index, then expressions by index,
recursing into exactly the specialization that pinned `CodeCache.cpp`
generated. At this revision a constructor child selects its permitted construct
block, a non-constructor selects its call block, and construct generation is
omitted for the pinned async modes. Hare does not invent a missing call block
or impose call-before-construct ordering.

The separate Function-executable adapter visits exactly the lazy
specializations required by its specialization plan. If that plan contains
more than one block, their stable order comes from H006's generated pinned
specialization manifest rather than a universal call-first rule. No traversal
uses hash-table order or memory addresses.

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
- complete direct-eval and Function-constructor contexts, including
  identity-bearing environment and realm handles;
- admitted dynamic-code syntax-error recipes attached to their runtime throw
  sites rather than promoted to build failures;
- explicit heap, region, root, barrier, ownership, lifetime, borrow, move, and
  pin operations where the semantic operation requires them;
- explicit native re-entry operations, continuation/root publication, and
  ambient-runtime-state operations;
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
   has been converted to owned data; every manifest row is classified and
   satisfied. No unresolved visitor token may remain.
2. **Core IR validation, before analysis or LLVM.** SSA dominance and block
   arguments are valid; types and representations agree; normal and abrupt
   successors are complete; effects, safepoints, roots, barriers, ownership,
   lifetimes, pins, direct-eval/Function contexts, re-entry publications, and
   ambient-state save/restore pairs are internally consistent.
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
- native re-entry, scheduling, yield, cancellation, interruption, and
  termination effects;
- ambient-state reads/writes, owner scope, and required save/restore token;
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

### Native re-entry protocol

Every path that invokes Hare-compiled application code from a generic helper,
host callback, getter, proxy trap, coercion, scheduler, Effect operation, or
other runtime service uses one `NativeReentry` protocol. The exact IR spelling
is H009's, but the state transitions are fixed.

Before the callback, native code:

1. materializes and publishes the caller's native continuation and frame
   instance together with its immutable frame-layout descriptor;
2. publishes the live-root descriptor or stable root handles for every managed
   value that survives the call, including values held by borrows or pins;
3. commits pending heap writes and barriers and ends any raw-pointer access
   that a callback or collection could invalidate;
4. supplies explicit active realm, worker/agent, and optional Effect-fiber
   handles plus their prior ambient-state token;
5. records the callback target, arguments, receiver/newTarget where applicable,
   ownership transfer, and declared re-entry/scheduling effects.

The callback is a Hare native entry point. It returns an explicit completion:
normal value, throw value, suspension/yield token, worker/agent termination, or
the other abrupt completion admitted by that operation. No JSC pending
exception or interpreter `CallFrame` carries the result.

On synchronous return or throw, the protocol:

1. restores the prior realm, worker, fiber, and other ambient state on every
   exit;
2. transfers the explicit result or exception to the caller's corresponding IR
   successor;
3. revalidates liveness, generation/version, detach state, aliases, ownership,
   borrow and pin validity, and any cached shape/storage pointer affected by
   callback mutation;
4. reloads managed pointers after revalidation and selects the defined generic
   path or abrupt edge when an assumption was invalidated;
5. unpublishes the continuation and temporary roots only after no resume path
   can observe them.

If the callback suspends or schedules later work, the continuation owner keeps
the frame, root handles, ambient token, and cancellation/termination state
alive until exactly one resume or terminal path consumes them. The manifest
states whether the operation may enqueue microtasks, jobs, worker messages, or
Effect scheduler work; yield; migrate between permitted workers; or terminate
an agent. Late callbacks use an explicit one-shot state transition and never
deref released frame state.

Nested re-entry forms an explicit stack/list owned by the active realm/worker
runtime service. Push, restore, suspend, resume, and termination are IR/runtime
operations with manifest effects. A thread-local cache may accelerate lookup
but cannot be the authoritative continuation, root, realm, exception, or fiber
state.

### Ambient runtime state

Observable ambient state is an explicit Hare operation and runtime-service
manifest family. Examples include active realm/worker identity, async context,
and Effect's current fiber. Every ambient-state entry declares:

- its stable state key and whether reads/writes are user-observable;
- process, agent, worker, realm, or Effect-runtime owner;
- value representation, root/ownership carrier, and cross-thread rule;
- read, write, push, save, restore, clear, and lookup operations;
- nesting and save/restore behavior across native re-entry;
- persistence across yield/suspension and ownership of the suspended token;
- scheduling, worker migration, throw, cancellation, interruption, and
  termination behavior;
- cleanup ordering on normal, abrupt, and owner-teardown exits.

The active state service is passed explicitly to IR operations and helpers. It
may store authoritative data in its declared realm/worker owner, but not in
hidden deoptimization metadata, an interpreter frame, or unowned TLS/global
state.

For Effect current fiber specifically, the state is realm/worker-scoped,
GC-rooted while installed, saved before nested callbacks, restored on every
normal or abrupt exit, and carried explicitly across yield/resume. Reads by
reachable Effect or user code observe the currently installed fiber in the
order required by the pinned implementation. Worker termination or fiber
completion clears/restores it before releasing its owner.

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
- ambient realm, worker, async-context, or Effect-fiber state hidden in
  deoptimization metadata, an interpreter frame, or unowned TLS/global state;
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

- **H006 — inventory:** generate the exhaustive extraction manifest and its
  coverage check for pinned opcodes, operands, widths, metadata, rare/source
  fields, constants, effects, optionality, extraction, ownership, and source
  provenance. It may identify missing visitor fields but may not invent IR
  semantics, pointer lifetimes, or runtime ABI behavior.
- **H007 — porting guide:** document repeatable JSC-to-visitor-to-IR mappings,
  validator patterns, and helper-selection templates keyed to the inventory
  schema. It can proceed from this contract and the pinned source, then consume
  generated H006 rows without changing their semantics. A mapping that changes
  a shared IR or helper meaning is escalated to H003, H009, or H010 rather than
  hidden in a template.
- **H008 — direct hook:** implement the two current call-site branches,
  DirectEval and FunctionExecutable adapters, lazy-specialization audit,
  build-VM owner/teardown, C++ protection scope, sealed Rust visitor,
  `noexcept`/panic containment, owned errors, and skip-serialization result
  plumbing. It owns unsafe justification. Any pointer escape, JSC execution,
  mutable execution state, stale VM lifetime, or inability to make
  exception/GC state explicit is an immediate escalation.
- **H009 — Hare IR:** choose concrete Rust node and storage spellings, build the
  importer and validators, represent the complete eval/Function contexts,
  syntax-error recipes, native re-entry, and ambient state, and keep all
  semantic effects and ownership explicit. It may not embed JSC layouts or
  weaken the import/core validation gates.
- **H010 — LLVM and ABI:** choose the exact target-compatible value, frame, and
  runtime-service layouts; generate helper declarations/manifests including
  re-entry, root publication, and ambient state; emit LLVM; and verify toolchain
  compatibility. It may not hide a semantic check, add runtime compilation, or
  turn a native Tier 1 miss into JSC execution.

H006 and H007 may proceed independently once they both treat this document as
normative. Exact opcode rows, Rust type names, C symbol names, LLVM struct
packing, and developer dump spelling remain their owners' implementation
decisions. Changing an invariant above does not.

## Acceptance examples

These examples are contract tests for downstream designs:

1. **Classic program with nested functions.** C++ protects the
   `UnlinkedProgramCodeBlock`, visits its root and recursively generated
   pinned call-or-construct specialization for each child in declaration and
   expression order, and Rust creates owned function IDs. Import validation
   succeeds before the lock is released. LLVM never sees a JSC cell pointer.
2. **ES module with async/generator code.** The root remains a module variant;
   lexical/script modes, suspension points, exception handlers, source spans,
   and nested parse modes survive import. Async/generator status is not inferred
   from a function name.
3. **Catchable admitted syntax errors.** `eval(")")` and
   `new Function("return )")` import owned parser-error recipes. Their runtime
   sites perform all earlier required operations, construct `SyntaxError`, and
   take an explicit throw edge catchable by surrounding `try`/`catch`. A syntax
   error in a root module remains a build diagnostic.
4. **Static direct eval.** A method containing `eval("this.#x")` supplies its
   realm, lexical/variable/private environments, `this`, method/class/private
   state, declaration rules, storage identities, and completion destination.
   A function containing `eval("new.target")` supplies the actual caller
   binding. Missing context is a compile error; it is not replaced with a global
   eval or snapshot environment.
5. **Static Function constructor.** Runtime argument `ToString`, host policy,
   realm, `newTarget`, prototype lookup, parameter/body boundary, and abrupt
   edges remain explicit. Build time protects the
   `UnlinkedFunctionExecutable`, materializes only the required lazy
   specializations, imports them, and serializes no function bytecode.
6. **Generic property read through a proxy.** Hare publishes the caller frame,
   continuation, roots, realm/worker/fiber state, and callback effects before
   invoking the native proxy trap. On return or throw it restores ambient state
   and revalidates liveness, aliases, detach state, and cached storage before
   resuming. The trap never enters a JSC interpreter.
7. **Effect current fiber.** Nested Effect callbacks observe the newly installed
   fiber; normal return, throw, yield/resume, interruption, and termination
   restore or clear the prior realm/worker-owned state through explicit
   operations.
8. **Allocation at a native slow path.** The helper manifest declares a
   safepoint and allocation effect. The caller reports all live runtime roots;
   the result has explicit ownership. A helper that discovers roots from a
   hidden bytecode frame fails review.
9. **Build VM teardown.** After the last import, the build context stops work,
   joins every VM-owning thread, clears TLS on those threads, and only then
   releases heap access and the VM owner. Starting another build cannot reuse
   the old pointer or generation.
10. **Extraction coverage.** Adding a pinned metadata or rare-data field makes
    H006's generator fail until the row classifies it, specifies optionality,
    extraction and ownership, and maps it to a validator or cache-only
    exclusion.
11. **Tier 2 guard miss.** The guard branches to an equivalent generic native
   Tier 1 subgraph. There is no deoptimization metadata or JSC destination.
12. **Developer dump.** Two builds with identical declared inputs produce the
   same text despite different addresses or thread scheduling. Deleting the
   dump does not affect the application artifact, and the compiler never reads
   it as input.

The H003 checkpoint is met when H006 can enumerate every required visitor
record, H007 can write mappings without choosing new ownership rules, H008 can
implement the protected synchronous bridge, H009 can build owned validated IR,
and H010 can specify exact LLVM layouts and helpers without adding a fallback
or hidden semantic state.

This repaired contract meets that checkpoint and unblocks H006 and H007. Their
remaining choices are generated inventory rows and mapping templates, not
shared eval, Function, VM, re-entry, ambient-state, ownership, or ABI policy.
