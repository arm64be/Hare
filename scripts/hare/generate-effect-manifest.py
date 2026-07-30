#!/usr/bin/env python3

import argparse
import base64
import hashlib
import json
import re
import subprocess
import tarfile
from pathlib import Path
from typing import Any, Callable


EXPECTED = {
    "name": "effect",
    "version": "3.22.0",
    "integrity": "sha512-jhYFe0zTlIRqYFrKTS+6luhmS/Tm0f+JLo0K9KUxvtFab1SUGEszQi2ehOP6QzAZvy831lDmTwwzvVDZSPNz3g==",
    "artifactSha256": "cb73fa0743025dac14d625b2e5a2ff2079fbccf1b5e3622f8e899ef4bd90c316",
    "upstreamRepository": "https://github.com/Effect-TS/effect.git",
    "upstreamTag": "effect@3.22.0",
    "upstreamCommit": "e670e0f6befb959b84208d5f77631276521020ae",
}

COMPARISON = {
    "version": "3.19.19",
    "integrity": "sha512-Yc8U/SVXo2dHnaP7zNBlAo83h/nzSJpi7vph6Hzyl4ulgMBIgPmz3UzOjb9sBgpFE00gC0iETR244sfXDNLHRg==",
    "artifactSha256": "02b9b83ed550df4e9ad16692fed159c31de4ba1ff7e0338bb57578d18b1b4f64",
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def integrity(data: bytes) -> str:
    digest = hashlib.sha512(data).digest()
    return "sha512-" + base64.b64encode(digest).decode("ascii")


def read_tarball(path: Path) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    with tarfile.open(path, "r:gz") as archive:
        for member in archive.getmembers():
            if member.isdir():
                continue
            if not member.isfile() or not member.name.startswith("package/"):
                raise RuntimeError(f"unsupported tar member: {member.name}")
            relative = member.name.removeprefix("package/")
            if not relative or relative.startswith("/") or ".." in Path(relative).parts:
                raise RuntimeError(f"unsafe tar member: {member.name}")
            if relative in files:
                raise RuntimeError(f"duplicate tar member: {relative}")
            extracted = archive.extractfile(member)
            if extracted is None:
                raise RuntimeError(f"cannot read tar member: {member.name}")
            files[relative] = extracted.read()
    return files


def read_tree(root: Path) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise RuntimeError(f"source tree contains a symlink: {path}")
        if path.is_file():
            relative = path.relative_to(root).as_posix()
            files[relative] = path.read_bytes()
    return files


def file_record(path: str, data: bytes) -> dict[str, Any]:
    return {"path": path, "sha256": sha256(data), "size": len(data)}


def records(
    files: dict[str, bytes], predicate: Callable[[str], bool]
) -> list[dict[str, Any]]:
    return [file_record(path, files[path]) for path in sorted(files) if predicate(path)]


def tree_hash(items: list[dict[str, Any]]) -> str:
    digest = hashlib.sha256()
    for item in items:
        digest.update(item["path"].encode("utf-8"))
        digest.update(b"\0")
        digest.update(item["sha256"].encode("ascii"))
        digest.update(b"\0")
        digest.update(str(item["size"]).encode("ascii"))
        digest.update(b"\n")
    return digest.hexdigest()


def verify_artifact(path: Path, expected: dict[str, str]) -> tuple[bytes, dict[str, bytes]]:
    artifact = path.read_bytes()
    if sha256(artifact) != expected["artifactSha256"]:
        raise RuntimeError(f"unexpected SHA-256 for {path}")
    if integrity(artifact) != expected["integrity"]:
        raise RuntimeError(f"unexpected registry integrity for {path}")
    files = read_tarball(path)
    package = json.loads(files["package.json"])
    if package["name"] != EXPECTED["name"] or package["version"] != expected["version"]:
        raise RuntimeError(f"unexpected package metadata in {path}")
    return artifact, files


def lock_tuple(lock_text: str, name: str) -> dict[str, Any]:
    pattern = re.compile(rf'^    "{re.escape(name)}": \[(.+)\],$', re.MULTILINE)
    match = pattern.search(lock_text)
    if match is None:
        raise RuntimeError(f"bun.lock has no package tuple for {name}")
    values = json.loads("[" + match.group(1) + "]")
    if len(values) != 4 or not isinstance(values[0], str):
        raise RuntimeError(f"unsupported bun.lock tuple for {name}")
    prefix = name + "@"
    if not values[0].startswith(prefix):
        raise RuntimeError(f"bun.lock tuple name mismatch for {name}")
    return {
        "name": name,
        "version": values[0][len(prefix) :],
        "metadata": values[2],
        "integrity": values[3],
    }


def dependency_graph(lock_text: str, package: dict[str, Any]) -> list[dict[str, Any]]:
    seen: set[str] = set()
    result: list[dict[str, Any]] = []

    def visit(name: str, requested: str) -> None:
        if name in seen:
            return
        seen.add(name)
        item = lock_tuple(lock_text, name)
        dependencies = item["metadata"].get("dependencies", {})
        result.append(
            {
                "name": name,
                "requested": requested,
                "resolved": item["version"],
                "integrity": item["integrity"],
                "dependencies": dependencies,
            }
        )
        for child, child_range in sorted(dependencies.items()):
            visit(child, child_range)

    for name, requested in sorted(package.get("dependencies", {}).items()):
        visit(name, requested)
    return result


def export_record(value: Any, files: dict[str, bytes]) -> Any:
    if isinstance(value, str):
        path = value.removeprefix("./")
        if path not in files:
            return {"path": path, "present": False}
        return {**file_record(path, files[path]), "present": True}
    if isinstance(value, dict):
        return {key: export_record(target, files) for key, target in value.items()}
    raise RuntimeError(f"unsupported export target: {value!r}")


def compare_files(left: dict[str, bytes], right: dict[str, bytes], prefix: str = "") -> dict[str, Any]:
    left_paths = {path for path in left if path.startswith(prefix)}
    right_paths = {path for path in right if path.startswith(prefix)}
    shared = left_paths & right_paths
    return {
        "added": sorted(right_paths - left_paths),
        "removed": sorted(left_paths - right_paths),
        "changed": sorted(path for path in shared if left[path] != right[path]),
        "unchangedCount": sum(left[path] == right[path] for path in shared),
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generate and verify Hare's canonical Effect package manifest."
    )
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--selected-tarball", type=Path, required=True)
    parser.add_argument("--comparison-tarball", type=Path, required=True)
    parser.add_argument("--upstream-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    repo = args.repo.resolve()
    selected_artifact, selected = verify_artifact(args.selected_tarball, EXPECTED)
    _, comparison = verify_artifact(args.comparison_tarball, COMPARISON)
    package = json.loads(selected["package.json"])

    installed = read_tree(repo / "node_modules" / "effect")
    installed_comparison = compare_files(selected, installed)
    if installed_comparison["added"] or installed_comparison["removed"] or installed_comparison["changed"]:
        raise RuntimeError("node_modules/effect does not match the selected registry artifact")

    root_package = json.loads((repo / "package.json").read_text())
    if root_package.get("devDependencies", {}).get("effect") != EXPECTED["version"]:
        raise RuntimeError("package.json does not contain the exact Effect pin")

    lock_text = (repo / "bun.lock").read_text()
    root_pin = re.findall(r'^        "effect": "([^"]+)",$', lock_text, re.MULTILINE)
    if root_pin != [EXPECTED["version"]]:
        raise RuntimeError("bun.lock does not contain one exact root Effect pin")
    effect_lock = lock_tuple(lock_text, "effect")
    if effect_lock["version"] != EXPECTED["version"] or effect_lock["integrity"] != EXPECTED["integrity"]:
        raise RuntimeError("bun.lock Effect tuple does not match the selected artifact")

    upstream_root = args.upstream_root.resolve()
    upstream_commit = subprocess.run(
        ["git", "-C", str(upstream_root), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if upstream_commit != EXPECTED["upstreamCommit"]:
        raise RuntimeError("upstream checkout is not the selected Effect tag commit")
    upstream_source = read_tree(upstream_root / "packages" / "effect" / "src")
    published_internal = {
        path.removeprefix("src/internal/"): data
        for path, data in selected.items()
        if path.startswith("src/internal/")
    }
    upstream_internal = {
        path.removeprefix("internal/"): data
        for path, data in upstream_source.items()
        if path.startswith("internal/")
    }
    internal_comparison = compare_files(published_internal, upstream_internal)
    if internal_comparison["added"] or internal_comparison["removed"] or internal_comparison["changed"]:
        raise RuntimeError("published internal runtime sources differ from the upstream tag")

    all_files = records(selected, lambda _: True)
    esm_files = records(selected, lambda path: path.startswith("dist/esm/"))
    cjs_files = records(selected, lambda path: path.startswith("dist/cjs/"))
    dts_files = records(selected, lambda path: path.startswith("dist/dts/"))
    source_files = records(selected, lambda path: path.startswith("src/"))
    generated_files = records(
        selected,
        lambda path: path.startswith("dist/") or path == "package.json" or path.endswith("/package.json"),
    )
    metadata_files = records(
        selected,
        lambda path: not path.startswith("src/")
        and not path.startswith("dist/")
        and path != "package.json"
        and not path.endswith("/package.json"),
    )

    comparison_package = json.loads(comparison["package.json"])
    candidate_diff = compare_files(comparison, selected)
    candidate_source_diff = compare_files(comparison, selected, "src/")
    candidate_internal_diff = compare_files(comparison, selected, "src/internal/")

    manifest = {
        "schemaVersion": 1,
        "package": EXPECTED["name"],
        "version": EXPECTED["version"],
        "rootDependency": {"kind": "devDependency", "specifier": EXPECTED["version"]},
        "registry": {
            "tarball": f"https://registry.npmjs.org/effect/-/effect-{EXPECTED['version']}.tgz",
            "integrity": EXPECTED["integrity"],
            "artifactSha256": sha256(selected_artifact),
        },
        "upstream": {
            "repository": EXPECTED["upstreamRepository"],
            "tag": EXPECTED["upstreamTag"],
            "commit": EXPECTED["upstreamCommit"],
            "publishedInternalRuntimeMatchesTag": True,
            "internalRuntimeFileCount": internal_comparison["unchangedCount"],
            "canonicalSnapshot": "registry artifact",
        },
        "lock": {
            "version": effect_lock["version"],
            "integrity": effect_lock["integrity"],
            "declaredDependencies": package.get("dependencies", {}),
            "resolvedDependencyGraph": dependency_graph(lock_text, package),
        },
        "hashAlgorithm": {
            "files": "SHA-256 over file bytes",
            "trees": "SHA-256 over sorted path + NUL + file SHA-256 hex + NUL + decimal size + LF records",
        },
        "sourceTreeSha256": tree_hash(source_files),
        "artifactTreeSha256": tree_hash(all_files),
        "exports": {key: export_record(value, selected) for key, value in package["exports"].items()},
        "artifactFiles": all_files,
        "esmFiles": esm_files,
        "cjsFiles": cjs_files,
        "dtsFiles": dts_files,
        "sourceFiles": source_files,
        "generatedFiles": generated_files,
        "metadataFiles": metadata_files,
        "reachableModules": [],
        "reachableSubset": {
            "status": "application-specific-not-computed",
            "contract": "Hare records a separate graph manifest for each compiled application; an empty array does not narrow the complete package contract.",
        },
        "candidateAudit": {
            "comparedVersion": COMPARISON["version"],
            "comparisonIntegrity": COMPARISON["integrity"],
            "comparisonArtifactSha256": COMPARISON["artifactSha256"],
            "exportCounts": {
                COMPARISON["version"]: len(comparison_package["exports"]),
                EXPECTED["version"]: len(package["exports"]),
            },
            "addedExports": sorted(set(package["exports"]) - set(comparison_package["exports"])),
            "removedExports": sorted(set(comparison_package["exports"]) - set(package["exports"])),
            "artifactPaths": candidate_diff,
            "sourcePaths": candidate_source_diff,
            "internalRuntimePaths": candidate_internal_diff,
        },
        "verification": {
            "packageMetadataMatchesRootAndLock": True,
            "lockIntegrityMatchesArtifact": True,
            "installedTreeMatchesArtifact": True,
            "publishedInternalRuntimeMatchesUpstreamTag": True,
            "comparisonCandidateAudited": True,
        },
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, indent=2, sort_keys=False) + "\n")


if __name__ == "__main__":
    main()
