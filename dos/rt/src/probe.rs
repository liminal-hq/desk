// Platform probes: what the machine under the program is, found out without changing it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Five questions, each answered by something the machine documents, and none of them leaves the machine different:
//
// - `video`: the video adapter's class, from the BIOS (INT 10h AX=1A00h, then AH=12h BL=10h, then the INT 11h equipment word).
// - `cpu`: the processor family from the EFLAGS AC (bit 18) and ID (bit 21) bits. CPUID is never executed, so a processor that has
//   it is reported only as "later than a 486".
// - `x87`: whether a floating-point unit answers, with the control word it had put back exactly as it was found (the float policy,
//   R138, is not settled by a probe).
// - `memory`: the DPMI host's free-memory report (0500h).
// - `joystick`: the game port at 0201h, read back through a bounded poll.
//
// The sound hardware (OPL, speaker, Sound Blaster) and the mouse are not probed here: drivers over `PortIo` do that.
use crate::dpmi;
use crate::hal::DosBios;
use core::arch::asm;
use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, PortIo};

/// What kind of video adapter the BIOS says is the active one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoClass {
    /// Monochrome Display Adapter: 80x25 text only, at B0000h.
    Mda,
    /// Colour Graphics Adapter (or something that answers as one): 320x200 and 640x200 graphics, no 640x350 modes.
    Cga,
    /// Enhanced Graphics Adapter, colour or monochrome: 640x350 in 16 colours, no 256-colour mode 13h.
    Ega,
    /// Multi-Color Graphics Array (PS/2 models 25 and 30): mode 13h, but not the 16-colour 640x480 mode 12h.
    Mcga,
    /// Video Graphics Array, or better: modes 12h and 13h are there.
    Vga,
    /// The BIOS did not say, or named something this table does not know (a plasma panel, a PGC, no display).
    Unknown,
}

/// Which BIOS question settled the class, from the most exact to the least.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoSource {
    /// INT 10h AX=1A00h: the display combination code (PS/2 and VGA BIOSes).
    DisplayCombination,
    /// INT 10h AH=12h BL=10h: "return EGA information" (an EGA BIOS, which has no 1Ah).
    EgaInformation,
    /// INT 11h: the equipment word's initial video mode, bits 5 and 4 (any PC BIOS; it cannot tell an EGA from a VGA).
    Equipment,
    /// Nothing answered.
    None,
}

/// The result of `video`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Video {
    pub class: VideoClass,
    pub source: VideoSource,
    /// The raw code the source gave: the display combination code, the EGA information's BH (0 colour, 1 monochrome), or the
    /// equipment word's bits 5 and 4.
    pub code: u8,
}

/// The class a display combination code of INT 10h AX=1A00h stands for. The codes: 00 none, 01 MDA, 02 CGA, 04 EGA colour,
/// 05 EGA monochrome, 06 PGC, 07 VGA monochrome, 08 VGA colour, 0A MCGA digital colour, 0B MCGA analogue monochrome, 0C MCGA
/// analogue colour, FFh unknown; 03 and 09 are reserved.
pub fn class_of_display_code(code: u8) -> VideoClass {
    match code {
        0x01 => VideoClass::Mda,
        0x02 => VideoClass::Cga,
        0x04 | 0x05 => VideoClass::Ega,
        0x07 | 0x08 => VideoClass::Vga,
        0x0A..=0x0C => VideoClass::Mcga,
        _ => VideoClass::Unknown,
    }
}

/// The class the initial video mode in the equipment word (bits 5 and 4) stands for: 11 is 80x25 monochrome (an MDA), 01 and 10
/// are colour 40x25 and 80x25 (a CGA). 00 means the adapter has its own BIOS (an EGA or a VGA), which the word cannot name.
pub fn class_of_equipment(word: u16) -> VideoClass {
    match word >> 4 & 3 {
        3 => VideoClass::Mda,
        1 | 2 => VideoClass::Cga,
        _ => VideoClass::Unknown,
    }
}

fn bios(vector: u8, eax: u32, ebx: u32) -> Option<BiosRegs> {
    let mut regs = BiosRegs {
        eax,
        ebx,
        ..BiosRegs::default()
    };
    DosBios.int(vector, &mut regs, None).ok().map(|()| regs)
}

/// The video adapter's class. Asks the VGA BIOS for the display combination code, then an EGA BIOS for its information, then
/// takes the equipment word; the first that answers wins, and `source` says which. A VGA or EGA BIOS that is present but wrong
/// about itself is believed.
pub fn video() -> Video {
    if let Some(r) = bios(0x10, 0x1A00, 0) {
        // AL is 1Ah if the function exists; BL is the active display, BH the alternate (00 if there is none).
        if r.eax as u8 == 0x1A {
            let active = r.ebx as u8;
            let alternate = (r.ebx >> 8) as u8;
            let code = if class_of_display_code(active) == VideoClass::Unknown {
                alternate
            } else {
                active
            };
            let class = class_of_display_code(code);
            if class != VideoClass::Unknown {
                return Video {
                    class,
                    source: VideoSource::DisplayCombination,
                    code,
                };
            }
        }
    }
    if let Some(r) = bios(0x10, 0x1200, 0x0010) {
        // BL stays 10h if there is no EGA BIOS; otherwise BH is 0 for a colour and 1 for a monochrome display (BL is then the memory size).
        if r.ebx as u8 != 0x10 {
            return Video {
                class: VideoClass::Ega,
                source: VideoSource::EgaInformation,
                code: (r.ebx >> 8) as u8,
            };
        }
    }
    if let Some(r) = bios(0x11, 0, 0) {
        let class = class_of_equipment(r.eax as u16);
        if class != VideoClass::Unknown {
            return Video {
                class,
                source: VideoSource::Equipment,
                code: (r.eax >> 4) as u8 & 3,
            };
        }
    }
    Video {
        class: VideoClass::Unknown,
        source: VideoSource::None,
        code: 0,
    }
}

/// The processor family, as far as EFLAGS can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cpu {
    /// The AC bit cannot be set: an 80386 (or a 286 or earlier, which cannot run this program).
    I386,
    /// AC can be set and ID cannot: an 80486 without CPUID (the early ones).
    I486,
    /// ID can be toggled: CPUID exists, so a late 486 or a Pentium or newer. CPUID itself is not run, so this is as far as it goes.
    Later,
}

const EFLAGS_AC: u32 = 1 << 18;
const EFLAGS_ID: u32 = 1 << 21;

/// Whether `bit` of EFLAGS can be changed, with EFLAGS left exactly as it was found.
fn eflags_bit_changes(bit: u32) -> bool {
    let changed: u32;
    // SAFETY: PUSHFD, POP, PUSH and POPFD touch only the stack and EFLAGS; the original EFLAGS value is pushed back and popped
    // before the block ends, so AC, ID and every other flag are as found. The one change lasts a few instructions, in which
    // nothing but aligned stack operands is touched.
    unsafe {
        asm!(
            "pushfd",
            "pop {orig}",
            "mov {t}, {orig}",
            "xor {t}, {mask}",
            "push {t}",
            "popfd",
            "pushfd",
            "pop {t}",
            "push {orig}",
            "popfd",
            "xor {t}, {orig}",
            "and {t}, {mask}",
            orig = out(reg) _,
            t = out(reg) changed,
            mask = in(reg) bit,
        );
    }
    changed != 0
}

/// The processor family, by toggling EFLAGS AC and then ID and putting both back.
pub fn cpu() -> Cpu {
    if !eflags_bit_changes(EFLAGS_AC) {
        Cpu::I386
    } else if !eflags_bit_changes(EFLAGS_ID) {
        Cpu::I486
    } else {
        Cpu::Later
    }
}

/// What the x87 probe found. The probe restores the unit's environment (control, status and tag words) as it found it. That values
/// on the x87 stack survive it is a property of FNINIT (which only marks the registers empty) and FLDENV on real hardware and in
/// DOSBox, not of the asm contract. And the "no unit" branch is unreachable while `_start` runs an unguarded FNINIT, which would
/// fault first on a machine with no FPU and CR0.EM set (R138).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct X87 {
    /// A floating-point unit answered: after FNINIT, the status word's exception and busy bits are clear and the control word has
    /// the documented power-on value in its masks (the low six bits set, the reserved bit 12 clear in the 387 and later).
    pub present: bool,
    /// The control word the unit held before the probe, and holds again after it. `None` if there is no unit.
    pub control_word: Option<u16>,
}

/// Whether an x87 is there. The environment (control, status and tag words, and the pointers) is saved first with FNSTENV and put
/// back with FLDENV, so the control word, the status word and the tag word are what they were, and the probe settles nothing about
/// the float policy. The values on the x87 stack are not touched by FNINIT, only their tags, which FLDENV restores. FNINIT, FNSTSW
/// and FNSTCW are the "no wait" forms, which do not hang on a machine with no unit.
pub fn x87() -> X87 {
    const SENTINEL: u16 = 0x5A5A;
    let mut env = [0u32; 7];
    let mut sw: u16 = SENTINEL;
    let mut cw: u16 = SENTINEL;
    // SAFETY: FNSTENV stores 28 bytes through `env` (32-bit protected-mode layout, exactly [u32; 7]); FNSTSW and FNSTCW store two
    // bytes each; the clobbers declare the x87 registers, whose tags FNINIT changes.
    unsafe {
        asm!(
            "fnstenv [{env}]",
            "fninit",
            "fnstsw word ptr [{sw}]",
            "fnstcw word ptr [{cw}]",
            env = in(reg) env.as_mut_ptr(),
            sw = in(reg) &mut sw,
            cw = in(reg) &mut cw,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    let present = sw & 0xFF == 0 && cw & 0x103F == 0x003F;
    if !present {
        return X87 {
            present,
            control_word: None,
        };
    }
    // SAFETY: FLDENV loads the 28 bytes FNSTENV stored above from `env`; it is run only when a unit answered.
    unsafe {
        asm!(
            "fldenv [{env}]",
            env = in(reg) env.as_ptr(),
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    X87 {
        present,
        control_word: Some(env[0] as u16),
    }
}

/// The DPMI host's free-memory report, as far as the host fills it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    /// The largest block that can be allocated, in bytes.
    pub largest_free_block: Option<u32>,
    /// Total free pages (4 KiB each), as the host counts them.
    pub free_pages: Option<u32>,
}

/// DPMI 0500h: free memory information. A field the host does not know comes back as 0xFFFFFFFF and is `None` here.
pub fn memory() -> Option<Memory> {
    let info = dpmi::free_memory_info().ok()?;
    Some(Memory {
        largest_free_block: info.largest_free_block,
        free_pages: info.free_pages,
    })
}

/// The game port.
pub const JOYSTICK_PORT: u16 = 0x201;
/// The most reads of the port one probe makes. An axis one-shot of a stick at full scale runs for about 1.1 ms; a port read is
/// about 1 us on ISA (more on the emulators), so 20,000 reads cover it with room, and the loop ends there even if nothing falls.
pub const JOYSTICK_POLLS: u32 = 20_000;

/// What the game port did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Joystick {
    /// The byte read straight after the trigger write. An absent port (no card, or a floating bus) reads 0xFF.
    pub first: u8,
    /// Bits 0 to 3: axis A X, A Y, B X, B Y fell from 1 to 0 within the bound, as a stick's timing one-shot does when it ends.
    /// Bits 0 and 1 are the first stick, 2 and 3 the second.
    pub axes: u8,
    /// How many reads the poll made: at most `JOYSTICK_POLLS`.
    pub reads: u32,
}

impl Joystick {
    /// An axis of the first stick answered.
    pub fn stick_a(&self) -> bool {
        self.axes & 0b0011 != 0
    }

    /// An axis of the second stick answered.
    pub fn stick_b(&self) -> bool {
        self.axes & 0b1100 != 0
    }
}

/// Writes to the game port, which fires the four one-shots, and polls until every axis bit that was high has fallen or
/// `JOYSTICK_POLLS` reads have been made, whichever is first. The write's value is not used by the hardware. A port with nothing
/// behind it reads 0xFF, the axis bits never fall, and `axes` is 0.
pub fn joystick<P: PortIo>(ports: &P) -> Joystick {
    ports.outb(JOYSTICK_PORT, 0xFF);
    let first = ports.inb(JOYSTICK_PORT);
    let live = first & 0x0F;
    let mut axes = 0u8;
    let mut reads = 1u32;
    while reads < JOYSTICK_POLLS && axes != live {
        axes |= live & !ports.inb(JOYSTICK_PORT);
        reads += 1;
    }
    Joystick { first, axes, reads }
}

/// `joystick` over the machine's own ports.
pub fn joystick_raw() -> Joystick {
    // SAFETY: a DPMI client under CWSDPMI runs with the I/O permission bitmap open (docs/D0-RUST-DOS.md); port 0201h is the game
    // port, and the one write to it only triggers the axis timers.
    let ports = unsafe { RawPorts::new() };
    joystick(&ports)
}
