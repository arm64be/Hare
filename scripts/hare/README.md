# Hare baseline harness

See [H001_BASELINE.md](H001_BASELINE.md) for the committed provenance and
results record from the pinned baseline run.

Run from the repository root:

```sh
scripts/hare/capture-debug-baseline.sh
```

The harness records toolchain and host state, runs the required debug build as
`bun bd`, exercises a deterministic upstream behavior through `bun bd -e`, and
measures a warm incremental `bun bd` after appending one newline to
`src/bun_core/lib.rs`. The probe requires that file to be clean, saves and
restores its bytes and metadata, and records hashes proving the restoration.

Raw command output and filesystem snapshots go under `tmp/hare-baseline/raw/`;
the derived metrics and summary go under `tmp/hare-baseline/derived/`. Set
`HARE_BASELINE_DIR` to collect elsewhere, or `HARE_INCREMENTAL_PROBE` to use a
different clean tracked source file. The harness never selects a release,
LTO, PGO, BOLT, or performance profile.
