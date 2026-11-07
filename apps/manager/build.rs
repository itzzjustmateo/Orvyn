use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/orvyn.ico");
    println!("cargo:rerun-if-env-changed=ORVYN_WINDRES");
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let rc = out.join("orvyn.rc");
    let icon = manifest.join("assets/orvyn.ico").display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 ICON \"{icon}\"\n")).unwrap();
    let msvc = env::var("CARGO_CFG_TARGET_ENV").unwrap() == "msvc";
    let resource = out.join(if msvc { "orvyn.res" } else { "orvyn.o" });
    let status = if msvc {
        Command::new("rc.exe").arg("/nologo").arg("/fo").arg(&resource).arg(&rc).status()
    } else {
        let compiler = env::var_os("ORVYN_WINDRES").map(PathBuf::from).unwrap_or_else(|| {
            manifest.join("../../tools/llvm-mingw-20260922-msvcrt-x86_64/bin/llvm-windres.exe")
        });
        Command::new(compiler).arg("-i").arg(&rc).arg("-o").arg(&resource).args(["-O", "coff"]).status()
    }.expect("Resource compiler could not start; initialize scripts/rust-environment.ps1 first");
    assert!(status.success(), "Icon resource compilation failed");
    println!("cargo:rustc-link-arg={}", resource.display());
}
