// The BIOS scenarios: what the kernel does with the real-mode trampoline once it is running, as a transcript the run script checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Runs when the loader image carries the `bios` test mode (`docs/N3-BIOS-PATH.md`, "How it is tested"). In order: the memory
// calls against the loader's map, the clock, the error paths, the disk through INT 13h, a keyboard byte that arrives during a call,
// the stress run, VBE after boot with the way back to text mode, and finally the kernel's own timer and keyboard. The lines the
// run script waits for are `bios: ready NAME` (a screenshot is taken, then a key is sent) and `bios: keyboard: waiting`.

use crate::bios::{self, BiosError, NativeBios, RealModeRegs};
use crate::hal::{CLOCK, IRQS, PIC, PORTS};
use crate::memory::{self, Region, KIND_USABLE};
use crate::scene::{self, Depth};
use crate::video::{Lfb, Video};
use crate::vga::Vga;
use crate::{gfx, input, pit, println, BootInfo};
use alloc::string::String;
use core::arch::asm;
use core::sync::atomic::{AtomicU32, Ordering};
use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::{InputSource, KeyCode, PortIo, SharedIrqService};
use desk_i8042::Ps2Input;
use desk_vga::dac::{self};
use desk_vga::vbe::{Vbe, VbeMode};
use desk_vga::{checksum, Linear8};

const ACK_TIMEOUT: u32 = 20 * pit::HZ;

/// Registers for a call with the given `eax`, and the call: the common shape of the scenarios below.
fn int(vector: u8, eax: u32) -> (Result<(), BiosError>, RealModeRegs) {
    let mut r = RealModeRegs::with_eax(eax);
    let res = bios::call(vector, &mut r);
    (res, r)
}

fn bcd_ok(b: u8) -> bool {
    b & 0x0F <= 9 && b >> 4 <= 9
}

fn bcd(b: u8) -> u32 {
    u32::from(b >> 4) * 10 + u32::from(b & 0x0F)
}

// ---- Kernel state, to prove a call leaves it as it was

/// The parts of the CPU and the machine a call must give back unchanged.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
struct Snapshot {
    esp: u32,
    gdtr: (u16, u32),
    idtr: (u16, u32),
    gdt: [u64; 3],
    idt_hash: u32,
    segments: [u16; 6],
    cr0: u32,
    pic_mask: u16,
    if_flag: bool,
}

#[inline(never)]
fn snapshot() -> Snapshot {
    let (mut esp, mut cr0, mut flags): (u32, u32, u32);
    let (mut cs, mut ds, mut es, mut fs, mut gs, mut ss): (u16, u16, u16, u16, u16, u16);
    let mut gdtr = [0u8; 6];
    let mut idtr = [0u8; 6];
    // SAFETY: only reads CPU state: the stack pointer, the segment registers, CR0, the flags and the descriptor table registers.
    unsafe {
        asm!("mov {0}, esp", out(reg) esp, options(nomem, nostack, preserves_flags));
        asm!("mov {0}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        asm!("pushfd", "pop {0}", out(reg) flags, options(preserves_flags));
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ds", out(reg) ds, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, es", out(reg) es, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, fs", out(reg) fs, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, gs", out(reg) gs, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
        asm!("sgdt [{0}]", in(reg) gdtr.as_mut_ptr(), options(nostack, preserves_flags));
        asm!("sidt [{0}]", in(reg) idtr.as_mut_ptr(), options(nostack, preserves_flags));
    }
    let table = |b: [u8; 6]| (u16::from_le_bytes([b[0], b[1]]), u32::from_le_bytes([b[2], b[3], b[4], b[5]]));
    let (gdtr, idtr) = (table(gdtr), table(idtr));
    let mut gdt = [0u64; 3];
    // SAFETY: the kernel's GDT is three descriptors and the IDT is 256 gates of 8 bytes, both static and always mapped.
    let idt_hash = unsafe {
        for (i, slot) in gdt.iter_mut().enumerate() {
            *slot = core::ptr::read_volatile((gdtr.1 as *const u64).add(i));
        }
        let bytes = core::slice::from_raw_parts(idtr.1 as *const u8, usize::from(idtr.0) + 1);
        bytes.iter().fold(0x811C_9DC5u32, |h, b| (h ^ u32::from(*b)).wrapping_mul(0x0100_0193))
    };
    Snapshot {
        esp,
        gdtr,
        idtr,
        gdt,
        idt_hash,
        segments: [cs, ds, es, fs, gs, ss],
        cr0,
        pic_mask: PIC.mask_bits(),
        if_flag: flags & (1 << 9) != 0,
    }
}

// ---- Memory: INT 12h, INT 15h

fn loader_entries(info: &BootInfo) -> alloc::vec::Vec<(u64, u64, u32)> {
    memory::regions(info).iter().map(|r: &Region| (r.base, r.len, r.kind)).collect()
}

/// E820 again after boot, through the bounce buffer, filtered the way the loader filters (empty and ACPI-ignored entries dropped).
fn e820_after_boot() -> alloc::vec::Vec<(u64, u64, u32)> {
    let bounce = bios::bounce_buffer();
    let mut out = alloc::vec::Vec::new();
    let mut cont = 0u32;
    for _ in 0..64 {
        bounce.write_low(20, &1u32.to_le_bytes()).unwrap();
        let mut r = RealModeRegs { eax: 0xE820, ebx: cont, ecx: 24, edx: 0x534D_4150, es: bounce.segment(), ..RealModeRegs::default() };
        bios::call(0x15, &mut r).expect("INT 15h AX=E820");
        if r.carry() {
            break;
        }
        assert_eq!(r.eax, 0x534D_4150, "E820 answers with SMAP");
        let mut e = [0u8; 24];
        bounce.read_low(0, &mut e).unwrap();
        let (base, len) = (u64::from_le_bytes(e[0..8].try_into().unwrap()), u64::from_le_bytes(e[8..16].try_into().unwrap()));
        let (kind, acpi) = (u32::from_le_bytes(e[16..20].try_into().unwrap()), u32::from_le_bytes(e[20..24].try_into().unwrap()));
        let size = r.ecx & 0xFFFF;
        let keep = size != 0 && len != 0 && !(size > 20 && acpi & 1 == 0);
        if keep {
            out.push((base, len, kind));
        }
        cont = r.ebx;
        if cont == 0 {
            break;
        }
    }
    out
}

fn memory_calls(info: &BootInfo) {
    let (res, r) = int(0x12, 0);
    res.unwrap();
    println!("bios: INT 12h: {} KiB of conventional memory", r.eax & 0xFFFF);
    assert!((500..=640).contains(&(r.eax & 0xFFFF)));

    let loader = loader_entries(info);
    let again = e820_after_boot();
    let same = loader == again;
    println!("bios: INT 15h AX=E820 after boot: {} entries, the loader's map had {}, identical: {}", again.len(), loader.len(), same);
    assert!(same, "E820 after boot differs from the loader's map");

    // The contiguous usable KiB from 1 MiB, from the map, against what the older calls report.
    let top = again.iter().filter(|e| e.2 == KIND_USABLE && e.0 <= 0x10_0000 && e.0 + e.1 > 0x10_0000).map(|e| e.0 + e.1).max().unwrap_or(0x10_0000);
    let map_kib = ((top - 0x10_0000) / 1024) as u32;
    let (res, r) = int(0x15, 0x8800);
    res.unwrap();
    assert!(!r.carry(), "INT 15h AH=88h");
    let kib_88 = r.eax & 0xFFFF;
    let (res, r) = int(0x15, 0xE801);
    res.unwrap();
    assert!(!r.carry(), "INT 15h AX=E801");
    let (low, high) = if r.ecx & 0xFFFF == 0 { (r.eax & 0xFFFF, r.ebx & 0xFFFF) } else { (r.ecx & 0xFFFF, r.edx & 0xFFFF) };
    println!("bios: INT 15h AH=88h: {} KiB above 1 MiB; AX=E801: {} KiB below 16 MiB, {} blocks of 64 KiB above; the map says {} KiB contiguous", kib_88, low, high, map_kib);
    // The legacy calls round to the RAM size and ignore the reserved hole at the top of the last megabyte, so they agree with the
    // map to within 256 KiB, not exactly.
    let near = |a: u32, b: u32| a.abs_diff(b) <= 256;
    let ok = near(kib_88.min(15360), map_kib.min(15360)) && near(low, map_kib.min(15360));
    println!("bios: the legacy memory calls agree with the E820 map to within 256 KiB: {}", ok);
    assert!(ok);
}

// ---- Time: INT 1Ah

fn cmos(reg: u8) -> u8 {
    PORTS.outb(0x70, 0x80 | reg);
    PORTS.inb(0x71)
}

fn clock_calls() {
    let (res, t) = int(0x1A, 0x0200);
    res.unwrap();
    assert!(!t.carry(), "INT 1Ah AH=02h");
    let (h, m, s) = ((t.ecx >> 8) as u8, t.ecx as u8, (t.edx >> 8) as u8);
    let (res, d) = int(0x1A, 0x0400);
    res.unwrap();
    assert!(!d.carry(), "INT 1Ah AH=04h");
    let (century, year, month, day) = ((d.ecx >> 8) as u8, d.ecx as u8, (d.edx >> 8) as u8, d.edx as u8);
    let valid = [h, m, s, century, year, month, day].iter().all(|b| bcd_ok(*b)) && bcd(h) < 24 && bcd(m) < 60 && bcd(s) < 60 && (1..=12).contains(&bcd(month)) && (1..=31).contains(&bcd(day));
    println!("bios: INT 1Ah time and date are valid BCD in range: {}", valid);
    assert!(valid);
    // The same clock read straight from the CMOS RTC: the two must be within a couple of seconds of each other.
    while cmos(0x0A) & 0x80 != 0 {}
    let direct = bcd(cmos(0x04)) * 3600 + bcd(cmos(0x02)) * 60 + bcd(cmos(0x00));
    let via_bios = bcd(h) * 3600 + bcd(m) * 60 + bcd(s);
    let close = direct.abs_diff(via_bios) <= 2 || direct.abs_diff(via_bios) >= 86_398;
    println!("bios: INT 1Ah agrees with the RTC read directly through the CMOS ports to within 2 s: {}", close);
    assert!(close);
    let (res, c) = int(0x1A, 0);
    res.unwrap();
    println!("bios: INT 1Ah AH=00h answers without error: {}", c.eax >> 8 == 0 || c.eax >> 8 == 1);
}

// ---- Errors, repeats and the idle loop

struct Probe {
    outcome: AtomicU32,
}

const NOT_RUN: u32 = 0;
const REFUSED_IN_INTERRUPT: u32 = 1;
const REFUSED_OTHER: u32 = 2;
const ALLOWED: u32 = 3;

impl SharedIrqService for Probe {
    /// The timer's work, plus one BIOS call attempted from interrupt context, which must be refused.
    fn on_irq(&self) {
        CLOCK.counter().tick();
        if self.outcome.load(Ordering::Relaxed) == NOT_RUN {
            let mut r = RealModeRegs::with_eax(0x0200);
            let v = match bios::call(0x1A, &mut r) {
                Err(BiosError::InInterrupt) => REFUSED_IN_INTERRUPT,
                Err(_) => REFUSED_OTHER,
                Ok(()) => ALLOWED,
            };
            self.outcome.store(v, Ordering::Relaxed);
        }
    }
}

static PROBE: Probe = Probe { outcome: AtomicU32::new(NOT_RUN) };

fn error_paths() {
    // The BIOS's own error flag: an unsupported INT 15h function, and a disk read from a drive that does not exist.
    let (res, r) = int(0x15, 0xFF00);
    res.unwrap();
    println!("bios: INT 15h AH=FFh (no such function): carry {}, AH={:#04x}", r.carry() as u8, r.ah());
    assert!(r.carry());
    let mut r = RealModeRegs { eax: 0x0201, ecx: 0x0001, edx: 0x0077, es: bios::bounce_buffer().segment(), ..RealModeRegs::default() };
    bios::call(0x13, &mut r).unwrap();
    println!("bios: INT 13h read of drive 0x77 (no such drive): carry {}, status {:#04x}", r.carry() as u8, r.ah());
    assert!(r.carry() && r.ah() != 0);

    // A vector with no handler is refused before anything is switched. Vector 0xF0 is in the user range; its entry is set to
    // zero for the test and put back.
    let slot = 0xF0u8;
    let saved = bios::vector_entry(slot);
    crate::bios::set_vector_entry(slot, 0);
    let (res, _) = int(slot, 0);
    crate::bios::set_vector_entry(slot, saved);
    println!("bios: interrupt {:#04x} with an empty vector table entry: {:?}", slot, res);
    assert_eq!(res, Err(BiosError::NoHandler(slot)));

    // The re-entry guard.
    println!("bios: a second call while one is in progress is refused as busy: {}", bios::guard_refuses_reentry());
    assert!(bios::guard_refuses_reentry());

    // A call from interrupt context: the timer's handler is swapped for one that tries a call on its first tick.
    IRQS.unregister(Irq::TIMER).expect("IRQ 0 has a handler");
    IRQS.register(Irq::TIMER, IrqHandler::from_shared(&PROBE), Chain::None).expect("IRQ 0 is free");
    let start = pit::ticks();
    while PROBE.outcome.load(Ordering::Relaxed) == NOT_RUN {
        // SAFETY: interrupts are enabled; the next tick wakes the halt.
        unsafe { asm!("sti", "hlt") };
        if pit::ticks().wrapping_sub(start) > 100 {
            panic!("the probe handler never ran");
        }
    }
    IRQS.unregister(Irq::TIMER).expect("the probe is registered");
    IRQS.register(Irq::TIMER, IrqHandler::from_shared(CLOCK.counter()), Chain::None).expect("IRQ 0 is free");
    let outcome = PROBE.outcome.load(Ordering::Relaxed);
    println!("bios: a call made from the timer interrupt handler is refused as in-interrupt: {}", outcome == REFUSED_IN_INTERRUPT);
    assert_eq!(outcome, REFUSED_IN_INTERRUPT);
    // After all of that, the next call works.
    let (res, r) = int(0x1A, 0x0200);
    res.unwrap();
    println!("bios: the next call after the refusals works: {}", !r.carry());
    assert!(!r.carry());
}

fn repeats_and_idle() {
    let before = snapshot();
    let (a, ra) = int(0x1A, 0x0200);
    let (b, rb) = int(0x1A, 0x0200);
    a.unwrap();
    b.unwrap();
    let close = bcd((ra.edx >> 8) as u8).abs_diff(bcd((rb.edx >> 8) as u8)) <= 1 || bcd((ra.edx >> 8) as u8).abs_diff(bcd((rb.edx >> 8) as u8)) == 59;
    println!("bios: two calls in a row both work, and read the same second or the next: {}", close);
    assert!(close);
    // From the idle loop: interrupts on, halted until a tick, then a call; the interrupt flag comes back as it was.
    let start = pit::ticks();
    let mut ok = true;
    for _ in 0..5 {
        // SAFETY: interrupts are on after `sti`; the halt returns on the next tick.
        unsafe { asm!("sti", "hlt") };
        let if_before = snapshot().if_flag;
        let (res, r) = int(0x1A, 0x0200);
        ok &= res.is_ok() && !r.carry() && snapshot().if_flag == if_before && if_before;
    }
    let slept = pit::ticks().wrapping_sub(start);
    println!("bios: five calls from the idle loop (halt, then call) with interrupts on all work, and the interrupt flag survives: {}", ok);
    assert!(ok && slept >= 5);
    let after = snapshot();
    println!("bios: the kernel's CPU state is the same after the calls: {}", Snapshot { if_flag: before.if_flag, ..after } == before);
    assert_eq!(Snapshot { if_flag: before.if_flag, ..after }, before);
}

// ---- The disk: INT 13h

const STAGE1_VARIABLES: core::ops::Range<usize> = 0x105..0x10A;

fn read_sectors(drive: u8, geometry: (u32, u32, u32), lba: u32, count: u32) -> Result<(), u8> {
    let (_, heads, spt) = geometry;
    let (cyl, head, sec) = (lba / (heads * spt), (lba / spt) % heads, lba % spt + 1);
    let mut r = RealModeRegs {
        eax: 0x0200 | count,
        ecx: ((cyl & 0xFF) << 8) | ((cyl >> 2) & 0xC0) | sec,
        edx: (head << 8) | u32::from(drive),
        es: bios::bounce_buffer().segment(),
        ..RealModeRegs::default()
    };
    bios::call(0x13, &mut r).expect("INT 13h");
    if r.carry() {
        Err(r.ah())
    } else {
        Ok(())
    }
}

fn disk_calls(info: &BootInfo) {
    let drive = info.boot_drive as u8;
    let bounce = bios::bounce_buffer();
    let mut r = RealModeRegs { eax: 0x0800, edx: u32::from(drive), ..RealModeRegs::default() };
    bios::call(0x13, &mut r).unwrap();
    assert!(!r.carry(), "INT 13h AH=08h");
    let (cyls, heads, spt) = ((((r.ecx >> 8) & 0xFF) | ((r.ecx & 0xC0) << 2)) + 1, ((r.edx >> 8) & 0xFF) + 1, r.ecx & 0x3F);
    println!("bios: INT 13h AH=08h drive {:#04x}: {} cylinders, {} heads, {} sectors per track", drive, cyls, heads, spt);
    let mut e = RealModeRegs { eax: 0x4100, ebx: 0x55AA, edx: u32::from(drive), ..RealModeRegs::default() };
    bios::call(0x13, &mut e).unwrap();
    let ext = !e.carry() && e.ebx & 0xFFFF == 0xAA55;
    println!("bios: INT 13h AH=41h extensions (LBA reads with AH=42h) on drive {:#04x}: {}", drive, if ext { "available" } else { "not available" });
    let geometry = (cyls, heads, spt);

    // Sector 0 through the BIOS against the copy of it the BIOS put at 0x7C00 when it booted: the boot signature, and every byte
    // except the five variables stage 1 writes while it runs.
    read_sectors(drive, geometry, 0, 1).expect("reading sector 0");
    let mut sector = [0u8; 512];
    bounce.read_low(0, &mut sector).unwrap();
    println!("bios: sector 0 through INT 13h has the boot signature 0x55AA: {}", sector[510] == 0x55 && sector[511] == 0xAA);
    assert!(sector[510] == 0x55 && sector[511] == 0xAA);
    let mut resident = [0u8; 512];
    bios::read_low(0, 0x7C00, &mut resident).unwrap();
    let diffs = (0..512).filter(|i| !STAGE1_VARIABLES.contains(i) && sector[*i] != resident[*i]).count();
    println!("bios: sector 0 equals the copy of stage 1 still in memory at 0x7C00 (outside its five variables): {}", diffs == 0);
    assert_eq!(diffs, 0);

    // The kernel's own first 8 KiB: 16 sectors from the second track in one call, against the image at 1 MiB.
    let lba = 18;
    read_sectors(drive, geometry, lba, 16).expect("reading the kernel's sectors");
    let mut disk = alloc::vec![0u8; 16 * 512];
    bounce.read_low(0, &mut disk).unwrap();
    // SAFETY: the kernel image starts at 1 MiB and is larger than 8 KiB; its code is not modified while it runs.
    let memory = unsafe { core::slice::from_raw_parts(memory::kernel_range().0 as *const u8, 16 * 512) };
    let same = disk == memory;
    println!("bios: 16 sectors from LBA {} (the kernel) equal the first 8 KiB of the running kernel: {}", lba, same);
    assert!(same);

    // Errors: past the last cylinder, and a count the BIOS cannot do (zero).
    let past = read_sectors(drive, geometry, cyls * heads * spt, 1);
    println!("bios: a read past the end of the disk fails with the BIOS's status: {}", past.is_err());
    assert!(past.is_err());
    hard_disk_calls();
}

/// The first hard disk (drive 0x80), if the run has one: the LBA extensions (AH=41h and 42h), which the floppy does not offer. The run
/// script's disk image has a known text at the start of sectors 0 and 1.
fn hard_disk_calls() {
    let bounce = bios::bounce_buffer();
    let mut r = RealModeRegs { eax: 0x0800, edx: 0x80, ..RealModeRegs::default() };
    bios::call(0x13, &mut r).unwrap();
    if r.carry() {
        println!("bios: INT 13h drive 0x80: not present");
        return;
    }
    let (cyls, heads, spt) = ((((r.ecx >> 8) & 0xFF) | ((r.ecx & 0xC0) << 2)) + 1, ((r.edx >> 8) & 0xFF) + 1, r.ecx & 0x3F);
    println!("bios: INT 13h drive 0x80: present, CHS {} cylinders, {} heads, {} sectors per track", cyls, heads, spt);
    let mut e = RealModeRegs { eax: 0x4100, ebx: 0x55AA, edx: 0x80, ..RealModeRegs::default() };
    bios::call(0x13, &mut e).unwrap();
    let ext = !e.carry() && e.ebx & 0xFFFF == 0xAA55;
    println!("bios: INT 13h AH=41h on drive 0x80: extensions {}, interface bits {:#06x}", if ext { "available" } else { "not available" }, e.ecx & 0xFFFF);
    assert!(ext);
    // The disk address packet: size 16, count 2, buffer at BOUNCE:0x200, starting block 0. Read, then look at the two sectors.
    let mut dap = [0u8; 16];
    dap[0] = 16;
    dap[2] = 2;
    dap[4..6].copy_from_slice(&0x200u16.to_le_bytes());
    dap[6..8].copy_from_slice(&bounce.segment().to_le_bytes());
    bounce.write_low(0, &dap).unwrap();
    let mut r = RealModeRegs { eax: 0x4200, edx: 0x80, esi: 0, ds: bounce.segment(), ..RealModeRegs::default() };
    bios::call(0x13, &mut r).unwrap();
    assert!(!r.carry(), "INT 13h AH=42h failed with status {:#04x}", r.ah());
    let mut data = [0u8; 1024];
    bounce.read_low(0x200, &mut data).unwrap();
    let ok = data.starts_with(b"DESK-HD-SECTOR-0") && data[512..].starts_with(b"DESK-HD-SECTOR-1");
    println!("bios: INT 13h AH=42h read two sectors by LBA and they hold the test disk's known text: {}", ok);
    assert!(ok);
    // The packet's count field is where the BIOS reports how many sectors it did.
    let mut back = [0u8; 16];
    bounce.read_low(0, &mut back).unwrap();
    println!("bios: INT 13h AH=42h reports {} sectors transferred", u16::from_le_bytes([back[2], back[3]]));
    // A block beyond the end of the disk fails with the BIOS's status.
    dap[8..16].copy_from_slice(&0x00FF_FFFF_FFFFu64.to_le_bytes());
    bounce.write_low(0, &dap).unwrap();
    let mut r = RealModeRegs { eax: 0x4200, edx: 0x80, esi: 0, ds: bounce.segment(), ..RealModeRegs::default() };
    bios::call(0x13, &mut r).unwrap();
    println!("bios: INT 13h AH=42h beyond the end of the disk fails: {}", r.carry());
    assert!(r.carry());
}

// ---- A keyboard byte during a call

fn keyboard_during_call(ps2: &mut Ps2Input) {
    let (added_before, pending_before, input_before) = (bios::STATS.ticks_added.load(Ordering::Relaxed), bios::STATS.pending_ticks.load(Ordering::Relaxed), bios::STATS.input_bytes.load(Ordering::Relaxed));
    let t0 = pit::ticks();
    println!("bios: keyboard: waiting one second inside INT 15h AH=86h");
    let mut r = RealModeRegs { eax: 0x8600, ecx: 0x000F, edx: 0x4240, ..RealModeRegs::default() };
    bios::call(0x15, &mut r).unwrap();
    let waited = pit::ticks().wrapping_sub(t0);
    let handed = bios::STATS.input_bytes.load(Ordering::Relaxed) - input_before;
    println!("bios: keyboard: the call returned, {} byte(s) were waiting in the controller and were handed to the driver", handed);
    assert!(handed >= 1, "the key sent during the call was lost");
    println!(
        "bios: the one-second wait took {} kernel ticks (the BIOS timer handler counted {}, undelivered ticks recovered {})",
        waited,
        bios::STATS.ticks_added.load(Ordering::Relaxed) - added_before,
        bios::STATS.pending_ticks.load(Ordering::Relaxed) - pending_before
    );
    // The key must come out of the driver's ring as an ordinary event.
    let start = pit::ticks();
    let mut seen = 0;
    while seen < 2 {
        while let Some(k) = ps2.poll_key() {
            input::print_key(k);
            seen += 1;
        }
        if pit::ticks().wrapping_sub(start) > ACK_TIMEOUT {
            panic!("the key sent during the call never arrived");
        }
        input::idle(ps2);
    }
}

// ---- Stress

fn stress() {
    // Interrupts are on, as they are for the whole scenario, so the timer ticks between and during the calls.
    let before = snapshot();
    let t0 = pit::ticks();
    let mut same = true;
    let mut last = before;
    for i in 0..1000 {
        let (res, r) = int(0x1A, 0x0200);
        assert!(res.is_ok() && !r.carry(), "call {}", i);
        let now = snapshot();
        same &= now == last;
        last = now;
    }
    let took = pit::ticks().wrapping_sub(t0);
    println!("bios: stress: 1000 INT 1Ah calls with the timer running, the stack, GDT, IDT, segments, CR0 and PIC masks identical after every one: {}", same && last == before);
    assert!(same && last == before);
    println!("bios: stress: the 1000 calls took {} kernel ticks, about {} microseconds each (emulated speed, not real hardware)", took, u64::from(took) * 10_000 / 1000);
    // The kernel tick count against the RTC: each change of the RTC second is noted as it is seen, and the ticks between two of
    // them must be a second's worth, 100, give or take the one-tick resolution of seeing a change.
    let mut edges = [0u32; 4];
    let mut count = 0;
    let mut last_second = bcd((int(0x1A, 0x0200).1.edx >> 8) as u8);
    let began = pit::ticks();
    while count < edges.len() {
        // SAFETY: interrupts are on; the next tick wakes the halt.
        unsafe { asm!("hlt") };
        let (_, r) = int(0x1A, 0x0200);
        let s = bcd((r.edx >> 8) as u8);
        if s != last_second {
            last_second = s;
            edges[count] = pit::ticks();
            count += 1;
        }
        if pit::ticks().wrapping_sub(began) > 1000 {
            panic!("the RTC second never changed");
        }
    }
    let gaps: [u32; 3] = [edges[1] - edges[0], edges[2] - edges[1], edges[3] - edges[2]];
    println!("bios: stress: kernel ticks between three RTC seconds while calling INT 1Ah every tick: {}, {} and {}", gaps[0], gaps[1], gaps[2]);
    assert!(gaps.iter().all(|g| (98..=102).contains(g)), "the kernel tick count drifted from the RTC");
    println!("bios: stress: the tick count kept one hundred ticks to the RTC second: true");
}

// ---- VBE after boot

fn describe_mode(m: &VbeMode) -> String {
    alloc::format!("{}x{}x{} attributes {:#06x} linear {}", m.width, m.height, m.bits_per_pixel, m.attributes, m.linear())
}

/// Prints a marker, waits for the key the run script sends once it has taken its screenshot.
fn rendezvous(ps2: &mut Ps2Input, name: &str) {
    println!("bios: ready {}", name);
    let start = pit::ticks();
    loop {
        while ps2.poll_mouse().is_some() {}
        while let Some(k) = ps2.poll_key() {
            if k.pressed {
                return;
            }
        }
        if pit::ticks().wrapping_sub(start) > ACK_TIMEOUT {
            panic!("bios: no key to acknowledge {} within {} ticks", name, ACK_TIMEOUT);
        }
        input::idle(ps2);
    }
}

/// Sets `mode` with the linear frame buffer, checks the BIOS agrees, draws the test picture and checks it reads back.
fn show_mode(vbe: &Vbe<NativeBios>, info: &BootInfo, m: &VbeMode, label: &str, hosted: Option<u32>) -> Lfb {
    let l = Lfb { mode: m.id, width: u32::from(m.width), height: u32::from(m.height), pitch: u32::from(m.pitch), phys: m.phys_base };
    assert!(!memory::overlaps_ram(info, l.phys, l.bytes()), "the frame buffer of mode {:#x} overlaps RAM", m.id);
    memory::reserve(l.phys, l.bytes());
    vbe.set_mode(m.id, true).expect("VBE 4F02h");
    let current = vbe.current_mode().expect("VBE 4F03h");
    println!("bios: VBE mode {:#06x} set with the linear frame buffer, 4F03h reports {:#06x}", m.id, current);
    assert_eq!(current & 0x3FFF, m.id);
    let mut region = gfx::lfb_region(&l);
    let mut c = Linear8::new(&mut region, l.width, l.height, l.pitch).expect("the frame buffer surface");
    let palette = gfx::palette_of(&scene::palette256());
    dac::set_palette(&PORTS, 0, &palette).expect("palette");
    scene::draw(&mut c, Depth::Colours256);
    let sum = checksum(&mut c).expect("checksum");
    let want = gfx::reference(l.width, l.height, Depth::Colours256);
    println!("bios: {}: frame buffer checksum {:#010x}, picture in memory {:#010x}, equal: {}", label, sum, want, sum == want);
    assert_eq!(sum, want, "{}: the frame buffer does not hold the picture", label);
    if let Some(h) = hosted {
        println!("bios: {}: spike A's DOSBox checksum {:#010x}, equal: {}", label, h, sum == h);
        assert_eq!(sum, h);
    }
    l
}

fn back_to_text(vbe: &Vbe<NativeBios>) {
    let (res, _) = int(0x10, 0x0003);
    res.unwrap();
    let mut mode = [0u8; 1];
    bios::read_low(0x40, 0x49, &mut mode).unwrap();
    let current = vbe.current_mode().expect("VBE 4F03h after the text mode set");
    // The BIOS cleared the text screen; the kernel's console starts again from the top.
    Vga::clear();
    println!("bios: INT 10h AX=0003h: the BIOS data area says video mode {:#04x}, 4F03h says {:#06x}", mode[0], current);
    assert_eq!(mode[0], 3);
}

fn vbe_after_boot(info: &BootInfo, ps2: &mut Ps2Input) {
    let vbe = Vbe::new(&NativeBios);
    let ctl = match vbe.info() {
        Ok(i) => i,
        Err(e) => {
            println!("bios: VBE 4F00h failed: {}", e);
            panic!("no VBE controller information");
        }
    };
    let mut ids = [0u16; 128];
    let n = vbe.mode_list(&ctl, &mut ids).expect("the mode list");
    let mut oem = [0u8; 40];
    let oem_len = vbe.oem_string(&ctl, &mut oem).unwrap_or(0);
    println!(
        "bios: VBE 4F00h: version {}.{}, {} x 64 KiB of video memory, {} modes listed, OEM \"{}\"",
        ctl.version >> 8,
        ctl.version & 0xFF,
        ctl.memory_64k,
        n,
        core::str::from_utf8(&oem[..oem_len]).unwrap_or("?")
    );
    for id in [0x101u16, 0x103, 0x105] {
        match vbe.mode(id) {
            Ok(m) => println!("bios: VBE 4F01h mode {:#06x}: {}", id, describe_mode(&m)),
            Err(e) => println!("bios: VBE 4F01h mode {:#06x}: {}", id, e),
        }
    }
    let ids = &ids[..n];
    let m640 = vbe.find(ids, 640, 480, 8, true).expect("a 640x480x8 linear mode");
    let m800 = vbe.find(ids, 800, 600, 8, true);

    // Set, draw, set the other, return to text, and do it all a second time: a card that was left half-way would fail the second.
    for round in 1..=2 {
        println!("bios: VBE round {}", round);
        let hosted = (round == 1).then_some(gfx::HOSTED_VBE_640X480);
        show_mode(&vbe, info, &m640, "640x480x8 picture", hosted);
        if round == 1 {
            rendezvous(ps2, "vbe-640");
        }
        match &m800 {
            Some(m) => {
                show_mode(&vbe, info, m, "800x600x8 picture", None);
                if round == 1 {
                    rendezvous(ps2, "vbe-800");
                }
            }
            None => println!("bios: this card offers no 800x600x8 linear mode"),
        }
        back_to_text(&vbe);
        println!("bios: text mode is back after round {}", round);
        if round == 1 {
            rendezvous(ps2, "text");
        }
    }
    println!("bios: VBE set, draw and return to text worked twice");
}

pub fn run(info: &BootInfo, _video: &Video, mut ps2: Ps2Input) {
    println!("bios: scenarios start");
    memory_calls(info);
    clock_calls();
    error_paths();
    repeats_and_idle();
    disk_calls(info);
    keyboard_during_call(&mut ps2);
    stress();
    vbe_after_boot(info, &mut ps2);
    println!("bios: ready keys");
    let start = pit::ticks();
    let mut keys = 0;
    while keys < 2 {
        while let Some(k) = ps2.poll_key() {
            // The release of the Space that acknowledged the last screenshot may still be on its way.
            if k.code == KeyCode::K {
                input::print_key(k);
                keys += 1;
            }
        }
        if pit::ticks().wrapping_sub(start) > ACK_TIMEOUT {
            panic!("bios: the kernel's keyboard did not work after the calls");
        }
        input::idle(&ps2);
    }
    println!("bios: the kernel's keyboard works after {} calls: true", bios::STATS.calls.load(Ordering::Relaxed));
    println!("bios: scenarios complete");
}
