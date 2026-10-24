# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (c) 2026 MDcard contributors
param(
    [string]$AppDirectory = $PSScriptRoot,
    [string]$ShortcutName = 'MDcard',
    [string]$DesktopDirectory = [Environment]::GetFolderPath('Desktop')
)
$ErrorActionPreference = 'Stop'
$AppDirectory = (Resolve-Path -LiteralPath $AppDirectory).Path
$exe = Join-Path $AppDirectory 'MDcard.exe'
if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { throw "MDcard.exe was not found beside this script: $exe" }
if ([string]::IsNullOrWhiteSpace($ShortcutName) -or $ShortcutName.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0) { throw 'ShortcutName must be a single valid file name' }
if (!(Test-Path -LiteralPath $DesktopDirectory -PathType Container)) { throw "Desktop directory not found: $DesktopDirectory" }
$shortcutPath = Join-Path $DesktopDirectory ($ShortcutName + '.lnk')
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $exe
$shortcut.WorkingDirectory = $AppDirectory
$shortcut.Arguments = ''
$shortcut.IconLocation = "$exe,0"
$shortcut.Description = 'MDcard - Markdown and text editor'
$shortcut.Save()
[Runtime.InteropServices.Marshal]::FinalReleaseComObject($shortcut) | Out-Null
[Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) | Out-Null

if (-not ('MDcardShortcutIdentity' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MDcardShortcutIdentity {
    [StructLayout(LayoutKind.Sequential)] public struct PropertyKey { public Guid format; public uint id; }
    [StructLayout(LayoutKind.Explicit, Size=24)] public struct PropVariant { [FieldOffset(0)] public ushort type; [FieldOffset(8)] public IntPtr value; }
    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPropertyStore {
        [PreserveSig] int GetCount(out uint count);
        [PreserveSig] int GetAt(uint index, out PropertyKey key);
        [PreserveSig] int GetValue(ref PropertyKey key, out PropVariant value);
        [PreserveSig] int SetValue(ref PropertyKey key, ref PropVariant value);
        [PreserveSig] int Commit();
    }
    [DllImport("shell32.dll", CharSet=CharSet.Unicode, PreserveSig=false)]
    static extern void SHGetPropertyStoreFromParsingName(string path, IntPtr context, uint flags, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IPropertyStore store);
    [DllImport("shell32.dll")] static extern void SHChangeNotify(uint change, uint flags, IntPtr item1, IntPtr item2);
    public static void Set(string path) {
        Guid iid=new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99");
        IPropertyStore store;
        SHGetPropertyStoreFromParsingName(path,IntPtr.Zero,2,ref iid,out store);
        var key=new PropertyKey {format=new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"),id=5};
        var value=new PropVariant {type=31,value=Marshal.StringToCoTaskMemUni("MDcard.Desktop")};
        try { Marshal.ThrowExceptionForHR(store.SetValue(ref key,ref value)); Marshal.ThrowExceptionForHR(store.Commit()); }
        finally { Marshal.FreeCoTaskMem(value.value); Marshal.FinalReleaseComObject(store); }
        IntPtr name=Marshal.StringToCoTaskMemUni(path);
        try { SHChangeNotify(0x00002000,0x0005,name,IntPtr.Zero); }
        finally { Marshal.FreeCoTaskMem(name); }
    }
}
'@
}
[MDcardShortcutIdentity]::Set($shortcutPath)
[PSCustomObject]@{
    Shortcut = $shortcutPath
    Target = $exe
    WorkingDirectory = $AppDirectory
    IconLocation = "$exe,0"
    AppUserModelID = 'MDcard.Desktop'
    BinarySHA256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
} | ConvertTo-Json
