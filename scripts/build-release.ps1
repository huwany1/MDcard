# SPDX-License-Identifier: AGPL-3.0-only
# Windows build host: Python 3.11+, Node 24.21.0/npm, Docker Desktop Linux engine, Git.
# Does not install the product, alter the portable App, or publish anything.
param([string]$OutputDirectory='dist\0.1.5',[string]$BuildImage='mdcard-release-build:1.92')
$ErrorActionPreference='Stop'
$repoRoot=Split-Path $PSScriptRoot
Set-Location -LiteralPath $repoRoot
function Run([string]$Program,[string[]]$Arguments) { & $Program @Arguments; if($LASTEXITCODE-ne 0){throw "$Program failed ($LASTEXITCODE)"} }
$version=(Get-Content -Raw renderer\package.json|ConvertFrom-Json).version
if($version-ne'0.1.5'){throw 'This packaging definition requires product version 0.1.5'}
$productCommit=(& git rev-parse HEAD).Trim()
if((& git diff --name-only HEAD -- Cargo.toml Cargo.lock build.rs src assets locales vendor renderer)){throw 'Freeze product input changes in Git before building'}
$stageRoot=Join-Path $repoRoot ('artifacts\release-'+$version+'-'+(Get-Date -Format yyyyMMdd-HHmmss))
New-Item -ItemType Directory -Path $stageRoot -Force|Out-Null
$out=[IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
New-Item -ItemType Directory -Path $out -Force|Out-Null
Run docker @('build','-f','packaging/Build.Dockerfile','-t',$BuildImage,'.')
Run npm @('ci','--ignore-scripts','--no-audit','--no-fund','--prefix','renderer')
New-Item -ItemType Directory -Force .local\renderer-linux|Out-Null
Copy-Item renderer\package.json,renderer\package-lock.json .local\renderer-linux
Run npm @('ci','--ignore-scripts','--no-audit','--no-fund','--os=linux','--cpu=x64','--libc=glibc','--prefix','.local/renderer-linux')
Run npm @('ci','--ignore-scripts','--no-audit','--no-fund','--prefix','scripts/release-tools')
Run python @('scripts/prepare-export-runtime.py','--platform','all')
$env:MDCARD_JAVA=Join-Path $repoRoot 'renderer\runtime\windows\java\bin\java.exe'
Run python @('scripts/prepare-rich-runtime.py','--reuse-npm')
Remove-Item Env:\MDCARD_JAVA
Run python @('scripts/prepare-release-jre.py')
$relativeStage=[IO.Path]::GetRelativePath($repoRoot,$stageRoot).Replace('\','/')
$buildShell='set -eu; cargo build --locked --release --target x86_64-pc-windows-gnu; cp /target/x86_64-pc-windows-gnu/release/mdcard.exe '+$relativeStage+'/MDcard.exe; cargo build --locked --release; cp /target/release/mdcard '+$relativeStage+'/mdcard; cargo test --locked; rustc -Vv; dpkg-query -W'
Run docker @('run','--rm','-v',"${repoRoot}:/work",'-v','mdcard-release-cargo:/usr/local/cargo','-v','mdcard-release-target:/target','-e','CARGO_TARGET_DIR=/target','-e','CARGO_BUILD_JOBS=2',$BuildImage,'bash','-c',$buildShell)
$trace=Join-Path $stageRoot 'dependency-trace.json'
Run node @('scripts/trace-release-runtime.cjs','renderer',$trace)
$win=Join-Path $stageRoot 'windows';$linux=Join-Path $stageRoot 'linux'
Run python @('scripts/stage-release.py','--platform','windows','--output',$win,'--node-modules','renderer/node_modules','--java','artifacts/release-inputs/minimal-windows','--trace',$trace,'--binary',(Join-Path $stageRoot 'MDcard.exe'),'--source-commit',$productCommit)
$linuxShell='set -eu; python3 scripts/stage-release.py --platform linux --output '+$relativeStage+'/linux --node-modules .local/renderer-linux/node_modules --java artifacts/release-inputs/minimal-linux --trace '+$relativeStage+'/dependency-trace.json --binary '+$relativeStage+'/mdcard --source-commit '+$productCommit+'; python3 scripts/package-deb.py '+$relativeStage+'/linux '+([IO.Path]::GetRelativePath($repoRoot,$out).Replace('\','/'))
Run docker @('run','--rm','-v',"${repoRoot}:/work",$BuildImage,'bash','-c',$linuxShell)
$inputs=Get-Content -Raw packaging\build-inputs.json|ConvertFrom-Json
$compiler=Join-Path $repoRoot '.local\tools\inno\ISCC.exe'
if(!(Test-Path -LiteralPath $compiler)) {
 $installer=Join-Path $stageRoot 'innosetup.exe'
 Invoke-WebRequest -Uri $inputs.innoSetup.url -OutFile $installer
 if((Get-FileHash $installer).Hash.ToLowerInvariant()-ne$inputs.innoSetup.sha256){throw 'Inno Setup SHA256 mismatch'}
 $signature=Get-AuthenticodeSignature $installer
 if($signature.Status-ne'Valid'-or$signature.SignerCertificate.Subject-notmatch'Pyrsys B.V.'){throw 'Inno Setup publisher verification failed'}
 $tool=Start-Process -FilePath $installer -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/CURRENTUSER',('/DIR="'+(Split-Path $compiler)+'"')) -WindowStyle Hidden -Wait -PassThru
 if($tool.ExitCode-ne 0){throw 'Inno Setup compiler installation failed'}
}
Run $compiler @("/DPayloadRoot=$win","/DReleaseOutput=$out",'packaging/windows/mdcard.iss')
Get-ChildItem -LiteralPath $out -File|Where-Object Extension -in '.exe','.deb'|ForEach-Object { (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()+'  '+$_.Name }|Set-Content -LiteralPath (Join-Path $out 'SHA256SUMS.txt') -Encoding utf8
Write-Host "Built local candidates in $out. Installation and functional acceptance are separate mandatory steps."
