# SPDX-License-Identifier: AGPL-3.0-only
# Existing Microsoft Pinyin only. Never installs/changes the user's language list.
param([string]$Exe,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
. "$PSScriptRoot\verify-ui-gpu-windows.ps1" -Exe $Exe -FunctionsOnly
. "$PSScriptRoot\recovery-json.ps1"
$output=[IO.Path]::GetFullPath($OutputDirectory);New-Item -ItemType Directory -Force $output|Out-Null
$settings=Join-Path $output 'settings.json';$fixtureJson=@{language='en';reduce_motion=$true;font_size=15}|ConvertTo-Json;[IO.File]::WriteAllText($settings,$fixtureJson,[Text.UTF8Encoding]::new($false))
$env:MDCARD_UI_REPORT=Join-Path $output 'ui.json';$env:MDCARD_RECOVERY_DIR=Join-Path $output 'recovery';$env:MDCARD_READING_STATE_PATH=Join-Path $output 'reading.json';$env:MDCARD_QA_CLEAN_ENV='1'
Add-Type @'
using System;using System.Runtime.InteropServices;
public class MDcardIme {
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr LoadKeyboardLayout(string name,uint flags);
 [DllImport("user32.dll")] public static extern IntPtr GetKeyboardLayout(uint thread);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint msg,UIntPtr w,IntPtr l);
 [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr h);
 [DllImport("imm32.dll")] public static extern bool ImmGetOpenStatus(IntPtr c);
 [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr c,bool open);
 [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr h,IntPtr c);
 [DllImport("user32.dll")] public static extern void keybd_event(byte key,byte scan,uint flags,UIntPtr extra);
}
'@
$fixture=Join-Path $output 'IME 中文.md';[IO.File]::WriteAllText($fixture,"# Pinyin IME`n`n",[Text.UTF8Encoding]::new($false));$originalLayout=[IntPtr]::Zero
try {
 Launch 'renderer' '' $fixture
 $c=(ReadJsonStable $env:MDCARD_UI_REPORT @('controls')).controls|Where-Object name -eq 'Write'|Select-Object -First 1;Click (($c.rect[0]+$c.rect[2])/2) (($c.rect[1]+$c.rect[3])/2)
 $c=(ReadJsonStable $env:MDCARD_UI_REPORT @('controls')).controls|Where-Object name -eq 'main-source-editor'|Select-Object -First 1;Click ($c.rect[0]+50) ($c.rect[1]+14);Keys '^{END}'
 $thread=[MdcardUi]::GetWindowThreadProcessId($script:app.MainWindowHandle,[IntPtr]::Zero);$originalLayout=[MDcardIme]::GetKeyboardLayout($thread)
 $pinyin=[MDcardIme]::LoadKeyboardLayout('00000804',0x80);[void][MDcardIme]::PostMessage($script:app.MainWindowHandle,0x50,[UIntPtr]::Zero,$pinyin);Start-Sleep -Milliseconds 500
 $context=[MDcardIme]::ImmGetContext($script:app.MainWindowHandle);$oldOpen=[MDcardIme]::ImmGetOpenStatus($context);[void][MDcardIme]::ImmSetOpenStatus($context,$true);[void][MDcardIme]::ImmReleaseContext($script:app.MainWindowHandle,$context)
 $c=(ReadJsonStable $env:MDCARD_UI_REPORT @('controls')).controls|Where-Object name -eq 'main-source-editor'|Select-Object -First 1;Click ($c.rect[0]+50) ($c.rect[1]+14);Keys '^{END}';Start-Sleep -Milliseconds 500
 foreach($vk in @(0x4e,0x49,0x48,0x41,0x4f)){[MDcardIme]::keybd_event([byte]$vk,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 60;[MDcardIme]::keybd_event([byte]$vk,0,2,[UIntPtr]::Zero);Start-Sleep -Milliseconds 120}
 Start-Sleep -Milliseconds 500;$preedit=ReadJsonStable $env:MDCARD_UI_REPORT @('autosave','document_text_bytes');Shot '01-real-pinyin-preedit'
 [MDcardIme]::keybd_event(0x20,0,0,[UIntPtr]::Zero);Start-Sleep -Milliseconds 80;[MDcardIme]::keybd_event(0x20,0,2,[UIntPtr]::Zero);Start-Sleep -Milliseconds 500;Keys '^s';Start-Sleep -Milliseconds 500;$text=[IO.File]::ReadAllText($fixture);Shot '02-pinyin-committed'
 $result=@{binary=(Resolve-Path $Exe).Path;sha256=(Get-FileHash $Exe).Hash;input='Win32 keybd_event virtual-key N I H A O followed by Space; no Unicode paste or synthesized egui IME events';layoutBefore=$originalLayout.ToInt64();layoutRequested=$pinyin.ToInt64();layoutAfter=[MDcardIme]::GetKeyboardLayout($thread).ToInt64();preeditActive=$preedit.autosave.ime;actualText=$text;containsNiHao=$text.Contains('你好');passed=($preedit.autosave.ime-and$text.Contains('你好'));provider='Existing Microsoft Pinyin; installed OS language list unchanged';immContext=$context.ToInt64();oldOpen=$oldOpen}
 $result|ConvertTo-Json -Depth 5|Set-Content (Join-Path $output 'result.json') -Encoding utf8
 if($originalLayout-ne[IntPtr]::Zero){[void][MDcardIme]::PostMessage($script:app.MainWindowHandle,0x50,[UIntPtr]::Zero,$originalLayout)}
 Keys '^q';$script:app.WaitForExit(5000)|Out-Null
 if(!$result.passed){throw 'Real Pinyin preedit/你好 commit not confirmed; see result and screenshots'}
} finally {
 if($script:app-and!$script:app.HasExited){if($originalLayout-ne[IntPtr]::Zero){[void][MDcardIme]::PostMessage($script:app.MainWindowHandle,0x50,[UIntPtr]::Zero,$originalLayout)};$script:app.Kill();$script:app.WaitForExit()}
}
