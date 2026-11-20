// What the stub hands over at entry: registers, selectors, the stub information block, the PSP, the environment, the x87.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use core::arch::asm;
use rt::{dpmi, env, println, start};

/// (limit via LSL, access rights via LAR) of a selector, as the CPU reports them now.
fn describe(sel: u32) -> (u32, u32) {
    let (limit, rights): (u32, u32);
    // SAFETY: LSL and LAR only read descriptor fields; both leave the destination unchanged on failure, which shows as zero here.
    unsafe {
        asm!("xor {l}, {l}", "xor {a}, {a}", "lsl {l}, {s}", "lar {a}, {s}", s = in(reg) sel, l = out(reg) limit, a = out(reg) rights, options(nostack));
    }
    (limit, rights >> 8 & 0xF0FF)
}

pub fn main() -> i32 {
    let r = start::entry_regs();
    println!(
        "scratch: eax={:08x} ebx={:08x} ecx={:08x} edx={:08x}",
        r.eax, r.ebx, r.ecx, r.edx
    );
    println!(
        "scratch: esi={:08x} edi={:08x} ebp={:08x} esp={:08x}",
        r.esi, r.edi, r.ebp, r.esp
    );
    println!(
        "entry: eflags={:08x} (DF={} IF={})",
        r.eflags,
        r.eflags >> 10 & 1,
        r.eflags >> 9 & 1
    );
    println!(
        "entry: cs={:04x} ds={:04x} es={:04x} fs={:04x} gs={:04x} ss={:04x}",
        r.cs, r.ds, r.es, r.fs, r.gs, r.ss
    );
    println!("entry: x87 control word {:#06x}, status word {:#06x}", r.fcw, r.fsw);
    for (name, sel) in [
        ("cs", r.cs),
        ("ds", r.ds),
        ("es", r.es),
        ("fs", r.fs),
        ("gs", r.gs),
        ("ss", r.ss),
    ] {
        let base = dpmi::get_segment_base(sel as u16).ok();
        let (limit, rights) = describe(sel);
        println!(
            "selector {}: {:04x} base {:?} limit {:#x} lar {:#06x}",
            name, sel, base, limit, rights
        );
    }
    let s = start::stub_info();
    let magic = core::str::from_utf8(&s.magic).unwrap_or("?").trim_end_matches('\0');
    println!(
        "stubinfo: magic {:?} size {:#x} minstack {:#x} memory_handle {:#x} initial_size {:#x}",
        magic, s.size, s.minstack, s.memory_handle, s.initial_size
    );
    println!(
        "stubinfo: minkeep {:#x} ds_selector {:#x} ds_segment {:#x} psp_selector {:#x} cs_selector {:#x} env_size {:#x}",
        s.minkeep, s.ds_selector, s.ds_segment, s.psp_selector, s.cs_selector, s.env_size
    );
    let name = |b: &[u8]| {
        core::str::from_utf8(b)
            .unwrap_or("?")
            .trim_end_matches('\0')
            .to_ascii_uppercase_str()
    };
    println!(
        "stubinfo: basename {:?} argv0 {:?} dpmi_server {:?}",
        name(&s.basename),
        name(&s.argv0),
        name(&s.dpmi_server)
    );
    println!(
        "image base (linear) {:#x}, psp base {:?}",
        dpmi::image_base(),
        dpmi::get_segment_base(s.psp_selector)
    );
    println!("command tail {:?}", core::str::from_utf8(&env::command_tail()).unwrap_or("?"));
    println!("args {:?}", env::args());
    let vars = env::vars();
    println!(
        "environment: {} variables, first {:?}",
        vars.len(),
        vars.first().map(|(k, _)| k.as_str())
    );
    println!("program path {:?}", env::program_path());
    println!("x87 control word now {:#06x}", rt::fpu::control_word());
    0
}

trait UpperAscii {
    fn to_ascii_uppercase_str(&self) -> alloc::string::String;
}
impl UpperAscii for str {
    fn to_ascii_uppercase_str(&self) -> alloc::string::String {
        self.to_ascii_uppercase()
    }
}
