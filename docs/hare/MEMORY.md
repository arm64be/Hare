# Hare memory, ownership, lifetime, and pin contract

This is the H004 contract for --hare. It applies to ordinary JavaScript and
TypeScript, to reachable Bun and Effect code, and to explicit Hare contracts.
It is not a port of MutMem and it does not define the exact Hare IR or ABI;
those interfaces belong to H003. The contract describes the facts that an IR
and a backend must preserve and the evidence required before an optimizer may
use them.

The same rule applies to a value whether its fact was inferred from ordinary
code or written as a contract. Ordinary JavaScript may violate TypeScript
annotations, so TypeScript is never an ownership, lifetime, alias, or pin
proof. An explicit Hare contract is binding only after the verifier accepts it;
violating it is Hare undefined behavior. The compiler must reject a defined
program when it cannot select a carrier that preserves its semantics. There is
no JSC runtime fallback.

## 1. The vocabulary

For every live reference, Hare records these separate facts:

- **Owner**: the storage or protocol responsible for keeping the value alive
  and releasing it exactly once. An owner can be an activation, continuation
  frame, region, GC-managed object, registration, transfer token, or an
  independently-lived native owner.
- **Root**: the reachability edge that keeps a managed value discoverable by
  the collector or keeps a native resource discoverable by its teardown
  protocol. A root is not an address guarantee.
- **Borrow**: a temporary access whose owner remains elsewhere. A shared borrow
  permits reads; a mutable borrow permits mutation and excludes overlapping
  aliases for its whole lifetime.
- **Pin**: a promise that a particular storage identity remains stable for the
  address-sensitive use. A pin is not a root and does not by itself keep an
  object reachable.
- **Region**: an owner for a bounded group of values and borrows. Region exit
  is an ownership event and runs on every applicable exit edge.
- **Carrier**: the concrete lifetime mechanism selected for a value or pin:
  stack slot, continuation frame, region allocation, GC handle, stable heap or
  external allocation, transfer token, or—only when justified—reference
  counting.
- **Exit**: every event that can end or bypass a use, including normal return,
  throw, cleanup, suspension close, cancellation, worker teardown, and a late
  foreign callback.

The owner answers “who releases this?”; the root answers “what keeps this
reachable?”; the pin answers “where may this address be used?”; the carrier
answers “what makes those promises true?” Conflating any two is a rejected
analysis result.

### Reachability and stable address are independent

An object can be strongly rooted while its collector-managed address changes.
Conversely, a native allocation can have a stable address while no managed
root keeps the object that owns the address alive. Therefore:

1. A managed reference held across allocation, a callback, a safepoint, or a
   suspension must be represented by a collector-visible root: a stack map,
   continuation-frame field, strong GC handle, registration, or another
   verified root carrier.
2. A raw pointer, slice, view, or interior address held across an operation
   that can move storage must also have a pin whose scope covers the complete
   use. A root alone is insufficient; reload from the rooted owner when a pin
   is not needed or cannot be kept.
3. A pin must name the owner it pins. A pin operation without a root or owner
   is not a valid carrier. A GC handle must not be described as a pin unless
   the collector contract also provides the specified stable-address guarantee.
4. If either fact is unknown, use a managed value and reload or copy through a
   generic native path. If no such path preserves the defined operation, fail
   compilation before emitting a raw use.

## 2. Evidence lattice and trust boundary

Every inferred fact carries exactly one evidence class. Merges are
conservative: contradictory facts become unknown, and a hint never upgrades
another fact.

| Evidence | Source | What it authorizes | What it cannot authorize |
| --- | --- | --- | --- |
| proof | JSC semantics, closed-world graph facts, control-flow/dataflow proof, or a verified explicit Hare contract | Correctness facts and optimizer-visible ownership, root, lifetime, alias, and pin metadata | Nothing beyond the proved scope or contract preconditions |
| guard | A native predicate checked before the specialized operation, with an exact generic Tier 1 successor on failure | A fact on the checked path; specialization and elimination of the guard only after later proof | A raw use before the check, or a silent deoptimization to JSC |
| hint | TypeScript annotation, profile, naming convention, or optimization advice | Search order, layout preference, and speculative code generation | Semantics, UB, rooting, pinning, exclusivity, or a compile-time acceptance decision |
| unknown | No trustworthy fact or a conflicting merge | Generic Tier 1 when its managed representation and exits are sufficient | An optimizer assumption, unregistered pointer, escaping borrow, or implicit cross-boundary ownership |

The exact trust boundary is the verifier and the emitted guard. Only a proof,
a verifier-accepted explicit Hare contract, or a fact established on the
success edge of a visible guard may cross into optimizer assumptions.
TypeScript and profiles remain hints even when they are always true in the
observed program. An explicit contract does not make a false statement true;
it makes violating the stated precondition UB after verification, and its
owner, root, pin, and exit obligations must still be represented.

A guard is ordinary native control flow. Its failure edge enters generic Tier 1
native semantics, not JSC and not an opaque “safe runtime.” A guard may test
shape, uniqueness, liveness, transfer state, or a carrier precondition. It may
not retroactively make an already-used pointer safe. Checks needed to preserve
defined behavior remain visible until proof removes them.

## 3. Ownership operations

### Values, moves, and copies

- A **unique value** has one owner. A move transfers that owner to the
  destination and ends the source binding at the move point. A later source use
  is a compile error when it is defined as an ownership operation; under an
  accepted explicit contract, it is UB. A move preserves identity and does
  not imply cloning.
- A **copy** leaves the source owner and creates a new owner. Copying a JS
  object is not inferred merely from an assignment: ordinary assignment keeps
  identity and creates another managed root/alias. An explicit clone or
  structured clone may allocate, invoke defined user behavior, or fail, and
  its failure must travel through the operation's defined error channel.
- A **shared alias** may be observed by several bindings. Hare must preserve
  strict identity, mutation visibility, prototype behavior, getters, proxies,
  and all other defined identity effects. Unknown aliases therefore use a
  generic managed representation rather than an inferred unique slot.
- A **shared borrow** is read-only and ends no later than its owner and the
  lexical callback/region that received it. It may be represented by a raw
  address only while its owner is rooted and pinned; otherwise it is a managed
  reference or a copy.
- A **mutable borrow** is exclusive for its full lifetime. All competing
  aliases, including aliases reachable through a callback or another fiber,
  must be excluded or synchronized. A callback that can re-enter the owner
  ends the borrow before re-entry, or the analysis rejects the operation.

These operations are inferred for ordinary JS/TS where possible. Explicit
contracts can state stronger unique, shared, mutable, or move obligations, but
the syntax and ABI for doing so are outside this document.

### Projection and disjointness

A projected borrow or pin names its parent owner. An immutable projection is
valid only while the parent remains rooted and any address-sensitive use stays
within the parent's pin. A mutable projection additionally needs a proof that
it is exclusive. Two projections such as left and right may overlap through
accessors, proxies, unions, or dynamic layout; field names alone are not a
disjointness proof.

For a statically laid-out object, Hare may prove disjoint offsets and create
field pins whose scope ends before a parent move. For a dynamic JS property,
the generic path reloads the property after every call that can run user code;
it never retains an interior pointer based on a property lookup. If parent
movement, shape, offset, or disjointness is unknown, use a parent handle and
reload, copy, or reject the required raw projection.

## 4. Carriers and regions

The compiler chooses the smallest carrier that satisfies the owner, root, pin,
and exit facts. Carrier choice is not a user-facing safety mode.

### Stack slots

A stack slot is valid for a value or pin when the value cannot escape the
activation, cross a suspension, reach a re-entrant callback, or be referenced by
a background thread. A slot is still rooted at a safepoint through the native
stack map when it contains a managed value. A stable slot can satisfy an
address pin for a synchronous non-escaping use; a normal stack address cannot
be retained after its owner exits.

### Continuation frames

An async, generator, or Effect continuation frame owns values live across
await, yield, resumption, rejection, interruption, cancellation, or iterator
close. The frame is a root carrier for managed fields. A pin may use a stable
field in that frame only if the frame carrier itself guarantees the address for
the pin's scope; otherwise the resumed code reloads from the frame. An
activation-local borrow cannot silently become a continuation borrow.

### Regions

A lexical region owns its allocations and borrows. Region destruction happens
on normal fallthrough, return, throw, rethrow, finally, cancellation,
interruption, and worker/operation teardown. A value or borrow may leave a
region only by an explicit move into a carrier whose lifetime dominates the
escape, by a defined copy, or by promotion. A region crossing suspension must
be continuation-owned or promoted; a stack-only region is not enough.

### GC handles and managed roots

A GC handle or stack map keeps a managed value reachable across allocation,
callbacks, and safepoints. Native code must not retain a raw managed pointer
in a heap allocation, standard container, FFI record, or queued callback
without a root carrier. A handle may be copied as a reference, but it does not
provide stable address identity unless the collector contract explicitly says
so. When a handle does not pin, reload the object or field after a possible
move.

Weak handles and WeakRef are intentionally not strong roots. A weak target may
disappear between observation and use; a finalizer is not an owner or a borrow
scope. The only valid use is an explicit liveness check followed by a use whose
root and pin requirements are independently satisfied.

### Heap promotion

Promotion changes a lexical owner into a heap or external owner when a closure,
queue, async frame, callback registration, Effect fiber, or FFI call outlives
the current frame/region. Promotion is often the right carrier and does not
imply reference counting. The promoted object still needs a strong root while
it is reachable and a pin only when an address-sensitive use requires one.

### Transfer tokens

A transfer token records a one-way ownership handoff. The sender loses the
transferred owner at the commit point, the receiver becomes the sole owner, and
the token/queue roots the value while delivery is pending. A failed enqueue
does not consume the source. A detached ArrayBuffer and a structured-clone
message are distinct operations: transfer preserves the backing storage with
one new owner, while clone creates a separate value.

### Reference counting

Reference counting is selected only when consumers truly have independently
lived ownership and no lexical region, continuation join, GC owner, registration,
or transfer protocol can determine the last release. It is appropriate for a
native resource shared by a worker and a main thread that may terminate in
either order, provided every reference has a named owner and every terminal
path releases it. It is not a default answer for async, Effect, closures,
ordinary JS aliases, or a value that already has a dominating scope. Counts do
not replace synchronization, rooting, pinning, or cancellation cleanup.

## 5. Suspension, re-entry, and boundaries

An operation that can allocate, call user code, suspend, or cross a thread is a
lifetime boundary. The analysis assumes the broadest defined behavior until it
proves otherwise:

- property access can invoke a getter or Proxy trap;
- coercion can invoke user code and throw;
- callbacks can synchronously re-enter and mutate or close their owner;
- allocation and safepoints can collect or move managed objects;
- async, generator, and Effect operations can suspend, reject, fail, defect,
  interrupt, or cancel;
- foreign code can retain a pointer, call back late, fail cancellation, or
  terminate independently;
- worker queues can delay delivery and worker termination can bypass normal
  user-level completion.

Close a borrow before a re-entrant call unless the callback's contract proves
that it cannot observe or mutate the owner. After the call, reload mutable
state and re-establish roots and pins. Never hold a non-reentrant mutable
borrow while invoking user code merely because the first call site is
syntactically synchronous.

### Async and generators

An async local that is live over await belongs to the continuation frame or a
promoted owner. A borrowed input cannot cross await as a stack address. Hare
must retain the rooted owner, copy the needed value, or reject the borrow. A
generator frame owns state across every next, throw, return, and iterator
close; a yielded view is copied or carries an explicit owner/root/pin that
outlives the caller's use.

### Effect

The contract covers the complete reachable Effect implementation, not a
whitelist of combinators. A scope owns acquired resources, finalizers, child
state, and continuation state across success, typed failure, defect,
interruption, cancellation, and nested scope unwinding. Forking a unique value
is a move into the child fiber; parallel immutable reads may share a rooted
owner; parallel mutation requires a defined synchronization/ownership carrier.
An interruption finalizer keeps its resource and cause rooted until cleanup
completes.

### FFI

An FFI boundary must classify each pointer as borrowed-for-call,
owned-transferred, copied, or retained-until-registration/cancellation. A
borrowed pointer ends at foreign return and cannot be stored. A retained
pointer needs an explicit external owner, root/registration, stable pin, late
callback rule, and cancellation/teardown release. A foreign callback is a
re-entry boundary: protect VM values with handles and do not hold a raw pointer
or exclusive borrow across it unless its contract proves that behavior.

### Threads and structured clone

JS heap objects are thread-affine unless a defined shared representation says
otherwise. Marshal immutable bytes or use a structured clone for a copy. Use a
transfer token for an exclusive ArrayBuffer handoff; the sender is detached at
commit and may not use the backing store. SharedArrayBuffer is shared mutable
storage and requires its defined atomic/locking semantics; it is not a license
for unsynchronized native aliases. A cross-thread native owner uses reference
counting only when its independent termination order requires it.

## 6. All-exit rule

For every inventory entry, the owner, root, pin, and carrier must be valid on
all exits that can occur in that case. The canonical exit set is:

| Exit | Required action |
| --- | --- |
| return / early return | End borrows before returning; move or copy returned values; release the old owner only after transfer is complete |
| throw / rethrow | Unwind regions and release roots/pins in reverse ownership order; preserve the original defined exception |
| finally | Run cleanup even when the body returned or threw; cleanup failure follows the language's defined replacement/chaining rule |
| async rejection | Settle the continuation and release its frame/registrations after rejection is observable; no pending root remains |
| Effect failure / defect | Preserve the failure/defect and unwind scopes/finalizers; failure context remains rooted until materialization is complete |
| interruption / cancellation | Keep the resource and cancellation cause rooted through finalizers; settle or cancel pending operations and release exactly once |
| callback re-entry / callback throw | Close temporary borrows before entry, reload after entry, and run the callback's own cleanup even when it throws |
| worker termination | Drain or cancel registrations according to the worker contract; transfer or release queued owners and complete native refs |
| FFI cancellation / late callback | Make cancellation and callback delivery race-safe; unregister before release or retain an explicit token until late delivery is impossible |
| GC safepoint | Publish every live managed reference to the collector and reload unpinned addresses after a possible move |

Finally is not a substitute for a root: cleanup state itself must remain
reachable until cleanup completes. A successful return is not the only release
path, and a finalized flag is not an ownership protocol. Any path omitted from
a carrier's exit set is an H004 inventory defect and must block a raw or
exclusive representation.

## 7. Representative cases

The snippets are semantic examples, not a proposed Hare syntax. The phrase
explicit contract means a verifier-checked Hare contract whose exact spelling
and IR/ABI representation H003 will define.

### Ordinary sync JS/TS: unknown aliases use Tier 1

~~~ts
function increment(box: { value: number }) {
  box.value++;
  return box;
}

const a = { value: 0 };
const b = a;
increment(a);
console.log(b.value); // 1: identity and mutation are observable
~~~

The TypeScript shape is a hint. a and b are shared aliases; a generic managed
Tier 1 object and roots preserve the observation. Hare must not infer a unique
owner merely because the annotation says value: number.

### Sync borrow, mutable borrow, move, and copy

~~~text
withBytes(bytes, callback) {
  // inferred shared borrow, valid only through callback return
  return callback(bytes);
}

withMutableBytes(buffer, callback) {
  // explicit contract: exclusive mutable borrow, no callback re-entry
  return callback(&mut buffer);
}

sendToWorker(move buffer); // transfer token moves the unique owner
clone = copy value;        // source remains owned; clone has a new owner
~~~

If callback stores the borrow, re-enters an alias, or suspends, the direct
borrow is invalid. Hare copies/promotes the value when that preserves defined
behavior, or emits a compile error when the requested explicit borrow has no
legal carrier.

### Projection and disjointness

~~~text
withTwoFields(&mut record.left, &mut record.right, callback)
~~~

This is a direct mutable projection only when left and right are proven
disjoint, the parent remains rooted, and the parent/pins outlive the callback.
If a getter, Proxy, dynamic shape, or unknown layout can make the projections
overlap, use a managed parent and reload/copy; do not manufacture two interior
pointers from field names.

### Async input and continuation ownership

~~~ts
async function readFirst(input: Uint8Array) {
  const view = input.subarray(0, 1);
  await flush();
  return view[0];
}
~~~

view and input are live across suspension. The continuation frame roots the
owner; the implementation either keeps a valid frame/heap carrier and reloads
the backing store, or copies the byte. A stack interior pointer is never held
across await. Rejection and cancellation release the frame on their own edges.

### Generator state and yielded views

~~~ts
function* chunks(input: Uint8Array) {
  yield input.subarray(0, 4);
  yield input.subarray(4);
}
~~~

The generator object owns and roots its continuation state through next, throw,
return, and close. Because a consumer may retain a yielded view after the next
call, Hare must copy it or expose an owner/root/pin whose scope covers that
consumer-visible lifetime.

### Effect scope and forked ownership

~~~text
Effect.scoped(
  acquireRelease(open(), resource => close(resource)),
  use
)
~~~

The scope owns resource, its finalizer, and its continuation. Success, typed
failure, defect, interruption, cancellation, and nested unwinding all run the
finalizer. If a unique resource is forked to a child fiber, ownership moves to
the child; the parent cannot use it while the child owns it. Shared immutable
state may instead use one rooted owner through the join scope.

### FFI borrowed and retained buffers

~~~text
ffi.consumeBorrowed(buffer)       // contract: reads only until return
ffi.registerCallback(move state)  // registration owns state until cancel/close
~~~

The first call may use a pinned stack/heap buffer only through foreign return.
The second promotes state, roots callback-visible values, and carries an
explicit registration token through success, native failure, cancellation,
late callback, and teardown. A TypeScript declaration saying ArrayBuffer is not
an FFI lifetime contract.

### GC root versus pin

~~~text
root object across nativeAllocate(object); // required strong root
pin object while nativeUsesAddress(object); // additionally required for address
~~~

The root prevents collection; the pin prevents movement or invalidation during
the address use. A handle that only roots must be dereferenced again after
nativeAllocate. An interior pointer is invalid unless its parent root and pin
cover the whole use.

### Weak reference

~~~ts
const weak = new WeakRef(target);
const live = weak.deref();
if (live !== undefined) use(live);
~~~

The weak reference does not own or pin target. The result of deref is a new,
ordinary managed reference whose root and pin facts are analyzed for use; a
finalizer cannot be used to justify a borrow or release a separate owner.

### Structured clone and transfer

~~~ts
port.postMessage({ payload });              // clone: sender keeps its owner
port.postMessage(buffer, [buffer]);         // transfer: receiver becomes owner
~~~

The queue roots a pending clone/transfer. Clone allocates a distinct value;
transfer commits an exclusive owner handoff and detaches the sender's buffer.
If enqueue fails, the transfer token does not consume the source.

### Cross-thread native state

~~~text
worker.postMessage(readonlyBytes);          // copy or immutable transfer
shareNativeState(main, worker);             // explicit independent owners
~~~

The first operation carries bytes or a defined immutable representation, never
a JS-heap pointer. The second requires synchronization and a release protocol
for either thread terminating first. Only this genuinely independently-lived
case justifies a reference count; a joining worker or lexical scope should use
that owner instead.

## 8. Unknown and compile-error boundary

Unknown is a valid input to generic Tier 1 when all of the following remain
true:

- managed values stay tagged/managed and preserve identity and mutation;
- every live managed reference has a visible root at allocation, callback,
  suspension, and safepoint boundaries;
- the carrier can perform all normal, exceptional, cancellation, and teardown
  releases;
- an address-sensitive operation can reload, copy, or use a generic stable
  carrier without assuming an unknown pin;
- dynamic property access, coercion, and re-entry keep their defined behavior;
- clone, transfer, and error behavior is represented by an ordinary native
  Tier 1 path.

Unknown is a compile error when any of these safety-critical facts is required
and no generic carrier resolves it:

- a managed reference or GC handle would be unrooted across a safepoint,
  callback, suspension, or queued work;
- a stack/region borrow would escape, cross await/yield, or outlive its owner;
- a raw pointer or interior projection would cross allocation, movement,
  re-entry, cancellation, or thread transfer without a verified pin;
- an explicit mutable borrow has an overlapping or re-entrant alias that is
  neither excluded nor synchronized;
- FFI retention, callback, cancellation, late delivery, allocator, or release
  ownership is missing or contradictory;
- a weak reference is being treated as a strong root or stable owner;
- a move/transfer has an unresolved source use, failed-commit path, or two
  possible owners;
- cross-thread JS-heap access or mutable native sharing lacks a defined carrier
  and synchronization protocol;
- worker termination, finalization, or finally can bypass the only release;
- a required projection's parent stability or disjointness is unknown.

These are compile errors even if a TypeScript hint or a profile claims the
case never occurs. A compile error is preferable to hidden runtime state or a
native path that has no defined ownership proof. An explicit contract can make
an otherwise unprovable precondition UB only when the verifier can still see
and validate its owner, root, pin, carrier, and exit obligations.

## 9. Optimizer obligations

The optimizer may use an ownership, alias, lifetime, root, or pin fact only
after one of these events:

1. whole-program inference proves it;
2. a visible guard establishes it on the current path and the failure path is
   the exact generic Tier 1 native path; or
3. the explicit Hare contract has been verified and its precondition is part of
   Hare's UB boundary.

It may then remove an allocation, root operation, pin bookkeeping, copy, or
check only when the resulting program still preserves all defined exits. It
must not turn a TypeScript type, profile, naming convention, WeakRef, or opaque
runtime flag into an assumption. It must not use noalias, lifetime markers,
stack placement, or an LLVM assume before the corresponding evidence and
carrier have been checked.

The preferred lowering order is to keep the generic managed value correct,
then specialize to a stack slot, continuation frame, region, stable heap,
handle, transfer, or independent reference-counted owner as the facts permit.
Arc/reference counting is never a universal pinning strategy, and a hidden
runtime ownership table is not a substitute for explicit Hare memory facts.

## 10. Inventory and acceptance

LIFETIMES.tsv is the concrete decision inventory. Each row has a stable ID, a
named owner, root, pin, carrier, applicable exits, evidence status, and an
escalation condition. The row is incomplete if any of those fields is empty or
says only “runtime magic.”

The status column uses the lattice names proof (P), guard (G), hint (H), and
unknown (U). P includes a verifier-accepted explicit contract; the contract is
not a fifth kind of evidence. U means that the generic carrier is the current
choice or that the escalation column names the condition that must become a
compile error. The exit values use these compact names where present: EC is
cancellation, EF is Effect failure, ED is Effect defect, EI is interruption,
and FC is FFI cancellation or late callback. The full all-exit obligations are
defined in Section 6.

H004's checkpoint is met when the inventory and examples cover sync, async,
generators, Effect success/failure/interruption/cancellation, FFI borrow and
retention, GC roots and safepoints, weak references, callback re-entry,
projection/disjointness, structured clone/transfer, and cross-thread sharing,
with every unknown boundary classified as generic Tier 1 or a compile error.
