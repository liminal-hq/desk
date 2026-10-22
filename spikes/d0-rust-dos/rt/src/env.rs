// The command tail and the environment, both read from the PSP.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The stub's information block holds a selector for the PSP, whose base is the PSP's linear address. The command tail is the
// length byte at PSP:80h and up to 126 characters after it. The word at PSP:2Ch is the environment, but as a protected-mode
// selector, not a real-mode segment: the DPMI host rewrites it when the program enters protected mode. Its base is the
// environment's linear address and its limit the size of the block. The runtime reaches all of it through the flat data segment
// (conventional memory is mapped at linear 0), so it never loads these selectors, only asks the host for their bases.
// A command line longer than 126 characters is not supported (DJGPP's "!proxy" convention is a C library feature).
use crate::dpmi;
use alloc::string::String;
use alloc::vec::Vec;

fn psp_ptr() -> *const u8 {
    let sel = crate::start::stub_info().psp_selector;
    let lin = dpmi::get_segment_base(sel).unwrap_or(0);
    dpmi::linear_to_ptr::<u8>(lin)
}

/// The raw command tail (without the leading space DOS keeps, and without the carriage return).
pub fn command_tail() -> Vec<u8> {
    let p = psp_ptr();
    // SAFETY: the PSP is 256 bytes at `p`; the tail is `len` <= 127 bytes at offset 0x81.
    unsafe {
        let len = (*p.add(0x80) as usize).min(127);
        let tail = core::slice::from_raw_parts(p.add(0x81), len);
        let end = tail.iter().position(|&b| b == b'\r').unwrap_or(len);
        let mut v = tail[..end].to_vec();
        if v.first() == Some(&b' ') {
            v.remove(0);
        }
        v
    }
}

/// The command-line arguments, split on spaces and tabs; double quotes group. There is no program name: see `program_path`.
pub fn args() -> Vec<String> {
    let tail = command_tail();
    let mut out = Vec::new();
    let mut cur = Vec::new();
    let mut quoted = false;
    let mut have = false;
    for &b in &tail {
        match b {
            b'"' => {
                quoted = !quoted;
                have = true;
            }
            b' ' | b'\t' if !quoted => {
                if have {
                    out.push(String::from_utf8_lossy(&cur).into_owned());
                    cur.clear();
                    have = false;
                }
            }
            _ => {
                cur.push(b);
                have = true;
            }
        }
    }
    if have {
        out.push(String::from_utf8_lossy(&cur).into_owned());
    }
    out
}

fn env_block() -> &'static [u8] {
    let p = psp_ptr();
    // SAFETY: PSP:2Ch is a word in the PSP. The selector's limit bounds the environment, which sits in conventional memory.
    unsafe {
        let sel = u16::from_le_bytes([*p.add(0x2C), *p.add(0x2D)]);
        let base = dpmi::get_segment_base(sel).unwrap_or(0);
        let limit = selector_limit(sel);
        core::slice::from_raw_parts(dpmi::linear_to_ptr::<u8>(base), limit.min(32768) as usize)
    }
}

/// The limit of a selector (LSL), 0 if the selector is not valid.
fn selector_limit(sel: u16) -> u32 {
    let (limit, ok): (u32, u32);
    // SAFETY: LSL reads a descriptor field; ZF reports validity.
    unsafe {
        core::arch::asm!("lsl {l}, {s:e}", "setz {o:l}", "movzx {o:e}, {o:l}", s = in(reg) sel as u32, l = out(reg) limit, o = out(reg_abcd) ok, options(nostack));
    }
    if ok != 0 {
        limit + 1
    } else {
        0
    }
}

/// The environment as `NAME=value` strings.
pub fn vars() -> Vec<(String, String)> {
    let mut v = Vec::new();
    for entry in env_block().split(|&b| b == 0) {
        if entry.is_empty() {
            break;
        }
        if let Some(eq) = entry.iter().position(|&b| b == b'=') {
            v.push((
                String::from_utf8_lossy(&entry[..eq]).into_owned(),
                String::from_utf8_lossy(&entry[eq + 1..]).into_owned(),
            ));
        }
    }
    v
}

/// One variable, by name (DOS names are upper case).
pub fn var(name: &str) -> Option<String> {
    vars().into_iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

/// The full path of the running program, which DOS 3 and later stores after the environment strings.
pub fn program_path() -> Option<String> {
    let b = env_block();
    let end = b.windows(2).position(|w| w == [0, 0])?;
    // After the double NUL: a word count (1), then the NUL-terminated path.
    let rest = b.get(end + 2 + 2..)?;
    let n = rest.iter().position(|&c| c == 0)?;
    Some(String::from_utf8_lossy(&rest[..n]).into_owned())
}
