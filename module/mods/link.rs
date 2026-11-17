// The link arguments of a module crate, shared by each crate's `build.rs` with `include!`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `DESK_LINK_BASE` (a decimal number or `0x` hex) moves the image from 0x400000, which `tools/build-modules.sh --link-base` uses for
// the packer's cross check. `entry` is the symbol the image starts at and `keep` the other entry points to keep through
// `--gc-sections`: a module with no `app_event` must not name it, because naming a missing symbol is a link error.
use std::path::Path;

fn emit_link_arguments(entry: &str, keep: &[&str]) {
    let script = Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("../module.ld")
        .canonicalize()
        .expect("module/mods/module.ld");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-env-changed=DESK_LINK_BASE");
    println!("cargo:rustc-link-arg=-T{}", script.display());
    println!("cargo:rustc-link-arg=-e{entry}");
    println!("cargo:rustc-link-arg=-u{entry}");
    for symbol in keep {
        println!("cargo:rustc-link-arg=-u{symbol}");
    }
    if let Ok(base) = std::env::var("DESK_LINK_BASE") {
        println!("cargo:rustc-link-arg=--defsym=__desk_link_base={base}");
    }
}
