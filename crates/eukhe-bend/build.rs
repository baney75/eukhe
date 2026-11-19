//! Compiles the compiled Bend core: `c/shim.c`, which includes the generated
//! `c/core.c` (`make bend-core`).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=c/shim.c");
    println!("cargo:rerun-if-changed=c/core.c");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let mut build = cc::Build::new();
    build.file("c/shim.c");
    if windows {
        windows_compat(&mut build);
    }
    build.include("c").std("gnu11").opt_level(2);
    // The emitted C jumps between its segments with `__attribute__((musttail))
    // return`: clang compiles it (Bend asks for clang 14+), GCC only from 15
    // (GCC 11, Ubuntu 22.04's cc, rejects it). A cc that cannot, unless CC
    // names it, gives way to `clang`. On Windows it is clang, or the one CC
    // names: cl.exe takes neither musttail nor GNU C.
    if !windows && !musttail() && !explicit_cc() {
        build.compiler("clang");
    }
    build.compile("eukhe_bend");
}

/// The Windows build: the POSIX headers core.c includes from `c/win32`
/// (ahead of the system's), clang with the C runtime Rust links, and the
/// kernel32 calls the compat makes.
fn windows_compat(build: &mut cc::Build) {
    println!("cargo:rerun-if-changed=c/win32");
    // cc passes cl.exe its CRT flag, not clang: name the one Rust links
    // (the DLL unless crt-static), or clang picks the static one.
    let static_crt = std::env::var("CARGO_CFG_TARGET_FEATURE")
        .is_ok_and(|features| features.split(',').any(|feature| feature == "crt-static"));
    let runtime = if static_crt { "static" } else { "dll" };
    // clang, or the one CC names (a clang off PATH).
    if !explicit_cc() {
        build.compiler("clang");
    }
    build
        // The compat's own check of its lazy mappings (src/ffi.rs tests).
        .file("c/win32/selftest.c")
        .include("c/win32")
        // The UCRT declares read, write and the rest without their leading
        // underscore only for this (clang leaves __STDC__ undefined there).
        .define("_CRT_DECLARE_NONSTDC_NAMES", "1")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .define("_CRT_NONSTDC_NO_DEPRECATE", None)
        .flag(format!("-fms-runtime-lib={runtime}"))
        // core.c's segments take their Env (two pointers) by value and join
        // by musttail. The Windows x64 convention passes that struct by a
        // pointer into the caller's frame, which a tail call leaves behind;
        // System V passes it in two registers, as on Linux. Its segments
        // and cold paths name their convention as preserve_none and
        // preserve_most (the attribute spellings, which macros reach), so
        // these make them sysv_abi; the shim's entries stay the platform's.
        .define("preserve_none", "sysv_abi")
        .define("preserve_most", "sysv_abi");
    println!("cargo:rustc-link-lib=kernel32");
}

/// Whether the default C compiler (the one `cc::Build::new` picks) takes a
/// `musttail` return statement.
fn musttail() -> bool {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let probe = out.join("musttail_probe.c");
    std::fs::write(
        &probe,
        "int eukhe_bend_probe_f(int x);\n\
         int eukhe_bend_probe_g(int x) { __attribute__((musttail)) return eukhe_bend_probe_f(x); }\n",
    )
    .expect("write the musttail probe");
    cc::Build::new()
        .file(&probe)
        .std("gnu11")
        .cargo_metadata(false)
        .warnings(false)
        .try_compile("eukhe_bend_musttail_probe")
        .is_ok()
}

/// Whether the environment names the C compiler (cc's own lookup keys).
fn explicit_cc() -> bool {
    let target = std::env::var("TARGET").unwrap_or_default();
    [
        "CC".to_owned(),
        "TARGET_CC".to_owned(),
        format!("CC_{target}"),
        format!("CC_{}", target.replace('-', "_")),
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_some())
}
