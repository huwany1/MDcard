# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
"""Run current source checks from any directory; no candidate packages required."""
import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--desktop", action="store_true", help="fmt, Clippy, tests, release")
parser.add_argument("--acceptance", action="store_true", help="all independent Rust test crates")
parser.add_argument("--renderer", action="store_true", help="build and real engine/process tests; install dependencies first")
parser.add_argument("--website", action="store_true", help="independent static website build")
args = parser.parse_args()
if not any(vars(args).values()):
    parser.error("select --desktop, --acceptance, --renderer and/or --website")


def run(command, directory=ROOT, env=None):
    print("+ " + " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=directory, env=env, check=True)


if args.desktop:
    for command in (
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"],
        ["cargo", "test", "--locked"],
        ["cargo", "build", "--locked", "--release"],
    ):
        run(command)
if args.acceptance:
    for manifest in sorted((ROOT / "tests").glob("*/Cargo.toml")):
        run(["cargo", "test", "--locked", "--manifest-path", str(manifest)])
if args.renderer:
    env = dict(os.environ)
    env.setdefault("PLAYWRIGHT_BROWSERS_PATH", str(ROOT / "renderer/runtime/browsers"))
    npm = "npm.cmd" if os.name == "nt" else "npm"
    for command in ([npm, "run", "build"], [npm, "test"],
                    ["node", "--test", "export-process.test.mjs"],
                    ["node", "--test", "entrypoint.test.mjs"]):
        run(command, ROOT / "renderer", env)
if args.website:
    run(["node", "website/scripts/build.mjs"])
