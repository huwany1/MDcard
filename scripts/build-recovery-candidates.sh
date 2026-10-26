#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Run with exclusive ownership of the Cargo output while this script is active.
set -eu
cd "$(dirname "$0")/.."
out=${1:?Pass a NEW candidate directory, e.g. artifacts/recovery-2026-10-26/final-r1}
if [ -e "$out" ]; then
    echo "Refusing to overwrite existing candidate directory: $out" >&2
    exit 2
fi
mkdir -p "$out"
cargo build --locked --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/mdcard.exe "$out/mdcard.exe"
cargo build --locked --release --target x86_64-pc-windows-gnu --features acceptance-faults
cp target/x86_64-pc-windows-gnu/release/mdcard.exe "$out/mdcard-faults.exe"
python3 scripts/recovery-source-identity.py --candidate "$out"
