// A text screen: 80x25 text mode, the cell buffer, the cursor and the keyboard, put back at exit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The mode switch, the cursor and the keyboard are BIOS calls (INT 10h and INT 16h) through `DosBios`. The cells are video memory in
// the first megabyte, reached as `vga::framebuffer` reaches A0000h: linear 0 through the 4 GiB data segment, with no mapping of
// its own. A monochrome adapter (`VideoClass::Mda`) uses mode 7 and B0000h; every other class uses mode 3 and B8000h.
//
// `Text::enter` registers `restore` with the clean-up list before it changes anything and rolls back if the list is full. `restore`
// puts the mode and the cursor shape back to what was found and is safe to call twice; drop, exit and a panic all run it. Only one
// `Text` can exist at a time: a second `enter` is refused (`TextError::Active`), because two of them would each remember a
// different "found" state and the second restore would undo the first one's.
use crate::cleanup;
use crate::dpmi;
use crate::hal::DosBios;
use crate::probe::{self, VideoClass};
use core::sync::atomic::{AtomicU32, Ordering};
use desk_hal::{Bios, BiosRegs, MmioRegion};

/// Columns and rows of the text screen.
pub const COLUMNS: usize = 80;
pub const ROWS: usize = 25;
/// Bytes of the cell buffer: a character and an attribute for each cell.
pub const BYTES: usize = COLUMNS * ROWS * 2;

/// The cursor shape that hides the cursor (start line with bit 5 set).
const CURSOR_OFF: u16 = 0x2000;

/// Why a text screen operation did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextError {
    /// A `Text` is already active.
    Active,
    /// The clean-up list is full; the mode was not changed.
    CleanupFull,
    /// The BIOS call did not complete.
    Bios,
    /// The coordinates, or the string, fall outside 80 by 25 cells; nothing was written.
    OutOfRange,
}

/// A key taken from the BIOS type-ahead buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    /// The ASCII code (0 or 0E0h for an extended key).
    pub ascii: u8,
    /// The scan code.
    pub scan: u8,
}

// What `restore` puts back. ACTIVE is 0 when no Text is held.
static ACTIVE: AtomicU32 = AtomicU32::new(0);
static FOUND_MODE: AtomicU32 = AtomicU32::new(0);
static FOUND_CURSOR: AtomicU32 = AtomicU32::new(0);

fn call(vector: u8, mut regs: BiosRegs) -> Result<BiosRegs, TextError> {
    DosBios.int(vector, &mut regs, None).map_err(|_| TextError::Bios)?;
    Ok(regs)
}

fn int(vector: u8, eax: u32, ecx: u32) -> Result<BiosRegs, TextError> {
    call(
        vector,
        BiosRegs {
            eax,
            ecx,
            ..BiosRegs::default()
        },
    )
}

/// The current video mode (INT 10h AH=0Fh), without the "do not clear" bit.
pub fn video_mode() -> Result<u8, TextError> {
    Ok(int(0x10, 0x0F00, 0)?.eax as u8 & 0x7F)
}

/// The cursor shape of page 0 (INT 10h AH=03h, BH=0): start line in the high byte, end line in the low byte.
pub fn cursor_shape() -> Result<u16, TextError> {
    Ok(int(0x10, 0x0300, 0)?.ecx as u16)
}

/// Sets the cursor shape of page 0 (INT 10h AH=01h), as `cursor_shape` reads it.
pub fn set_cursor_shape(shape: u16) -> Result<(), TextError> {
    int(0x10, 0x0100, u32::from(shape)).map(|_| ())
}

/// Writes one cell through the BIOS (INT 10h AH=02h to place the cursor, AH=09h to write the character and attribute), page 0. It
/// does not use this module's pointer to the buffer, so a test can see that the BIOS and `get` and `put` agree on where the buffer
/// is. It moves the cursor.
pub fn bios_write_cell(x: usize, y: usize, ch: u8, attr: u8) -> Result<(), TextError> {
    if x >= COLUMNS || y >= ROWS {
        return Err(TextError::OutOfRange);
    }
    let edx = ((y as u32) << 8) | x as u32;
    call(
        0x10,
        BiosRegs {
            eax: 0x0200,
            edx,
            ..BiosRegs::default()
        },
    )?;
    call(
        0x10,
        BiosRegs {
            eax: 0x0900 | u32::from(ch),
            ebx: u32::from(attr),
            ecx: 1,
            ..BiosRegs::default()
        },
    )
    .map(|_| ())
}

/// Reads one cell through the BIOS (INT 10h AH=02h, then AH=08h), page 0: the character and the attribute. Moves the cursor.
pub fn bios_read_cell(x: usize, y: usize) -> Result<(u8, u8), TextError> {
    if x >= COLUMNS || y >= ROWS {
        return Err(TextError::OutOfRange);
    }
    let edx = ((y as u32) << 8) | x as u32;
    call(
        0x10,
        BiosRegs {
            eax: 0x0200,
            edx,
            ..BiosRegs::default()
        },
    )?;
    let r = call(
        0x10,
        BiosRegs {
            eax: 0x0800,
            ..BiosRegs::default()
        },
    )?;
    Ok((r.eax as u8, (r.eax >> 8) as u8))
}

/// Puts a key into the BIOS type-ahead buffer (INT 16h AH=05h). False if the buffer is full.
pub fn stuff_key(ascii: u8, scan: u8) -> bool {
    int(0x16, 0x0500, (u32::from(scan) << 8) | u32::from(ascii)).is_ok_and(|r| r.eax as u8 == 0)
}

fn set_mode(mode: u8) -> Result<(), TextError> {
    int(0x10, u32::from(mode), 0).map(|_| ())
}

/// Puts the mode and the cursor shape back to what `Text::enter` found. Safe to call twice, and when no `Text` was ever entered.
/// A mode set clears the screen and homes the cursor, so what was on the screen before `enter` is gone, whatever mode was found (a
/// graphics mode comes back empty too): only the mode and the cursor shape are restored, not the contents or the cursor position.
fn restore() {
    if ACTIVE.swap(0, Ordering::SeqCst) != 0 {
        let _ = set_mode(FOUND_MODE.load(Ordering::SeqCst) as u8);
        let _ = set_cursor_shape(FOUND_CURSOR.load(Ordering::SeqCst) as u16);
    }
    cleanup::forget(restore);
}

/// The 80 by 25 text screen. Dropping it puts the video mode and the cursor shape back.
pub struct Text {
    cells: MmioRegion,
    mono: bool,
}

impl Text {
    /// Switches to 80 by 25 text: mode 7 at B0000h on a monochrome adapter, mode 3 at B8000h on every other class. The mode and the
    /// cursor shape found are put back by drop, at exit or on a panic. The cursor is hidden. The screen is cleared by the mode set.
    pub fn enter() -> Result<Text, TextError> {
        if ACTIVE.load(Ordering::SeqCst) != 0 {
            return Err(TextError::Active);
        }
        let mono = probe::video().class == VideoClass::Mda;
        let found_mode = video_mode()?;
        let found_cursor = cursor_shape()?;
        cleanup::try_register(restore).map_err(|_| TextError::CleanupFull)?;
        FOUND_MODE.store(u32::from(found_mode), Ordering::SeqCst);
        FOUND_CURSOR.store(u32::from(found_cursor), Ordering::SeqCst);
        ACTIVE.store(1, Ordering::SeqCst);
        if let Err(e) = set_mode(if mono { 7 } else { 3 }).and_then(|()| set_cursor_shape(CURSOR_OFF)) {
            restore();
            return Err(e);
        }
        let base: u32 = if mono { 0xB0000 } else { 0xB8000 };
        // SAFETY: B0000h to B8FFFh is in the first megabyte, mapped at linear 0 and reached through the 4 GiB data segment; the
        // `BYTES` at the base for the mode set above are the text buffer, and nothing else in the program holds a reference to it.
        let cells = unsafe { MmioRegion::new(dpmi::linear_to_ptr::<u8>(base), BYTES) };
        Ok(Text { cells, mono })
    }

    /// Whether the monochrome adapter's mode 7 is in use (otherwise mode 3).
    pub fn is_mono(&self) -> bool {
        self.mono
    }

    /// The mode set: 7 on a monochrome adapter, 3 otherwise.
    pub fn mode(&self) -> u8 {
        if self.mono {
            7
        } else {
            3
        }
    }

    /// The segment of the text buffer: 0B000h or 0B800h.
    pub fn segment(&self) -> u16 {
        if self.mono {
            0xB000
        } else {
            0xB800
        }
    }

    fn offset(x: usize, y: usize) -> Result<usize, TextError> {
        if x < COLUMNS && y < ROWS {
            Ok((y * COLUMNS + x) * 2)
        } else {
            Err(TextError::OutOfRange)
        }
    }

    /// Writes one cell.
    pub fn put(&mut self, x: usize, y: usize, ch: u8, attr: u8) -> Result<(), TextError> {
        let at = Self::offset(x, y)?;
        self.cells.write8(at, ch).map_err(|_| TextError::OutOfRange)?;
        self.cells.write8(at + 1, attr).map_err(|_| TextError::OutOfRange)
    }

    /// Reads one cell: the character, then the attribute.
    pub fn get(&self, x: usize, y: usize) -> Result<(u8, u8), TextError> {
        let at = Self::offset(x, y)?;
        let ch = self.cells.read8(at).map_err(|_| TextError::OutOfRange)?;
        let attr = self.cells.read8(at + 1).map_err(|_| TextError::OutOfRange)?;
        Ok((ch, attr))
    }

    /// Writes the bytes of `s` from (x, y) along the row, each as a code page 437 character (bytes outside the printable ones are
    /// written as given). A string that would run past the end of the row is refused whole, and nothing is written.
    pub fn write_str(&mut self, x: usize, y: usize, s: &str, attr: u8) -> Result<(), TextError> {
        Self::offset(x, y)?;
        if x + s.len() > COLUMNS {
            return Err(TextError::OutOfRange);
        }
        for (i, b) in s.bytes().enumerate() {
            self.put(x + i, y, b, attr)?;
        }
        Ok(())
    }

    /// Sets every cell.
    pub fn fill(&mut self, ch: u8, attr: u8) -> Result<(), TextError> {
        for i in 0..COLUMNS * ROWS {
            self.cells.write8(i * 2, ch).map_err(|_| TextError::OutOfRange)?;
            self.cells.write8(i * 2 + 1, attr).map_err(|_| TextError::OutOfRange)?;
        }
        Ok(())
    }

    /// Blanks the screen: spaces in light grey on black.
    pub fn clear(&mut self) -> Result<(), TextError> {
        self.fill(b' ', 0x07)
    }

    /// Takes a key from the BIOS type-ahead buffer if there is one (INT 16h AH=01h to test, AH=00h to take). Never blocks on a
    /// key that is there. It uses the legacy functions, which on some BIOSes with a 101-key keyboard skip keys that have no legacy
    /// code (F11 and F12 are the usual ones) and could then wait for the next key: the extended functions (AH=11h and AH=10h)
    /// would avoid that and are not used here, so a program that must see those keys should not rely on this.
    pub fn poll_key(&self) -> Option<Key> {
        let test = int(0x16, 0x0100, 0).ok()?;
        // ZF (bit 6 of the flags) is set when the buffer is empty.
        if test.flags & 0x40 != 0 {
            return None;
        }
        let key = int(0x16, 0x0000, 0).ok()?;
        Some(Key {
            ascii: key.eax as u8,
            scan: (key.eax >> 8) as u8,
        })
    }
}

impl Drop for Text {
    fn drop(&mut self) {
        restore();
    }
}
