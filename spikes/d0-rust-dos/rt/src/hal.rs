// The DOS platform's side of the HAL: real-mode BIOS calls through DPMI, and the display (VGA modes, VBE, the linear frame buffer).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything that is specific to DOS lives here and nothing else does: `desk-vga` is given port I/O (`RawPorts`), memory regions
// and a `Bios`, and never learns about INT 10h, a transfer buffer or a DPMI mapping.
//
// - `DosBios` runs a real-mode interrupt with DPMI 0300h. A buffer for the call is a block of conventional memory allocated once
//   (DPMI 0100h), which is also where `read_low` finds the VBE strings and mode list the BIOS leaves in it.
// - `DosDisplay` implements `Display`: the mode list is the two VGA modes plus whatever VBE reports; a mode is set by INT 10h
//   (13h and 12h) or VBE 4F02h with the linear frame buffer bit; the linear frame buffer is mapped with DPMI 0800h. The return
//   to text mode, and the 0801h unmapping, are on the clean-up list, so an exit or a panic puts the screen back.
use crate::dpmi::{self, RealRegs};
use crate::{cleanup, vga};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, Display, FrameBuffer, HalError, MmioRegion, ModeInfo, PixelLayout, Rgb6};
use desk_vga::vbe::Vbe;

/// Paragraphs of the conventional-memory buffer: 512 bytes, a VBE information block.
const BUFFER_PARAGRAPHS: u16 = 32;
static BUFFER_SEGMENT: AtomicU32 = AtomicU32::new(0);

fn buffer_segment() -> Result<u16, HalError> {
    let seg = BUFFER_SEGMENT.load(Ordering::Relaxed);
    if seg != 0 {
        return Ok(seg as u16);
    }
    let (segment, _selector) = dpmi::alloc_dos_memory(BUFFER_PARAGRAPHS).map_err(|_| HalError::OutOfMemory)?;
    BUFFER_SEGMENT.store(segment.into(), Ordering::Relaxed);
    Ok(segment)
}

/// Real-mode software interrupts through the DPMI host.
#[derive(Clone, Copy, Debug, Default)]
pub struct DosBios;

impl Bios for DosBios {
    fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
        let mut r = RealRegs {
            eax: regs.eax,
            ebx: regs.ebx,
            ecx: regs.ecx,
            edx: regs.edx,
            esi: regs.esi,
            edi: regs.edi,
            ebp: regs.ebp,
            es: regs.es,
            ds: regs.ds,
            ..RealRegs::default()
        };
        let mut block = None;
        if let Some(b) = buffer {
            if b.len() > desk_hal::bios::BIOS_BUFFER_MAX {
                return Err(HalError::InvalidArgument);
            }
            let seg = buffer_segment()?;
            let p = dpmi::linear_to_ptr::<u8>(u32::from(seg) << 4);
            // SAFETY: the block is 512 bytes of conventional memory (mapped at linear 0 by the host) and `b.len()` is at most 512.
            unsafe { core::ptr::copy_nonoverlapping(b.as_ptr(), p, b.len()) };
            r.es = seg;
            r.edi = 0;
            block = Some((b, p));
        }
        dpmi::simulate_int(vector, &mut r).map_err(|e| HalError::Platform(e.0))?;
        if let Some((b, p)) = block {
            // SAFETY: as above.
            unsafe { core::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), b.len()) };
        }
        regs.eax = r.eax;
        regs.ebx = r.ebx;
        regs.ecx = r.ecx;
        regs.edx = r.edx;
        regs.esi = r.esi;
        regs.edi = r.edi;
        regs.ebp = r.ebp;
        regs.es = r.es;
        regs.ds = r.ds;
        regs.flags = r.flags;
        Ok(())
    }

    fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
        let start = (u32::from(segment) << 4) + u32::from(offset);
        if u64::from(start) + out.len() as u64 > 0x10_0000 {
            return Err(HalError::InvalidArgument);
        }
        let p = dpmi::linear_to_ptr::<u8>(start);
        for (i, o) in out.iter_mut().enumerate() {
            // SAFETY: inside the first megabyte, which the host maps at linear 0, reached through the 4 GiB data segment.
            *o = unsafe { p.add(i).read_volatile() };
        }
        Ok(())
    }
}

/// The linear address of the live frame buffer mapping, or 0.
static MAPPED: AtomicU32 = AtomicU32::new(0);

/// Back to 80x25 text, then the frame buffer mapping is released. Safe to call twice.
fn restore() {
    vga::leave();
    let linear = MAPPED.swap(0, Ordering::Relaxed);
    if linear != 0 {
        let _ = dpmi::unmap_physical(linear);
    }
    cleanup::forget(restore);
}

fn int10(ax: u32) -> Result<(), HalError> {
    let mut regs = BiosRegs {
        eax: ax,
        ..BiosRegs::default()
    };
    DosBios.int(0x10, &mut regs, None)
}

/// The BIOS mode numbers of the two VGA modes the display offers.
pub const MODE_13H: u16 = 0x13;
pub const MODE_12H: u16 = 0x12;

/// The video hardware seen through the DOS runtime.
pub struct DosDisplay {
    ports: RawPorts,
    modes: Vec<ModeInfo>,
    vbe: bool,
}

impl DosDisplay {
    /// Builds the mode list: VGA modes 13h and 12h, and every graphics mode VBE reports that the Desk has a layout for. A machine
    /// with no VBE simply has the two VGA modes.
    pub fn new() -> DosDisplay {
        // SAFETY: a DPMI client under CWSDPMI runs with the I/O permission bitmap open (docs/D0-RUST-DOS.md); the first access
        // (the palette read-back in the `vgavbe` check) is the evidence, and a host that traps port access emulates it.
        let ports = unsafe { RawPorts::new() };
        let modes = alloc::vec![
            ModeInfo {
                id: MODE_13H,
                width: 320,
                height: 200,
                layout: PixelLayout::Indexed8,
                linear: true,
            },
            ModeInfo {
                id: MODE_12H,
                width: 640,
                height: 480,
                layout: PixelLayout::Planar4,
                linear: false,
            },
        ];
        let mut display = DosDisplay {
            ports,
            modes,
            vbe: false,
        };
        let vbe = Vbe::new(&DosBios);
        if let Ok(info) = vbe.info() {
            let mut ids = [0u16; desk_vga::vbe::MAX_MODES];
            if let Ok(n) = vbe.mode_list(&info, &mut ids) {
                display.vbe = true;
                for id in &ids[..n] {
                    if let Some(m) = vbe.mode(*id).ok().and_then(|m| m.mode_info()) {
                        display.modes.push(m);
                    }
                }
            }
        }
        display
    }

    /// Whether a VBE BIOS answered.
    pub fn has_vbe(&self) -> bool {
        self.vbe
    }

    /// The port bus, for drivers that work on the hardware directly (register programming, the DAC).
    pub fn ports(&self) -> &RawPorts {
        &self.ports
    }

    fn vga_window(&self, info: ModeInfo, pitch: u32) -> FrameBuffer {
        // SAFETY: A0000h to AFFFFh is in the first megabyte, mapped at linear 0 and reached through the 4 GiB data segment; it is
        // video memory for as long as a graphics mode is set and nothing else in the program holds a reference to it.
        let region = unsafe { MmioRegion::new(dpmi::linear_to_ptr::<u8>(0xA0000), 0x10000) };
        FrameBuffer {
            info,
            pitch,
            region,
            phys: 0xA0000,
        }
    }
}

impl Default for DosDisplay {
    fn default() -> DosDisplay {
        DosDisplay::new()
    }
}

impl Display for DosDisplay {
    fn modes(&self) -> &[ModeInfo] {
        &self.modes
    }

    fn set_mode(&mut self, id: u16) -> Result<FrameBuffer, HalError> {
        let info = *self.modes.iter().find(|m| m.id == id).ok_or(HalError::InvalidArgument)?;
        cleanup::forget(restore);
        match id {
            MODE_13H => {
                int10(u32::from(MODE_13H))?;
                cleanup::register(restore);
                Ok(self.vga_window(info, 320))
            }
            MODE_12H => {
                int10(u32::from(MODE_12H))?;
                cleanup::register(restore);
                // The BIOS leaves the EGA-compatible attribute palette; the Desk loads DAC entries 0 to 15 and wants them shown.
                desk_vga::regs::attribute_palette_identity(&self.ports);
                Ok(self.vga_window(info, 80))
            }
            _ => {
                let vbe = Vbe::new(&DosBios);
                let mode = vbe.mode(id)?;
                if !mode.linear() {
                    return Err(HalError::Unsupported);
                }
                let len = (mode.frame_bytes() + 0xFFF) & !0xFFF;
                let linear = dpmi::map_physical(mode.phys_base, len).map_err(|e| HalError::Platform(e.0))?;
                // Registered before the mode is set so that a failure or a panic after it cannot leave graphics on the screen.
                MAPPED.store(linear, Ordering::Relaxed);
                cleanup::register(restore);
                if let Err(e) = vbe.set_mode(id, true) {
                    restore();
                    return Err(e);
                }
                // SAFETY: `linear` is the host's mapping of the frame buffer, `len` bytes, valid until `restore` unmaps it, and
                // reached through the 4 GiB data segment.
                let region = unsafe { MmioRegion::new(dpmi::linear_to_ptr::<u8>(linear), len as usize) };
                Ok(FrameBuffer {
                    info,
                    pitch: u32::from(mode.pitch),
                    region,
                    phys: mode.phys_base,
                })
            }
        }
    }

    fn restore_text_mode(&mut self) {
        restore();
    }

    fn set_palette(&mut self, first: u8, colours: &[Rgb6]) -> Result<(), HalError> {
        desk_vga::set_palette(&self.ports, first, colours)
    }

    fn palette(&self, first: u8, out: &mut [Rgb6]) -> Result<(), HalError> {
        desk_vga::read_palette(&self.ports, first, out)
    }

    fn wait_vretrace(&self) -> Result<(), HalError> {
        desk_vga::wait_vretrace(&self.ports)
    }
}
