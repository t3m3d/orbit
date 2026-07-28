use std::{env, path::PathBuf, process::Command};

fn find_resource_compiler() -> PathBuf {
    if let Some(path) = env::var_os("RC") {
        return PathBuf::from(path);
    }
    if Command::new("where.exe")
        .arg("rc.exe")
        .output()
        .is_ok_and(|output| output.status.success())
    {
        return PathBuf::from("rc.exe");
    }
    if let Some(program_files) = env::var_os("ProgramFiles(x86)") {
        let kits_bin = PathBuf::from(program_files)
            .join("Windows Kits")
            .join("10")
            .join("bin");
        if let Ok(entries) = std::fs::read_dir(kits_bin) {
            let mut candidates = entries
                .flatten()
                .map(|entry| entry.path().join("x64").join("rc.exe"))
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            candidates.sort();
            if let Some(path) = candidates.pop() {
                return path;
            }
        }
    }
    PathBuf::from("rc.exe")
}

fn main() {
    println!("cargo:rerun-if-changed=assets/orbit-icon.ico");
    println!("cargo:rerun-if-changed=assets/orbit-icon-simple.rc");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"))
        .join("orbit-icon.res");
    let compiler = find_resource_compiler();
    let status = Command::new(&compiler)
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()))
        .arg("assets/orbit-icon-simple.rc")
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "Windows resource compiler {} could not be started: {error}",
                compiler.display()
            )
        });
    assert!(status.success(), "rc.exe failed to compile the Orbit icon");

    println!("cargo:rustc-link-arg-bin=orbit-native={}", output.display());
}
