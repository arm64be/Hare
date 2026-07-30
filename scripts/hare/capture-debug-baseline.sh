#!/usr/bin/env bash

set -u -o pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
artifact_dir=${HARE_BASELINE_DIR:-"$repo_root/tmp/hare-baseline"}
raw_dir="$artifact_dir/raw"
derived_dir="$artifact_dir/derived"
mkdir -p "$raw_dir" "$derived_dir"

for stale_marker in "$raw_dir/selected-upstream-behavior.skipped" "$raw_dir/incremental-probe.skipped"; do
  if [[ -e "$stale_marker" ]]; then
    rm -f "$stale_marker"
  fi
done

metrics_file="$derived_dir/metrics.tsv"
commands_file="$raw_dir/commands.txt"
toolchain_file="$artifact_dir/toolchain-and-environment.txt"
: > "$metrics_file"
: > "$commands_file"
: > "$toolchain_file"
printf 'phase\tstatus\telapsed_ms\tmax_rss_kib\tbuild_debug_kib\tdisk_available_kib\n' > "$metrics_file"

capture_tool() {
  local label=$1
  shift
  {
    printf '\n[%s]\n$' "$label"
    printf ' %q' "$@"
    printf '\n'
    if command -v "$1" >/dev/null 2>&1; then
      "$@"
    else
      printf 'unavailable: %s\n' "$1"
      return 127
    fi
  } >> "$toolchain_file" 2>&1 || true
}

capture_fs_state() {
  local label=$1
  local output="$raw_dir/${label}.filesystem.txt"
  {
    printf 'label=%s\n' "$label"
    date --iso-8601=seconds
    printf '\ndf -Pk %q\n' "$repo_root"
    df -Pk "$repo_root"
    printf '\npath\tdu_kib\n'
    for relative in build build/debug vendor node_modules .git; do
      if [[ -e "$repo_root/$relative" ]]; then
        printf '%s\t' "$relative"
        du -sk "$repo_root/$relative" | awk '{ print $1 }'
      else
        printf '%s\tmissing\n' "$relative"
      fi
    done
    if [[ -n "${BUN_INSTALL:-}" && -e "$BUN_INSTALL" ]]; then
      printf 'BUN_INSTALL=%s\t' "$BUN_INSTALL"
      du -sk "$BUN_INSTALL" | awk '{ print $1 }'
    else
      printf 'BUN_INSTALL\tmissing\n'
    fi
  } > "$output" 2>&1
}

capture_tool "git" git --version
capture_tool "bun" bun --version
capture_tool "bun revision" bun --revision
capture_tool "bun process versions" bun -e 'console.log(JSON.stringify(process.versions, null, 2))'
capture_tool "node" node --version
capture_tool "rustc" rustc --version --verbose
capture_tool "cargo" cargo --version --verbose
capture_tool "clang" clang --version
capture_tool "clang++" clang++ --version
capture_tool "ld.lld" ld.lld --version
capture_tool "lld" lld --version
capture_tool "selected clang21" /usr/lib/llvm21/bin/clang --version
capture_tool "selected clang++21" /usr/lib/llvm21/bin/clang++ --version
capture_tool "selected ld.lld21" /usr/lib/llvm21/bin/ld.lld --version
capture_tool "selected llvm-ar21" /usr/lib/llvm21/bin/llvm-ar --version
capture_tool "selected llvm-ranlib21" /usr/lib/llvm21/bin/llvm-ranlib --version
capture_tool "ninja" ninja --version
capture_tool "cmake" cmake --version
capture_tool "cc" cc --version
capture_tool "c++" c++ --version
capture_tool "ar" ar --version
capture_tool "uname" uname -a
capture_tool "lscpu" lscpu
capture_tool "free" free -h
capture_tool "uptime" uptime
capture_tool "nproc" nproc
capture_tool "installed baseline packages" pacman -Q clang21 lld21 llvm21 llvm21-libs compiler-rt21 ninja

{
  printf 'repo_root=%s\n' "$repo_root"
  printf 'artifact_dir=%s\n' "$artifact_dir"
  printf 'captured_at='
  date --iso-8601=seconds
  printf '\nGit state\n'
  git status --short --branch
  git rev-parse --show-toplevel
  git rev-parse --abbrev-ref HEAD
  git rev-parse HEAD
  git rev-parse bbe3f6a2629adf808adbd0da199ae8c94a3c0d47^{commit}
  git merge-base HEAD bbe3f6a2629adf808adbd0da199ae8c94a3c0d47
  git log -1 --format='commit=%H%nparent=%P%nauthor=%an <%ae>%ndate=%aI%nsubject=%s' HEAD
  printf '\nRelevant tracked input hashes\n'
  for file in package.json bun.lock scripts/build.ts scripts/build/config.ts scripts/build/profiles.ts; do
    if [[ -f "$repo_root/$file" ]]; then
      sha256sum "$repo_root/$file"
    fi
  done
  printf '\nDependency checkout state\n'
  git submodule status
  git diff --stat -- vendor scripts/build/deps package.json bun.lock
  printf '\nRelevant environment\n'
  env | LC_ALL=C sort | awk -F= '/^(BUN_|CARGO|CC|CFLAGS|CMAKE|CXX|CXXFLAGS|LD|LDFLAGS|NINJA|PATH|RUST|SDKROOT|TARGET|TMPDIR|WEBKIT|XDG_CACHE_HOME)=/ { print }'
  printf '\nKernel and memory snapshots\n'
  cat /proc/loadavg 2>/dev/null || true
  cat /proc/meminfo 2>/dev/null || true
} >> "$toolchain_file" 2>&1

capture_fs_state initial

run_measured() {
  local name=$1
  shift
  local stdout_file="$raw_dir/${name}.stdout"
  local stderr_file="$raw_dir/${name}.stderr"
  local time_file="$raw_dir/${name}.time"
  local start_ns end_ns elapsed_ns elapsed_ms status max_rss user_seconds system_seconds build_kib available_kib

  {
    printf '%s:' "$name"
    printf ' %q' "$@"
    printf '\n'
  } >> "$commands_file"
  capture_fs_state "${name}.before"
  start_ns=$(date +%s%N)
  if python3 "$repo_root/scripts/hare/measure-command.py" --time-output "$time_file" -- "$@" > "$stdout_file" 2> "$stderr_file"; then
    status=0
  else
    status=$?
  fi
  end_ns=$(date +%s%N)
  capture_fs_state "${name}.after"

  elapsed_ns=$((end_ns - start_ns))
  elapsed_ms=$(awk -v value="$elapsed_ns" 'BEGIN { printf "%.3f", value / 1000000 }')
  max_rss=$(awk -F: '$1 == "Maximum resident set size (kbytes)" { gsub(/^[ \t]+/, "", $2); print $2 }' "$time_file" | tail -n 1)
  user_seconds=$(awk -F: '$1 == "User time (seconds)" { gsub(/^[ \t]+/, "", $2); print $2 }' "$time_file" | tail -n 1)
  system_seconds=$(awk -F: '$1 == "System time (seconds)" { gsub(/^[ \t]+/, "", $2); print $2 }' "$time_file" | tail -n 1)
  build_kib=$(du -sk "$repo_root/build/debug" 2>/dev/null | awk '{ print $1 }')
  available_kib=$(df -Pk "$repo_root" | awk 'END { print $4 }')
  max_rss=${max_rss:-unknown}
  user_seconds=${user_seconds:-unknown}
  system_seconds=${system_seconds:-unknown}
  build_kib=${build_kib:-0}
  available_kib=${available_kib:-unknown}
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$status" "$elapsed_ms" "$max_rss" "$build_kib" "$available_kib" >> "$metrics_file"
  printf '%s: status=%s elapsed_ms=%s max_rss_kib=%s user_seconds=%s system_seconds=%s build_debug_kib=%s disk_available_kib=%s\n' \
    "$name" "$status" "$elapsed_ms" "$max_rss" "$user_seconds" "$system_seconds" "$build_kib" "$available_kib" >&2
  LAST_STATUS=$status
}

run_measured upstream-debug-build bun bd
upstream_build_status=$LAST_STATUS

selected_status=125
if [[ -x "$repo_root/build/debug/bun-debug" ]]; then
  selected_code='console.log(JSON.stringify({bun: Bun.version, platform: process.platform, arch: process.arch, squares: [1, 2, 3].map(value => value * value), pathname: new URL("https://example.test/path?q=1").pathname, base64: Buffer.from("hare").toString("base64")}))'
  run_measured selected-upstream-behavior bun bd -e "$selected_code"
  selected_status=$LAST_STATUS
else
  printf 'build/debug/bun-debug is unavailable; selected behavior was not run\n' > "$raw_dir/selected-upstream-behavior.skipped"
fi

probe=${HARE_INCREMENTAL_PROBE:-"$repo_root/src/bun_core/lib.rs"}
if [[ "$probe" != /* ]]; then
  probe="$repo_root/$probe"
fi
probe_rel=${probe#"$repo_root/"}
incremental_status=125
probe_backup=
restore_probe() {
  if [[ -n "$probe_backup" && -e "$probe_backup" ]]; then
    cp -p "$probe_backup" "$probe"
  fi
}

if [[ -f "$probe" ]] && git ls-files --error-unmatch -- "$probe_rel" >/dev/null 2>&1 \
  && git diff --quiet -- "$probe_rel" \
  && git diff --cached --quiet -- "$probe_rel"; then
  probe_backup=$(mktemp "${TMPDIR:-/tmp}/hare-H001-probe.XXXXXX")
  cp -p "$probe" "$probe_backup"
  probe_before_hash=$(sha256sum "$probe" | awk '{ print $1 }')
  printf 'probe=%s\nprobe_sha256_before=%s\nprobe_edit=append one newline\n' "$probe_rel" "$probe_before_hash" > "$raw_dir/incremental-probe.txt"
  trap restore_probe EXIT
  printf '\n' >> "$probe"
  run_measured warm-incremental-edit-rebuild bun bd
  incremental_status=$LAST_STATUS
  restore_probe
  probe_after_hash=$(sha256sum "$probe" | awk '{ print $1 }')
  printf 'probe_sha256_after=%s\n' "$probe_after_hash" >> "$raw_dir/incremental-probe.txt"
  if cmp -s "$probe_backup" "$probe"; then
    printf 'restored_exactly=yes\n' >> "$raw_dir/incremental-probe.txt"
  else
    printf 'restored_exactly=no\n' >> "$raw_dir/incremental-probe.txt"
    printf 'ERROR: incremental probe was not restored exactly\n' >&2
    incremental_status=126
  fi
  rm -f "$probe_backup"
  probe_backup=
  trap - EXIT
else
  printf 'probe=%s\nstatus=not-run\nreason=probe missing or already modified\n' "$probe_rel" > "$raw_dir/incremental-probe.skipped"
fi

capture_fs_state final
{
  printf 'H001 baseline metrics\n\n'
  cat "$metrics_file"
  printf '\nFinal Git state\n'
  git status --short --branch
} > "$derived_dir/summary.txt"

overall_status=0
if [[ "$upstream_build_status" -ne 0 || "$selected_status" -ne 0 || "$incremental_status" -ne 0 ]]; then
  overall_status=1
fi
printf 'Artifacts written to %s\n' "$artifact_dir" >&2
exit "$overall_status"
