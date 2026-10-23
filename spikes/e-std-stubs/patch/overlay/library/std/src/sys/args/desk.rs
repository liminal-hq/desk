// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ffi::{OsStr, OsString};
use crate::sys::pal::abi;
use crate::{fmt, slice};

pub fn args() -> Args {
    Args { i: 0, n: unsafe { abi::desk_arg_count() } }
}

pub struct Args {
    i: usize,
    n: usize,
}

impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rest = Args { i: self.i, n: self.n };
        f.debug_list().entries(rest).finish()
    }
}

impl Iterator for Args {
    type Item = OsString;

    fn next(&mut self) -> Option<OsString> {
        if self.i >= self.n {
            return None;
        }
        let mut len = 0usize;
        let p = unsafe { abi::desk_arg(self.i, &mut len) };
        self.i += 1;
        if p.is_null() {
            return None;
        }
        let bytes = unsafe { slice::from_raw_parts(p, len) };
        Some(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }.to_os_string())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.n - self.i, Some(self.n - self.i))
    }
}

impl ExactSizeIterator for Args {
    fn len(&self) -> usize {
        self.n - self.i
    }
}

impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<OsString> {
        if self.i >= self.n {
            return None;
        }
        self.n -= 1;
        let mut len = 0usize;
        let p = unsafe { abi::desk_arg(self.n, &mut len) };
        if p.is_null() {
            return None;
        }
        let bytes = unsafe { slice::from_raw_parts(p, len) };
        Some(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }.to_os_string())
    }
}
