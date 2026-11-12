// Placing a parsed module: copy the image, zero the tail, relocate for the destination and bind every import.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::error::LoadError;
use crate::header::rd32;
use crate::module::Module;

/// Where a module landed: the absolute addresses its caller needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// The first byte of the image.
    pub base: u32,
    /// One past the last byte of the image, `.bss` tail included.
    pub end: u32,
    pub start: u32,
    pub event: Option<u32>,
    pub stop: Option<u32>,
}

/// Copies the image into `dest`, zeroes the tail, relocates it for `dest_addr` and binds every import through `resolve`.
///
/// `dest` must hold at least `mem_size` bytes and `dest_addr` must be a multiple of `align`; `kind` is what the caller can load
/// (a module loader passes `KIND_MODULE`). Everything that can fail is checked before `dest` is touched, so a refused module
/// leaves it as it was. Nothing is called.
pub fn load_into<'a>(
    m: &Module<'a>,
    kind: u16,
    dest: &mut [u8],
    dest_addr: u32,
    resolve: &dyn Fn(&str) -> Option<u32>,
) -> Result<Placed, LoadError<'a>> {
    let h = m.header();
    if h.kind != kind {
        return Err(LoadError::KindNotLoadable {
            kind: h.kind,
            wanted: kind,
        });
    }
    // Address 0 is the null pointer, so no image goes there (for the same reason the link base is not 0).
    if dest_addr == 0 {
        return Err(LoadError::DestNull);
    }
    if !dest_addr.is_multiple_of(h.align) {
        return Err(LoadError::DestMisaligned {
            addr: dest_addr,
            align: h.align,
        });
    }
    if (dest.len() as u64) < h.mem_size as u64 {
        return Err(LoadError::DestTooSmall {
            need: h.mem_size,
            have: dest.len(),
        });
    }
    let end = match dest_addr.checked_add(h.mem_size) {
        Some(end) => end,
        None => {
            return Err(LoadError::DestWraps {
                addr: dest_addr,
                need: h.mem_size,
            })
        }
    };

    let image = m.image();
    // Every stored value must point into the image (one past its end is a valid pointer).
    let low = h.link_base as u64;
    let high = low + h.mem_size as u64;
    for (index, offset) in m.relocations().enumerate() {
        let value = rd32(image, offset as usize);
        if (value as u64) < low || (value as u64) > high {
            return Err(LoadError::RelocTargetOutOfRange {
                index: index as u32,
                offset,
                value,
            });
        }
    }
    // Each import is resolved once, before anything is written, and the answers are what get written: a resolver whose answer
    // changes between two calls cannot leave a slot unbound.
    let mut addresses: Vec<u32> = Vec::with_capacity(h.import_count as usize);
    for (index, import) in m.imports().enumerate() {
        match resolve(import.name) {
            Some(address) => addresses.push(address),
            None => {
                return Err(LoadError::MissingImport {
                    index: index as u32,
                    name: import.name,
                })
            }
        }
    }

    let file_size = image.len();
    dest[..file_size].copy_from_slice(image);
    dest[file_size..h.mem_size as usize].fill(0);
    let delta = dest_addr.wrapping_sub(h.link_base);
    for offset in m.relocations() {
        let o = offset as usize;
        let value = rd32(&dest[..file_size], o).wrapping_add(delta);
        dest[o..o + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (import, address) in m.imports().zip(addresses) {
        let o = import.slot as usize;
        dest[o..o + 4].copy_from_slice(&address.to_le_bytes());
    }

    let entry = |offset: u32| dest_addr.wrapping_add(offset);
    Ok(Placed {
        base: dest_addr,
        end,
        start: entry(h.start),
        event: (h.event != 0).then(|| entry(h.event)),
        stop: (h.stop != 0).then(|| entry(h.stop)),
    })
}
