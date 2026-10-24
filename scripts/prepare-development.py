# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Prepare only the pinned PlantUML jar and optional host browser for development."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("--browser", action="store_true", help="download Playwright Chromium")
args = parser.parse_args()
record = json.loads((root / "renderer/runtime-sources.json").read_text("utf8"))["plantuml"]
target = root / "renderer/vendor/plantuml.jar"
target.parent.mkdir(parents=True, exist_ok=True)
if not target.exists() or hashlib.sha256(target.read_bytes()).hexdigest() != record["sha256"]:
    fd, name = tempfile.mkstemp(dir=target.parent, prefix="plantuml-", suffix=".partial")
    os.close(fd)
    temporary = Path(name)
    try:
        with urllib.request.urlopen(record["url"], timeout=180) as response, temporary.open("wb") as out:
            while block := response.read(1024 * 1024):
                out.write(block)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != record["sha256"]:
            raise RuntimeError("PlantUML SHA256 mismatch; original target retained")
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)
print("Verified pinned PlantUML MIT jar:", record["sha256"])
if args.browser:
    env = dict(os.environ)
    env["PLAYWRIGHT_BROWSERS_PATH"] = str(root / "renderer/runtime/browsers")
    subprocess.run(
        ["npx.cmd" if os.name == "nt" else "npx", "--no-install", "playwright", "install", "chromium"],
        cwd=root / "renderer", env=env, check=True,
    )
print("Keep system Node and Java 17+ available; no private install paths are required.")
