// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::io::{self, BorrowedCursor};
use crate::sys::pal::abi;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin
    }
}

impl io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = unsafe { abi::desk_read(0, buf.as_mut_ptr(), buf.len()) };
        if n < 0 { Err(io::Error::from(io::ErrorKind::Other)) } else { Ok(n as usize) }
    }

    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_, u8>) -> io::Result<()> {
        let n = unsafe { abi::desk_read(0, cursor.as_mut().as_mut_ptr().cast(), cursor.capacity()) };
        if n < 0 {
            return Err(io::Error::from(io::ErrorKind::Other));
        }
        unsafe { cursor.advance(n as usize) };
        Ok(())
    }
}

fn write(fd: i32, buf: &[u8]) -> io::Result<usize> {
    let n = unsafe { abi::desk_write(fd, buf.as_ptr(), buf.len()) };
    if n < 0 { Err(io::Error::from(io::ErrorKind::Other)) } else { Ok(n as usize) }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(1, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(2, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const STDIN_BUF_SIZE: usize = crate::sys::io::DEFAULT_BUF_SIZE;

pub fn is_ebadf(_err: &io::Error) -> bool {
    false
}

pub fn panic_output() -> Option<impl io::Write> {
    Some(Stderr::new())
}
