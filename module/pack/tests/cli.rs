// Tests of the `dmdpack` command line: its subcommands, their output and the exit statuses 0 (done), 1 (an input refused) and 2 (usage).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
mod elfgen;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use desk_module::{crc32, Builder, Module};
use elfgen::*;

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dmdpack"))
        .args(args.iter().map(|a| a.as_ref()))
        .output()
        .unwrap()
}

fn status(o: &Output) -> i32 {
    o.status.code().expect("exited with a status")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// `a` linked again at `base`, as the linker would.
fn rebased(file: &[u8], base: u32) -> Vec<u8> {
    let m = Module::parse(file, u16::MAX).unwrap();
    let h = m.header();
    let mut image = m.image().to_vec();
    for r in m.relocations() {
        let at = r as usize;
        let v = u32::from_le_bytes(image[at..at + 4].try_into().unwrap()).wrapping_add(base - h.link_base);
        image[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: base,
        image,
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs: m.relocations().collect(),
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    }
    .write()
    .unwrap()
}

#[test]
fn pack_writes_a_module_and_info_describes_it() {
    let d = dir("pack");
    let elf = write(&d, "good.elf", &good().build());
    let out = d.join("GOOD.DMD");
    let o = run(&[&"pack", &elf, &out]);
    assert_eq!(status(&o), 0, "{}", text(&o.stderr));
    assert!(Module::parse(&std::fs::read(&out).unwrap(), 1).is_ok());
    let o = run(&[&"info", &out]);
    assert_eq!(status(&o), 0);
    assert!(text(&o.stdout).starts_with("kind          module (.DMD)\n"));
}

#[test]
fn pack_writes_an_application_with_kind_app() {
    let d = dir("app");
    let mut e = good();
    e.symbol("app_start").name = "desk_main".into();
    e.symbol("app_event").name = "helper_a".into();
    e.symbol("app_stop").name = "helper_b".into();
    let elf = write(&d, "app.elf", &e.build());
    let out = d.join("APP.DKX");
    let o = run(&[&"pack", &"--kind", &"app", &"--sdk", &"2", &elf, &out]);
    assert_eq!(status(&o), 0, "{}", text(&o.stderr));
    let info = text(&run(&[&"info", &out]).stdout);
    assert!(
        info.contains("application (.DKX)") && info.contains("sdk           2"),
        "{info}"
    );
}

#[test]
fn pack_refuses_another_link_base_without_the_flag_and_an_object_file() {
    let d = dir("refuse");
    let elf = write(&d, "good.elf", &good().build());
    let out = d.join("X.DMD");
    let o = run(&[&"pack", &"--link-base", &"0x00800000", &elf, &out]);
    assert_eq!(status(&o), 1);
    assert!(
        text(&o.stderr).contains("linked at 0x400000, not at 0x800000"),
        "{}",
        text(&o.stderr)
    );
    assert!(!out.exists(), "nothing is written for a refused input");
    let mut rel = good().build();
    put16(&mut rel, E_TYPE, 1);
    let rel = write(&d, "obj.o", &rel);
    let o = run(&[&"pack", &rel, &out]);
    assert_eq!(status(&o), 1);
    assert!(text(&o.stderr).contains("ET_EXEC"), "{}", text(&o.stderr));
    assert!(!out.exists());
}

#[test]
fn a_missing_input_is_a_refusal_and_bad_arguments_are_usage_errors() {
    let d = dir("usage");
    let o = run(&[&"pack", &d.join("no.elf"), &d.join("X.DMD")]);
    assert_eq!(status(&o), 1);
    assert!(text(&o.stderr).contains("no.elf"));
    assert_eq!(status(&run(&[])), 2);
    assert_eq!(status(&run(&[&"frobnicate"])), 2);
    assert_eq!(status(&run(&[&"pack", &"one-argument"])), 2);
    assert_eq!(status(&run(&[&"pack", &"--kind", &"library", &"a", &"b"])), 2);
    assert_eq!(status(&run(&[&"pack", &"--sdk", &"x", &"a", &"b"])), 2);
    assert_eq!(status(&run(&[&"pack", &"--link-base", &"a", &"b"])), 2);
    assert_eq!(status(&run(&[&"pack", &"--bogus", &"a", &"b"])), 2);
    assert_eq!(status(&run(&[&"info"])), 2);
    assert_eq!(status(&run(&[&"cross", &"only-one"])), 2);
    // `--sdk` belongs to `pack`.
    assert_eq!(status(&run(&[&"elf", &"--sdk", &"1", &"a"])), 2);
}

#[test]
fn info_refuses_a_damaged_file() {
    let d = dir("info");
    let elf = write(&d, "good.elf", &good().build());
    let out = d.join("GOOD.DMD");
    assert_eq!(status(&run(&[&"pack", &elf, &out])), 0);
    let mut bytes = std::fs::read(&out).unwrap();
    bytes[0x70] ^= 1;
    let bad = write(&d, "BAD.DMD", &bytes);
    let o = run(&[&"info", &bad]);
    assert_eq!(status(&o), 1);
    assert!(text(&o.stderr).contains("checksum"), "{}", text(&o.stderr));
}

#[test]
fn cross_passes_for_two_links_and_names_the_offset_of_a_difference() {
    let d = dir("cross");
    let elf = write(&d, "good.elf", &good().build());
    let a = d.join("A.DMD");
    assert_eq!(status(&run(&[&"pack", &elf, &a])), 0);
    let a_bytes = std::fs::read(&a).unwrap();
    let b = write(&d, "B.DMD", &rebased(&a_bytes, 0x0080_0000));
    let o = run(&[&"cross", &a, &b]);
    assert_eq!(status(&o), 0, "{}", text(&o.stderr));
    // One relocated word of the second link changed (image offset 0x1008).
    let mut changed = std::fs::read(&b).unwrap();
    changed[96 + 0x1008] ^= 0x10;
    changed[84..88].copy_from_slice(&[0; 4]);
    let crc = crc32(&changed);
    changed[84..88].copy_from_slice(&crc.to_le_bytes());
    let c = write(&d, "C.DMD", &changed);
    let o = run(&[&"cross", &a, &c]);
    assert_eq!(status(&o), 1);
    assert!(text(&o.stderr).contains("image offset 0x1008"), "{}", text(&o.stderr));
    // A file that is not an image.
    let junk = write(&d, "J.DMD", b"junk");
    assert_eq!(status(&run(&[&"cross", &a, &junk])), 1);
}

#[test]
fn elf_prints_what_the_reader_found() {
    let d = dir("elf");
    let elf = write(&d, "good.elf", &good().build());
    let o = run(&[&"elf", &elf]);
    assert_eq!(status(&o), 0);
    let out = text(&o.stdout);
    assert!(
        out.contains("link base   0x00400000") && out.contains("relocations 4") && out.contains("imports     2"),
        "{out}"
    );
}
