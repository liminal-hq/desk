// dmdpack: the module packer. For now only `dmdpack elf <file>`, which prints what the ELF reader found, for debugging.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

use std::process::ExitCode;

use dmdpack::{read, KIND_APP, KIND_MODULE, LINK_BASE};

const USAGE: &str = "usage: dmdpack elf [--kind module|app] [--link-base ADDR] <file>";

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

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("elf") {
        return usage();
    }
    let (mut kind, mut base, mut path) = (KIND_MODULE, LINK_BASE, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--kind" => match args.next().as_deref() {
                Some("module") => kind = KIND_MODULE,
                Some("app") => kind = KIND_APP,
                _ => return usage(),
            },
            "--link-base" => match args.next().as_deref().and_then(parse_addr) {
                Some(addr) => base = addr,
                None => return usage(),
            },
            _ if path.is_none() && !arg.starts_with("--") => path = Some(arg),
            _ => return usage(),
        }
    }
    let Some(path) = path else { return usage() };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("dmdpack: {path}: {e}");
            return ExitCode::from(1);
        }
    };
    match read(&bytes, base, kind) {
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
        Err(e) => {
            eprintln!("dmdpack: {path}: {e}");
            ExitCode::from(1)
        }
    }
}
