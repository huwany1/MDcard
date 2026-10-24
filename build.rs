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
    fs::write(
        &rc,
        format!("1 ICON \"{}\"\n", icon.to_string_lossy().replace('\\', "/")),
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
