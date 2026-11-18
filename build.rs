// SPDX-License-Identifier: MIT
use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=src/native.c");
    println!("cargo:rerun-if-changed=src/native.h");
    println!("cargo:rerun-if-changed=vendor/poulsback");
    println!("cargo:rerun-if-changed=tools/build-native.sh");
    println!("cargo:rerun-if-changed=tools/native-sources.txt");
    for name in ["CC", "AR", "CFLAGS", "PKG_CONFIG", "PKG_CONFIG_LIBDIR"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }
    let target = env::var("TARGET").expect("Cargo target");
    if !target.contains("linux") || !(target.starts_with("i686-") || target.starts_with("x86_64-"))
    {
        panic!("native Poulsback presentation requires x86 Linux");
    }
    let out = env::var("OUT_DIR").expect("Cargo output directory");
    let mut command = Command::new("sh");
    command.arg("tools/build-native.sh").arg(&out);
    if target.starts_with("i686-") {
        command.env(
            "CFLAGS",
            format!("{} -m32", env::var("CFLAGS").unwrap_or_default()),
        );
    }
    if !command.status().expect("start native compiler").success() {
        panic!("native Poulsback compilation failed");
    }
    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=static=facet_native");
    println!("cargo:rustc-link-lib=drm");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-arg=-flto=auto");
    // The native archive uses GCC LTO, which rust-lld cannot read.
    println!("cargo:rustc-link-arg=-fuse-ld=bfd");
}
