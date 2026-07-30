# Hare Labour Model

Hare uses two model pools: brain labour for decisions with project-wide blast
radius, and slave labour for massively fanned-out implementation. Slave models
are assumed to be extremely capable. They are not restricted to rote edits or
tiny helper functions.

The split is based on coordination cost and mission risk, not intelligence.

## Brain Labour

Use the smartest available model for decisions that many other workers will
encode and amplify:

- language semantics and the UB boundary
- the direct JSC compiler hook
- Hare IR and validator contracts
- runtime ABI and representation rules
- exception, GC, safepoint, and Tier 2-to-Tier 1 state reconstruction semantics
- whole-program analysis and proof trust boundaries
- generalized MutMem, lifetime, and pinning semantics
- complete Effect lowering architecture
- LLVM toolchain, bitcode, LTO, and PGO integration
- build graph and standalone executable architecture
- resolution of incompatible worker assumptions
- final acceptance of correctness and performance claims

Brain labour produces mission artifacts before fanout:

- `PORTING.md` for repeated source-to-Hare mappings
- `OPCODES.tsv` for instruction ownership and status
- `LIFETIMES.tsv` for cross-boundary ownership, pin, and lifetime decisions
- focused RFCs for shared IR, ABI, manifest, and runtime contracts
- generators and templates where consistency matters more than hand-written code
- explicit examples of defined behavior, compile errors, and UB

The brain model should not spend its context implementing the hundredth opcode
handler after the pattern is established.

## Slave Labour

Slave models may own large, intellectually difficult, independently reviewable
missions such as:

- an entire JSC instruction family
- a complete feature domain such as objects, closures, modules, exceptions,
  async functions, generators, or promises
- full Effect runtime and optimizer areas
- Result and backtrace lowering
- ownership inference, escape analysis, regions, or pinning implementations
- complete Tier 1 runtime-helper families
- Tier 2 and Tier 3 optimization passes
- platform backends and executable formats
- thousands of compiler or linker errors grouped by root cause
- broad feature-support sweeps
- fuzzing, differential generation, corpus minimization, and bug-hunting passes
- performance investigations and optimization campaigns after correctness

Each slave mission gets a stable shared contract, owned paths, source-of-truth
references, and escalation conditions. Inside that boundary, the worker may
design local data structures, refactor, generate code, and solve hard problems.
It should propose a contract change when needed rather than silently forking the
architecture.

## Jarred-Style Execution Loop

Hare intentionally separates mass implementation from convergence.

1. Brain labour serializes the mission, mappings, lifetimes, and shared APIs.
2. Slave workers fan out across non-overlapping feature and instruction shards.
3. Workers commit coherent progress even when the combined compiler does not
   build yet.
4. Two independent slave reviewers assume each shard is wrong and report
   semantic, ownership, platform, and equivalence failures.
5. Fixers apply accepted findings without waiting for a full build.
6. After broad coverage lands, one centralized debug build produces the compiler
   error queue.
7. Compiler errors are grouped by crate, file, diagnostic family, and likely
   root cause, then fanned out again.
8. Once compilation converges, linker failures become the next fanned-out queue.
9. Once linking converges, test failures become the next queue.
10. Correctness is followed by fuzz, security, optimization, and platform waves.

Millions of intermediate compiler errors are acceptable during a deliberate
fanout wave. What is not acceptable is losing attribution, overwriting another
worker's files, or claiming that incomplete code works.

## Build And Test Policy

Workers do not build and test every commit.

- Debug incremental builds are the only normal development builds.
- Release, LTO, PGO, BOLT, and performance builds wait until Tier 1 correctness.
- A fanout task does not run `bun bd` unless its mission explicitly owns a
  convergence checkpoint.
- Cheap generator self-checks and local static checks are allowed when useful,
  but they are not mandatory ceremony.
- The integrator owns the shared build token and reusable build directory.
- Full debug builds happen after implementation batches, not after every shard.
- Tests begin after the relevant compiler and linker convergence gates.
- A failed central build is serialized into a work queue instead of being fixed
  ad hoc by the build runner.

This keeps the workstation responsive and avoids making every smart worker wait
on the same expensive Rust and C++ build.

## Branch And Worktree Model

- Each mission uses `claude/hare-<task-id>-<slug>` in its own worktree.
- Wave integration uses `claude/hare-w<wave>-integration`.
- Every active mission owns explicit paths or generated table ranges.
- Workers never use `git stash`, destructive reset, destructive checkout, or
  force push.
- Workers commit only mission-owned changes and do not clean up unrelated code.
- Generated output changes through its generator where one exists.
- Upstream synchronization is a dedicated brain-labour mission.
- A commit may be intentionally uncompilable, but its message and task record
  must say what was added rather than claim completion.

The integrator may cherry-pick many incomplete commits into the wave branch.
The stable branch advances only at declared convergence gates.

## Mission Packet

Every fanned-out mission contains:

- objective and non-goals
- owned files or generated table slice
- accepted shared contracts and source-of-truth revision
- neighboring implementation example when one exists
- known dependencies and downstream consumers
- expected error or test queue it contributes to
- conditions that require brain-labour review
- commit-message prefix

A mission packet describes outcomes, not a line-by-line implementation recipe.
The worker is trusted to solve the subsystem.

## Review Pools

Reviewers are also strong slave models. They receive the mission packet and
diff without the implementer's rationale for their first pass.

### Semantic Review

Find defined inputs whose native behavior differs from ECMAScript, Bun, Web,
Node, Effect, or Hare specifications. Distinguish an unimplemented defined case
from behavior that Hare explicitly defines as UB.

### Memory And Pin Review

Assume every value aliases, every callback re-enters, every suspension extends
a lifetime, and every runtime call can allocate. Trace owners, borrows, pins,
roots, regions, moves, and projections across all exits.

### Instruction Equivalence Review

Compare each opcode's operands, widths, constants, metadata, exceptions,
control-flow successors, and observable effects against the pinned JSC source.
Look for missing variants and generator drift.

### Effect Review

Check complete Effect semantics rather than a public-combinator whitelist:
laziness, defects, typed failure, scopes, finalizers, services, fibers,
interruption, scheduling, concurrency, async registration, and cancellation.

### Optimization Review

Assume a transformation exploited defined behavior by mistake. Verify its
facts are proven, guarded with an exact Tier 1 path, or explicitly contractual
UB. Confirm optimizer-visible checks remain correct before LLVM removes them.

### Adversarial Bug Hunt

Generate hostile programs, differential inputs, malformed IR, deep graphs,
large modules, re-entrant callbacks, exceptions, detached buffers, races, and
resource exhaustion. Minimize every discrepancy before assigning it.

## Queue Convergence

Convergence proceeds in this order:

1. generated inventory coverage
2. Rust and C++ syntax/type errors
3. cross-crate API errors
4. native link errors
5. minimal executable boot
6. instruction differential failures
7. feature and Effect failures
8. ownership, pinning, GC, and exception failures
9. fuzz and stress failures
10. release performance and platform work

Do not prematurely optimize a compiler that cannot yet lower and link the
defined program graph. Do not prematurely demand a green build while the
instruction-set fanout is still intentionally incomplete.

## Task Statuses

| Status | Meaning |
| --- | --- |
| `spec` | Brain labour is defining the shared contract |
| `ready` | Mission packet is complete and may be claimed |
| `fanout` | One or more workers are landing owned shards |
| `review` | Independent static/adversarial review is active |
| `converge` | Central compiler, linker, or test queue is being reduced |
| `blocked` | A shared decision or prerequisite is missing |
| `done` | The mission passed its declared wave checkpoint |

`HARE_TASKS.tsv` is the shared mission ledger. It is expected to grow into
generated opcode, feature, error, and test-failure queues rather than remain a
small hand-maintained checklist.
