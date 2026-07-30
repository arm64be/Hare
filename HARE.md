# Hare

Hare is a closed-world native compiler mode for Bun. It lowers the complete
statically reachable JavaScript, TypeScript, Effect, and Bun module graph
through JavaScriptCore bytecode into LLVM bitcode, then links the application
with Bun and the required runtime code under LTO and PGO.

The first target is the Bun 1.4 canary lineage at upstream revision
`bbe3f6a2629adf808adbd0da199ae8c94a3c0d47` on Linux x86-64.

JSC is Hare's build-time JavaScript frontend and semantic reference. It is not
an interpreter or deoptimization target in a Hare application. The final
application does not contain JavaScript source, JSC bytecode, a bytecode
interpreter, a JIT, or runtime JavaScript-to-bytecode compilation.

## Product Shape

Hare has one user-facing switch:

```sh
bun build --compile --hare ./app.ts --outfile app
```

There are no user-selectable safety, fallback, profiling, or required modes.
`--hare` means:

- discover the complete application graph
- compile every reachable JavaScript function to native code
- fail the build when defined behavior cannot be lowered
- reject runtime-generated JavaScript that cannot be known at build time
- emit one application-specific native executable

`eval("literal source")`, `new Function("literal source")`, and dynamic imports
with compile-time-known targets are additional build inputs and can be lowered.
Source text, module targets, or functions assembled from runtime data are build
errors. Hare never ships a compiler or interpreter as an escape hatch.

## Direct Compiler Hook

```text
TypeScript / JavaScript / Effect
              |
       Bun parser and bundler
              |
       source and type hints
              |
     JSC UnlinkedCodeBlock        build time only
              |
       Hare direct hook
              |
     Hare IR + whole-program analysis
              |
   automatic Tier 1 / Tier 2 / Tier 3 lowering
              |
          LLVM bitcode
              |
   Bun Rust + JSC runtime C++ + app bitcode
              |
       LTO + PGO + rust-lld
              |
     app-specific executable
```

Bun currently obtains an `UnlinkedCodeBlock` and passes it to JSC's cached
bytecode encoder. Hare should hijack that exact point: hand the live code block
to the Rust compiler, lower it, and skip bytecode serialization for the Hare
artifact. A deterministic dump is useful for debugging and fanout work, but an
intermediate external bytecode format is not part of the production pipeline.

The build-time compiler can still use JSC code-block metadata, constants,
identifiers, exception handlers, source origins, and opcode definitions. The
application link retains only runtime helpers that native code can reach. LTO
and section garbage collection remove the JSC parser, bytecode compiler,
interpreter, JIT, and unused runtime paths.

## Automatic Native Tiers

Tiers are internal compiler decisions, not user modes, work phases, or feature
subsets. One function may contain regions from multiple tiers. Hare promotes
each region as far as its facts permit.

| Tier | Scope | Lowering |
| --- | --- | --- |
| Tier 1 | Complete native semantics | Lower every reachable JSC bytecode operation, all Effect code, exceptions, objects, GC interactions, Result, backtrace, ownership, and pin rules using generic tagged values and optimizer-visible runtime checks. |
| Tier 2 | Inferred and specialized native | Use type, shape, call-target, escape, effect, ownership, lifetime, and profile information to unbox values, devirtualize operations, specialize layouts, remove allocations, and turn failed speculation into a native Tier 1 path. |
| Tier 3 | Full whole-program native | Resolve the closed graph, erase representations and checks proven unnecessary, fuse Effect control flow, lower ownership and pinning to concrete storage, use direct calls and native error edges, and expose the complete program to LLVM LTO and PGO. |

Tier 1 is deliberately broad. It is not a tiny integer-only tier and it is not
an interpreter written in LLVM. JavaScript semantics become ordinary native
control flow and runtime helper calls. Helpers and checks must be expressed so
Rust, Clang, LLVM, and the linker can inline, specialize, fold, and delete them.

Tier 2 speculation never returns to JSC. A guard that cannot be proven enters
the equivalent generic Tier 1 block. When analysis proves the guarded case is
the only defined case, LLVM can remove both the guard and generic path.

Tier 3 is the full-native result. It is not required for every input-dependent
check to disappear, but no boxed dispatch, GC barrier, bounds check, ownership
check, or runtime helper remains merely because Hare hid it behind an opaque
boundary.

## Defined Behavior And UB

Hare's semantic contract is the pinned combination of:

- ECMAScript
- Bun's documented APIs and compatibility contracts
- the applicable Web and Node.js specifications
- the complete `effect@3.22.0` implementation and public contract pinned in
  `docs/hare/EFFECT.md`
- the Hare memory, pinning, Result, and backtrace specifications

Defined behavior that Hare has not implemented is a compile error. It is not
silently changed and it does not fall back to JSC.

Behavior outside those specifications is undefined behavior under `--hare`.
The optimizer may assume it does not occur. This includes violating explicit
Hare ownership, aliasing, mutability, lifetime, or pinning contracts. Such
contracts may lower to `llvm.assume`, alias metadata, lifetime markers, or
equivalent facts after the verifier accepts them.

TypeScript types are different. They are hints, not contracts, because ordinary
JavaScript can violate them. A false TypeScript hint must still execute through
correct Tier 1 native semantics unless the programmer explicitly promotes it to
a Hare contract.

## Hard Invariants

1. `--hare` lowers the complete statically reachable graph or fails the build.
2. A Hare executable contains no JavaScript source, JSC bytecode, interpreter,
   JIT, or runtime source compiler.
3. There is no runtime fallback to JSC. Tier 2 guards enter native Tier 1 code.
4. Native execution preserves every behavior defined by the Hare semantic
   contract.
5. Behavior not defined by that contract is UB and may be used for optimization.
6. TypeScript types and profiles are hints until verified or explicitly made
   contractual.
7. Explicit memory and pin contracts are verified before becoming LLVM facts.
8. Any runtime semantic, memory, or ownership check is visible in Hare IR and
   removable when proof makes it unreachable.
9. Any runtime object reference live across a safepoint is visible to the
   selected collector or region owner.
10. Backtrace strings and captured dictionaries are materialized only after a
    failure edge is taken.
11. LLVM bitcode uses Bun's exact data layout, target features, and compatible
    LLVM toolchain.
12. Native compilation is deterministic for identical source, dependencies,
    compiler revision, target, and profile inputs.

## Hare IR

Hare IR is a typed SSA layer between JSC bytecode and LLVM. All important
semantics appear in the IR from Tier 1 onward so later passes can reason across
them rather than treating them as opaque runtime calls.

The IR includes:

- constants, locals, phi nodes, branches, switches, loops, and returns
- tagged and unboxed booleans, integers, doubles, strings, symbols, bigints,
  objects, functions, and references
- JavaScript coercion, comparison, arithmetic, property, prototype, and call
  semantics
- closures, generators, async functions, promises, modules, and static eval
- explicit throws, catches, cancellation, interruption, and failure edges
- heap and region loads/stores with shape, alias, and barrier metadata
- safepoints, roots, regions, lifetimes, moves, copies, borrows, and pins
- Effect nodes and scheduling operations without restricting the public API
- Result and backtrace nodes with static error, source, and data-schema IDs
- generic native slow paths represented as ordinary control-flow subgraphs

IR validation runs after construction and after transformations that can affect
control flow, types, ownership, or safepoints. Validation can be deferred during
mass fanout, but invalid IR never reaches a shipping link.

## Type And Memory Inference

Hare replaces the current project-specific MutMem checker with a general
whole-program type, effect, escape, alias, ownership, lifetime, and pin
analysis. Explicit MutMem syntax remains useful input, but non-MutMem code gets
the same inference automatically.

The analysis uses a JVM-like dataflow verifier over registers, stack state,
locals, objects, and control-flow merges. Each fact records its evidence:

| Evidence | Meaning | Compiler use |
| --- | --- | --- |
| Proven | Derived from bytecode semantics, constants, closed-world analysis, or a verified contract | May be used directly for correctness and optimization |
| Guarded | Valid after an emitted native check | Tier 2 specialization with a native Tier 1 miss path |
| Hint | TypeScript annotation, profile observation, or optimization advice | Seeds inference and code layout; never changes semantics alone |
| Unknown | No useful fact | Tier 1 generic native representation |

The inference pass runs for all code and propagates:

- primitive and object types
- object shapes and prototype stability
- call targets and closure environments
- reads, writes, throws, suspension, cancellation, and other effects
- allocation escape and scalar-replacement eligibility
- unique, shared, and mutable aliases
- moves, copies, borrows, and lifetime endpoints
- region membership and cross-thread transfer
- stable-address and pin requirements

Runtime rules are emitted as ordinary checks and control flow, not opaque
framework calls. LLVM can remove them when inlining, constant propagation,
alias analysis, or whole-program reasoning proves the failure edge unreachable.

## Generalized MutMem

The existing MutMem API is a prototype and will change. Hare's general-purpose
memory system has three inputs:

1. Facts inferred from ordinary JavaScript and TypeScript.
2. Non-binding hints from TypeScript and optimization annotations.
3. Explicit Hare contracts for ownership, aliasing, mutation, lifetime, and
   pinning whose violation is UB.

The compiler should infer the common case without annotations. Explicit
contracts exist for cases the compiler cannot prove, API boundaries, FFI, and
programmer-directed representation choices.

Tier 1 may retain checks or generic ownership operations. Tier 2 specializes
them. Tier 3 turns verified ownership into stack slots, regions, direct moves,
`noalias`, lifetime markers, scalar values, or nothing at all.

## Pinning

Hare pinning gives a borrow a stable storage identity across operations that
would otherwise move or invalidate it. It is designed to avoid making `Arc` or
heap allocation the default answer.

The compiler may satisfy a pin using:

- a stable stack slot
- an async or Effect continuation frame
- a scoped region or arena
- a pinned runtime/GC handle
- heap promotion
- reference counting only when ownership actually crosses independently-lived
  or cross-thread consumers

Pins can be inferred or explicit. A pinned owner may expose shared or mutable
pin projections to fields when projection cannot invalidate the parent. A pin
may cross suspension when the owning continuation frame is itself stable. It
ends when the compiler proves the last pinned borrow is dead, allowing storage
to be reclaimed or moved again where the contract permits.

Tier 1 implements pin validity with explicit native state when necessary. Tier
2 chooses cheaper carriers from escape and suspension analysis. Tier 3 erases
pin bookkeeping when stable placement and lifetimes are fully proven.

## Result And Backtrace

Result and backtrace semantics exist in Tier 1 and are optimized continuously;
they are not a late feature tier.

A native Result uses a compact tagged representation selected per call graph.
The success path does not capture a stack, copy a dictionary, allocate an
Effect failure, or resolve source strings. Static tables hold literal error,
source, logical Effect frame, and data-schema identifiers.

Tier 1 can materialize the complete user-facing failure from native tables.
Tier 2 propagates known result tags and removes dead failure or success edges.
Tier 3 lowers surviving errors to direct native control flow and retains only
failure data reachable in the final program.

## Effect

Hare supports all of the pinned Effect package. It does not define or ship a
supported combinator subset.

Tier 1 gets completeness by lowering Effect's complete reachable JavaScript
implementation to native code like every other dependency. An Effect-aware IR
pass then identifies its data representations, continuations, scopes,
finalizers, services, fibers, interruption, scheduling, concurrency, and async
state without depending on a small whitelist of public functions.

Tier 2 specializes Effect nodes, fuses continuations, unboxes environments,
removes known scheduler and interruption branches, and chooses storage for
fiber and scope state. Tier 3 lowers the whole known Effect graph into native
state machines and direct control flow wherever semantics permit.

Checks required by Effect semantics remain correct native checks. Because they
are visible in IR, LLVM may delete them when the complete program proves that
interruption, failure, cancellation, finalization, or concurrency cannot occur
on that path.

## Profiles

Hare uses two kinds of profile after correctness is established:

1. Semantic profiles provide non-binding hints about values, shapes, calls,
   branches, exceptions, Effect operations, and hot Tier 1 paths.
2. LLVM IR-PGO guides native inlining, layout, and optimization across the app,
   Rust Bun runtime, and C++ runtime code.

Profiles never define behavior or authorize memory safety. A stale or false
semantic profile merely takes a generic native Tier 1 path.

## Development Build Policy

Correctness and coverage come before release performance. Development uses
debug incremental builds:

- use `bun bd` and focused debug crate builds
- reuse one integration build directory and dependency cache
- do not run release, LTO, PGO, BOLT, or performance builds during feature
  fanout and compile-error convergence
- do not require every worker commit or fanout batch to build or test
- allow intermediate integration branches to contain large compile-error queues
- run centralized debug builds only at convergence checkpoints
- begin release benchmarking only after Tier 1 feature completeness and the
  correctness corpus pass

The initial baseline records the pinned upstream debug build, selected upstream
behavior, incremental rebuild cost, peak build memory, and disk use. It does not
set runtime performance targets. Runtime baselines are collected later from a
correct release build.

## Engineering Waves

These are convergence waves for development, not user modes or Effect/JavaScript
feature subsets.

### W0: Serialize The Mission

- Freeze the semantic contract and UB boundary.
- Write the JSC hook, Hare IR, runtime ABI, inference, pinning, and lowering
  contracts.
- Generate opcode, lifetime, ownership, and feature inventories.
- Capture the upstream debug and incremental-build baseline.

Exit gate: fanout workers can implement large shards without inventing shared
interfaces.

### W1: Direct Hook And Compiler Skeleton

- Hijack the live `UnlinkedCodeBlock` path.
- Establish Hare IR, analysis, LLVM emission, and build integration crates.
- Generate debug dumps and instruction tables.
- Link one native function through the debug build.

Exit gate: the build-time path reaches linked native code without serializing
application bytecode.

### W2: Tier 1 Mass Fanout

- Fan out complete JSC instruction families across workers.
- Lower modules, builtins, exceptions, closures, async, generators, objects,
  runtime calls, Result, backtrace, memory rules, pinning, and all Effect code.
- Commit large coherent shards without requiring the combined tree to compile.
- Treat missing support as an explicit compile error, never a runtime fallback.

Exit gate: all generated instruction and reachable-feature inventory entries
have an implementation owner and landed code.

### W3: Compiler And Linker Convergence

- Turn compiler errors into sharded work queues.
- Then turn linker errors into sharded work queues.
- Resolve interface drift centrally rather than letting every worker invent a
  compatibility shim.
- Use debug incremental builds only.

Exit gate: the full Tier 1 compiler and a representative Hare application link
without JavaScript bytecode or runtime compilation.

### W4: Correctness Convergence

- Run upstream compatibility, differential, adversarial, generated, fuzz,
  Effect, Result, backtrace, ownership, pinning, GC, exception, and async tests.
- Shard each failure family to implementer and adversarial-review loops.
- Fix systemic generators or specifications when failures share a cause.

Exit gate: the complete defined Tier 1 surface passes the selected correctness
corpus and unsupported defined behavior fails at build time.

### W5: Tier 2 Fanout

- Deepen the conservative Tier 1 type and memory inference into interprocedural
  facts for annotated and ordinary code.
- Fan out specialization, unboxing, devirtualization, escape analysis, region,
  pinning, Effect, Result, and generic-path optimization passes.
- Add semantic profiling as hints.

Exit gate: every Tier 2 guard has an exact native Tier 1 miss path and every
optimization survives differential testing.

### W6: Tier 3 Whole-Program Native

- Resolve closed-world calls, layouts, effects, ownership, pins, and state
  machines across the complete graph.
- Remove build-time-only JSC payloads and unreachable runtime compiler/JIT code.
- Expose app and runtime bitcode to full LTO and PGO.
- Verify the final executable contains no source, bytecode, interpreter, JIT, or
  runtime compiler path.

Exit gate: the target corpus builds as standalone Tier 3 native applications.

### W7: Performance, Hardening, And Platforms

- Establish release baselines only now.
- Run adversarial timing, memory, binary-size, and build-resource benchmarks.
- Run ASAN, Miri where applicable, fuzzing, differential generation, and stress.
- Add platforms incrementally without weakening the semantic contract.

Exit gate: performance and platform claims include correctness, memory, binary,
build, and variance evidence.

## Decision Gates

The mission-critical lane decides when:

- the pinned specs disagree or leave behavior undefined
- a JSC runtime object crosses a native safepoint
- exception, GC, Effect, or pin state cannot be represented explicitly in IR
- a contract would need to become hidden runtime bookkeeping
- LLVM data layouts, bitcode versions, or LTO units disagree
- a worker needs to change a shared IR, ABI, manifest, or semantic rule
- a proposed optimization changes defined behavior instead of exploiting UB

The fanout and review process lives in `HARE_WORKSTREAMS.md`. The shared work
queue lives in `HARE_TASKS.tsv`.
