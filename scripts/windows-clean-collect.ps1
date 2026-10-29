# SPDX-License-Identifier: AGPL-3.0-only
$ErrorActionPreference='Stop';$out=Join-Path $env:USERPROFILE 'MDcard-clean-QA-r2'
$runtime=Get-Content -Raw -Encoding UTF8 (Join-Path $out 'engines-only-result.json')|ConvertFrom-Json
$document=Join-Path $out 'manual.md';$text=[IO.File]::ReadAllText($document,[Text.Encoding]::UTF8)
$ui=Get-Content -Raw -Encoding UTF8 (Join-Path $out 'manual-ui.json')|ConvertFrom-Json
$basic=@{actualKeyboardEditAndCtrlS=($text.Contains('edited offline')-and$text.Contains('saved locally')-and$text-ne'Initial.');savedText=$text;document=$document;documentSHA256=(Get-FileHash -LiteralPath $document).Hash;dirty=$ui.dirty;mode=$ui.mode;documentBytes=$ui.document_text_bytes;reportDocument=$ui.document_path;method='Ordinary QEMU keyboard input: Ctrl+E to Write, text input, composition commit, Ctrl+S; screenshots 44/45/46';newOpenSaveAs='Full native New/Open/SaveAs separately passed on actual installed same 933f binary on host; guest complex selector attempts retained as failed QA';nativeSHA256=$runtime.nativeSHA256}
$basic|ConvertTo-Json -Depth 8|Set-Content (Join-Path $out 'manual-native-result.json') -Encoding UTF8
$result=@{passed=($runtime.runtimePassed-and$basic.actualKeyboardEditAndCtrlS-and-not$ui.dirty-and$ui.mode-eq'Write');runtime=$runtime;nativeBasic=$basic;network='No virtual NIC; QEMU host arguments recorded';qaOnly=$true;completedUTC=[DateTime]::UtcNow.ToString('o')}
if(-not$result.passed){throw 'Actual disk/UI or runtime acceptance did not pass; preserve raw evidence'}
$result|ConvertTo-Json -Depth 18|Set-Content (Join-Path $out 'result.json') -Encoding UTF8
$zip=Join-Path $env:TEMP 'MDcard-clean-QA-r2.zip';Compress-Archive -Path (Join-Path $out '*') -DestinationPath $zip -Force
$serial=New-Object IO.Ports.SerialPort COM1,115200,None,8,one;$serial.Open()
$serial.WriteLine('MDCARD_QA_RESULT '+[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($result|ConvertTo-Json -Depth 18 -Compress))))
$serial.WriteLine('MDCARD_QA_ZIP_BEGIN');$encoded=[Convert]::ToBase64String([IO.File]::ReadAllBytes($zip));for($i=0;$i-lt$encoded.Length;$i+=12000){$serial.WriteLine($encoded.Substring($i,[Math]::Min(12000,$encoded.Length-$i)))};$serial.WriteLine('MDCARD_QA_ZIP_END');$serial.Close()
Write-Output 'Collected independent offline runtime + native basic acceptance. No automatic shutdown.'
