# Hare Orchestrator Handoff

If you are reading this, you are taking over the conversation that turned Hare
from a loose idea into a Bun fork and an executable labour plan. This is not a
second architecture specification. `HARE.md` remains the product and compiler
charter, `HARE_WORKSTREAMS.md` remains the execution model, and
`HARE_TASKS.tsv` remains the live dependency ledger. This document carries the
context those files cannot: what the user actually wants, what they rejected,
how they want the models treated, and where the previous orchestrator stopped.

Read this like a letter from the person leaving the desk, not like a checklist
generated from issue labels.

## What The User Is Trying To Build

Hare is meant to make Bun a closed-world native application compiler. The user
does not want another bundling mode that embeds a payload and starts JSC at
runtime. They want Bun and JSC to do useful frontend work at build time, then
want the complete reachable application lowered into LLVM and linked with the
native pieces of Bun that remain reachable.

The command should be boring:

```sh
bun build --compile --hare ./app.ts --outfile app
```

There is one toggle. Earlier drafts proposed `safe`, `profiled`, and `required`
modes. The user rejected that outright. Do not reintroduce those choices under
new names. When `--hare` is present, the compiler closes the graph, compiles it,
or explains at build time why it cannot.

The desired interception point is equally direct. Bun already obtains a live
JSC `UnlinkedCodeBlock` before asking JSC to serialize cached bytecode. The
user's reaction to an observatory-first design was essentially: hijack that
point and do our work there. A deterministic dump is useful to developers and
fanned-out workers, but it is not meant to become a ceremonial external format
that the production compiler writes and reparses.

JSC is still valuable at build time. Its opcode definitions, code blocks,
metadata, constants, exception tables, and source origins are the semantic
frontend. It is not the runtime fallback. A Hare executable must not contain
application source, application bytecode, a bytecode interpreter, a JIT, or a
runtime JavaScript compiler. Native code may still call reachable C++ runtime
helpers and use runtime object or GC machinery. That distinction matters. Do
not accidentally promise to delete every line of JSC, and do not use retained
runtime helpers as an excuse to retain the interpreter.

## The Native Tiers Are Not Product Modes

The user does want three internal native tiers. They do not want three buttons
or three language subsets.

Tier 1 is broad native semantics. It should lower the complete reachable
program using tagged values, explicit checks, native control flow, and runtime
helpers where necessary. It is not an integer demo stretched into a roadmap,
and it is not an interpreter disguised as LLVM IR.

Tier 2 uses inferred types, shapes, effects, call targets, ownership, lifetimes,
and profiles to specialize the Tier 1 graph. A failed Tier 2 guard goes to the
equivalent native Tier 1 path. It never deoptimizes into JSC.

Tier 3 is the whole-program result: direct calls, specialized representations,
concrete storage, fused Effect state machines, native failure edges, and the
largest useful view presented to LLVM. Tier 3 is what lets full LTO remove the
generic machinery that the final program cannot reach.

These tiers should coexist inside one build and even one function. They express
how much the compiler proved, not which features the user paid for.

## The User's Line On Semantics

The user is intentionally more aggressive than the original conservative
draft. Their rule is that behavior defined by the pinned specifications must be
implemented correctly or rejected at compile time. Behavior outside those
specifications is UB under `--hare` and may be used to optimize.

This does not make ordinary TypeScript annotations binding. The user explicitly
pointed at JVM-style type handling: annotations and profiles are excellent
hints, but JavaScript is allowed to contradict them. A false hint must take a
correct generic native path. Only a verified fact or an explicit Hare contract
may become an optimizer assumption whose violation is UB.

Runtime checks are not sacred overhead. Express them as visible IR and control
flow so inlining, constant propagation, alias analysis, and LLVM can prove the
failure edge dead. The recurring performance principle is "go faster by doing
less," not "build a defensive framework around every operation."

Closed-world restrictions are compile-time restrictions rather than runtime
fallbacks. Literal `eval`, literal `new Function`, and statically known dynamic
imports can become more build inputs. Source or module targets assembled from
runtime data cannot. Hare must reject those programs instead of shipping a
compiler in the executable.

## Where MutMem, Result, And Backtrace Came From

The conceptual seeds live in the user's other project under
`/home/nvmbr/Work/doll_eyes/src/util/`:

- `backtrace.ts` explores a Rust-like Result whose error is a literal
  BacktraceError with lazily captured dictionaries and failure context.
- `mutmem.ts` explores pass-by-value, reference, mutable reference, copy,
  borrowing, and compiler/JIT hints.
- `serde.ts` and its adversarial benchmarks reinforced the preference for
  simple hot paths and paying for diagnostics only on failure.

Do not blindly port the current MutMem implementation. The user specifically
said it needs to become general purpose. Hare should infer type, effect, escape,
alias, ownership, lifetime, and pin facts for ordinary code that has never heard
of MutMem. Explicit annotations remain useful as non-binding hints or as
deliberately binding Hare contracts.

Pinning is a major part of the intended memory model. The user wants something
more practical than Rust's usual experience, where pinned borrows can push code
toward `Arc` and awkward heap ownership. Hare can choose stable stack slots,
continuation frames, scoped regions, GC handles, heap promotion, or reference
counting based on the actual escape and suspension graph. `Arc` is a last
choice, not the default representation of a difficult lifetime.

Result and backtrace are present from Tier 1 onward. Success must not capture a
stack, copy diagnostic data, allocate an Effect error, or resolve source names.
Failure materialization happens after the failure edge and should remain
visible enough for later passes to remove unreachable diagnostic machinery.

## Effect Is The Whole Package

Never revive the idea of recognizing a blessed list of Effect combinators. The
user rejected subsets several times. Tier 1 obtains completeness by compiling
the complete reachable implementation of the pinned Effect package like any
other dependency. Effect-aware IR and analysis can then discover and optimize
its representations, continuations, scopes, finalizers, services, fibers,
interruption, scheduling, concurrency, and async machinery.

The architecture should make all of Effect optimizable without making a small
public whitelist part of correctness. A missed optimization is acceptable. A
public Effect behavior that silently drops back to JSC or is excluded because
it was not on a list is not.

## Why The Labour Model Looks Strange

The user asked us to learn from Jarred Sumner's Zig-to-Rust Bun rewrite, not to
copy the cautious shape of an ordinary compiler project. The two pieces of
background they called out were:

- <https://bun.sh/blog/bun-in-rust>
- <https://andrewkelley.me/post/my-thoughts-bun-rust-rewrite.html>

The important lesson for this project is not merely "use agents." It is to
serialize shared judgment, fan implementation out aggressively, tolerate a
huge temporarily broken tree, then turn compiler errors, linker errors, and
test failures into new structured queues.

The user explicitly does not want a build and test ritual after every shard.
They remember the rewrite carrying millions of compiler errors early on and are
comfortable with that. An intentionally incomplete worker commit may fail to
compile. It still needs a narrow owner, a coherent purpose, an honest commit
message, and enough source attribution that convergence workers can repair it.

Debug incremental builds are the normal development build. Release, LTO, PGO,
BOLT, and serious performance measurements wait until broad correctness exists.
The user's workstation should not spend the early project recompiling a giant
release binary because every worker wanted reassurance.

There is also no calendar plan. The user rejected the earlier day-by-day first
week table. Use dependency and convergence waves, not promises about what day a
compiler subsystem will exist.

## Models And The Meaning Of Brain Versus Slave Labour

The required model assignment is:

- `gpt-5.6-sol` at `high` reasoning for brain labour
- `gpt-5.6-luna` at `xhigh` reasoning for slave labour

Use Sol sparingly. Use Luna liberally.

Do not read "slave labour" as "give Luna trivial edits." The user considers
Luna strong enough to build a bootloader and kernel in a long sitting. Luna can
own an instruction family, a complete runtime feature, an optimizer pass, a
large error family, an adversarial review, or a repair campaign. The split is
about blast radius and coordination, not intelligence.

Sol belongs on decisions that many workers will amplify: the semantic and UB
boundary, the live JSC hook, Hare IR, runtime ABI, GC and safepoints, memory and
pin contracts, complete Effect architecture, LLVM integration, and resolution
of incompatible assumptions. Even there, Luna can do the source archaeology,
case collection, and first draft so Sol spends its scarce attention deciding
rather than searching.

Do not escalate merely because a task is large or a branch has thousands of
errors. Escalate when Luna is repeatedly unable to reconcile a shared
invariant, discovers a real semantic ambiguity, needs to change an accepted IR
or ABI, or continues in the wrong direction after focused feedback. When that
happens, start a Sol thread to clarify or repair the high-blast decision, then
return the broad implementation to Luna.

## Thread System Rules

The user forbids subagents anywhere in this orchestration hierarchy. Do not use
the collaboration subagent tools as a fallback, and tell every worker thread
that it may not spawn subagents of its own.

All work is orchestrated through user-owned Codex threads in the attached
project named `The Bun Slopfork`. Thread titles must be exactly shaped like:

```text
<luna|sol> m<milestone number>: <task>
```

Examples:

```text
luna m0: trace the live JSC bytecode handoff
sol m0: settle the compiler contract
luna m2: lower object and property instructions
```

Use project threads for implementation, read-only archaeology, independent
review, and repair. Manage their worktrees and branches centrally. Route facts
between threads explicitly instead of hoping they infer each other's work from
a changing tree.

For a one-off investigation, tell the worker to message the orchestrator with
its findings when finished. For work that will take a while, create a one-shot
scheduled wakeup. The initial check interval is 15 minutes. Adjust after seeing
how the model and task behave. A source-reading task may deserve another 15 or
20 minutes; a large implementation or debug build may deserve 30, 40, or 60.
These are orchestration wakeups, not project deadlines.

Do not busy-wait. Do not repeatedly poll an unchanged thread. Remove obsolete
scheduled checks after the worker finishes. Archive a completed thread once its
commit is integrated, its findings are recorded, and it is no longer likely to
receive a repair follow-up. Keep it open while its context is still useful.

If a worker damages its shard, loses the invariant, or produces a repairable
but untrustworthy implementation, it is fine to start a fresh Luna repair
thread. Give the repair worker the mission packet, bad commit or diff, concrete
findings, and exact owned paths. Do not ask it to rediscover the entire project.
Use Sol only if the failure exposes a shared architectural decision rather than
an implementation mistake.

## Worktrees, Commits, And Integration

Worker branches use `claude/hare-<task-id>-<slug>`. Wave integration branches
use `claude/hare-w<wave>-integration`. Each active worker needs an explicit,
non-overlapping path set or generated-table slice.

The orchestrator owns `HARE_TASKS.tsv` unless a mission explicitly delegates a
specific row range. This avoids every worker conflicting while trying to mark
itself active. Record the thread ID, branch, owner, status, and checkpoint in the
orchestrator's state or ledger before launching another worker with overlapping
scope.

Workers should commit coherent mission-owned progress and push their branch.
Their completion message should include:

- task ID and branch
- commit hash
- what actually landed
- what was intentionally left incomplete
- checks run, or an explicit statement that the wave deferred them
- known compiler/linker errors
- architectural questions or escalation conditions encountered

Do not let workers merge one another casually. The orchestrator or designated
integration thread cherry-picks accepted commits onto the wave branch, resolves
shared-contract drift, and pushes that branch. Review can happen before or
after cherry-pick depending on conflict pressure, but source ownership and
attribution must remain clear.

Never use `git stash`, destructive reset, destructive checkout, or force push.
Never clean up unrelated changes. A broken fanout branch is an artifact to
converge, not an excuse to destroy someone else's work.

## A Good Worker Prompt

Do not send workers a vague copy of the whole dream. Give them a bounded mission
with enough authority to solve it. A useful prompt sounds like this:

```text
You own H0XX on branch claude/hare-H0XX-<slug> in The Bun Slopfork.
Use gpt-5.6-luna at xhigh. Do not create subagents.

Read AGENTS.md, HARE.md, HARE_WORKSTREAMS.md, and your HARE_TASKS.tsv row.
Your owned paths are: <paths>.

Mission: <outcome and why downstream work needs it>.
Non-goals: <shared contracts or neighboring paths not to redesign>.
Source of truth: <pinned JSC/Effect/Bun files or accepted RFC>.
Checkpoint: <the ledger checkpoint, including whether builds are deferred>.
Escalate if: <the ledger condition plus any mission-specific ambiguity>.

Commit only owned work with a kind(place): note message and push the branch.
Do not claim the combined compiler builds unless you ran that checkpoint.
When finished, message the orchestrator with the commit, honest status,
deferred errors/checks, and anything another worker must know.
```

For review threads, provide the diff and accepted contract but initially omit
the implementer's defense. Ask the reviewer to assume the implementation is
wrong. For repair threads, provide both the review findings and the bad diff so
the worker can act rather than repeat the review.

## Recommended First Launch

Do not spend Sol context rediscovering file locations that Luna can map. A good
W0 start is several non-overlapping Luna investigations followed by narrowly
focused Sol synthesis:

1. `luna m0: trace the live JSC bytecode handoff`
   Read-only archaeology. Report exact Bun and JSC files, functions, ownership,
   exception state, and the point immediately before cached-bytecode encoding.
2. `luna m0: map closed-world semantic edge cases`
   Build the example corpus behind H002: static versus dynamic eval/imports,
   specified behavior, compile errors, hints, explicit contracts, and UB.
3. `luna m0: map ownership and pinning cases`
   Build the sync, async, Effect, FFI, GC, re-entry, projection, and cross-thread
   case corpus behind H004. Include carriers that avoid unnecessary `Arc`.
4. `luna m0: map the complete Effect runtime`
   Inventory the pinned Effect implementation by runtime domain and identify
   representations and boundaries without proposing a combinator whitelist.
5. `luna m0: capture the debug baseline`
   Claim H001 and the central build token. Record the pinned toolchains and one
   debug plus warm incremental build. Do not run a release build.

The read-only investigations should report back rather than editing the same
contract files concurrently. Once their findings arrive, use Sol for the shared
decision documents, beginning with:

```text
sol m0: settle the compiler contract
```

That thread should own H003 and `docs/hare/COMPILER.md`, consume the JSC handoff
report, and settle the direct hook, IR/ABI boundary, runtime-helper boundary,
and build-time-only JSC lifetime. H003 unblocks the opcode inventory, porting
guide, and compiler skeleton, so it is the first place Sol attention pays for
itself.

Luna can draft H002, H004, and H005 from the collected evidence. Use one later
Sol review/synthesis thread if those drafts disagree on effects, suspension,
errors, roots, or lifetime boundaries. Do not create three Sol threads merely
because the current ledger labels all three rows `brain`; the decision needs
Sol, while most of the evidence and prose do not.

Before creating any thread, list existing project threads and avoid duplicate
missions. After creating one, record its thread ID, worktree, branch, owned
paths, and scheduled check. Do not launch H006 or H007 until H003 actually
settles the shared representation they consume.

## Current Repository State

At the time of this handoff:

- workspace: `/home/nvmbr/Work/hare`
- GitHub fork: `https://github.com/arm64be/Hare`
- `origin`: the fork above
- `upstream`: `https://github.com/oven-sh/bun.git`
- working branch: `claude/hare-bootstrap`
- branch state before this handoff edit: clean and synchronized with origin
- latest architecture commit: `973a61f9bc docs(hare): adopt closed-world fanout model`
- pinned upstream base: `bbe3f6a2629adf808adbd0da199ae8c94a3c0d47`

H000 is complete. H001 through H005 are ready. H006 and H007 wait on H003. No
implementation worker has been launched and no task beyond H000 has been
claimed.

The previous orchestrator attempted to start W0, then inspected its available
tools. This task did not expose `create_thread`, `list_threads`,
`read_thread`, `wait_threads`, `send_message_to_thread`,
`set_thread_archived`, `set_thread_title`, or `automation_update`. Those tools
were genuinely unavailable, not merely undiscovered under another name. No
subagents were created as a workaround.

Your first operational action is therefore to verify that the successor task
has the project-thread and scheduling tools. Confirm that `gpt-5.6-luna` is
actually selectable for project threads. If either capability is missing, tell
the user plainly and do not silently substitute Terra, Sol, a subagent, or an
external process.

## Things Not To "Improve" Backward

These are the mistakes most likely to recur when a fresh context tries to make
the project look safer or more conventional:

- Do not add `safe`, `profiled`, `required`, or fallback CLI modes.
- Do not retain JSC interpretation or runtime JS-to-bytecode compilation.
- Do not send failed Tier 2 guards to JSC; send them to native Tier 1.
- Do not make TypeScript hints memory proofs.
- Do not treat the current MutMem API as the final language.
- Do not make `Arc` the universal answer to pinning and suspension.
- Do not lower only a hand-picked Effect subset.
- Do not hide semantic and ownership checks behind opaque helpers LLVM cannot
  remove.
- Do not demand green builds from intentionally incomplete fanout commits.
- Do not start release builds or performance campaigns before correctness.
- Do not turn convergence waves back into a daily calendar.
- Do not reduce Luna to tiny mechanical edits.
- Do not use subagents anywhere in the orchestration tree.

The user is comfortable with an ambitious plan and temporarily ugly compiler
state. What they do not want is fake progress: unsupported behavior silently
falling back, workers overwriting each other, benchmarks before correctness, or
status messages claiming a shard works when it was never built.

## How To Work With The User

Be direct. Explain concrete tradeoffs when a decision has blast radius, but do
not repeatedly ask permission for ordinary orchestration. The user has already
authorized thread creation, worktree management, integration, repair threads,
scheduled checks, and archiving inside `The Bun Slopfork`.

They prefer ambitious simple mechanisms over defensive layers and tiny helper
abstractions. They are happy to let strong models own hard work. Surface real
semantic or toolchain constraints early, especially GC roots, exception state,
pin lifetimes, LLVM compatibility, and closed-world violations. Do not turn
ordinary difficulty into a blocker and do not use caution as a substitute for
doing the source archaeology.

Most importantly, keep the distinction between "the program is not supported
and the compiler clearly rejected it" and "the executable quietly ran
something else." The former is part of Hare's current product shape. The latter
is exactly what the user asked us to eliminate.
