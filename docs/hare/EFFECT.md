# H005: Effect semantic and lowering contract

Status: accepted H005 contract. Hare pins `effect@3.22.0` exactly and records
the selected registry artifact, upstream source identity, complete package
manifest, and the reviewed `3.19.19` comparison below. The checkpoint covers
the complete package contract; it does not promise that H020 lowering exists.

## 1. Contract boundary

Hare `--hare` must lower the complete statically reachable implementation of
the selected Effect package. Reachability is determined by the whole program,
not by a list of public combinators or by an Effect-specific allowlist. A
reachable internal module, custom `Effectable` value, service implementation,
data structure, interpreter, or host edge is part of the Tier 1 contract.

Tier 1 is generic native compilation of the ordinary JavaScript/TypeScript
implementation. Effect-aware IR recognition, continuation fusion, unboxing,
state-machine formation, and other optimizations are optional and may improve
performance only. A missed recognition path must remain correct generic native
code. No failed specialization may execute JSC, retain application bytecode,
or invoke a runtime JavaScript interpreter.

The lowering must preserve, including for values and modules not recognized by
an optimization pass:

- laziness and callback/closure invocation order;
- continuation stack order and cooperative yielding;
- the complete `Cause` topology, including sequential and parallel branches;
- the distinction between typed failure, defect, and interruption;
- fiber-local state inheritance and join behavior;
- child ownership, observer notification, and fiber termination;
- scope close behavior, exit-aware finalizers, strategy, and ordering;
- async one-shot resumption, registration failures, cancellation, and
  `blockingOn` identity;
- scheduler, clock, retry, queue, and backpressure behavior;
- STM journal validation, retry registration, commit, and wake-up behavior;
- Channel/Stream/Sink pull, emit, leftovers, failure, and finalization state;
- host bridge boundaries for callbacks, promises, timers, abort signals, and
  observability; and
- the distinct `Micro` engine rather than treating it as ordinary `Effect`.

The owners below are implementation boundaries for later work. They are not a
shared IR or ABI design:

| Owner | H005 contract consumed |
| --- | --- |
| H020 | Complete reachable Effect lowering and Effect-aware analysis: the core run loop, fibers, scopes, layers, resources, coordination primitives, requests, STM, Channel/Stream/Sink, Schedule, Config/Schema edges, observability services, and Micro. |
| H021 | Lossless `Cause`/`Exit`/Result failure representation, typed-failure/defect/interruption propagation, and lazy failure/backtrace materialization. |
| H018 | Generic optimizer-visible native helpers and host bridges for timers, abort/cancellation, promises/callbacks, scheduler wakeups, logging/tracing/metrics sinks, and other external effects. |
| H019 | Ownership, borrow, region, lifetime, pin, root, and cross-thread rules for fibers, continuations, scopes, resources, queues, and async registrations. |
| H015 | Generic closure, environment, generator, async-function, callback, and continuation-frame lowering used by Effect registration and user callbacks. |
| H016 | Native throws/catches/finally, abrupt completion, Promise rejection edges, and conversion of host exceptions into Effect defects. |
| H027 | Differential and adversarial coverage of the complete selected pin, including scopes, fibers, interruption, concurrency, async, streams, and STM. |

H005 does not authorize H020 to invent a supported subset. If a reachable
defined behavior cannot be lowered, the build must fail at compile time.
H005/H020 must escalate any selected-source disagreement or any
decision that cannot represent the shared ABI, re-entry, exception-containment,
ownership/lifetime, cross-realm/thread, scheduler/interruption, or no-fallback
semantics in the Hare contracts. No current evidence requires JSC execution or
a whitelist; either would be an escalation rather than a Tier 1 exception.

## 2. Canonical pin and provenance

Hare selects `effect@3.22.0`. Root `package.json` contains the exact direct
development dependency `"effect": "3.22.0"`; the dependency is a compiler
semantic input and source corpus, not a runtime package installed beside a
Hare executable. Root `bun.lock` records the same exact workspace specifier and
this package tuple:

| Field | Selected value |
| --- | --- |
| Registry artifact | `https://registry.npmjs.org/effect/-/effect-3.22.0.tgz` |
| Registry integrity | `sha512-jhYFe0zTlIRqYFrKTS+6luhmS/Tm0f+JLo0K9KUxvtFab1SUGEszQi2ehOP6QzAZvy831lDmTwwzvVDZSPNz3g==` |
| Artifact SHA-256 | `cb73fa0743025dac14d625b2e5a2ff2079fbccf1b5e3622f8e899ef4bd90c316` |
| Unpacked artifact tree SHA-256 | `1d7dda8385f655400f0c31776d42270f935ac86db1a010c52e8445086fc407eb` |
| Published source tree SHA-256 | `6c14f8bf293b7f0e57f1e3785171385c2a6d2780944e602c17207bf22f67e48b` |
| Upstream tag | `effect@3.22.0` |
| Upstream commit | `e670e0f6befb959b84208d5f77631276521020ae` |
| Machine manifest | `generated/hare/effect-3.22.0.manifest.json` |
| Reproducer | `scripts/hare/generate-effect-manifest.py` |

The registry artifact is the canonical snapshot because it is the code users
resolve. Its 186 `src/internal/` runtime files match the upstream tag commit
byte-for-byte. Upstream public source files are transformed during publishing,
primarily to materialize API documentation, so their registry bytes—not the
checkout's pre-publish bytes—define Hare's source hashes. The manifest contains
all 2,715 published files with size and SHA-256, all 179 export maps and their
condition targets, and separate complete ESM, CJS, declaration, source,
generated, and metadata arrays. The three `./.index` condition targets are
declared but absent from both candidate artifacts; the manifest records them
with `present: false` instead of inventing files.

`reachableModules` is deliberately empty in this package-level manifest. A
reachable subset is application-specific and is recorded by the graph compiler
for each Hare build. An empty application subset does not narrow the complete
selected-package contract or authorize an Effect feature whitelist.

The lock resolves the selected package's dependency graph as follows:

| Package | Requested by parent | Exact lock result |
| --- | --- | --- |
| `@standard-schema/spec` | `^1.0.0` | `1.1.0`, integrity `sha512-l2aFy5jALhniG5HgqrD6jXLi/rUWrKvqN/qJx6yoJsgKhblVd+iqqU4RCXavm/jPityDo5TCvKMnpjKnOriy0w==` |
| `fast-check` | `^3.23.1` | `3.23.2`, integrity `sha512-h5+1OzzfCC3Ef7VbtKdcv7zsstUQwUDlYpUTvjeUsJAssPgLn7QzbboPtL5ro04Mq0rPOsMzl7q5hIbRs2wD1A==` |
| `pure-rand` | `^6.1.0` | `6.1.0`, integrity `sha512-bVWawvoZoBYpp6yIoQtQXHZjmz35RSVHnUOTefl8Vcjr8snTPY1wnpSPMWekcFwbxI6gtmT7rSYPFvz71ldiOA==` |

### 2.1 Candidate source audit

The `3.19.19` and `3.22.0` registry tarballs were compared directly. They
contain the same 179 export keys, the same dependency ranges, and the same
2,715 artifact paths. Of 362 published source files, 21 changed and 341 are
byte-identical; eight of the changed files are under `src/internal/`. The
comparison artifact is fixed by integrity
`sha512-Yc8U/SVXo2dHnaP7zNBlAo83h/nzSJpi7vph6Hzyl4ulgMBIgPmz3UzOjb9sBgpFE00gC0iETR244sfXDNLHRg==`
and SHA-256
`02b9b83ed550df4e9ad16692fed159c31de4ba1ff7e0338bb57578d18b1b4f64`.

The runtime-domain review found these selected semantic differences:

| Domain | `3.22.0` contract relative to `3.19.19` |
| --- | --- |
| Scheduler and fiber wakeups | `SchedulerRunner` isolates pending priority buckets per fiber and `scheduleTask` carries the optional fiber through default, sync, controlled, matrix, batched, semaphore, latch, and fiber-runtime submission sites. |
| Requests and cleanup | `invokeWithInterrupt` uses `ensuring`, so completion of every still-pending request runs on success, failure, or interruption rather than only after the main async effect succeeds. |
| Tracing | Current-span annotation and tracer logging filter disabled propagation and walk to the selected propagated parent span. |
| `Cause` rendering | Pretty errors are materialized as populated ordinary `Error` values instead of instances of an internal `PrettyError` subclass. |
| `RcMap` | Idle TTL may be selected per key and is stored on each entry; release and touch use that entry-specific duration. |
| `Schedule.cron` | A `+Infinity` clock input terminates with the previous interval instead of attempting another cron calculation. |
| Public generic code | Cron, Graph, Data, Effect, Layer, schema/type helpers, and related emitted artifacts include additional defined APIs or behavior that Tier 1 compiles generically when reachable. |

The async instruction and registration machinery in `src/internal/core.ts`,
runtime flags, STM, Channel/Stream/Sink executors, Config internals, and the
distinct `Micro` engine are byte-identical across the candidates. The selected
changes strengthen cleanup and scheduler isolation and match the complete
source used to derive this document. Hare therefore adopts `3.22.0` rather
than treating the older indirect fixture tuple as its semantic pin.

## 3. Semantic model and generic lowering rules

Effect values are ordinary values. The inspected implementation uses a symbol
brand (`EffectTypeId`), an operation tag (`_op`), and up to three generic
instruction fields (`effect_instruction_i0`, `i1`, and `i2`). The internal
primitive union includes `Async`, `Commit`, `Failure`, success/failure
continuations, `Success`, `Sync`, runtime-flag updates, `While`, iterator,
`WithRuntime`, `Yield`, blocked requests, and other tagged values
(`node_modules/effect/src/internal/core.ts:83-380`).
`Commit` invokes a method that returns another Effect, and `Effectable.Class`
permits custom commit-based values
(`node_modules/effect/src/Effectable.ts:95-106`).

This representation imposes no closed list of constructors. Tier 1 must lower
generic property access, brand/tag checks, closures, iterators, custom
committers, and arbitrary data carried in instruction fields. A pass may
recognize the known primitive shapes, but recognition is not the definition of
support.

### 3.1 Custom Effectable/Commit is user-code re-entry

`Effectable.Class` and `StructuralClass` are user-extensible instruction
boundaries, not data-only records. In the inspected 3.22.0 source,
`fiberRuntime.ts:1352-1353` dispatches `OP_COMMIT` as
`internalCall(() => op.commit())`, and `Effectable.ts:95-106` defines the
custom `commit(): Effect` contract. These are selected-pin source obligations.

The lowering of this boundary must:

- retain the actual receiver, its prototype/brand state, and every captured
  value through the call, any allocation or safepoint, and the returned Effect;
- use the explicit H003 runtime-context and call/re-entry protocol for the
  realm/worker, current fiber, current Context, caller continuation, exception
  transfer, and root descriptor. H005 adds no ABI or hidden ambient slot: these
  are operands/obligations of the H003 operation that enters user code;
- revalidate the returned value generically. A non-Effect or invalid operation
  must follow the selected source's invalid-effect defect behavior, not become
  an unchecked native call;
- preserve the selected source's exception boundary. In the inspected source,
  `commit()` is invoked inside the `runLoop` `try`, so an ordinary throw is
  caught by the run-loop conversion to a `Die`; the special
  `InterruptedException` path retains its sequential defect/interruption
  topology. This selected call site does not create a blanket rule for every
  callback;
- allow the returned Effect to be any reachable instruction, including a
  synchronous result, nested user re-entry, async registration, yield,
  interruption, or cancellation. The `commit()` call itself is not assumed to
  cancel merely because its returned Effect can be cancelled; and
- keep receiver/capture roots, the returned Effect, continuation, cancellation
  token, and late-callback state alive until the selected source's completion,
  interruption, finalizer, or termination path releases them. Apply the
  H004-C01/C04/C05 and H004-F02/F03/F04 carriers, with H019 owning the ledger
  and H015/H016 owning callback and exception edges.

The run loop is an explicit machine, not recursive host-language evaluation.
`FiberRuntime` stores a continuation stack, fiber refs, child set, inbox,
observers, async interrupt state, exit, scheduler, tracer, context, and
supervisor (`internal/fiberRuntime.ts:293-339`). `runLoop` dispatches the
operation tag and checks scheduler yield points, but its callback containment is
deliberately asymmetric: the supervisor `onEffect` hook, running-inbox drain,
and scheduler `shouldYield` call occur before the main `try`; tracer dispatch,
custom `Commit`, and primitive handlers occur inside it
(`internal/fiberRuntime.ts:1361-1430`). Only the latter group receives the
selected source's run-loop catch conversion. The continuation stack and all
values live across a yield or async boundary require the H019/H015 lifetime and
pin rules; no native lowering may rely on the JavaScript call stack surviving a
suspension.

### 3.2 Observable current-fiber state and the H003 re-entry protocol

The 3.22.0 source exposes an observable ambient current-fiber property:
`internal/fiber.ts:384-388` reads `globalThis['effect/FiberCurrent']` through
`currentFiberURI`. `internal/fiberRuntime.ts:666-680` saves the previous value,
publishes the running fiber while draining its queue, and restores it in a
`finally`; `:998-1007` does the same for `start`; and `:1040-1042` republishes
the fiber from `patchRuntimeFlags`. These locations are selected-pin evidence.

H005 consumes H003's explicit ambient-state and call/re-entry protocol. It does
not authorize a hidden native TLS/global current-fiber cell and does not define
a new shared ABI. An H003-declared operation that enters Effect or user code
must make the realm/worker owner, current fiber, current Context, continuation
or caller frame, live-root descriptor, exception/result transfer, and
suspend/resume edge explicit. An implementation may use a controlled
realm-scoped carrier to preserve the package's observable property, but the
carrier cannot hide the frame, continuation, exception, completion kind, or
root set prohibited by H003's runtime ABI contract (`docs/hare/COMPILER.md`,
Runtime ABI section).

The required observable behavior is:

- **Realm and worker ownership:** the property belongs to the executing
  JavaScript realm's `globalThis`. Each VM/realm/worker has its own owner and
  value; a worker or realm boundary cannot observe or borrow another one's
  current fiber. H019 owns the carrier and teardown; H020 owns when it is
  published.
- **Entry and nested re-entry:** on fiber entry, save the prior value and
  publish the entered fiber. Nested user callbacks and runtime re-entry see the
  innermost fiber and restore the prior value in strict LIFO order, including
  when the nested call throws. The explicit H003 operation, not an accidental
  host global, is the source of truth for the re-entry edge.
- **Yield and resume:** the property remains visible during the active run,
  including finalizer/observer work reached before the run returns. When a
  fiber yields or registers async work, restore the prior ambient value before
  control leaves the carrier; publish it again on the resume/drain edge. A
  suspended fiber must not remain discoverable as current on an executor that
  is running another fiber.
- **Throw and interruption:** restore the saved value before an exception,
  defect, or interruption escapes the entry carrier. Preserve the selected
  source's distinction between an ordinary user throw, `Die`, and
  `InterruptedException`; restoring ambient state must not rewrite its Cause.
  Interruption remains visible to the fiber while its interruptible operation
  and selected finalizers execute, then the carrier is restored at the same
  boundary as normal completion.
- **Termination and late work:** termination restores and clears the owner's
  current-fiber publication after exit/observer processing, and no dead fiber
  is retained by the ambient carrier. A late async, scheduler, message, or
  worker callback uses its explicit registration/cancellation token and H004
  lifetime rows; it cannot resume by consulting stale ambient state.

### 3.3 Callback exception containment and ordering

The following table is the required selected-pin source map. “Escapes” means the
selected source does not convert the throw at that call site; it may therefore
escape the current run entry or be handled by an outer, separately documented
boundary. The table is evidence, not permission to normalize all callbacks to
defects. H020/H016 must preserve every row during lowering and re-audit it for
any future pin change.

| Callback or hook | Source order and containment evidence | Required lowering behavior | Owner |
| --- | --- | --- | --- |
| Supervisor `onResume` | `evaluateEffect` invokes it before its `try` (`fiberRuntime.ts:941-949`). | A throw escapes this evaluation boundary; publish/restore current-fiber state around it and preserve the selected escape. | H020/H016 |
| Supervisor `onSuspend` | Runs from `evaluateEffect`'s `finally` (`:985-987`). | It runs after the body even on exit/throw; a throw escapes and follows JavaScript `finally` masking/order. Do not convert without source evidence. | H020/H016 |
| Supervisor `onEffect` | `runLoop` calls it before the main `try` (`:1364-1366`). | A throw escapes before dispatch/catch conversion; it must not be silently turned into `Die`. | H020/H016 |
| Scheduler `shouldYield` | Called before the main `try` (`:1371-1379`). | Preserve call order and direct escape behavior; a thrown scheduling decision is not a run-loop defect unless the selected pin proves an outer conversion. | H018/H016 |
| Running fiber-message handlers | `drainQueueWhileRunning` dispatches the message table before the main `try` (`:1367-1369`; `:723-734`). | Handler order, one-shot behavior, and direct throw escape are observable. Do not move the drain under the dispatch catch without evidence. | H020/H018/H016 |
| Suspended message handlers / `onFiber` | `evaluateMessageWhileSuspended` is reached from the queue-drain `try/finally` (`:902-933`, `:666-679`), while a stateful message's `onFiber` is user callback code. | Preserve the outer queue `finally` and message order; it does not provide a catch conversion for `onFiber`. Distinguish a nested `evaluateEffect` run-loop conversion from a direct `onFiber`/interruptor throw. | H020/H016 |
| Tracer context and primitive dispatch | `currentTracer.context(() => this[(cur)._op](cur), this)` is inside the main `try` (`:1380-1406`). | A normal callback/handler throw is converted to `Die`; an `InterruptedException` gets the selected sequential defect/interruption Cause. | H020/H016 |
| Custom `Commit` | `OP_COMMIT` dispatches `internalCall(() => op.commit())` (`:1352-1353`) inside the same `try`. | Root receiver/captures and preserve the exact run-loop conversion, invalid-op handling, and returned-effect suspension rules in §3.1. | H020/H015/H016 |
| Exit observers and reporting | `setExitValue` reports then calls observers in reverse order (`:846-853`); `addObserver` may call immediately (`:531-540`). | Preserve report-before-observer and reverse order. There is no local blanket catch; the first observer throw stops later observers and escapes its source boundary. | H020/H016/H019 |
| Async registration and resumption | `initiateAsync` catches `asyncRegister` throws and feeds a one-shot defect resume (`:1052-1071`); later callback calls `tell` directly. | Convert registration throw exactly as source; retain one-shot suppression and cancellation race. Do not infer the same conversion for a later callback/tell throw. | H018/H015/H016/H019 |
| Scheduling/wakeup submission | `drainQueueLaterOnExecutor` submits through `currentScheduler.scheduleTask` (`:708-714`) outside the run-loop dispatch catch. | Preserve host submission order and its selected direct-escape/outer-boundary behavior. | H018/H016 |
| Other service, logger, metric, and test callbacks | They are ordinary user/service calls reached from Context/FiberRefs and may occur in or outside the run-loop `try` depending on the call chain. | Track each call edge, receiver/root lifetime, current fiber/context, and exact source catch/escape site; no “all callbacks become defects” rule. | H020/H015/H016/H018 |

This ordering is part of the contract. In particular, moving supervisor hooks,
message drains, or `shouldYield` into the main catch changes observable escape
and defect behavior even if ordinary Effect success cases are unchanged.

`unsafeAsync` and its variants register a callback, retain a cancellation
effect, optionally create an `AbortController`, and carry a `blockingOn` FiberId
(`node_modules/effect/src/internal/core.ts:488-545`).
`initiateAsync` guards resumption with a one-shot flag, queues the resume on the
fiber, stores an interruptor when interruptible, and turns registration
exceptions into defects (`internal/fiberRuntime.ts:1045-1071`). The native
bridge must preserve synchronous completion, completion-before-registration
return, repeated callback suppression, cancellation races, and callback
exceptions. H018 owns the host mechanism; H015 owns callback closures; H016
owns host exception conversion.

## 4. Complete reachable runtime inventory

All source paths in this section refer to the canonical `effect@3.22.0`
registry source installed at `node_modules/effect/src` and hashed by the
machine manifest in §2. The source map is organized by execution domain, not by
a whitelist of public API names.

### 4.1 Causes, exits, typed failure, defects, and interruption

- **Source and representation:** `Cause.ts:254-557` defines `Empty`, `Fail`,
  `Die`, `Interrupt`, `Sequential`, and `Parallel`. `Exit.ts:26-70` is a
  `Success(value)` or `Failure(cause)` Effect-compatible value. Failure is a
  typed `Fail`; a thrown or unexpected value is a `Die`; cancellation is an
  `Interrupt` carrying FiberId information.
- **Obligations:** retain the entire cause tree and branch topology; never
  flatten sequential and parallel composition into one error or choose only
  the first leaf. Preserve exit-aware finalizer input, interruption identity,
  defect visibility, cause matching, pretty-printing, and host conversion.
  Promise rejection, callback failure, synchronous throw, and `FiberFailure`
  are boundaries with explicit mapping rules, not interchangeable tags.
- **Owner:** H021 owns the representation and failure/backtrace edges; H016
  owns native exception/abrupt-completion conversion; H020 threads causes
  through every Effect domain.

### 4.2 Fibers, FiberIds, FiberRefs, inboxes, and observers

- **Source and representation:** `internal/fiberRuntime.ts:293-339` shows the
  runtime state; `internal/fiberMessage.ts:1-120` defines messages; `FiberId.ts`
  defines None, Runtime, and Composite identities. `internal/fiberRefs.ts:14-150`
  stores a Map from FiberRef to non-empty stacks of `(FiberId, value)` pairs,
  applies each ref's `fork` function on child creation, and computes a diff and
  `join` patch on child join.
- **Obligations:** fork must inherit the correct refs and child ownership;
  join must apply the FiberRef-specific diff/join policy; messages and
  observers must be delivered in order; child interruption and fiber exit must
  not leak a continuation, callback, scope, or service context. Fiber-local
  context, scheduler, logging, tracing, metrics, request batching, and runtime
  flags are behavior, not incidental fields.
- **Owner:** H020 owns fiber state and execution; H019 owns storage, roots,
  pins, cross-thread transfer, and termination cleanup; H018 owns native wakeup
  helpers.

### 4.3 Scopes, finalizers, and resource lifetime

- **Source and representation:** `Scope.ts:51-107` defines closeable scopes,
  execution strategy, and finalizers receiving an `Exit`. The close path and
  finalizer collection are integrated with `internal/fiberRuntime.ts` and
  `internal/core.ts`'s scope/finalizer implementation. Strategies include
  sequential, parallel, and bounded-parallel execution.
- **Obligations:** every acquired resource registers its finalizer before an
  observable failure can escape; close runs finalizers with the original exit,
  masks interruption as required by the selected strategy, combines finalizer
  failures with the original cause, and closes child scopes exactly once.
  Ordering, parallelism, interruption masking, and failure combination are
  observable. Finalizers must also remain live across async suspension.
- **Owner:** H020 owns scope and finalizer semantics; H019 owns lifetime,
  pinning, and exactly-once release; H021 owns cause combination.

### 4.4 Resources and scheduled refresh

- **Source and representation:** `Resource.ts:25-72` exposes an
  Effect-backed resource; `internal/resource.ts:16-72` implements manual and
  automatic refresh through scoped state and background fibers. Resource
  acquire, refresh, get, and release are tied to Scope/ScopedRef state.
- **Obligations:** acquire, replacement, refresh failure, interruption,
  shutdown, and scope close must preserve the associated Exit and release
  exactly once. Automatic refresh must not outlive its owner, and a new value
  must not become visible before its acquire/registration protocol completes.
- **Owner:** H020 owns the resource state machine and Schedule integration;
  H019 owns resource/refresh fiber lifetime and pins; H021 owns refresh and
  release failure topology.

### 4.5 Context, services, and FiberRef-provided state

- **Source and representation:** `Context.ts:36-101,181-290` defines `Tag`,
  `GenericTag`, `Reference`, service typing, and Context operations.
  `internal/context.ts:17-334` uses tagged identity, a Map-backed context,
  default-value references, and global default caches. The current Context and
  default services are held in FiberRuntime state.
- **Obligations:** preserve tag identity and key equality, missing-service
  failure, Reference defaults, override/provide scope, local provisioning,
  context merging, and service lookup ordering. A service can be a user object
  with methods that re-enter the runtime; its methods are not native constants
  merely because the tag is known.
- **Owner:** H020 owns Context and service semantics; H019 owns fiber-local
  storage and lifetime; H015/H016 own user service closures and exceptions.

### 4.6 Layers and memoization

- **Source and representation:** `internal/layer.ts:43-185` brands layers and
  distinguishes fresh layers; `internal/layer.ts:319-375,791-934` builds layers
  through a synchronized MemoMap, scopes, Deferred waiters, and effectful
  construction. The internal instruction family includes extend-scope,
  fold, fresh, from-effect, scoped, suspend, provide/provide-merge,
  merge-all, and zip-with forms.
- **Obligations:** shared layers build once per MemoMap, waiters observe the
  same result, Fresh bypasses memoization, construction failure wakes all
  waiters consistently, and the layer's scope closes its finalizers exactly
  once. Layer graph order, environment provision, concurrent construction,
  retry, and memo invalidation are semantic.
- **Owner:** H020 owns Layer evaluation and memoization; H019 owns MemoMap,
  Deferred, scope, and child-fiber lifetime; H021 owns construction failure
  propagation.

### 4.7 Interruption, runtime flags, scheduler, and clock

- **Source and representation:** `internal/fiberRuntime.ts:936-981,1031-1071`
  evaluates interruption and asynchronous termination; runtime flags include
  interruptibility and cooperative yielding. `Scheduler.ts:22-39` defines the
  scheduler contract, while `internal/clock.ts:12-95` provides the Clock tag,
  timer scheduler, current time, and bounded timer duration handling.
- **Obligations:** interruption is a Cause, is deferred by uninterruptible
  regions, is delivered at interruptible boundaries, and interrupts children
  under the documented ownership rules. Scheduler priority/yield decisions,
  microtask versus timer ordering, clock reads, sleep cancellation, and timer
  overflow behavior must remain stable. A native optimization may remove a
  branch only after proving the corresponding flag/clock behavior impossible.
- **Owner:** H020 owns runtime flags and scheduler/clock semantics; H018 owns
  native event-loop/timer/wakeup bridges; H019 owns state across yields and
  interruption races.

### 4.8 Async registration, cancellation, and host boundaries

- **Source and representation:** `internal/core.ts:488-545` creates Async
  instructions, cancellation effects, AbortSignals, and `blockingOn` IDs;
  `internal/fiberRuntime.ts:1045-1071` implements one-shot callback resumption
  and registration-defect handling. Promise/callback runners and runtime
  adapters are additional public-to-host edges in `Effect.ts` and
  `Runtime.ts`.
- **Obligations:** registration is lazy; the register function runs only when
  the fiber reaches Async; the callback may complete synchronously or before
  registration returns; it may be called more than once by a hostile host but
  only the first resume wins; cancellation must reach the registered canceler;
  and abort and interruption must not resume a completed fiber twice. A
  synchronous throw from `asyncRegister` at `initiateAsync` becomes a one-shot
  defect resume. A later callback, `tell`, canceler, or host-adapter throw keeps
  the exact catch-or-escape behavior of its selected call site in §3.3; it is
  not normalized into a defect. Native code must expose those distinct
  boundaries without retaining JSC execution state.
- **Owner:** H018 owns host callback/promise/AbortSignal bridges; H015 owns
  closure and callback frames; H016 owns thrown-host-error mapping; H019 owns
  cross-boundary roots and cancellation lifetime; H020 owns Async semantics.

### 4.9 Deferred, Queue, PubSub, and coordination state

- **Source and representation:** `internal/deferred.ts:22-46` has `Pending`
  with joiners and `Done` with a completed Effect. `internal/queue.ts:67-265`
  stores a backing queue, suspended takers, shutdown Deferred/flag, and a
  strategy; `Queue.ts:515-700` implements BackPressure, Dropping, and Sliding.
  `internal/pubsub.ts:225-395,406-575` contains ring/array storage,
  publisher/subscriber cursors, replay windows, and subscription cleanup.
- **Obligations:** Deferred completes once and wakes all current joiners;
  late awaiters observe the same result. Queue offer/take races, shutdown,
  taker removal on interruption, bounded overflow, backpressure, dropping,
  sliding eviction, and await-shutdown completion are observable. PubSub must
  preserve cursor/replay behavior, per-subscriber delivery, and scoped
  unsubscribe without losing or duplicating a publication.
- **Owner:** H020 owns coordination algorithms; H019 owns atomicity, roots,
  interruption cleanup, and lifetime; H018 owns scheduler wakeups where a
  native queue crosses a host boundary.

### 4.10 Concurrency primitives and structured ownership

- **Source and representation:** the runtime uses the fiber/scope/Deferred/
  queue machinery above. Reachable modules include execution plans and
  strategies, semaphores, pools, FiberMap/FiberSet, Mailbox, Handoff, RcMap,
  RcRef, RateLimiter, SubscriptionRef, synchronized refs, and supervisor
  implementations. Their source modules are under
  `node_modules/effect/src/internal/` and the corresponding top-level modules.
- **Obligations:** preserve execution strategy (sequential, parallel, raced,
  bounded, or inherited), fairness/order, permits, cancellation, child
  ownership, queue backpressure, resource release, and supervisor callbacks.
  Concurrency helpers are not reducible to `Promise.all`; failure and
  interruption produce different Cause topologies and finalizer behavior.
- **Owner:** H020 owns the complete primitive implementations; H019 owns
  permits, shared state, and child/resource lifetime; H021 owns composed causes;
  H027 covers adversarial races and cancellation.

### 4.11 Requests and batching

- **Source and representation:** `internal/blockedRequests.ts:15-145`
  represents blocked work as `Empty`, `Single`, `Par`, and `Seq`, then folds it
  while retaining parallel/sequence structure. `internal/request.ts:75-175`
  tracks request entries and marks each one completed through an Exit.
- **Obligations:** batching must preserve sequential versus parallel resolver
  structure, resolver identity, FiberRef/context behavior, interruption, and
  exactly-once request completion. A resolver failure must settle every
  affected Deferred without settling unrelated requests or leaving blocked
  fibers suspended forever.
- **Owner:** H020 owns request planning and execution; H019 owns Deferred,
  blocked-fiber, and listener lifetime; H021 owns request Exit/Cause delivery.

### 4.12 STM and transactional collections

- **Source and representation:** `internal/stm/core.ts:33-120` defines a
  second tagged instruction family—runtime access, success/failure/retry,
  sync, provide, interrupt, and defect—committed through the STM driver.
  `internal/stm/journal.ts:1-123` uses a `Map<TRef, Entry>`, validates and
  commits entries, collects retry todos, and wakes transactions. `tExit.ts`
  defines success, typed failure, defect, interruption, and retry outcomes.
  TRef, TMap, TQueue, TPubSub, TSemaphore, TSet, and related modules share this
  journal. STM values are also branded for Effect/Stream/Sink/Channel
  integration in the inspected source.
- **Obligations:** transactions are lazy until atomically committed; reads and
  writes use one journal; invalid journals retry rather than partially commit;
  retry registers todos on the read set; commit applies writes atomically and
  wakes the right transactions; defects, typed failures, interruption, and
  retry remain distinct. No partial write may escape a failed or retried
  transaction, and nested transactional composition must retain its journal
  semantics.
- **Owner:** H020 owns STM instructions, driver, journal, and transactional
  collections; H019 owns journal storage and suspension lifetime; H021 owns
  TExit/Cause mapping; H027 owns contention and retry coverage.

### 4.13 Channel, Stream, and Sink

- **Source and representation:** `internal/core-stream.ts:60-150` defines
  Channel primitives including `BracketOut`, `Bridge`, `ConcatAll`, `Emit`,
  `Ensuring`, `Fail`, `Fold`, `FromEffect`, `PipeTo`, `Provide`, `Read`,
  `Succeed`, `SucceedNow`, and `Suspend`. `internal/channel/channelExecutor.ts`
  maintains current channel, input executor, emitted value, done stack,
  active child executor, cancellation, environment, and finalizers, and
  returns executor states `Done`, `Emit`, `FromEffect`, or `Read`.
  `internal/stream.ts:77-120` wraps lazy chunked Channels; `Sink.ts:43-80`
  models a Channel consumer with a result and leftovers.
- **Obligations:** preserve pull/emit ordering, chunk boundaries where
  observable, pipe-to handoff, child executor decisions, bridge input,
  backpressure, leftovers, failure cause, scoped acquisition, and finalizer
  order. A Stream is not just an iterable and a Sink is not just a reducer;
  both reach the full Effect, Fiber, Scope, Queue, and Channel machinery.
- **Owner:** H020 owns Channel execution and Stream/Sink lowering; H019 owns
  active executors, buffers, pins, and finalizers; H021 owns failure/Exit
  propagation; H018 owns external stream I/O bridges.

### 4.14 Schedule, retries, repeats, and timing

- **Source and representation:** `Schedule.ts:60-140` models a Schedule as
  state plus an effectful `step(now, input, state)` producing the next state,
  output, and continuation decision. Schedule drivers and interval/retry/
  repeat combinators consume Clock and Effect state.
- **Obligations:** retain lazy step evaluation, state transition order,
  elapsed-time and interval boundaries, output values, delay/cancellation,
  retry/repeat termination, and interaction with resource refresh and stream
  scheduling. Clock substitution, including TestClock, must affect every
  schedule edge consistently.
- **Owner:** H020 owns Schedule state machines; H018 owns time/clock bridges;
  H019 owns scheduled fiber and resource lifetime; H027 covers deterministic
  and real-clock behavior.

### 4.15 Config and Schema edges

- **Source and representation:** `Config.ts:25-75,243-330,520-736` exposes
  lazy Config values whose result is an Effect with a structured ConfigError;
  `internal/config.ts:28-605` brands and interprets Config operations and
  provider access. `Schema.ts:15-50,496-632` connects Schema AST/ParseResult
  validation, decoding, encoding, transformations, and effectful parse
  errors. Related source includes `SchemaAST.ts`, `ParseResult.ts`,
  `ConfigProvider.ts`, and `ConfigError.ts`.
- **Obligations:** preserve lazy provider reads, nested/alternative config
  error topology, defaults, secret/redaction behavior, parse/encode direction,
  transformation order, accumulated versus first errors, schema annotations,
  memoization, and service/context requirements. Schema and Config are not a
  second fiber engine, but their lazy interpreters and Effect-returning edges
  are reachable semantics.
- **Owner:** H020 owns the complete Config/Schema integration; H018 owns
  filesystem/environment/provider bridges; H015/H016 own user transformations
  and exceptions; H021 owns error conversion where it enters Effect causes.

### 4.16 Observability, default services, and test services

- **Source and representation:** `Logger.ts:28-80`, `Tracer.ts:16-148`, and
  `Metric.ts:23-150` define service brands and registries used by fibers.
  `TestClock.ts:76-131,438-550` provides a Context service with deterministic
  time and suspended-sleep state. `TestServices.ts:28-49` installs a
  FiberRef-backed test service Context. Runtime supervisor, annotations,
  spans, metric labels, and log causes are read from Context/FiberRefs in
  `fiberRuntime.ts`.
- **Obligations:** preserve service replacement, fiber-local annotations,
  span parentage, log level/cause data, metric update order, supervisor
  callbacks, deterministic TestClock advancement, and test-service isolation.
  These are observable when applications install custom services or use test
  utilities; they cannot be erased as diagnostics merely because the normal
  path does not inspect them.
- **Owner:** H020 owns service semantics and fiber integration; H018 owns
  native logging/tracing/metric sinks and clock wakeups; H019 owns FiberRef
  state and cleanup; H027 owns deterministic observability/test-service cases.

### 4.17 The distinct Micro engine

- **Source and representation:** `Micro.ts:40-72` brands a separate Micro
  value and MicroExit type. `Micro.ts:477-620` implements `MicroFiber` state and
  an explicit Micro run loop; `Micro.ts:1742-1885` defines `MicroExit` success
  and failure and the MicroCause failure/defect/interruption distinctions.
  `Micro.ts:4272-4352` has independent scopes/finalizers, and
  `Micro.ts:5431-5440` provides synchronous execution. Micro also has its own
  async cancellation, child interruption, observers, and scheduler paths.
- **Obligations:** do not route Micro through ordinary FiberRuntime or assume
  ordinary Cause/Exit object identity. Preserve Micro's continuation order,
  synchronous-run behavior, async one-shot cancellation, child ownership,
  finalizer exit input, observer notification, and MicroCause topology. Shared
  helpers may be specialized only when their representation and ordering are
  proven equivalent.
- **Owner:** H020 owns the separate Micro interpreter and its complete reachable
  modules; H018 owns Micro host async/scheduler bridges; H019 owns Micro fiber,
  scope, and callback lifetime; H021 owns any explicit conversion at the
  Effect/Micro boundary.

## 5. Package boundaries, generation, and dynamic behavior

The inspected 3.22.0 package contains source TypeScript plus prebuilt ESM/CJS
and declaration artifacts. No Effect-specific source-generation pipeline,
JavaScript source-evaluating `eval(...)`, `new Function(...)`, or runtime source
loader was found in the available package inventory. Micro has methods named
`eval`, but those are internal primitive-dispatch methods, not source
evaluation. Hare must still compile all reachable generated or prebuilt module
code as ordinary application code and must reject any dynamic source/module
behavior that violates Hare's closed-world rules.

`GlobalValue.ts:42-58` stores singleton values in a Map attached to the
executing realm's `globalThis`, using the fixed key `effect/GlobalValue`; the
source explicitly uses this to make mixed ESM/CJS imports and reloads agree.
`ModuleVersion.ts:10-18` forwards to a module-local version variable, initially
`3.22.0` in the inspected tree, and allows a framework author to change it.
`fiberRuntime.ts:1382-1396` compares an Effect value's embedded `_V` to the
runtime version and may log a mismatch. These are observable identity/version
boundaries, not permission to collapse all package instances or silently ignore
duplicate-version behavior.

The package exports ordinary data modules as well as interpreters. Collections,
data types, ASTs, error objects, services, and utility modules may carry custom
methods, lazy thunks, symbols, iterators, and user callbacks. The compiler's
complete graph must therefore use generic JavaScript semantics first and treat
Effect-aware recognition as a proof-driven optimization over that graph.

### 5.1 GlobalValue, ModuleVersion, and package-instance ownership

The selected source's identity rules require explicit ownership:

- **GlobalValue owner and identity:** one `effect/GlobalValue` Map belongs to
  one JavaScript global/realm. A VM or worker with a distinct global owns a
  distinct Map; it is not a process-wide singleton and must not be shared by a
  native static without an explicit realm key. H020 owns Effect identity
  semantics and H019 owns the realm handle, roots, and disposal edge.
- **Mixed-format duplicates:** ESM and CJS artifacts of the same selected
  package in one realm must reuse the same global Map and the same value for an
  equal global-value key, including reload paths. That does not make every
  module object, prototype, Fiber, Context, or Effect value identical across
  artifacts.
- **Different versions or duplicate copies:** preserve the source's
  `ModuleVersion` and `_V` comparison behavior. A duplicate package copy may
  have a separate module-local version variable while sharing a GlobalValue key;
  the compiler must not silently merge incompatible versions, rewrite `_V`, or
  turn the source's mismatch observation into an arbitrary hard failure. The
  selected-pin audit must test same-version ESM/CJS duplication and deliberate
  different-version duplication in one realm and across realms.
- **Per-VM/process bookkeeping:** any native registry, scheduler, tracer, or
  cache used to implement these semantics must be keyed by the owning VM/realm
  (and worker where applicable). Process identity is only an outer lifetime
  boundary; it is not a substitute for realm identity or current-fiber
  ownership.
- **Teardown:** before a realm/VM/worker is destroyed, fibers, scopes,
  finalizers, GlobalValue-held resources, scheduler tasks, observers, and async
  registrations must be closed or canceled according to their selected source
  semantics. Clear the current-fiber publication and release the realm's
  GlobalValue Map only after late-callback tokens are closed. A callback after
  teardown is a rejected/ignored registration event according to the selected
  source, never a dereference of a dead realm or fiber. H019 owns teardown;
  H018 owns host cancellation and H020 owns observable package behavior.

### 5.2 H003 re-entry and H004 lifetime cross-reference

H005 consumes the explicit runtime-context, call/re-entry, root, and exception
protocol in H003's Runtime ABI section (`docs/hare/COMPILER.md`). H003 permits an
explicit realm/runtime context but forbids hiding the current frame, PC,
exception, completion kind, live roots, ownership ledger, or continuation in
thread-local/global helper state. The current-fiber behavior in §3.2 must
therefore be implemented through that existing protocol; this document does not
invent shared IR or ABI fields. H015/H016/H019/H020 must attach each user-code
and host callback edge to an H003 operation with explicit current fiber/Context,
re-entry, suspension, result/exception, and root operands.

H004's expanded lifetime rows are the carrier and exit ledger for the concrete
Effect state below. The row IDs are not optional annotations: every reachable
state must have one owner, one rooted carrier, a pin/borrow rule, and all of the
listed normal, throw, defect, interruption, cancellation, worker-termination,
and safepoint exits. A missing domain-specific row remains an H019 blocker.

| Reachable Effect state | H004 lifetime rows to bind | Required carrier, owner, and exits |
| --- | --- | --- |
| FiberRuntime frame/continuations, current-fiber publication, inbox, children, and observers | H004-E17, H004-C01/C04/C05, H004-A03, H004-F04 | FiberRuntime handle plus explicit H003 re-entry token owns frame, queue, observer list, and current-fiber save/restore; H004-E17 is the dedicated ambient-state carrier/restore obligation. H020/H019 own it through return, throw/defect, interruption, cancellation, worker termination, and safepoint. |
| FiberRefs, Context, services, Layer MemoMap, and default caches | H004-C01/C05, H004-C04, H004-E01/E03/E04 | Fiber or realm-owned Map/stack roots values and service receivers; H020 owns semantics and H019 owns fork/join, scope close, cross-realm exclusion, and all exits including async rejection. |
| Scopes, finalizers, Resource/ScopedRef state, and refresh fibers | H004-E01/E03/E04, H004-A03, H004-C05, H004-F04 | Scope owns resource, child, finalizer, refresh fiber, and original Exit; H020/H019 preserve exactly-once close, strategy/order, interruption masking, finalizer throw, worker termination, and safepoint exits. |
| Deferred, Queue, PubSub, blocked requests, waiters, and wakeups | H004-A03, H004-E04, H004-F04, H004-C04/C05 | Deferred/queue/subscriber owner roots each waiter and cancellation token until one settlement or removal; H020/H019 preserve backpressure, cursor/order, shutdown, interruption, late callback, worker, and safepoint exits. |
| STM journal, TRef entries, retry todos, and transactional collections | H004-E04, H004-C01/C05, H004-A03 | Transaction owns one rooted journal and todo set through validation/commit/retry; H020/H019 preserve atomic commit versus retry, wakeup, defect/failure/interruption, cancellation, worker, and safepoint exits. |
| Channel executor, Stream chunks, Sink leftovers, buffers, and finalizers | H004-E01/E03/E04, H004-G01, H004-C01/C05, H004-F04 | Executor/scope owns current node, done stack, child, buffer, leftovers, environment, and finalizers; H020/H019 preserve pull/emit/read/yield, async rejection, throw/defect, interruption, close, worker, and safepoint exits. |
| Schedule state, Clock/TestClock, timers, retries, and Resource refresh timing | H004-A03, H004-F02/F04, H004-C04/C05 | Schedule driver and timer registration own state, callback, and cancel token; H018/H020/H019 preserve clock substitution, ordering, cancellation race, retry termination, worker teardown, and safepoint exits. |
| Logger/Tracer/Metric, supervisors, annotations, TestServices, and Micro fibers/scopes | H004-C01/C04/C05, H004-A03, H004-F02/F04, H004-E01 | Service/observer receiver and Micro's separate fiber engine retain their own roots and callbacks; H020/H018/H019 preserve callback containment, deterministic services, Micro-specific exits, cancellation, worker termination, and safepoint cleanup. |

Ordinary Effect closure capture is not an implicit unique move. H004-E02 applies
only where a verified move is explicit; otherwise use the managed shared roots
and synchronization rows (H004-E03/E04 and H004-C01/C05). This is especially
important for layer memoization, queue waiters, STM entries, stream buffers,
and custom `Effectable` receivers.

The current-fiber row is therefore explicitly dependent on H004-E17 in addition
to the generic root, callback, async, and termination carriers above; omitting
H004-E17 leaves the ambient publication and restoration ledger incomplete.

## 6. Required downstream checkpoints

Before H020 claims Tier 1 coverage, the selected-pin source manifest must be
recorded and every reachable domain in §4 must have an implementation owner.
H020's inventory must explicitly include custom `Effectable`, all internal
instruction variants, the STM and Channel interpreters, and Micro; a public
API list is not an acceptable substitute.

H021 must verify that every domain preserves `Cause`/`Exit` topology and that
success paths do not allocate or materialize failure/backtrace data merely in
anticipation of an error. H019 must audit every value live across a fiber yield,
async registration, child join, scope close, queue wait, STM retry, or Channel
executor transition. H015/H016 must audit re-entry and thrown-callback paths.
H018 must make scheduler, clock, AbortSignal, Promise, and observability
boundaries explicit and optimizer-visible. H027 must derive its differential
corpus from the selected package and verify it against the manifest hashes.

## 7. Checkpoint

H005 ledger checkpoint: **met**. The exact root pin and lock graph, registry
integrity and artifact hash, upstream tag and commit, selected source tree,
complete per-file/export manifest, candidate diff, runtime-domain review,
ownership mapping, and exception-containment rules are recorded and
reproducibly checked.

Future Effect pin changes must regenerate the manifest and repeat the semantic
audit before changing this contract. H020 still owns implementation. It may not
narrow the package to a whitelist, normalize callback exceptions, weaken the
shared ABI/lifetime contracts, or use JSC as a fallback.
