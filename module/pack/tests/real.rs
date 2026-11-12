// Reads modules that `rust-lld` really linked: a small no_std module is built from source for the kernel's target with the module
// linker script, and the reader must give what the source says.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The test needs `rustc` with the library source (`rust-src`, for `-Zbuild-std`) and the `rust-lld` that ships with the compiler. When
// either is missing it prints why and passes, so a host without them still runs the rest; the CI image has both, and a run there sets
// `DMD_REQUIRE_REAL_MODULE=1`, which turns "missing" into a failure so the test cannot be skipped by accident:
//
//     DMD_REQUIRE_REAL_MODULE=1 cargo test --locked --test real
//
// Nothing binary is checked in: the sources are `tests/real/realmod.rs` and `tests/real/module.ld`, and the build happens under
// `CARGO_TARGET_TMPDIR`.
use std::path::{Path, PathBuf};
use std::process::Command;

use dmdpack::{read, ElfError, Extract, KIND_APP, KIND_MODULE, LINK_BASE};

fn here(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

/// `Some(reason)` when the compiler cannot build for the kernel's target here.
fn unavailable() -> Option<String> {
    let out = match Command::new("rustc").args(["--print", "sysroot"]).output() {
        Ok(out) if out.status.success() => out,
        _ => return Some("no rustc on the path".into()),
    };
    let sysroot = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    if !sysroot.join("lib/rustlib/src/rust/library/core").is_dir() {
        return Some(format!("no rust-src in {} (needed for -Zbuild-std)", sysroot.display()));
    }
    let has_lld = std::fs::read_dir(sysroot.join("lib/rustlib"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|d| d.path().join("bin/rust-lld").is_file());
    if !has_lld {
        return Some(format!("no rust-lld in {}", sysroot.display()));
    }
    None
}

/// Builds the module (with its optional entry points unless `bare`) and returns the linked ELF file.
fn build(bare: bool) -> Vec<u8> {
    build_with(bare, false)
}

/// As `build`; with `merged`, the import slots are put in the `.data` output section instead of one of their own.
fn build_with(bare: bool, merged: bool) -> Vec<u8> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(match (bare, merged) {
        (false, false) => "realmod",
        (true, _) => "realmod-bare",
        (false, true) => "realmod-merged",
    });
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::copy(here("tests/real/realmod.rs"), dir.join("src/main.rs")).unwrap();
    let mut script = std::fs::read_to_string(here("tests/real/module.ld")).unwrap();
    if bare {
        script = script.replace("EXTERN(app_event)\n", "").replace("EXTERN(app_stop)\n", "");
    }
    if merged {
        let own = ".desk.imp : ALIGN(4) { KEEP(*(.desk.imp)) }\n    .data : ALIGN(4) { *(.data .data.*) }";
        assert!(script.contains(own));
        script = script.replace(own, ".data : ALIGN(4) { KEEP(*(.desk.imp)) *(.data .data.*) }");
    }
    std::fs::write(dir.join("module.ld"), script).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"realmod\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[features]\nbare = []\n\n[workspace]\n\n\
         [profile.release]\npanic = \"abort\"\nopt-level = \"s\"\n",
    )
    .unwrap();
    let target_json = here("../../kernel/i486-desk-none.json");
    let flags = format!(
        "build.rustflags=['-Clink-arg=-T{}','-Clink-arg=-nostdlib','-Clink-arg=--emit-relocs','-Clink-arg=-zmax-page-size=0x1000']",
        dir.join("module.ld").display()
    );
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&dir)
        .env("RUSTC_BOOTSTRAP", "1")
        .env("CARGO_TARGET_DIR", dir.join("target"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args(["build", "--release", "--target"])
        .arg(&target_json)
        .args([
            "-Zjson-target-spec",
            "-Zbuild-std=core,compiler_builtins",
            "-Zbuild-std-features=compiler-builtins-mem",
        ])
        .args(["--config", &flags]);
    if bare {
        cmd.args(["--features", "bare"]);
    }
    let out = cmd.output().expect("cargo did not start");
    assert!(
        out.status.success(),
        "building the real module failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read(dir.join("target/i486-desk-none/release/realmod")).unwrap()
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn check_common(elf: &[u8], x: &Extract) {
    assert_eq!(x.link_base, LINK_BASE);
    // The entry point of the file is the start the reader found.
    assert_eq!(word(elf, 24), LINK_BASE + x.start);
    assert!(x.start < x.ro_size);
    assert!(x.ro_size as usize <= x.image.len());
    assert!(x.image.len() as u32 <= x.mem_size);
    // The two import slots are in the writable part, after the read-only part, and are zero in the file.
    let mut names: Vec<&str> = x.imports.iter().map(|(n, _)| n.as_str()).collect();
    names.sort();
    assert_eq!(names, ["desk_alloc", "desk_log"]);
    let mut slots: Vec<u32> = x.imports.iter().map(|(_, s)| *s).collect();
    slots.sort();
    assert_eq!(slots[1], slots[0] + 4);
    assert!(slots[0] >= x.ro_size);
    assert_eq!(slots[0] % 0x1000, 0, "the script starts the import section on a page");
    for s in &slots {
        assert_eq!(word(&x.image, *s as usize), 0);
    }
    // Relocations: ascending, four apart, and each word points into the image. The module's code loads and calls through the slot of
    // `desk_log`, so a relocated word must hold that slot's address, and the table of string pointers is in the read-only data.
    assert!(x.relocs.len() >= 4, "{} relocations", x.relocs.len());
    assert!(x.relocs.windows(2).all(|w| w[1] - w[0] >= 4));
    let values: Vec<u32> = x.relocs.iter().map(|&r| word(&x.image, r as usize)).collect();
    let log_slot = x.imports.iter().find(|(n, _)| n == "desk_log").unwrap().1;
    assert!(
        values.contains(&(LINK_BASE + log_slot)),
        "no relocated word points at the desk_log slot"
    );
    assert!(
        x.relocs
            .iter()
            .zip(&values)
            .any(|(&r, &v)| r >= x.start && r < x.ro_size && v - LINK_BASE < x.ro_size),
        "no relocated pointer into the read-only data"
    );
    for v in &values {
        assert!(*v >= LINK_BASE && *v <= LINK_BASE + x.mem_size);
    }
}

#[test]
fn a_module_linked_by_rust_lld_is_read() {
    if let Some(why) = unavailable() {
        assert!(
            std::env::var_os("DMD_REQUIRE_REAL_MODULE").is_none(),
            "DMD_REQUIRE_REAL_MODULE is set but {why}"
        );
        eprintln!("skipped: {why}");
        return;
    }
    let elf = build(false);
    let x = read(&elf, LINK_BASE, KIND_MODULE).unwrap_or_else(|e| panic!("the real module was refused: {e}"));
    check_common(&elf, &x);
    // The optional entry points survive linking and are inside the read-only part, apart from each other and from `app_start`.
    let (event, stop) = (
        x.event.expect("app_event was dropped by the linker"),
        x.stop.expect("app_stop was dropped by the linker"),
    );
    assert!(event < x.ro_size && stop < x.ro_size);
    let mut entries = [x.start, event, stop];
    entries.sort();
    assert!(entries[0] != entries[1] && entries[1] != entries[2]);
    // Read as an application, a file that defines `app_event` is refused.
    assert_eq!(
        read(&elf, LINK_BASE, KIND_APP),
        Err(ElfError::ApplicationHasModuleEntry { name: "app_event" })
    );
    // At a link base the file was not linked at.
    assert_eq!(
        read(&elf, 0x0080_0000, KIND_MODULE),
        Err(ElfError::WrongLinkBase {
            found: LINK_BASE,
            wanted: 0x0080_0000
        })
    );
}

#[test]
fn a_module_without_the_optional_entry_points_is_read() {
    if let Some(why) = unavailable() {
        assert!(
            std::env::var_os("DMD_REQUIRE_REAL_MODULE").is_none(),
            "DMD_REQUIRE_REAL_MODULE is set but {why}"
        );
        eprintln!("skipped: {why}");
        return;
    }
    let elf = build(true);
    let x = read(&elf, LINK_BASE, KIND_MODULE).unwrap_or_else(|e| panic!("the bare module was refused: {e}"));
    check_common(&elf, &x);
    assert_eq!((x.event, x.stop), (None, None));
}

#[test]
fn import_slots_merged_into_data_by_the_linker_are_refused() {
    if let Some(why) = unavailable() {
        assert!(
            std::env::var_os("DMD_REQUIRE_REAL_MODULE").is_none(),
            "DMD_REQUIRE_REAL_MODULE is set but {why}"
        );
        eprintln!("skipped: {why}");
        return;
    }
    // Why the script gives `.desk.imp` an output section of its own: the slots are found by the section their symbols are in.
    let elf = build_with(false, true);
    match read(&elf, LINK_BASE, KIND_MODULE) {
        Err(ElfError::ImportWrongSection { section, .. }) => assert_eq!(section, ".data"),
        other => panic!("expected the slots in .data to be refused, got {other:?}"),
    }
}
