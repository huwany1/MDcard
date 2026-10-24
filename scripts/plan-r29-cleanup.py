# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Plan only: preserve source and verify archived copies before proposing deletion."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "evidence/release-r29-2026-10-24"


def read(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def linked(info):
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info, "st_file_attributes", 0) & 0x400)


def measure(path):
    files = size = links = 0
    pending = [path]
    while pending:
        current = pending.pop()
        info = current.lstat()
        if linked(info):
            links += 1
        elif stat.S_ISDIR(info.st_mode):
            pending.extend(current.iterdir())
        else:
            files += 1
            size += info.st_size
    return dict(files=files, bytes=size, links=links)


def main():
    assert not (OUT / "cleanup-plan.json").exists()
    promotion = read(OUT / "promotion.json")
    assert promotion["passed"] and sha(ROOT / "dist/latest.json") == promotion["latest_sha256"]
    tracked = set(subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0"))
    closing = read(ROOT / "artifacts/reading-precision/delivery/DELIVERY_SHA256.json")
    sidecar = read(ROOT / closing["evidence_sidecar"]["path"])
    assert sidecar["every_member_verified"]
    assert sidecar["archive_sha256"] == closing["evidence_archive"]["sha256"]
    # The complete archive itself was freshly hashed by archive-preflight.json.
    preflight = read(OUT / "archive-preflight.json")
    assert preflight["passed"] and any(r["sha256"] == sidecar["archive_sha256"] for r in preflight["files"])
    entries, retained, whole = [], [], []

    def eligible(name):
        path = ROOT / name
        assert not Path(name).is_absolute() and ".." not in Path(name).parts
        assert path.absolute().is_relative_to(ROOT)
        assert not any(t == name or t.startswith(name + "/") for t in tracked)
        return path

    def add_tree(path, reason, category):
        name = path.relative_to(ROOT).as_posix()
        eligible(name)
        assert not linked(path.lstat()), name
        entries.append(dict(path=name, absolute=str(path), kind="tree" if path.is_dir() else "file",
                            category=category, reason=reason, **measure(path)))
        whole.append(name)

    for path in sorted(ROOT.glob("target-*")):
        if path.is_dir():
            add_tree(path, "可由锁文件和工具链重新生成的本项目编译缓存", "build_cache")
    for path in sorted((ROOT / "tests").glob("*/target")):
        add_tree(path, "独立测试的可再生Cargo缓存；已执行测试ELF保存在证据归档", "build_cache")
    for path in [ROOT / "scripts/__pycache__", ROOT / "dist/.before-r29-promotion-20261024",
                 ROOT / "dist/reading-state-final-20261023", ROOT / "artifacts/reading-precision/packages",
                 ROOT / "artifacts/reading-state/packages", ROOT / "artifacts/reading-precision/extracted-r29-windows"]:
        if path.exists():
            add_tree(path, "R29正式目录及完整包已验证；旧运行目录、旧包、重复展开或Python缓存已无当前用途", "superseded_output")
    accepted_archives = {Path(record["archive"]).name for record in promotion["packages"]}
    for path in sorted((ROOT / "dist").glob("MDcard-*")):
        if path.is_file() and path.name not in accepted_archives:
            add_tree(path, "已被正式R29双平台完整包替代；历史身份/验收仍保留，不承诺旧包位级重建", "superseded_archive")
            entries[-1]["sha256_before_delete"] = sha(path)

    def in_whole(name):
        return any(name == value or name.startswith(value + "/") for value in whole)

    protected = {"artifacts/reading-precision/stage-b-r29-source.json",
                 "artifacts/reading-precision/master/final-0bea1e9-source.zip",
                 "artifacts/reading-precision/master/final-0bea1e9-from-264d42d.patch"}
    for record in sidecar["files"]:
        name = record["path"]
        if not name.startswith(("artifacts/", "evidence/")) or name in tracked or in_whole(name):
            continue
        if name in protected or name.startswith("evidence/reading-precision/master/b-r29-") or name.startswith("artifacts/reading-precision/delivery/"):
            retained.append(dict(path=name, reason="保留当前验收摘要、源码快照或正式证据归档，便于直接审计"))
            continue
        path = ROOT / name
        if not path.exists():
            continue
        eligible(name)
        info = path.lstat()
        assert not linked(info) and stat.S_ISREG(info.st_mode), name
        if info.st_size != record["bytes"] or sha(path) != record["sha256"]:
            retained.append(dict(path=name, reason="归档后内容变化，保留当前文件，不用旧归档替代"))
            continue
        entries.append(dict(path=name, absolute=str(path), kind="file", category="verified_archive_copy",
                            files=1, bytes=info.st_size, links=0, sha256=record["sha256"],
                            mtime_ns=info.st_mtime_ns,
                            reason="完整证据ZIP已有同路径/同长度/同SHA256原件；删除重复展开，原始失败仍在ZIP中"))

    metadata = ROOT / "artifacts/reading-precision/archive-metadata-r29"
    excluded = read(metadata / "excluded.json")
    for record in excluded:
        name = record["path"]
        expected = record.get("sha256_from_existing_build_metadata")
        if not expected or in_whole(name) or name in protected:
            continue
        path = ROOT / name
        if path.is_file() and not linked(path.lstat()) and sha(path) == expected:
            eligible(name)
            entries.append(dict(path=name, absolute=str(path), kind="file", category="obsolete_binary",
                                files=1, bytes=path.stat().st_size, links=0, sha256=expected,
                                reason="旧应用/测试二进制，现有正式包或已归档测试取代；历史SHA和源码记录保留"))
    for record in read(metadata / "links.json"):
        name = record["path"]
        path = ROOT / name
        if in_whole(name):
            continue
        try:
            info = path.lstat()
        except FileNotFoundError:
            continue
        if not linked(info):
            continue
        eligible(name)
        assert linked(path.lstat())
        entries.append(dict(path=name, absolute=str(path), kind="link", category="archived_link_metadata",
                            files=0, bytes=0, links=1, reason="链接目标元数据已归档；只删除链接本体，不遍历运行时目标"))
    names = [entry["path"] for entry in entries]
    assert len(names) == len(set(names))
    result = dict(created_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), workspace=str(ROOT),
                  promotion_sha256=sha(OUT / "promotion.json"),
                  evidence_archive=closing["evidence_archive"], plan_only=True, no_follow=True,
                  targets=entries, retained_exceptions=retained,
                  files=sum(e["files"] for e in entries), bytes=sum(e["bytes"] for e in entries),
                  boundary="Explicit workspace-only targets. Source/docs/Git/dev dependencies/user data excluded; changed or unarchived evidence retained. Directory moves and deletes are performed separately by native PowerShell after absolute path and release revalidation. Logical bytes are not promised physical free-space gain.")
    (OUT / "cleanup-plan.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"targets": len(entries), "files": result["files"], "bytes": result["bytes"], "retained_exceptions": len(retained)}))


if __name__ == "__main__":
    main()
