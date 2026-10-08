use std::{env, process::Command};

fn main() {
    // Cargo must refresh provenance after a commit as well as after source edits.
    for name in ["HEAD", "index", "refs"] {
        if let Ok(output) = Command::new("git").args(["rev-parse", "--git-path", name]).output() {
            if output.status.success() { println!("cargo:rerun-if-changed={}", String::from_utf8_lossy(&output.stdout).trim()); }
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=../skate-core/src");
    println!("cargo:rerun-if-changed=../skate-data/src");
    println!("cargo:rerun-if-changed=../skate-net/src");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    for path in ["../../Cargo.toml", "Cargo.toml", "../skate-core/Cargo.toml", "../skate-data/Cargo.toml", "../skate-net/Cargo.toml", "../../vendor/bevy_pbr", "../../vendor/bevy_core_pipeline"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let git = |args: &[&str]| Command::new("git").args(args).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    let revision = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "revision-unavailable".into());
    let dirty = git(&["status", "--porcelain", "--untracked-files=normal"]).map(|s| !s.is_empty());
    let compiler = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())).arg("--version").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    println!("cargo:rustc-env=SKATE_BUILD_ID={} revision={} dirty={dirty:?} build_unix_ns={stamp} target={} profile={} dynamic={} compiler={compiler}", env::var("CARGO_PKG_VERSION").unwrap(), revision, env::var("TARGET").unwrap(), env::var("PROFILE").unwrap(), env::var_os("CARGO_FEATURE_DEV_DYNAMIC").is_some());
    // Dev builds link Bevy and std dynamically. Record where those libraries
    // live so the binary also runs outside `cargo run` (setup, launchers).
    if env::var_os("CARGO_FEATURE_DEV_DYNAMIC").is_some() {
        let libdir = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--print", "target-libdir", "--target", &env::var("TARGET").unwrap()])
            .output().ok().filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .expect("rustc --print target-libdir");
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path/deps");
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{libdir}");
    }
}
