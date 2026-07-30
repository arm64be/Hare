# Hare tooling

## Effect provenance manifest

`generate-effect-manifest.py` verifies Hare's exact `effect@3.22.0` root pin,
lock tuple and transitive graph, registry integrity, installed package tree,
and upstream tag identity. It emits the complete per-file and export-condition
manifest consumed by H005 and later Effect lowering work.

Fetch both immutable registry artifacts and the selected upstream tag into an
ignored temporary directory, then run:

```sh
python3 scripts/hare/generate-effect-manifest.py \
  --selected-tarball tmp/hare-effect-audit/effect-3.22.0.tgz \
  --comparison-tarball tmp/hare-effect-audit/effect-3.19.19.tgz \
  --upstream-root tmp/hare-effect-audit/upstream-3.22.0 \
  --output generated/hare/effect-3.22.0.manifest.json
```

The generator refuses a different artifact, installed tree, root pin, lock
integrity, or upstream commit. The tarballs and checkout remain ignored; the
verified machine manifest is committed.

## Debug baseline harness

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
