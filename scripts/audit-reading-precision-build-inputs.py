# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Verify retained compiler inputs, executable bytes and real test logs."""
import argparse
import datetime
import hashlib
import json
import posixpath
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    manifest_path, output = (ROOT / args.manifest).resolve(), (ROOT / args.output).resolve()
    assert manifest_path.is_relative_to(ROOT) and output.is_relative_to(ROOT)
    assert not output.exists(), "Preserve previous audit"
    manifest = read(manifest_path)
    expected = manifest["source_sha256"] | manifest["compile_resource_inputs"]
    observed = {}
    for name, digest in expected.items():
        path = ROOT / name
        assert path.resolve().is_relative_to(ROOT) and not path.is_symlink()
        assert sha(path) == digest, name
        observed[name] = digest
    actual_inputs, dependency_files = set(), {}
    for role, record in manifest["compiler_dep_info"].items():
        path = ROOT / record["path"]
        assert sha(path) == record["sha256"]
        first = path.read_text(encoding="utf-8").splitlines()[0]
        actual = {posixpath.normpath(item.removeprefix("/work/"))
                  for item in first.split(": ", 1)[1].split()}
        assert actual == set(record["inputs"]), (role, actual ^ set(record["inputs"]))
        assert actual.issubset(expected), (role, actual - expected.keys())
        actual_inputs.update(actual)
        dependency_files[role] = {"path": record["path"], "sha256": sha(path), "inputs": sorted(actual)}
    binaries = {}
    for role, record in manifest["binaries"].items():
        path = ROOT / record["path"]
        assert path.stat().st_size == record["bytes"] and sha(path) == record["sha256"]
        with path.open("rb") as stream:
            magic = stream.read(4)
        assert magic == b"\x7fELF" if role == "linux" else magic.startswith(b"MZ")
        binaries[role] = record
    logs = {}
    for role, name in manifest["logs"].items():
        path = ROOT / name
        logs[role] = {"path": name, "sha256": sha(path)}
        if role in ("core_execution", "native_execution"):
            run = read(path)
            assert run["exit_code"] == 0
            stdout = path.parent / "stdout.log"
            summary = [line for line in stdout.read_text(encoding="utf-8").splitlines()
                       if line.startswith("test result:")]
            expected_summary = manifest["checks"]["core" if role == "core_execution" else "native"]
            assert summary == expected_summary, (role, summary)
            assert run["explicit_environment"] and run["cwd"]
            if role == "native_execution":
                assert run["binary_sha256"] == binaries["windows_native_tests"]["sha256"]
            logs[role].update(stdout_sha256=sha(stdout), summaries=summary,
                              stderr_sha256=sha(path.parent / "stderr.log"),
                              binary_sha256=run["binary_sha256"], cwd=run["cwd"],
                              explicit_environment=run["explicit_environment"])
    result = {"audit_passed": True, "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "manifest": args.manifest, "manifest_sha256": sha(manifest_path),
              "listed_input_count": len(expected), "actual_local_compiler_input_count": len(actual_inputs),
              "source_sha256": observed, "compiler_dep_info": dependency_files,
              "binaries": binaries, "logs": logs,
              "boundary": "Raw current source/resources and actual retained five compiler dep-info files are independently parsed and checked. Actual native execution logs are checked against counts and isolated launch metadata. This audit does not substitute for GUI, timing or Git commit binding; Linux core ELF retention is separate."}
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"audit_passed": True, "listed": len(expected), "actual": len(actual_inputs),
                      "dependency_roles": len(dependency_files), "binaries": len(binaries)}))


if __name__ == "__main__":
    main()
