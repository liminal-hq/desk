// dmdpack: the module packer. `pack` turns a linked ELF file into a `.DMD` or `.DKX`, `info` describes one, `cross` compares two links of
// one module at different bases, and `elf` prints what the ELF reader found, for debugging.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::process::ExitCode;

use dmdpack::pack::DEFAULT_SDK;
use dmdpack::{cross, info, pack, read, KIND_APP, KIND_MODULE, LINK_BASE};

const USAGE: &str = "usage: dmdpack pack [--kind module|app] [--link-base ADDR] [--sdk N] <in.elf> <out>
       dmdpack info <file>
       dmdpack cross <a> <b>
       dmdpack elf [--kind module|app] [--link-base ADDR] <file>

exit status: 0 done, 1 an input was refused (the reason is printed), 2 a usage error";

fn parse_addr(text: &str) -> Option<u32> {
    match text.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(&hex.replace('_', ""), 16).ok(),
        None => text.parse().ok(),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn refused(what: &str, why: impl std::fmt::Display) -> ExitCode {
    eprintln!("dmdpack: {what}: {why}");
    ExitCode::from(1)
}

fn read_file(path: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|e| refused(path, e))
}

/// The options and positional arguments of `pack` and `elf`.
struct Options {
    kind: u16,
    link_base: u32,
    sdk: u16,
    files: Vec<String>,
}

fn options(args: impl Iterator<Item = String>, sdk_allowed: bool) -> Option<Options> {
    let mut o = Options {
        kind: KIND_MODULE,
        link_base: LINK_BASE,
        sdk: DEFAULT_SDK,
        files: Vec::new(),
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--kind" => match args.next().as_deref() {
                Some("module") => o.kind = KIND_MODULE,
                Some("app") => o.kind = KIND_APP,
                _ => return None,
            },
            "--link-base" => o.link_base = args.next().as_deref().and_then(parse_addr)?,
            "--sdk" if sdk_allowed => o.sdk = args.next()?.parse().ok()?,
            _ if !arg.starts_with("--") => o.files.push(arg),
            _ => return None,
        }
    }
    Some(o)
}

fn run_pack(args: impl Iterator<Item = String>) -> ExitCode {
    let Some(o) = options(args, true) else { return usage() };
    let [input, output] = o.files.as_slice() else { return usage() };
    let elf = match read_file(input) {
        Ok(b) => b,
        Err(code) => return code,
    };
    let packed = match pack(&elf, o.kind, o.link_base, o.sdk) {
        Ok(b) => b,
        Err(e) => return refused(input, e),
    };
    match std::fs::write(output, &packed) {
        Ok(()) => {
            println!("dmdpack: {output}: {} bytes", packed.len());
            ExitCode::SUCCESS
        }
        Err(e) => refused(output, e),
    }
}

fn run_info(args: impl Iterator<Item = String>) -> ExitCode {
    let files: Vec<String> = args.collect();
    let [path] = files.as_slice() else { return usage() };
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    match info(&bytes) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => refused(path, e),
    }
}

fn run_cross(args: impl Iterator<Item = String>) -> ExitCode {
    let files: Vec<String> = args.collect();
    let [a, b] = files.as_slice() else { return usage() };
    let (fa, fb) = match (read_file(a), read_file(b)) {
        (Ok(fa), Ok(fb)) => (fa, fb),
        (Err(code), _) | (_, Err(code)) => return code,
    };
    match cross(&fa, &fb) {
        Ok(()) => {
            println!("dmdpack: {a} loaded at the link base of {b} equals it");
            ExitCode::SUCCESS
        }
        Err(e) => refused(&format!("{a} against {b}"), e),
    }
}

fn run_elf(args: impl Iterator<Item = String>) -> ExitCode {
    let Some(o) = options(args, false) else { return usage() };
    let [path] = o.files.as_slice() else { return usage() };
    let bytes = match read_file(path) {
        Ok(b) => b,
        Err(code) => return code,
    };
    match read(&bytes, o.link_base, o.kind) {
        Ok(x) => {
            println!("link base   {:#010x}", x.link_base);
            println!(
                "image       {} bytes, memory {} bytes, read-only {} bytes",
                x.image.len(),
                x.mem_size,
                x.ro_size
            );
            println!("start       {:#x}", x.start);
            match x.event {
                Some(e) => println!("event       {e:#x}"),
                None => println!("event       none"),
            }
            match x.stop {
                Some(s) => println!("stop        {s:#x}"),
                None => println!("stop        none"),
            }
            println!("relocations {}", x.relocs.len());
            for r in &x.relocs {
                println!("  {r:#x}");
            }
            println!("imports     {}", x.imports.len());
            for (name, slot) in &x.imports {
                println!("  {slot:#x} {name}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => refused(path, e),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("pack") => run_pack(args),
        Some("info") => run_info(args),
        Some("cross") => run_cross(args),
        Some("elf") => run_elf(args),
        _ => usage(),
    }
}
