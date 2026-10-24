# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Collect unmodified dependency license files from the locked Cargo registry."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=root))
parts = ["MDcard third-party Rust dependency notices\nGenerated from Cargo.lock. Fonts have their own accompanying OFL files.\n"]
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["source"] is None:
        continue
    folder = pathlib.Path(package["manifest_path"]).parent
    parts.append("\n" + "=" * 72 + f"\n{package['name']} {package['version']}\nLicense: {package.get('license') or 'see license file'}\nSource: {package.get('repository') or package['source']}\n")
    files = sorted({p for p in folder.iterdir() if p.is_file() and p.name.lower().startswith(("license", "licence", "copying", "copyright", "notice"))})
    if package.get("license_file"):
        candidate = folder / package["license_file"]
        if candidate.is_file() and candidate not in files:
            files.append(candidate)
    for path in files:
        parts.append(f"\n--- {path.name} ---\n" + path.read_text(encoding="utf-8", errors="replace") + "\n")
(root / "THIRD_PARTY_NOTICES.txt").write_text("".join(parts), encoding="utf-8")
