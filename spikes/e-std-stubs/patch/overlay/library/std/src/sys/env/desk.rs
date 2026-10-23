// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub use super::common::Env;
use crate::ffi::{OsStr, OsString};
use crate::io;
use crate::slice;
use crate::sys::pal::abi;

pub fn env() -> Env {
    let mut vars = Vec::new();
    let mut i = 0;
    loop {
        let mut len = 0usize;
        let p = unsafe { abi::desk_env_entry(i, &mut len) };
        if p.is_null() {
            break;
        }
        i += 1;
        let bytes = unsafe { slice::from_raw_parts(p, len) };
        // A leading '=' is part of the name (DOS keeps such entries), so the split looks from the second byte.
        if let Some(eq) = bytes.iter().skip(1).position(|&b| b == b'=').map(|n| n + 1) {
            let name = unsafe { OsStr::from_encoded_bytes_unchecked(&bytes[..eq]) }.to_os_string();
            let value = unsafe { OsStr::from_encoded_bytes_unchecked(&bytes[eq + 1..]) }.to_os_string();
            vars.push((name, value));
        }
    }
    Env::new(vars)
}

pub fn getenv(key: &OsStr) -> Option<OsString> {
    let k = key.as_encoded_bytes();
    let mut len = 0usize;
    let p = unsafe { abi::desk_env_get(k.as_ptr(), k.len(), &mut len) };
    if p.is_null() {
        return None;
    }
    let bytes = unsafe { slice::from_raw_parts(p, len) };
    Some(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) }.to_os_string())
}

pub unsafe fn setenv(_: &OsStr, _: &OsStr) -> io::Result<()> {
    Err(io::const_error!(io::ErrorKind::Unsupported, "cannot set env vars on this platform"))
}

pub unsafe fn unsetenv(_: &OsStr) -> io::Result<()> {
    Err(io::const_error!(io::ErrorKind::Unsupported, "cannot unset env vars on this platform"))
}
