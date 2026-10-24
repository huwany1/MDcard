// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("assets/branding/mdcard-icon/ico/mdcard-color.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-env-changed=RC");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let rc = out.join("mdcard-icon.rc");
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo product version");
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let minor = env::var("CARGO_PKG_VERSION_MINOR").unwrap();
    let patch = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    fs::write(
        &rc,
        format!(
            r#"1 ICON "{}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "MDcard\0"
   VALUE "FileVersion", "{version}.0\0"
   VALUE "InternalName", "MDcard\0"
   VALUE "OriginalFilename", "MDcard.exe\0"
   VALUE "ProductName", "MDcard\0"
   VALUE "ProductVersion", "{version}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
"#,
            icon.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("write application icon resource");
    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let object = out.join(if msvc {
        "mdcard-icon.res"
    } else {
        "mdcard-icon.o"
    });
    let compiler = env::var("RC").unwrap_or_else(|_| {
        if msvc {
            "rc.exe".into()
        } else {
            "x86_64-w64-mingw32-windres".into()
        }
    });
    let mut command = Command::new(&compiler);
    if msvc {
        command
            .arg("/nologo")
            .arg(format!("/fo{}", object.display()))
            .arg(&rc);
    } else {
        command
            .args(["--input-format=rc", "--output-format=coff"])
            .arg(&rc)
            .arg(&object);
    }
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("run resource compiler {compiler}: {error}"));
    assert!(
        status.success(),
        "application icon resource compilation failed"
    );
    println!("cargo:rustc-link-arg-bin=mdcard={}", object.display());
}
