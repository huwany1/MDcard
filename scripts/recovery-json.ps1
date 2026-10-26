# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
# Windows diagnostics may truncate before rewriting; never interpret an empty
# read as a valid application state. Recovery store files themselves are atomic.
function ReadJsonStable([string]$Path,[string[]]$Required=@(),[int]$TimeoutMilliseconds=4000){
 $until=[DateTime]::UtcNow.AddMilliseconds($TimeoutMilliseconds)
 do {
  try {
   $raw=[IO.File]::ReadAllText($Path)
   if(![string]::IsNullOrWhiteSpace($raw)){
    $value=$raw|ConvertFrom-Json -ErrorAction Stop
    $valid=$null-ne$value
    foreach($field in $Required){if($value.PSObject.Properties.Name-notcontains$field){$valid=$false}}
    if($valid){return $value}
   }
  }catch{}
  Start-Sleep -Milliseconds 15
 }while([DateTime]::UtcNow-lt$until)
 throw "No complete diagnostic JSON snapshot: $Path"
}
