#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
# Verify normalized archive permissions on a native filesystem, rather than the
# synthetic modes exposed by a Windows bind mount. Keep this named stage for review.
stage=$(mktemp -d /tmp/mdcard-export-review.XXXXXX)
printf '%s\n' "$stage" > artifacts/export-linux-extract.txt
version=$(python3 -c 'import tomllib; from pathlib import Path; print(tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"])')
tar -xzf "${1:-dist/MDcard-$version-linux-x64.tar.gz}" -C "$stage"
renderer/runtime/node scripts/verify-export-packages.mjs "$stage"
