// INT 21h: console and file I/O, the exit call.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// CWSDPMI does not translate pointers for a protected-mode INT 21h: the call is reflected to real mode with the registers as they
// are, so DS:EDX names whatever real-mode DS and the low 16 bits of EDX happen to address (in the tests: CWSDPMI's own data).
// Every DOS call that takes a pointer therefore goes through a transfer buffer in conventional memory and the DPMI real-mode
// call (0300h), as DJGPP's C library does: copy the data into the buffer, call DOS with a real-mode segment:offset, copy the
// result out. The buffer is 32 KiB, so larger reads and writes loop. Calls that take no pointer (AH=4Ch, AH=3Eh) are made
// directly.
use crate::dpmi::{self, RealRegs};
use alloc::vec::Vec;
use core::arch::asm;
use core::sync::atomic::{AtomicU32, Ordering};

/// A DOS error code (the AX returned with the carry flag set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DosError(pub u16);

/// The size of the transfer buffer in paragraphs (32 KiB).
const TB_PARAGRAPHS: u16 = 0x800;
pub const TB_SIZE: usize = TB_PARAGRAPHS as usize * 16;

/// Real-mode segment of the transfer buffer, 0 until first use.
static TB_SEGMENT: AtomicU32 = AtomicU32::new(0);

fn tb_segment() -> u16 {
    let seg = TB_SEGMENT.load(Ordering::Relaxed);
    if seg != 0 {
        return seg as u16;
    }
    match dpmi::alloc_dos_memory(TB_PARAGRAPHS) {
        Ok((segment, _selector)) => {
            TB_SEGMENT.store(segment as u32, Ordering::Relaxed);
            segment
        }
        Err(_) => die(b"dos: no conventional memory for the transfer buffer\r\n"),
    }
}

/// The transfer buffer as a pointer in the flat data segment (conventional memory is mapped at linear 0).
fn tb_ptr() -> *mut u8 {
    dpmi::linear_to_ptr::<u8>((tb_segment() as u32) << 4)
}

/// Runs INT 21h in real mode with `regs`, and turns the carry flag into an error.
fn rm21(regs: &mut RealRegs) -> Result<(), DosError> {
    dpmi::simulate_int(0x21, regs).map_err(|e| DosError(e.0))?;
    if regs.flags & 1 != 0 {
        Err(DosError(regs.eax as u16))
    } else {
        Ok(())
    }
}

/// Ends the program with `code` after running the clean-up list (restores hooked interrupts and the video mode).
pub fn exit(code: i32) -> ! {
    crate::cleanup::run_all();
    exit_now(code)
}

/// Ends the program at once: AH=4Ch with the status in AL. The host releases the program's memory and descriptors.
pub fn exit_now(code: i32) -> ! {
    loop {
        // SAFETY: AH=4Ch takes no pointer and does not return; the loop is for the impossible case that it does.
        unsafe { asm!("int 0x21", in("eax") 0x4C00 | (code as u32 & 0xFF), options(nostack)) };
    }
}

/// Reports a failure before the runtime is up, and exits with status 255. No allocation, no formatting.
pub fn die(msg: &[u8]) -> ! {
    let _ = write(1, msg);
    exit_now(255)
}

/// AH=09h: print a `$`-terminated string to the console through the transfer buffer. AH=40h is the one to use for anything binary.
pub fn print_dollar(s: &[u8]) {
    let n = s.len().min(TB_SIZE - 1);
    // SAFETY: the buffer is TB_SIZE bytes and n + 1 <= TB_SIZE.
    unsafe {
        core::ptr::copy_nonoverlapping(s.as_ptr(), tb_ptr(), n);
        tb_ptr().add(n).write(b'$');
    }
    let mut r = RealRegs {
        eax: 0x0900,
        ds: tb_segment(),
        ..RealRegs::default()
    };
    let _ = rm21(&mut r);
}

/// AH=40h: write to a handle through the transfer buffer. Returns the bytes DOS reports written (all, unless the disk is full).
pub fn write(handle: u16, buf: &[u8]) -> Result<usize, DosError> {
    let mut done = 0;
    while done < buf.len() {
        let n = (buf.len() - done).min(TB_SIZE);
        // SAFETY: the buffer is TB_SIZE bytes of conventional memory and n <= TB_SIZE.
        unsafe { core::ptr::copy_nonoverlapping(buf.as_ptr().add(done), tb_ptr(), n) };
        let mut r = RealRegs {
            eax: 0x4000,
            ebx: handle as u32,
            ecx: n as u32,
            ds: tb_segment(),
            ..RealRegs::default()
        };
        rm21(&mut r)?;
        let w = r.eax as usize & 0xFFFF;
        done += w;
        if w < n {
            break;
        }
    }
    Ok(done)
}

/// Writes everything, or fails.
pub fn write_all(handle: u16, buf: &[u8]) -> Result<(), DosError> {
    if write(handle, buf)? == buf.len() {
        Ok(())
    } else {
        Err(DosError(0x1D))
    }
}

/// AH=3Fh: read from a handle through the transfer buffer, looping until `buf` is full or the file ends. Returns 0 at end of file.
pub fn read(handle: u16, buf: &mut [u8]) -> Result<usize, DosError> {
    let mut done = 0;
    while done < buf.len() {
        let n = (buf.len() - done).min(TB_SIZE);
        let mut r = RealRegs {
            eax: 0x3F00,
            ebx: handle as u32,
            ecx: n as u32,
            ds: tb_segment(),
            ..RealRegs::default()
        };
        rm21(&mut r)?;
        let got = r.eax as usize & 0xFFFF;
        // SAFETY: `got <= n <= TB_SIZE` bytes were read into the buffer, and `buf[done..]` has room for n.
        unsafe { core::ptr::copy_nonoverlapping(tb_ptr(), buf.as_mut_ptr().add(done), got) };
        done += got;
        if got < n {
            break;
        }
    }
    Ok(done)
}

/// Copies a file name into the buffer as an ASCIIZ string and returns its length including the NUL.
fn put_path(path: &str) -> Result<(), DosError> {
    if path.len() + 1 > TB_SIZE {
        return Err(DosError(3));
    }
    // SAFETY: bounds checked above.
    unsafe {
        core::ptr::copy_nonoverlapping(path.as_ptr(), tb_ptr(), path.len());
        tb_ptr().add(path.len()).write(0);
    }
    Ok(())
}

/// How to open a file.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Read,
    /// Create or truncate, for writing
    Create,
}

/// An open file. Closed on drop.
pub struct File {
    handle: u16,
}

impl File {
    /// AH=3Dh (open for reading) or AH=3Ch (create).
    pub fn open(path: &str, mode: Mode) -> Result<File, DosError> {
        put_path(path)?;
        let ax = match mode {
            Mode::Read => 0x3D00,
            Mode::Create => 0x3C00,
        };
        let mut r = RealRegs {
            eax: ax,
            ecx: 0,
            ds: tb_segment(),
            ..RealRegs::default()
        };
        rm21(&mut r)?;
        Ok(File { handle: r.eax as u16 })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, DosError> {
        read(self.handle, buf)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), DosError> {
        write_all(self.handle, buf)
    }

    /// AH=42h: seek from the start; returns the new position.
    pub fn seek_start(&mut self, pos: u32) -> Result<u32, DosError> {
        self.seek(0, pos)
    }

    /// The length (seek to the end and back).
    pub fn len(&mut self) -> Result<u32, DosError> {
        let here = self.seek(1, 0)?;
        let end = self.seek(2, 0)?;
        self.seek(0, here)?;
        Ok(end)
    }

    pub fn is_empty(&mut self) -> Result<bool, DosError> {
        Ok(self.len()? == 0)
    }

    fn seek(&mut self, whence: u32, offset: u32) -> Result<u32, DosError> {
        let mut r = RealRegs {
            eax: 0x4200 | whence,
            ebx: self.handle as u32,
            ecx: offset >> 16,
            edx: offset & 0xFFFF,
            ..RealRegs::default()
        };
        rm21(&mut r)?;
        // DX:AX is the new position.
        Ok(((r.edx & 0xFFFF) << 16) | (r.eax & 0xFFFF))
    }

    /// Reads the whole file into a vector, calling `read` with at most `chunk` bytes at a time (0 = everything in one call).
    pub fn read_to_vec(&mut self, chunk: usize) -> Result<Vec<u8>, DosError> {
        let len = self.len()? as usize;
        let mut v = alloc::vec![0u8; len];
        let mut done = 0;
        while done < len {
            let want = if chunk == 0 { len - done } else { chunk.min(len - done) };
            let n = self.read(&mut v[done..done + want])?;
            if n == 0 {
                break;
            }
            done += n;
        }
        v.truncate(done);
        Ok(v)
    }

    /// The DOS handle.
    pub fn handle(&self) -> u16 {
        self.handle
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let mut r = RealRegs {
            eax: 0x3E00,
            ebx: self.handle as u32,
            ..RealRegs::default()
        };
        let _ = rm21(&mut r);
    }
}

/// AH=41h: delete a file.
pub fn remove(path: &str) -> Result<(), DosError> {
    put_path(path)?;
    let mut r = RealRegs {
        eax: 0x4100,
        ds: tb_segment(),
        ..RealRegs::default()
    };
    rm21(&mut r)
}

/// What a protected-mode INT 21h does with a pointer: AH=40h with DS:EDX naming `buf`, no transfer buffer. Returns the count DOS
/// reports. The data DOS actually wrote is whatever sat at the real-mode DS:DX, not `buf`; this exists to demonstrate that.
pub fn write_direct_unsafe(handle: u16, buf: &[u8]) -> Result<usize, DosError> {
    let ax: u32;
    let carry: u32;
    // SAFETY: the call is made with a valid handle and length; DOS reads the bytes it is pointed at, which are in memory.
    unsafe {
        asm!(
            "push ebx",
            "mov ebx, {h}",
            "int 0x21",
            "sbb {c}, {c}",
            "pop ebx",
            h = in(reg) handle as u32,
            c = out(reg) carry,
            inout("eax") 0x4000u32 => ax,
            in("ecx") buf.len() as u32,
            in("edx") buf.as_ptr() as usize as u32,
        );
    }
    if carry != 0 {
        Err(DosError(ax as u16))
    } else {
        Ok(ax as usize & 0xFFFF)
    }
}
