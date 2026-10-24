#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
set -eu
cd /work
# Windows bind mounts expose broad synthetic modes. Normalize in a native fs.
staging=$(mktemp -d /tmp/mdcard-package.XXXXXX)
trap 'case "$staging" in /tmp/mdcard-package.*) rm -rf "$staging" ;; esac' EXIT
cp -R dist/linux/. "$staging/"
find "$staging" -type d -exec chmod 755 {} +
find "$staging" -type f -exec chmod 644 {} +
chmod 755 "$staging/mdcard"
tar -czf /work/dist/MDcard-0.1.0-linux-x64.tar.gz -C "$staging" .
tar -tvzf /work/dist/MDcard-0.1.0-linux-x64.tar.gz >"${MDCARD_PACKAGE_CONTENTS:-evidence/linux/package-contents.txt}"
