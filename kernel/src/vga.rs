// VGA text mode: 80 by 25 characters at 0xB8000, scrolling.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;
use core::ptr::{read_volatile, write_volatile};

const BASE: *mut u16 = 0xB8000 as *mut u16;
const COLS: usize = 80;
const ROWS: usize = 25;
const ATTR: u16 = 0x0700; // light grey on black

struct State {
    row: usize,
    col: usize,
}

static mut STATE: State = State { row: 0, col: 0 };

pub struct Vga;

impl Vga {
    pub fn clear() {
        for i in 0..COLS * ROWS {
            // SAFETY: the VGA text buffer is 80x25 cells of memory-mapped I/O.
            unsafe { write_volatile(BASE.add(i), ATTR | b' ' as u16) };
        }
        // SAFETY: single processor, and the console is not used from interrupt handlers.
        unsafe {
            (*core::ptr::addr_of_mut!(STATE)).row = 0;
            (*core::ptr::addr_of_mut!(STATE)).col = 0;
        }
    }

    fn put(&mut self, b: u8) {
        // SAFETY: as in `clear`; STATE is only touched through `Vga`.
        let st = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
        match b {
            b'\n' => {
                st.col = 0;
                st.row += 1;
            }
            b'\r' => st.col = 0,
            _ => {
                let b = if (0x20..0x7F).contains(&b) { b } else { b'?' };
                // SAFETY: row and col are kept inside the buffer below.
                unsafe { write_volatile(BASE.add(st.row * COLS + st.col), ATTR | b as u16) };
                st.col += 1;
                if st.col == COLS {
                    st.col = 0;
                    st.row += 1;
                }
            }
        }
        if st.row == ROWS {
            for i in 0..(ROWS - 1) * COLS {
                // SAFETY: both cells are inside the buffer.
                unsafe { write_volatile(BASE.add(i), read_volatile(BASE.add(i + COLS))) };
            }
            for i in (ROWS - 1) * COLS..ROWS * COLS {
                // SAFETY: inside the buffer.
                unsafe { write_volatile(BASE.add(i), ATTR | b' ' as u16) };
            }
            st.row = ROWS - 1;
        }
    }
}

impl fmt::Write for Vga {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(|b| self.put(b));
        Ok(())
    }
}
