# SPDX-License-Identifier: AGPL-3.0-only
param([string]$Directory)
$ErrorActionPreference='Stop';$all=@()
Get-ChildItem -LiteralPath $Directory -Filter '*.ps1'|ForEach-Object{$tokens=$null;$errors=$null;[void][Management.Automation.Language.Parser]::ParseFile($_.FullName,[ref]$tokens,[ref]$errors);$all+=@($errors)}
if($all.Count){$all|Format-List|Out-String|Write-Error;exit 1}
Write-Output ('Stock PowerShell '+$PSVersionTable.PSVersion+' parsed all QA scripts successfully.')
