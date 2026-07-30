# Hare

Hare is an application-specific native compilation mode for Bun. It combines
JavaScriptCore bytecode, Effect programs, MutMem ownership proofs, and compact
failure backtraces into LLVM bitcode that can participate in Bun's existing
cross-language LTO and PGO build.

The first target is the Bun 1.4 canary lineage at upstream revision
`bbe3f6a2629adf808adbd0da199ae8c94a3c0d47` on Linux x86-64.

Hare is not a new JavaScript language and it is not a replacement for JSC. JSC
remains the compatibility runtime, interpreter, and deoptimization target.
Native compilation is an optimization that must never change observable
program behavior.

## Product Shape

The intended release command is conceptually:

```sh
bun build --compile --native=safe ./app.ts --outfile app
```

Build modes:

| Mode | Meaning |
| --- | --- |
| `off` | Existing Bun behavior. No Hare analysis or native application code. |
| `safe` | Compile only statically proven regions. Everything else stays in JSC. |
| `profiled` | Add guarded specialization from a semantic training profile. |
| `required` | Fail the build when a requested native region cannot be proven or lowered. |

The CLI names are provisional until the first end-to-end spike proves the
build shape.

## Pipeline

```text
TypeScript / JavaScript / Effect
              |
       Bun parser and bundler
              |
       source and type metadata
              |
       JSC UnlinkedCodeBlock
              |
       decoded JSC bytecode
              |
          Hare IR (SSA)
              |
       ownership and type proof
              |
          LLVM bitcode
              |
   Bun Rust + JSC C++ + app bitcode
              |
     IR-PGO + LTO + rust-lld
              |
     app-specific executable
```

The lowerer consumes JSC bytecode, but it should not begin by reverse-parsing
the serialized `.jsc` cache. Bun currently asks JSC to encode an
`UnlinkedCodeBlock` into an opaque cached-bytecode payload. Hare should expose a
small C++ bridge that walks the live JSC bytecode and emits a versioned,
machine-readable description before that representation is serialized. This
keeps opcode semantics sourced from the exact JSC revision used by the build.

MutMem and source metadata are produced by the Bun frontend and joined to JSC
functions using stable function identifiers and source ranges. TypeScript types
must not be treated as proof after erasure.

## Compilation Tiers

Every function has an explicit tier. Tier changes are observable in build
reports, never inferred silently after a failure.

| Tier | Internal representation | Correctness mechanism |
| --- | --- | --- |
| JSC | Existing bytecode/JIT | JavaScriptCore |
| Generic native | Boxed `JSValue` and runtime calls | Exact JSC-compatible operations |
| Profiled native | Unboxed values behind guards | Side exit or deoptimization to JSC |
| Proven native | Unboxed values and native ownership | MutMem verifier and closed-world restrictions |

Generic native code is the correctness bridge, not the performance goal. It
lets us validate control flow, calls, exceptions, and linking before adding
speculation. Proven native code is where stack allocation, scalar replacement,
`noalias`, moves, and zero-cost result handling become available.

## Hard Invariants

1. `--native=off` is behaviorally and structurally equivalent to upstream Bun.
2. Unsupported bytecode falls back to JSC or fails `--native=required`; it never
   emits guessed semantics.
3. Native and JSC execution produce the same values, exceptions, side effects,
   ordering, and cancellation behavior.
4. A proof manifest is versioned and bound to the source, bundled output, JSC
   bytecode, target triple, and compiler revision.
5. TypeScript annotations alone never authorize unsafe memory behavior.
6. Any JSC heap pointer live across a safepoint is visible to the collector.
7. Any speculative guard has enough metadata to reconstruct JSC-visible state.
8. Native borrows cannot cross unknown calls, suspension, re-entrancy, GC, or
   storage boundaries unless the verifier has an explicit rule for it.
9. Backtrace collection allocates and resolves strings only on failure paths.
10. LLVM bitcode uses the exact data layout, target features, and compatible
    LLVM version selected by Bun's build.
11. Native compilation is deterministic for identical inputs and profiles.
12. Performance claims include variance, memory, binary size, build time, and
    fallback rate, not only a selected throughput number.

## Hare IR

Hare IR is a small typed SSA layer between JSC bytecode and LLVM. Lowering
directly from every JSC opcode to LLVM would mix JavaScript semantics,
speculation, ownership, and machine representation in one unreviewable step.

The initial IR needs:

- constants, locals, phi nodes, branches, switches, and returns
- boxed and unboxed booleans, integers, doubles, strings, and references
- checked arithmetic and JavaScript conversion operations
- explicit calls, throws, catches, and side exits
- heap loads/stores with shape and write-barrier metadata
- safepoints and GC roots
- ownership operations: own, move, copy, shared borrow, mutable borrow, end borrow
- Effect operations: succeed, fail, flat-map, suspend, scope, interrupt check
- backtrace operations that carry static error and source identifiers

IR validation runs after construction and after every optimization pass. A
failed validation is a compiler error, never a reason to continue codegen.

## MutMem Contract

The existing MutMem checker is a useful prototype, not yet a sound native
memory proof. Hare must move its analysis into Bun's parser/type metadata path
and define conservative escape rules for JavaScript.

A proven region initially forbids:

- `eval`, `with`, proxies, and dynamic scope changes
- getters or setters not resolved to a proven direct target
- monkey-patched prototypes or unresolved property shapes
- borrowed values captured by closures
- borrowed values stored in objects, arrays, globals, or JSC heap cells
- mutable borrows across Effect suspension, `await`, callbacks, or re-entry
- unknown calls while a mutable borrow is live
- resizable or detachable buffers without an explicit pin

The verifier should reject uncertain code instead of adding runtime ownership
machinery. The optimization strategy is to do less work after proving a small
region, not to simulate Rust dynamically.

## Result And Backtrace Contract

A native Result uses a compact tagged representation selected per function.
The common success path must not allocate, capture a stack, copy a dictionary,
or construct an Effect failure object.

Static build tables hold:

- literal error identifiers
- source file, line, and column records
- logical Effect call frames
- data-field schemas for each failure site

Failure materialization may allocate and cross into Effect/JSC. Dynamic error
data is evaluated only after the failure branch is taken. Native frames and
logical Effect frames must merge into the same user-facing backtrace format.

## Effect Contract

Hare does not initially compile arbitrary Effect internals. It recognizes a
small, pinned Effect surface and lowers its semantics explicitly.

First synchronous subset:

- `Effect.succeed`
- `Effect.fail`
- `Effect.sync`
- `Effect.suspend`
- `Effect.map`
- `Effect.flatMap`
- `Effect.catchAll`
- `Effect.mapError`
- `Effect.zipRight`

Later subsets add scopes, finalizers, services, fibers, interruption,
concurrency, and async state machines. Unsupported combinators remain ordinary
Effect code in JSC. A recognized combinator is compiled only when its identity
resolves to the pinned Effect package and has not been replaced dynamically.

## Profiles

Hare uses two distinct profiles:

1. A semantic profile records JSC value kinds, structures/shapes, branches,
   calls, exceptions, and fallback frequency. It decides which speculative IR
   is legal and useful.
2. LLVM IR-PGO records native control-flow frequency. Bun's existing Rust and
   C++ PGO plumbing can consume the merged profile during the final LTO link.

Profile data may improve code but may never relax MutMem safety rules. Missing
or stale semantic data falls back to generic or JSC execution.

## Milestones

### M0: Reproducible Baseline

- Build the pinned Bun revision without Hare changes.
- Record test, startup, throughput, RSS, binary-size, and build-time baselines.
- Capture the exact Rust, LLVM, linker, WebKit, and Effect revisions.
- Add a one-command focused validation harness.

Exit gate: two clean builds agree, the selected Bun tests pass, and benchmark
variance is understood before performance work begins.

### M1: Bytecode Observatory

- Add an opt-in JSC bridge that exports decoded bytecode and metadata.
- Generate an opcode inventory from the pinned JSC source.
- Produce deterministic JSON for small CJS and ESM fixtures.
- Differentially compare source, cached bytecode execution, and decoded form.

Exit gate: the observatory can describe every opcode in the selected fixtures
without changing execution.

### M2: Hare IR Frontend

- Create isolated `api.rs`, `abstract.rs`, and `impl/` crate boundaries.
- Decode constants, locals, branches, loops, arithmetic, calls, and returns.
- Validate control-flow graphs and SSA construction.
- Preserve source and bytecode locations.

Exit gate: fixtures round-trip into stable IR snapshots and invalid graphs are
rejected deterministically.

### M3: First Native Function

- Lower a pure integer function from JSC bytecode through Hare IR to LLVM.
- Emit compatible bitcode and link it into a Bun executable.
- Register a native entry trampoline and call it from JSC.
- Compare results against JSC over generated and adversarial inputs.

Exit gate: constants, arithmetic, a branch, and a loop execute from linked
native code with no semantic differences in the supported domain.

### M4: Generic Runtime And Fallback

- Define the boxed runtime ABI.
- Add JavaScript conversions, calls, exceptions, and side exits.
- Record why every function or operation falls back.
- Make `--native=required` diagnostics actionable.

Exit gate: unsupported operations reliably continue in JSC, including thrown
exceptions and re-entrant calls.

### M5: MutMem Proofs

- Port ownership contracts into Bun frontend metadata.
- Add a versioned proof manifest and independent verifier.
- Lower proven values to unboxed native storage.
- Fuzz aliases, closures, branches, loops, exceptions, suspension, detached
  buffers, getters, proxies, and re-entry.

Exit gate: every known escape attempt is rejected or falls back, and proof
checking is independent from proof generation.

### M6: Result And Backtrace

- Lower literal failures and contexts to tagged native control flow.
- Generate static frame and error tables.
- Materialize existing user-facing backtraces only on failure.
- Test nested, parallel, interrupted, malformed, and data-capture failures.

Exit gate: success has no backtrace allocation and failure output matches the
TypeScript implementation.

### M7: Effect Synchronous Subset

- Recognize pinned Effect combinators.
- Fuse supported graphs into Hare IR.
- Preserve laziness, error channels, defects, final ordering, and environment.
- Fall back at unsupported graph boundaries.

Exit gate: differential tests cover supported compositions and hostile dynamic
replacement of combinators.

### M8: Standalone Native Build

- Add the opt-in CLI/build API.
- Replace payload-only standalone creation with an app-specific link step when
  native code is present.
- Cache runtime bitcode and unchanged native modules.
- Retain assets, source maps, bytecode, module metadata, and cross-compilation.

Exit gate: one command emits a relocatable, app-specific executable that runs
without the source tree.

### M9: Semantic PGO And Full LTO

- Record and merge semantic training profiles.
- Generate and consume shared LLVM IR-PGO profiles.
- Enable full release LTO and hot/cold layout.
- Measure native coverage, guard failures, deoptimizations, and fallback rate.

Exit gate: representative eligible workloads improve by at least 30 percent
geometric mean without a whole-suite regression, excessive binary growth, or
unreported fallback.

### M10: Hardening And Platforms

- Run ASAN, Miri where applicable, fuzzing, differential generation, and stress.
- Add Linux arm64, macOS arm64/x64, Windows arm64/x64, and musl incrementally.
- Validate reproducibility, code signing, executable formats, and crash reports.

Exit gate: platform support is claimed only after native and fallback paths pass
the same correctness corpus.

## First Week

| Day | Judgment work | Mechanical work | Required artifact |
| --- | --- | --- | --- |
| 1 | Freeze semantics and integration seams | Bootstrap build and baseline collection | Baseline report |
| 2 | Specify decoded bytecode schema and function identity | Generate opcode inventory and fixtures | Observatory RFC |
| 3 | Specify Hare IR and runtime ABI v0 | Scaffold crates, validators, snapshots | IR RFC plus compiling crates |
| 4 | Design native registration and fallback | Implement four pure op families and differential cases | First linked function |
| 5 | Audit GC, exceptions, ownership, and deoptimization | Fuzz supported operations and minimize failures | Go/no-go review for M4 |

The week is successful if M0 through M3 are proven on a deliberately tiny
subset. Opcode count and benchmark speed are secondary to validating the full
source-to-linked-native path.

## Measurement Gates

Every benchmark record includes:

- exact commit and dirty state
- target CPU and power mode
- compiler, LLVM, linker, JSC, and Effect revisions
- warmup and sample counts
- median, p95, dispersion, and outliers
- wall time, CPU time, peak RSS, allocations, and binary size
- native coverage, fallback count, guard failures, and deoptimizations
- comparison against upstream JSC and Hare with native mode disabled

Only the integration worker runs timing-sensitive benchmarks. Other workers do
not compile large projects during a benchmark window.

## Decision Gates

Stop and make an explicit architecture decision when:

- a JSC heap reference must survive a native safepoint
- deoptimization cannot reconstruct an exact bytecode state
- exception scope rules differ between native and JSC execution
- a borrow might cross suspension or re-entry
- serialized bytecode and live `UnlinkedCodeBlock` disagree
- LLVM versions or LTO unit settings are incompatible
- an optimization needs runtime bookkeeping on every success path
- a benchmark gain depends on removing observable JavaScript behavior

The task queue and execution rules live in `HARE_TASKS.tsv` and
`HARE_WORKSTREAMS.md`.
