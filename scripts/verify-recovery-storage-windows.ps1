# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param([string]$Container='mdcard-dev',[string]$OutputDirectory="$PSScriptRoot\..\evidence\recovery\2026-10-26\storage")
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory|Out-Null
$testExe=(docker exec $Container sh -c 'find target/debug/deps -maxdepth 1 -name "mdcard-*" -type f -executable -printf "%T@ %p\n" | sort -nr | head -1').Split(' ',2)[1]
if(!$testExe){throw 'Build current Linux tests with cargo test --no-run first'}
docker exec --env MDCARD_RECOVERY_OS_TEST_DIR=/dev/shm $Container "/work/$testExe" --exact recovery::tests::real_os_disk_full_preserves_last_valid_copy --ignored --nocapture *> (Join-Path $OutputDirectory 'enospc.log')
if($LASTEXITCODE-ne0){throw 'Actual OS ENOSPC validation failed'}
docker exec --user 65534 $Container "/work/$testExe" --exact recovery::tests::real_os_permission_denied_preserves_last_valid_copy --ignored --nocapture *> (Join-Path $OutputDirectory 'permission-denied.log')
if($LASTEXITCODE-ne0){throw 'Actual unprivileged permission-denied validation failed'}
@{date=[DateTimeOffset]::Now.ToString('o');container=$Container;test_exe=$testExe;filesystem='bounded dev-container /dev/shm tmpfs, temporary isolated directory';enospc='actual OS28; last valid checkpoint retained';permission_denied='uid65534 actual OS13; last valid checkpoint retained'}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'result.json') -Encoding utf8
