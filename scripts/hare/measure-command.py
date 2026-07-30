#!/usr/bin/env python3

import argparse
import os
import resource
import subprocess
import sys
import time


def process_parent(pid: int) -> int | None:
    try:
        with open(f"/proc/{pid}/stat", encoding="utf-8") as stream:
            line = stream.read()
        closing_paren = line.rfind(")")
        fields = line[closing_paren + 2 :].split()
        return int(fields[1])
    except (FileNotFoundError, PermissionError, ValueError, IndexError):
        return None


def process_rss_kib(pid: int) -> int:
    try:
        with open(f"/proc/{pid}/status", encoding="utf-8") as stream:
            for line in stream:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except (FileNotFoundError, PermissionError, ValueError):
        pass
    return 0


def process_tree_rss_kib(root_pid: int) -> int:
    parents: dict[int, int] = {}
    for entry in os.scandir("/proc"):
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        parent = process_parent(pid)
        if parent is not None:
            parents[pid] = parent

    pids = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, parent in parents.items():
            if parent in pids and pid not in pids:
                pids.add(pid)
                changed = True
    return sum(process_rss_kib(pid) for pid in pids)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--time-output", required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("a command is required")

    started = time.monotonic()
    process = subprocess.Popen(args.command, start_new_session=True)
    peak_rss_kib = 0
    while process.poll() is None:
        peak_rss_kib = max(peak_rss_kib, process_tree_rss_kib(process.pid))
        time.sleep(0.05)
    return_code = process.wait()
    peak_rss_kib = max(peak_rss_kib, process_tree_rss_kib(process.pid))
    elapsed = time.monotonic() - started
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    with open(args.time_output, "w", encoding="utf-8") as stream:
        stream.write(f"User time (seconds): {usage.ru_utime:.6f}\n")
        stream.write(f"System time (seconds): {usage.ru_stime:.6f}\n")
        stream.write(f"Elapsed (wall clock) seconds: {elapsed:.6f}\n")
        stream.write(f"Maximum resident set size (kbytes): {peak_rss_kib}\n")
        stream.write(f"Exit status: {return_code}\n")
    return return_code if return_code >= 0 else 128 - return_code


if __name__ == "__main__":
    sys.exit(main())
