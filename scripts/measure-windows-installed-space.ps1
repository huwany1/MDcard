# SPDX-License-Identifier: AGPL-3.0-only
param([string]$InstalledRoot,[string]$BaselineRoot,[string]$OutputPath)
$ErrorActionPreference='Stop'
Add-Type @'
using System;using System.IO;using System.Runtime.InteropServices;using Microsoft.Win32.SafeHandles;
public class FileAllocationQA{
[StructLayout(LayoutKind.Sequential)]public struct StandardInfo{public long AllocationSize,EndOfFile;public uint Links;[MarshalAs(UnmanagedType.U1)]public bool DeletePending;[MarshalAs(UnmanagedType.U1)]public bool Directory;}
[DllImport("kernel32.dll",SetLastError=true)]static extern bool GetFileInformationByHandleEx(SafeFileHandle handle,int kind,out StandardInfo info,uint size);
public static long Allocation(string file){using(var stream=new FileStream(file,FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete)){StandardInfo info;if(!GetFileInformationByHandleEx(stream.SafeFileHandle,1,out info,(uint)Marshal.SizeOf(typeof(StandardInfo))))throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());return info.AllocationSize;}}
}
'@
$rows=@()
foreach($entry in @(@{kind='installed';root=$InstalledRoot},@{kind='portable-baseline';root=$BaselineRoot})){
 $files=@(Get-ChildItem -LiteralPath $entry.root -File -Recurse);[long]$logical=0;[long]$allocated=0
 foreach($file in $files){$allocated+=[FileAllocationQA]::Allocation($file.FullName);$logical+=$file.Length}
 $rows+=@{kind=$entry.kind;root=$entry.root;files=$files.Count;logicalFileBytes=$logical;allocatedFileBytes=$allocated;method='NTFS FILE_STANDARD_INFO.AllocationSize sum; regular files only, directory/MFT allocation and shared registry metadata excluded'}
}
$rows|ConvertTo-Json -Depth 4|Set-Content -LiteralPath $OutputPath -Encoding utf8
