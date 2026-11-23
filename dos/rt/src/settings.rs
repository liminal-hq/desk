// Safe settings writes: a file replaced so that a crash at any point leaves the old or the new one, whole.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `save_replacing(name, bytes)` writes the new bytes to a temporary file beside `name`, reads it back and compares, deletes the old
// file and renames the temporary one into place. The temporary name is the 8.3 stem of `name` with `.NEW`: `STATE.INI` becomes
// `STATE.NEW`, `STATE` becomes `STATE.NEW`, `A:\CFG\STATE.INI` becomes `A:\CFG\STATE.NEW`. A `name` that is not a clean 8.3 file name (empty, ending in a
// separator, no stem, a stem over 8 characters, an extension over 3, more than one dot) or that already has the extension `.NEW`
// is refused (`ERR_BAD_NAME`) before anything is touched: DOS truncates a longer part on its own, so two long names could share a
// temporary file, and a `.NEW` name would be its own.
//
// The crash windows, in order:
//
// 1. while `.NEW` is being written or compared: `name` is untouched and whole; a partly written `.NEW` is left behind and is
//    overwritten by the next save. `read_recovering` ignores a `.NEW` while `name` exists.
// 2. after the delete and before the rename: only `.NEW` exists, and it is whole, because the delete happens only after the
//    read-back matched. `read_recovering` completes the rename and returns it.
//
//    The exception is the very first save, when `name` does not exist yet: a crash while `.NEW` is being written then leaves a
//    partly written `.NEW` and no `name`, and `read_recovering` cannot tell that from a whole one (it has no checksum to look at).
//    A program for which that matters keeps a length or a checksum in its settings format and checks it on reading.
// 3. after the rename: the new file is in place.
//
// Between the delete and the rename there is a moment at which `name` does not exist; a program other than this one that looks at
// that moment sees no settings. What is not covered: a disk that lies about a write it reports as done and then reads back the same
// (the compare only catches a write that reads back differently), and a power cut that loses the FAT update of a rename. Every call
// runs under a `FailCritical` guard, so a missing disk is an error code, not a prompt.
use crate::critical::FailCritical;
use crate::dos::{self, DosError, File, Mode};
use alloc::string::String;
use alloc::vec::Vec;

/// Not a DOS code: `name` is not a name `save_replacing` can use. It is empty, ends in a separator, has no stem, is not 8.3 (a stem
/// over 8 characters, an extension over 3, or more than one dot), or has the extension `.NEW`, which would make it its own
/// temporary file.
pub const ERR_BAD_NAME: u16 = 0x8002;
/// Not a DOS code: the file read back from the disk is not the bytes written.
pub const ERR_MISMATCH: u16 = 0x8003;

const NOT_FOUND: u16 = 2;

/// The temporary name for `name`: its directory part and stem with `.NEW`. `None` if `name` is not a clean 8.3 file name (see
/// `ERR_BAD_NAME`): DOS truncates a longer part on its own, so two long names could share one temporary file, and a name that is
/// already `.NEW` would be its own.
pub fn temp_name(name: &str) -> Option<String> {
    let component = name.rfind(['\\', '/', ':']).map_or(0, |i| i + 1);
    let file = &name[component..];
    let (stem, ext) = match file.split_once('.') {
        Some((stem, ext)) => (stem, Some(ext)),
        None => (file, None),
    };
    if stem.is_empty() || stem.len() > 8 || ext.is_some_and(|e| e.len() > 3 || e.contains('.') || e.eq_ignore_ascii_case("NEW")) {
        return None;
    }
    let mut tmp = String::from(&name[..component]);
    tmp.push_str(stem);
    tmp.push_str(".NEW");
    Some(tmp)
}

/// Whether what was read back is what was written: the same length and the same bytes.
pub fn matches(written: &[u8], read_back: &[u8]) -> bool {
    written == read_back
}

fn write_new(tmp: &str, bytes: &[u8]) -> Result<(), DosError> {
    let mut f = File::open(tmp, Mode::Create)?;
    f.write_all(bytes)
}

fn read_all(name: &str) -> Result<Vec<u8>, DosError> {
    File::open(name, Mode::Read)?.read_to_vec(0)
}

/// Replaces `name` with `bytes`, as the module comment describes. On any error before the delete `name` is as it was.
pub fn save_replacing(name: &str, bytes: &[u8]) -> Result<(), DosError> {
    let tmp = temp_name(name).ok_or(DosError(ERR_BAD_NAME))?;
    let _guard = FailCritical::new()?;
    let checked = write_new(&tmp, bytes).and_then(|()| {
        if matches(bytes, &read_all(&tmp)?) {
            Ok(())
        } else {
            Err(DosError(ERR_MISMATCH))
        }
    });
    if let Err(e) = checked {
        let _ = dos::remove(&tmp);
        return Err(e);
    }
    match dos::remove(name) {
        Ok(()) => {}
        Err(DosError(NOT_FOUND)) => {}
        Err(e) => {
            let _ = dos::remove(&tmp);
            return Err(e);
        }
    }
    // A failure here leaves the whole new file as `.NEW`, which `read_recovering` finishes.
    dos::rename(&tmp, name)
}

/// Reads `name`. If it is missing and its `.NEW` exists (a crash between the delete and the rename), completes the rename first. If
/// the rename cannot be done the `.NEW` is read as it is.
pub fn read_recovering(name: &str) -> Result<Vec<u8>, DosError> {
    let _guard = FailCritical::new()?;
    match read_all(name) {
        Err(DosError(NOT_FOUND)) => {}
        other => return other,
    }
    let Some(tmp) = temp_name(name) else {
        return Err(DosError(NOT_FOUND));
    };
    let bytes = read_all(&tmp)?;
    let _ = dos::rename(&tmp, name);
    Ok(bytes)
}
