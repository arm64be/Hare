# H001 debug baseline provenance and results

Captured 2026-07-30 at `2026-07-30T22:08:37+02:00` in
`/home/nvmbr/.codex/worktrees/82d8/hare`. This is a debug-build resource
baseline, not a runtime performance claim.

## Source and runtime ancestry

| Item | Exact value |
| --- | --- |
| Pinned upstream base | `bbe3f6a2629adf808adbd0da199ae8c94a3c0d47` |
| Baseline repository commit | `f0e436c0b38296157df25e2e13e9143a992dd352` (`docs(hare): preserve orchestration context`) |
| Baseline ancestry after upstream | `88b0cbb0ce68b93983d3bc9685314a781d017819` → `973a61f9bc684201dca349d49b77d00dd7c2b5fa` → `f0e436c0b38296157df25e2e13e9143a992dd352` |
| `package.json` Bun line | `1.4.0` |
| Bootstrap Bun used to run `bun bd` | Bun `1.3.14`, revision `1.3.14-canary.1+0d9b296af` |
| Bootstrap Bun WebKit dependency | `process.versions.webkit = 5488984d20e0dbfe4be2c3ba8fb18eb81a5e0e8b` |
| Hare build WebKit pin | `scripts/build/deps/webkit.ts` `WEBKIT_VERSION = 34c01d13391e00c06862a3d2c5b7fff350ac87e0` |
| Built debug WebKit verification | `bun bd -e 'console.log(process.versions.webkit)'` → `34c01d13391e00c06862a3d2c5b7fff350ac87e0`, exit 0 |

The timed baseline was run before this provenance commit, at the baseline
repository commit above. A later `bun bd --revision` sanity check necessarily
reported the then-current commit-bearing debug revision
`1.4.0-debug+4350f183c` and is not used as a timed-baseline measurement.

## Toolchain and host

The debug build selected the parallel LLVM 21 installation at
`/usr/lib/llvm21/bin`:

| Tool or condition | Exact value |
| --- | --- |
| clang / clang++ / ld.lld / llvm-ar / llvm-ranlib | `21.1.8` |
| Installed LLVM packages | `clang21 21.1.8-1`, `lld21 21.1.8-1`, `llvm21 21.1.8-1`, `llvm21-libs 21.1.8-1`, `compiler-rt21 21.1.8-1` |
| Ninja | `1.13.2-3` / `1.13.2` |
| Rust compiler | `rustc 1.99.0-nightly`, commit `9f36de775bc636c8e88c31a173c2bcb6995956a0`, LLVM `22.1.8` |
| Cargo | `1.99.0-nightly`, commit `3efb1f477e99b42974b982d939fd100303cdf7db` |
| CMake | `4.4.0` |
| GCC `cc`/`c++` | `16.1.1 20260625` |
| Git | `2.55.0` |
| Node.js | `v26.2.0` |
| Host | Arch Linux x86-64, kernel `7.1.4-arch1-1` |
| CPU | AMD Ryzen 5 5600X 6-Core Processor, 6 cores / 12 threads |
| RAM snapshot | `32778084 kB` total (31 GiB); `19618444 kB` available at capture |
| Swap snapshot | `36973084 kB` total; `13471420 kB` free |

At the initial filesystem snapshot, `build/debug` was absent and `/home` had
`254144224 KiB` available. `vendor` was `375660 KiB`, `node_modules` was
`162776 KiB`, and `BUN_INSTALL` was unset/missing in the captured condition.

## Commands and results

All build invocations used the required debug `bun bd` path. No release, LTO,
PGO, BOLT, or performance profile was selected.

| Phase | Exact command or action | Exit | Command elapsed | Peak RSS | `build/debug` size | Disk available after |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Cold upstream build | `bun bd` | 0 | `428.903447 s` | `13092404 KiB` | `8235124 KiB` | `249129544 KiB` |
| Selected behavior | `bun bd -e 'console.log(JSON.stringify({bun: Bun.version, platform: process.platform, arch: process.arch, squares: [1, 2, 3].map(value => value * value), pathname: new URL("https://example.test/path?q=1").pathname, base64: Buffer.from("hare").toString("base64")}))'` | 0 | `1.853355 s` | `507948 KiB` | `8235124 KiB` | `249130032 KiB` |
| Warm incremental rebuild | Append one newline to the clean probe, run `bun bd`, then restore it | 0 | `126.762794 s` | `9164712 KiB` | `9907388 KiB` | `248054520 KiB` |
| Focused test | `bun bd test test/js/bun/ini/ini.test.ts` | 0 | not resource-timed | not resource-timed | existing warm tree | existing warm tree |

The harness TSV also records its outer wall interval, including filesystem
sampling: cold `429.052956 s`, behavior `1.892517 s`, and warm
`126.805251 s`. The RSS value is aggregate process-tree RSS sampled from Linux
`/proc` every 50 ms by `measure-command.py`, not the RSS of one compiler child
and not total machine memory.

The selected behavior printed:

```text
{"bun":"1.4.0-debug","platform":"linux","arch":"x64","squares":[1,4,9],"pathname":"/path","base64":"aGFyZQ=="}
```

The focused test completed with `62 pass`, `0 fail`, and `65 expect() calls`.

## Incremental probe and artifacts

The warm probe was `src/bun_core/lib.rs`. The harness appended one newline,
then restored the file and metadata. SHA-256 before and after restoration was
`c358a41dbf66deb545528a82ec174cc05896ffc73aa03081b18326f5bc402800`, and the
raw probe record says `restored_exactly=yes`.

Raw command output, timing files, filesystem snapshots, and derived metrics
remain intentionally ignored under `tmp/hare-baseline/`; the committed
reproduction harness is `scripts/hare/capture-debug-baseline.sh`.
