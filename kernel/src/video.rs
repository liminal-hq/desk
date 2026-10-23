// What the loader learned about the display before it left real mode: the video block, the BIOS font, and the checks that make the
// frame buffer safe to touch.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Protected mode cannot call the BIOS, so the loader asks VBE while it still can (`boot/stage2.asm`, `video_setup`) and leaves the
// answer at a fixed address named in the boot information block. The layout is in `docs/NATIVE-BOOT.md`. A kernel older than boot
// information version 1 never reads these fields, and this one reads them only when the version says they exist.

use crate::{memory, println, BootInfo};

/// The block stage 2 writes, 32 bytes.
#[repr(C)]
struct VideoBlock {
    /// 0 the loader did not try VBE, 1 a linear 8-bit mode is set, 2 VBE was tried and is not usable.
    state: u32,
    vbe_version: u16,
    /// With state 2: 1 no VBE BIOS, 2 older than VBE 2.0, 3 no 8-bit linear mode, 4 the BIOS refused to set the mode.
    reason: u16,
    mode: u16,
    width: u16,
    height: u16,
    pitch: u16,
    bits_per_pixel: u8,
    memory_model: u8,
    attributes: u16,
    phys_base: u32,
    memory_kib: u32,
    _reserved: u32,
}

/// The highest address the loader's low buffers can be at: the first megabyte.
const LOW_LIMIT: u32 = 0x10_0000;
pub const FONT_BYTES: usize = 4096;

/// A linear frame buffer the loader has already switched the display to.
#[derive(Clone, Copy)]
pub struct Lfb {
    pub mode: u16,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub phys: u32,
}

impl Lfb {
    pub fn bytes(&self) -> u32 {
        self.pitch * self.height
    }
}

/// What the loader reports about the display.
pub struct Video {
    pub lfb: Option<Lfb>,
    pub font: Option<&'static [u8; FONT_BYTES]>,
}

fn reason_text(reason: u16) -> &'static str {
    match reason {
        1 => "no VBE BIOS",
        2 => "older than VBE 2.0",
        3 => "no 8-bit linear frame buffer mode",
        4 => "the BIOS refused to set the mode",
        _ => "unknown",
    }
}

/// Reads the block, checks the frame buffer against the memory map, prints what it found and returns it.
pub fn init(info: &BootInfo) -> Video {
    let mut video = Video {
        lfb: None,
        font: None,
    };
    if info.version < 1 {
        println!(
            "video: boot info version {} has no video block",
            info.version
        );
        return video;
    }
    println!("video: boot info version {}", info.version);
    if info.font != 0 && info.font as u64 + FONT_BYTES as u64 <= u64::from(LOW_LIMIT) {
        // SAFETY: the loader copied 4096 bytes there, below 1 MiB where the frame allocator never gives memory out.
        video.font = Some(unsafe { &*(info.font as *const [u8; FONT_BYTES]) });
        println!("video: 8x16 font, {} bytes at {:#x}", FONT_BYTES, info.font);
    } else {
        println!("video: no 8x16 font from the loader, text mode cannot be restored");
    }
    if info.video == 0 || info.video + core::mem::size_of::<VideoBlock>() as u32 > LOW_LIMIT {
        println!("video: no video block");
        return video;
    }
    // SAFETY: the loader wrote the block at that address (below 1 MiB, outside the kernel and the frame allocator's memory).
    let v = unsafe { &*(info.video as *const VideoBlock) };
    match v.state {
        0 => println!("video: the loader did not try VBE (the screen is in text mode)"),
        2 => println!(
            "video: the loader found no usable VBE mode: {}",
            reason_text(v.reason)
        ),
        1 => {
            println!(
                "video: the loader set VBE {}.{} mode {:#06x}: {}x{}x{}, pitch {}, memory model {}, {} KiB on the card",
                v.vbe_version >> 8,
                v.vbe_version & 0xFF,
                v.mode,
                v.width,
                v.height,
                v.bits_per_pixel,
                v.pitch,
                v.memory_model,
                v.memory_kib
            );
            let lfb = Lfb {
                mode: v.mode,
                width: u32::from(v.width),
                height: u32::from(v.height),
                pitch: u32::from(v.pitch),
                phys: v.phys_base,
            };
            if v.bits_per_pixel != 8 || lfb.pitch < lfb.width {
                println!("video: the mode is not a usable 8-bit linear one, not using it");
            } else if memory::overlaps_ram(info, lfb.phys, lfb.bytes()) {
                println!(
                    "video: frame buffer {:#x}-{:#x} overlaps RAM or the kernel, not using it",
                    lfb.phys,
                    lfb.phys as u64 + lfb.bytes() as u64
                );
            } else {
                memory::reserve(lfb.phys, lfb.bytes());
                println!(
                    "video: mode {:#06x} frame buffer {:#x}-{:#x} overlaps no RAM, heap or kernel; reserved",
                    lfb.mode,
                    lfb.phys,
                    lfb.phys as u64 + lfb.bytes() as u64
                );
                video.lfb = Some(lfb);
            }
        }
        _ => println!("video: unknown state {} in the video block", v.state),
    }
    video
}
