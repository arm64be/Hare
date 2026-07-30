# H002 — Hare semantic contract

Status: draft contract for the W0 H002 checkpoint. This document defines the
meaning of `bun build --compile --hare`; it does not define a compiler
implementation, a test plan, or a second product mode.

The source snapshot for Bun-specific evidence is
`bbe3f6a2629adf808adbd0da199ae8c94a3c0d47`. The checked-out Hare fork may
contain later documentation or source edits, but a claim about Bun behavior is
accepted here only when it is backed by the pinned source tree and its
compatibility contract.

## 1. Verdict vocabulary

Every reviewed construct has exactly one Hare verdict:

| Verdict | Meaning | Runtime consequence |
| --- | --- | --- |
| Preserved defined behavior | The behavior is defined by the applicable reference contract and Hare admits it. | Native code must preserve its observable ECMAScript, Bun, Node, or Web behavior. |
| Defined compile error | The behavior is defined, but the closed-world artifact cannot represent it with the admitted inputs. | `--hare` rejects the build and never emits a fallback executable. A surrounding `try`, `catch`, or promise rejection handler does not change this verdict. |
| Explicit Hare UB | A separately visible Hare ownership, alias, mutability, lifetime, or pin contract was accepted and then violated. | The optimizer may assume the violation does not occur. |

Unsupported defined behavior is never UB merely because Hare does not yet lower
it. Ordinary JavaScript values, false TypeScript assertions, stale profiles,
missing modules, and malformed external input remain in the defined/error
domain and must not be converted into a memory assumption.

## 2. One mode, one closed-world rule

Hare has one user-facing mode: `--hare`. There is no safe, profiled, required,
fallback, interpreter, JSC-runtime, or runtime-JavaScript-compiler mode.

The final executable contains native code for the complete statically reachable
application graph and the reachable Bun runtime helpers. It does not contain
application source, application bytecode, a bytecode interpreter, a JIT, or a
runtime source compiler. A Tier 2 miss is a native Tier 1 path, never a jump to
JSC.

The graph is closed before linking:

1. Every entry point and every statically reachable module is resolved from the
   build inputs, package metadata, and pinned Bun resolver.
2. Every accepted static import, re-export, dynamic-import target, CommonJS
   target, asset, and static file worker contributes a graph edge.
3. Builtins are graph references to their admitted Bun implementation, not
   permission to load host JavaScript later.
4. No JavaScript module, asset, worker, or source string is deferred to the
   host filesystem, a package resolver, a network, or an embedded compiler at
   runtime.
5. If a defined operation cannot be lowered while keeping this graph closed,
   the build fails at the operation's source location.

The normal Bun bundler's `allowUnresolved` behavior is not an exception to this
contract. It can intentionally pass dynamic `import()`, `require()`, and
`require.resolve()` targets to a runtime resolver; Hare rejects that escape
unless a finite target-set proof below succeeds.

## 3. Reference hierarchy

The semantic reference is the following, in order:

| Reference | Use in H002 |
| --- | --- |
| [ECMAScript 2025 — `PerformEval`](https://tc39.es/ecma262/2025/multipage/global-object.html#sec-performeval) | `eval`, directness, realms, strictness, declarations, and completion behavior. |
| [ECMAScript 2025 — Function constructor](https://tc39.es/ecma262/2025/multipage/fundamental-objects.html#sec-function-constructor) and [`CreateDynamicFunction`](https://tc39.es/ecma262/2025/multipage/fundamental-objects.html#sec-createdynamicfunction) | `Function`/`new Function`, parameter and body construction, global scope, and syntax errors. |
| [ECMAScript 2025 — import calls](https://tc39.es/ecma262/2025/multipage/ecmascript-language-expressions.html#sec-import-calls) | Promise-returning dynamic module loading and import-call evaluation. |
| [Node.js v26.5.1 CommonJS API](https://nodejs.org/api/modules.html#requireid) and [`require.resolve`](https://nodejs.org/api/modules.html#requireresolverequest-options) | CommonJS cache, cycles, resolution-only behavior, and `MODULE_NOT_FOUND`. This is the v26.5.1 page snapshot checked on 2026-07-30; Node documentation is a compatibility reference, not permission to defer loading to Node. |
| [Node.js v26.5.1 VM API](https://nodejs.org/api/vm.html#class-vmscript) | The source-compilation meaning of `vm.Script`, `runIn*`, and related APIs. |
| [Node.js v26.5.1 worker API](https://nodejs.org/api/worker_threads.html#new-workerfilename-options) | File/data worker inputs and `{ eval: true }`. |
| [WHATWG HTML — workers](https://html.spec.whatwg.org/multipage/workers.html#the-worker-interface) | Web Worker creation, worker script loading, `importScripts`, errors, and messaging. This is a living standard; the section is identified rather than assigned a fictitious edition. |
| [Bun workers](https://bun.com/docs/runtime/workers) and [Bun loaders](https://bun.com/docs/bundler/loaders) | Bun-specific worker entry resolution, Blob workers, and asset loader behavior. The pinned local implementation remains authoritative for this fork. |

When a Bun document and the pinned Bun source disagree, the pinned source and
its tests identify the compatibility behavior to preserve or reject. When a
defined reference behavior is not admitted by this document, the verdict is a
defined compile error, not a silent semantic substitution.

## 4. Finite target-set proof

An expression is an admitted module target only when Hare proves an exact,
finite set of possible string values and resolves every member at build time.
The proof is a semantic fact, not a guess about a variable's type.

For an expression `e` in a particular importer and import kind, the proof must
produce a set `S = {s1, ..., sn}` such that every execution reaching the load
uses exactly one `si`, and every `si` is in the resolved closed graph. The
compiler may include more than one member in the graph; the runtime operation
still chooses the actual member and preserves its normal Promise, throw, and
evaluation behavior.

The proof may use:

- string literals and exact constants;
- constant propagation through bindings whose possible values are proven;
- concatenation and template literals whose substitutions have finite proven
  sets;
- a runtime conditional whose branches each have finite proven sets, taking
  the union of both branches;
- an intrinsic conversion whose exact result is proven, including the
  ECMAScript `ToString` behavior required by the operation; and
- an exact unreachable-code proof that removes a load which cannot execute.

The proof must reject or remain unknown when a value can come from a user call,
getter, Proxy, `process.env`, filesystem or network input, randomness, FFI,
unbounded string construction, an unresolved package condition, or a profile.
The proof also fails if any member is missing, external to the artifact, or
resolves differently under an unrecorded import condition.

Examples:

| Example | H002 verdict | Reason |
| --- | --- | --- |
| `import("./a.js")` | Preserved | Singleton target set. |
| `const p = "./" + "a.js"; import(p)` | Preserved | Constant-folded singleton target set. |
| `const p = flag ? "./a.js" : "./b.js"; import(p)` | Preserved if both resolve | Runtime choice, finite set `{a, b}`, and both graph edges are present. |
| ``import(`./locales/${lang}.js`)`` | Defined compile error unless `lang` has a proven finite set and every expansion resolves | A template shape alone is not a closed graph. |
| `import(process.env.MODULE)` | Defined compile error | Runtime source of the target. |
| `try { require(getRequest()) } catch {}` | Defined compile error | Catching a runtime resolution failure does not prove the target set. |
| `if (false) import("./missing.js")` | Omit only with a proven unreachable branch; otherwise defined compile error | Dead-code proof is the only graph-edge exception. |

An exact finite proof is required independently for each load operation. A
proof for one call does not authorize another call that receives the same
TypeScript type, profile, or string-shaped value.

## 5. Preserved source evaluation

H002 admits exactly two ECMAScript source-compilation families: intrinsic
`eval` and intrinsic `Function`/`new Function`. They are additional build
inputs, not runtime compilation. Their source text must be exactly known at
build time, and any source they contain is recursively subject to this
contract.

### 5.1 `eval`: direct and indirect

The direct/indirect distinction is semantic. Hare must use the ECMAScript
direct-eval rule: a call is direct only when the evaluated reference has the
name `eval` and its value is the intrinsic `%eval%`. The text `eval` in a
source file is not enough.

| Example | H002 verdict | Required semantics |
| --- | --- | --- |
| `eval("x + 1")` with the intrinsic binding | Preserved if the string is proven | Direct eval sees the caller's realm and lexical/private environment, preserves strictness, and preserves direct-eval declaration and completion rules. |
| `"use strict"; eval("var x = 1")` | Preserved if the string is proven | Strict direct-eval scope behavior is retained; it is not rewritten as an unrelated global function. |
| `eval("x = 1")` in sloppy caller code | Preserved if the string is proven | Sloppy direct-eval writes and `var` behavior remain those of the caller's environment. |
| `(0, eval)("x + 1")`, `globalThis.eval("x + 1")`, or `const e = eval; e("x + 1")` | Preserved if the string is proven | Indirect eval uses the global environment of the intrinsic eval realm, not the caller's local scope. |
| `const eval = custom; eval(source)` | Ordinary call; compile error if `custom` itself is unsupported | A shadowed or replaced binding is not silently treated as intrinsic eval. |
| `eval(42)` or `eval(new String("x"))` | Preserved | Non-string input is returned unchanged; it is not source compilation. |
| `eval(runtimeSource)` or `eval(readFileSync(path, "utf8"))` | Defined compile error | Runtime source generation cannot be represented in the artifact, even inside `try/catch`. |
| `eval({ toString() { return runtimeSource; } })` | Preserved only as non-string object behavior, if the surrounding object code is lowerable | `eval` does not call `toString` to turn a non-string argument into source. |

For an admitted literal, the compiler must preserve evaluation order, source
text identity after any proven constant computation, syntax errors, thrown
values, completion values, declaration effects, and all nested loads. A
compile-time-known syntax error is not permission to change a catchable runtime
`SyntaxError` into an unconditional build failure; the accepted native path
must retain the error edge at the call site. If Hare cannot lower the exact
direct or indirect semantics, the call is a defined compile error.

### 5.2 `Function` and `new Function`

The intrinsic `Function` constructor is admitted in both call and construct
form when every parameter and body argument has an exact build-time value after
the constructor's specified string conversion. `new Function("a", "return a")`
therefore creates a native function with global-function scope semantics; it
does not close over the caller's lexical variables. `Function("return 1")` has
the same constructor semantics.

The contract preserves parameter-list parsing, body parsing, strictness from
the body, `this`, `arguments`, function identity and construction behavior,
syntax errors, and nested source/module operations. Invalid literal source must
retain its normal catchable construction-time error. Any unknown argument,
runtime concatenation, or runtime source read is a defined compile error.

```js
new Function("a", "return a + 1");          // preserved: exact global function
Function("return typeof localName");        // preserved: no caller lexical capture
new Function("return eval(runtimeText)");   // compile error: nested dynamic source
new Function(runtimeBody);                   // compile error: unknown body
```

The admission of `eval` and `Function` does not admit arbitrary APIs that happen
to receive literal source. Those APIs are classified in section 8.

## 6. Modules, imports, and graph observability

### 6.1 ESM and dynamic import

Static imports and re-exports are preserved when the resolver can include their
complete transitive graph. Literal or finite-target `import()` is also
preserved. Its asynchronous Promise behavior, module namespace identity,
evaluation ordering, top-level-await behavior, rejection behavior, import
attributes that affect resolution, and nested graph edges remain observable.

An accepted dynamic import is not rewritten into a synchronous require merely
because the target is known. The implementation may share native machinery, but
the observable import-call contract remains.

Unknown or runtime-generated targets are compile errors, including in these
forms:

```js
await import(moduleName);
await import(`./plugins/${name}.js`);
try { await import(runtimeSpecifier); } catch {}
import(runtimeSpecifier).catch(() => {});
```

`allowUnresolved`, an `external` configuration, a promise rejection handler,
or a profile cannot turn any of these into a closed Hare graph.

### 6.2 CommonJS loading, cache, and cycles

Hare preserves the semantics of an admitted literal or finite-target:

```js
require("./cjs.cjs");
module.require("./cjs.cjs");
createRequire(import.meta.url)("./cjs.cjs");
import.meta.require("./asset.txt");
```

The call is admitted only when the receiver and loader identity are proven to
be the applicable Bun/Node loader and every target resolves into the graph.
Overridden `Module.prototype.require`, reassigned loader functions, unknown
`paths` options, and runtime-selected requests are compile errors unless Hare
has an equally precise contract for them.

The resolved module identity is the cache key. The cache entry is created
before evaluating a CommonJS module, so a cycle can observe the partially
initialized exports object. A successful second load returns the same cached
module without re-executing it. A failed load removes the provisional entry in
the same cases as the pinned Bun loader. ESM/CJS wrappers and interop are part
of the result; they are not optional bundler conveniences.

```js
// cjs-a.cjs
exports.name = "a-start";
exports.b = require("./cjs-b.cjs");
exports.name = "a-done";

// cjs-b.cjs
exports.seen = require("./cjs-a.cjs").name; // "a-start", not a duplicate module
```

The graph must retain both the resolved identity and the import kind. Bun's
resolver selects package export conditions using `require`/`require.resolve`
versus ESM import conditions; collapsing these into one specifier key can
select a different file and is observable.

The local AST names these distinctions as `Stmt`, `Require`, `Dynamic`, and
`RequireResolve` (`src/ast/lib.rs:40-59`). The resolver separately chooses the
`require` condition for `Require` and `RequireResolve`
(`src/resolver/resolver.rs:2651-2678`). Link and wrap behavior for `Require`
and `Dynamic` is likewise distinct (`src/bundler/linker_context/scanImportsAndExports.rs:225-247`).

### 6.3 Resolution-only calls and virtual paths

`require.resolve()` and `import.meta.resolve()` do not execute a module, but
their returned string or URL is observable. A host filesystem filename cannot
be substituted for a path in a single-file executable.

H002 therefore makes the following decision explicit:

1. A dynamic or finite-but-unresolved `require.resolve`, `import.meta.resolve`,
   `Bun.resolveSync`, or equivalent call is a defined compile error.
2. A literal resolution-only call is also a defined compile error until Hare's
   public virtual-path contract specifies the exact returned string/URL,
   normalization, platform spelling, query handling, embedded-file mapping,
   and failure behavior. It is not UB and it is not an excuse to expose a host
   path.
3. This is the only open H002 design item: the classification is settled
   (reject until specified), while the exact path/URL spelling is deferred to a
   path contract. No other graph rule depends on that spelling.

Static asset imports remain admissible when their bytes are build inputs. The
asset loader must embed the bytes or package them in the declared artifact and
must provide a stable virtual identity when the imported value is a path. It
must not derive the value from the process's runtime working directory. The
current Bun compile corpus observes `$bunfs`-style paths on Unix and a
Windows-specific virtual spelling (`test/bundler/bundler_compile.test.ts:526-542`)
and preserves embedded asset names and contents across a changed working
directory (`test/regression/issue/31575.test.ts:7-67`). Those observations are
evidence for the required contract, not a silent H002 promise of an unpinned
public path format.

## 7. Workers and other graph entries

### 7.1 Admitted static workers

A worker whose entry is a statically resolved file/module is an additional graph
entry. These forms are admissible when the URL/path and all preload entries are
known and resolve at build time:

```js
new Worker("./worker.ts");
new Worker(new URL("./worker.ts", import.meta.url));
new Worker(new URL("./worker.ts", import.meta.url).href, {
  preload: ["./worker-preload.js"],
});
```

The native worker must preserve the applicable Bun/Web/Node behavior: separate
worker global state, module evaluation, structured cloning and transfer,
message ordering, error events, termination/close behavior, ref/unref behavior,
and worker-data or environment semantics. A worker build failure remains a
build error; a runtime exception in an admitted worker remains a runtime worker
error.

### 7.2 Source and runtime worker targets

These are defined APIs but are not admitted source inputs by H002:

| Example | Verdict | Why |
| --- | --- | --- |
| `new Worker(runtimePath)` | Defined compile error | Runtime path is a graph escape. |
| `new Worker(source, { eval: true })` | Defined compile error, even when `source` is a literal | Node defines this as source execution; H002 admits source only through `eval`/`Function`. |
| `new Worker(URL.createObjectURL(new Blob([runtimeSource])))` | Defined compile error | Blob URL execution compiles in-memory source at runtime. |
| `new Worker(URL.createObjectURL(new Blob(["literal source"])))` | Defined compile error | A literal Blob worker is a different source-loading API and H002 does not define its realm, URL, MIME/loader, and worker graph semantics. |
| `importScripts(runtimeUrl)` | Defined compile error | Runtime worker graph discovery. |
| `new Worker(new URL("./worker.ts", import.meta.url))` | Preserved | The file entry and its graph are statically known. |

This explicit rejection is deliberate: a future contract may admit literal
Blob/data workers only after defining their source provenance, loader, URL
identity, graph closure, and worker runtime semantics. Until then they are
unsupported defined behavior, never UB.

## 8. Source-loading APIs not admitted by H002

The following APIs are source or graph compilers/loaders in the pinned Bun or
Node surface. H002 classifies calls to them as defined compile errors, even if
the source argument is a literal, because this document does not define their
additional realm/context/loader/cache semantics and Hare does not ship their
runtime compiler:

| Surface | Examples | Verdict |
| --- | --- | --- |
| Node VM script/evaluation | `new vm.Script(source)`, `vm.runInThisContext(source)`, `vm.runInContext(source, ctx)`, `vm.runInNewContext(source, ctx)`, `vm.compileFunction(source, params)` | Defined compile error. `vm.Script` compilation without execution and later repeated execution are also not reduced to `eval`. |
| VM module source | `new vm.SourceTextModule(source)`, `vm.Module` linking/evaluation | Defined compile error: runtime module records and linker callbacks are not an admitted graph source. |
| ShadowRealm | `realm.evaluate(source)`, `realm.importValue(specifier, name)` | Defined compile error: both source evaluation and a distinct module realm are outside H002's admitted inputs. |
| Bun transpiler | `new Bun.Transpiler(...).transform(source)`, `transformSync`, `scan`, and `scanSync` | Defined compile error: this is runtime parser/transpiler work. |
| Bun bundler | `Bun.build({ entrypoints: runtimeEntries })` and plugin `onLoad` source generation | Defined compile error: runtime graph discovery, plugins, and output generation are not present in a Hare executable. |
| Other source compilers | Future Bun/Node/Web APIs that compile user-provided source | Defined compile error until a later contract specifies the complete semantics and closed-world provenance. |

This table is the required classification for “other literal source APIs”: no
literal source API is implicitly admitted merely because its text is constant.
The exception is the explicitly specified `eval`/`Function` family in section
5 and statically resolved file workers in section 7.

The local evidence for this boundary is direct: `node:vm` constructs
`Script` from source (`src/js/node/vm.ts:92-119`), Node workers convert
`{ eval: true }` source to a Blob URL (`src/js/node/worker_threads.ts:946-973`),
`Bun.Transpiler` is a parser/transpiler host (`src/runtime/api/JSTranspiler.rs:1-20`),
and `Bun.build` accepts entrypoints and plugins (`src/runtime/api/JSBundler.rs:1-2,805-818,1341-1377`).

## 9. TypeScript and profile facts

TypeScript declarations, assertions, overloads, `@ts-expect-error`, and
optimization profiles are hints. They can guide analysis, but cannot establish
source provenance, module closure, memory safety, or UB.

| Example | Verdict | Reason |
| --- | --- | --- |
| `const p: string = "./known.js"; import(p)` | Preserved if value analysis proves the singleton | A broad type does not prevent a sound constant proof. |
| `const p = getPath() as "./known.js"; import(p)` | Defined compile error unless the runtime value is independently proven | A literal assertion does not constrain JavaScript execution. |
| `const source = runtimeValue as string; eval(source)` | Defined compile error | The assertion does not make source text static. |
| A profile says `moduleName` is usually `"./a.js"` | Defined compile error if other values remain possible | A profile is a layout/speculation hint, not a graph proof. |
| A false type/profile predicts a number but runtime supplies a string | Preserved through generic native semantics, or a defined compile error if it crosses a required closed-world boundary | It is never UB merely because the hint was false. |
| `const n = 42 as unknown as string; eval(n)` | Preserved as `eval(42)` if the value is proven to be the number `42`; otherwise defined compile error | ECMAScript's non-string eval rule controls. |

Only a verified fact derived from program semantics or a visible Hare contract
can become an optimizer assumption. A stale profile must use the native generic
path, not JSC and not UB.

## 10. Explicit Hare UB boundary

H002 introduces no new memory syntax. It records the boundary that later Hare
memory contracts must obey:

- A contract must be visible to the programmer and named as a Hare ownership,
  alias, mutability, lifetime, or pin contract.
- The verifier must establish that the contract applies at the boundary before
  the optimizer uses it as an assumption.
- Violating that accepted contract is explicit Hare UB.
- Without that contract, a dangling/aliased/mutable source view is not a license
  to miscompile JavaScript; it is either ordinary defined behavior or a defined
  compile error.

Illustrative contract-shaped cases (not proposed syntax):

```text
Hare contract: `source_view` is immutable, uniquely owned, and pinned until
the admitted source operation returns.
Caller mutates it, aliases it through a forbidden mutable reference, or lets it
die before the operation returns: explicit Hare UB.
```

```text
Hare contract: `target_view` is an immutable, valid module-specifier borrow for
the duration of closed-graph resolution.
Caller passes a dangling view or mutates the backing bytes during resolution:
explicit Hare UB.
```

The following are not UB: `eval(runtimeString)`, a false `as "./x.js"`
assertion, a stale profile, an unresolved module, a missing asset, a caught
`SyntaxError`, a caught `MODULE_NOT_FOUND`, a runtime Blob URL, or an ordinary
JavaScript alias not covered by a Hare contract. Each is preserved or rejected
by the verdict rules above.

## 11. Adversarial review corpus

The following examples cover the boundary without relying on a type annotation
or a promise/error handler to hide an open edge:

| Category | Example | H002 verdict |
| --- | --- | --- |
| Static ESM | `import { value } from "./value.js"` | Preserved; include transitive graph and live bindings. |
| Static re-export | `export * from "./value.js"` | Preserved; include transitive graph. |
| Finite dynamic import | `import(flag ? "./a.js" : "./b.js")` | Preserved if both resolve; include both. |
| CJS literal | `require("./a.cjs")` | Preserved; retain wrapper/cache/cycle behavior. |
| Builtin | `require("node:fs")` | Preserved through the admitted Bun builtin. |
| Direct literal eval | `eval("let x = 1; x")` | Preserved with direct lexical/strictness semantics. |
| Indirect literal eval | `(0, eval)("globalThis.x = 1")` | Preserved with global semantics. |
| Literal Function | `new Function("a", "return a + 1")` | Preserved with global-function semantics. |
| Non-string eval | `eval(42)` | Preserved as `42`; no source input. |
| Static asset | `import path from "./asset.txt" with { type: "file" }` | Preserved if embedded and virtual identity is provided. |
| Static worker | `new Worker(new URL("./worker.ts", import.meta.url))` | Preserved; include worker graph. |
| Runtime eval | `eval(await readSource())` | Defined compile error. |
| Runtime Function | `new Function(runtimeBody)` | Defined compile error. |
| Runtime import | `import(process.env.PLUGIN)` | Defined compile error. |
| Caught runtime import | `try { await import(name) } catch {}` | Defined compile error. |
| Caught unresolved require | `try { require(name) } catch {}` | Defined compile error. |
| VM source | `new vm.Script("1 + 2")` | Defined compile error; literal VM source is not implicitly admitted. |
| Literal Blob worker | `new Worker(URL.createObjectURL(new Blob(["..."])))` | Defined compile error; source API not admitted. |
| Runtime transpilation | `new Bun.Transpiler().transform(runtimeSource)` | Defined compile error. |
| Runtime bundling | `Bun.build({ entrypoints: [runtimeEntry] })` | Defined compile error. |
| Resolution-only | `require.resolve("./asset.txt")` or `import.meta.resolve("./asset.txt")` | Defined compile error until virtual path/URL semantics are pinned. |
| False TS source proof | `eval(getSource() as string)` | Defined compile error, never UB. |
| False profile | Profile predicts one plugin, runtime chooses another | Defined compile error at the open graph boundary, never UB. |
| Explicit Hare contract | A verified pinned immutable source borrow is mutated | Explicit Hare UB, and only because the visible contract says so. |

No example in this corpus is intentionally left between the three verdicts.

## 12. Local evidence index

These are the independently checked local sources for the contract:

| Evidence | Observation |
| --- | --- |
| `src/ast/lib.rs:40-59` | `ImportKind` distinguishes statement, require, dynamic import, and require-resolve records. |
| `src/js/builtins/CommonJS.ts:18-20,36-77,107-143` | CommonJS resolution, cache lookup, cache insertion before evaluation, ESM interop, and error cleanup. |
| `src/js/builtins/CommonJS.ts:147-158` | `require.resolve` resolves without evaluating the module. |
| `src/resolver/resolver.rs:2651-2678` | Package export conditions distinguish require/require-resolve from import. |
| `src/bundler/linker_context/scanImportsAndExports.rs:225-247` | Require and dynamic-import edges have distinct wrapping/link behavior. |
| `test/bundler/bundler_allow_unresolved.test.ts:100-173` | Empty unresolved policy rejects dynamic import, require, and require.resolve, including try/catch. |
| `src/js/node/vm.ts:92-119` | `runIn*` and `createScript` accept source strings and construct scripts. |
| `src/js/node/worker_threads.ts:946-973` | Node worker `{ eval: true }` converts source into a Blob URL. |
| `test/js/web/workers/worker_blob.test.ts:3-64` | Bun executes JavaScript and TypeScript Blob worker source and reports resolution errors. |
| `src/runtime/api/JSTranspiler.rs:1-20,1302-1774` | `Bun.Transpiler` exposes runtime scan/transform operations over source. |
| `src/runtime/api/JSBundler.rs:1-2,805-818,1341-1377` | `Bun.build` accepts entrypoints, plugins, and runtime build configuration. |
| `packages/bun-types/globals.d.ts:396-412` | The exposed `ShadowRealm` surface includes `evaluate` and `importValue`. |
| `test/bundler/bundler_compile.test.ts:402-451,506-525,526-542` | Compiled assets, `import.meta.require.resolve`, and platform-specific virtual paths are observable in Bun's compile corpus. |
| `test/regression/issue/31575.test.ts:7-67` | Embedded asset names/content remain available when the executable runs from another working directory. |
| `test/bundler/transpiler/transpiler.test.js:2392-2409` | The transpiler folds proven CommonJS expressions but preserves unknown expressions as runtime calls. |

This document is a semantic boundary. It does not authorize an implementation
to weaken a defined behavior, add a fallback mode, or treat the open
resolution-only path spelling as an optimizer assumption.
