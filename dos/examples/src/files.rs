// The test program: safe settings writes and the INT 24h guard.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Modes: none (the whole run), `leave-held` (exit with a guard still held; a record of the vector goes to FILES.VEC) and `check`
// (run afterwards in the same DOSBox: compares the vector with that record).
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use rt::critical::FailCritical;
use rt::dos::{self, DosError, File, Mode};
use rt::{cleanup, env, println, settings};

const INI: &str = "FILES.INI";
const NEW: &str = "FILES.NEW";
const VEC: &str = "FILES.VEC";

// The vector found at the start, segment << 16 | offset.
static BEFORE: AtomicU32 = AtomicU32::new(0);

fn vector() -> u32 {
    let (s, o) = dos::get_vector(0x24).expect("AH=35h");
    ((s as u32) << 16) | o as u32
}

fn wipe() {
    for n in [INI, NEW, VEC] {
        let _ = dos::remove(n);
    }
}

fn read(name: &str) -> Result<Vec<u8>, DosError> {
    File::open(name, Mode::Read)?.read_to_vec(0)
}

fn exists(name: &str) -> bool {
    File::open(name, Mode::Read).is_ok()
}

/// Registered before any guard, so it runs after the guard's restore: says whether the clean-up list put the vector back.
fn report() {
    println!(
        "files: after the clean-up list INT 24h is back as it was found: {}",
        vector() == BEFORE.load(Ordering::SeqCst)
    );
}

/// Does nothing, differently for each `N`, so that no two instances are the same function.
fn filler<const N: usize>() {
    core::hint::black_box(N);
}

pub fn main() -> i32 {
    let args = env::args();
    BEFORE.store(vector(), Ordering::SeqCst);
    match args.first().map(alloc::string::String::as_str) {
        Some("leave-held") => return leave_held(),
        Some("check") => return check(),
        _ => {}
    }
    assert!(cleanup::register(report), "clean-up list full");
    wipe();

    // The temporary names.
    for n in ["STATE.INI", "STATE", "A:\\CFG.D\\STATE.INI", "STATE.NEW", "desk.new", "A.B.C"] {
        match settings::temp_name(n) {
            Some(t) => println!("files: temp_name {} -> {}", n, t),
            None => println!("files: temp_name {} -> refused", n),
        }
    }
    println!(
        "files: save_replacing of a .NEW name refused: {}",
        settings::save_replacing("X.NEW", b"x") == Err(DosError(settings::ERR_TEMP_IS_NAME))
    );

    // Save, read back, replace.
    wipe();
    println!("files: save: {:?}", settings::save_replacing(INI, b"first=1\r\n"));
    println!("files: no temporary file left: {}", !exists(NEW));
    println!(
        "files: read: {:?}",
        settings::read_recovering(INI).map(|v| v == b"first=1\r\n")
    );
    println!("files: second save: {:?}", settings::save_replacing(INI, b"second=22\r\n"));
    println!(
        "files: read: {:?}",
        settings::read_recovering(INI).map(|v| v == b"second=22\r\n")
    );
    let big: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    println!("files: save of 70000 bytes: {:?}", settings::save_replacing(INI, &big));
    println!("files: read: {:?}", settings::read_recovering(INI).map(|v| v == big));

    // A crash between the delete and the rename: only the .NEW exists.
    wipe();
    File::open(NEW, Mode::Create)
        .and_then(|mut f| f.write_all(b"recovered=3\r\n"))
        .expect("write the .NEW");
    println!("files: simulated crash: {} is missing: {}", INI, !exists(INI));
    println!(
        "files: read_recovering: {:?}",
        settings::read_recovering(INI).map(|v| v == b"recovered=3\r\n")
    );
    println!(
        "files: the name is back and the temporary is gone: {}",
        exists(INI) && !exists(NEW)
    );
    println!("files: neither file: {:?}", {
        wipe();
        settings::read_recovering(INI)
    });

    // The comparison that guards a corrupt write. The end-to-end case is not provoked: the OS cannot be made to write wrongly.
    println!(
        "files: compare: same {}, one byte differs {}, short {}",
        settings::matches(b"abc", b"abc"),
        settings::matches(b"abc", b"abd"),
        settings::matches(b"abc", b"ab")
    );

    // Rename on its own: it refuses to overwrite.
    wipe();
    File::open(INI, Mode::Create).and_then(|mut f| f.write_all(b"a")).unwrap();
    File::open(NEW, Mode::Create).and_then(|mut f| f.write_all(b"b")).unwrap();
    println!("files: rename onto an existing file: {:?}", dos::rename(NEW, INI));
    dos::remove(INI).unwrap();
    println!("files: rename: {:?}", dos::rename(NEW, INI));
    println!("files: rename of a missing file: {:?}", dos::rename("NOFILE.NEW", VEC));
    wipe();

    // The guard.
    let before = vector();
    let guard = FailCritical::new().expect("guard");
    let during = vector();
    println!("files: INT 24h before, during: moved {}", before != during);
    println!(
        "files: during points at the stub: {}",
        FailCritical::stub_address().map(|(s, o)| ((s as u32) << 16) | o as u32) == Some(during)
    );
    let mut r = dpmi_regs();
    rt::dpmi::simulate_int(0x24, &mut r).expect("INT 24h");
    println!("files: the stub called through INT 24h returns AL={}", r.eax & 0xFF);
    let inner = FailCritical::new().expect("nested guard");
    println!(
        "files: nested guard: depth {}, same stub {}",
        FailCritical::depth(),
        vector() == during
    );
    drop(inner);
    println!(
        "files: inner dropped: depth {}, still held {}",
        FailCritical::depth(),
        vector() == during
    );
    drop(guard);
    println!(
        "files: after drop: depth {}, vector back {}, no stub {}",
        FailCritical::depth(),
        vector() == before,
        FailCritical::stub_address().is_none()
    );

    // A guard is refused when the clean-up list is full, and the vector is not touched.
    println!(
        "files: the guard is a clean-up entry only while held: {}",
        cleanup::len() == 1
    );
    let fillers: [fn(); 15] = [
        filler::<0>,
        filler::<1>,
        filler::<2>,
        filler::<3>,
        filler::<4>,
        filler::<5>,
        filler::<6>,
        filler::<7>,
        filler::<8>,
        filler::<9>,
        filler::<10>,
        filler::<11>,
        filler::<12>,
        filler::<13>,
        filler::<14>,
    ];
    for f in fillers {
        assert!(cleanup::register(f));
    }
    println!("files: the list holds {} of {}", cleanup::len(), cleanup::SLOTS);
    println!(
        "files: a guard on a full list: refused {}, vector untouched {}",
        FailCritical::new().err() == Some(DosError(rt::critical::ERR_CLEANUP_FULL)),
        vector() == before
    );
    println!(
        "files: save_replacing on a full list: refused {}",
        settings::save_replacing(INI, b"x") == Err(DosError(rt::critical::ERR_CLEANUP_FULL))
    );
    println!("files: exiting");
    0
}

fn dpmi_regs() -> rt::dpmi::RealRegs {
    rt::dpmi::RealRegs::default()
}

fn leave_held() -> i32 {
    wipe();
    assert!(cleanup::register(report), "clean-up list full");
    File::open(VEC, Mode::Create)
        .and_then(|mut f| f.write_all(&BEFORE.load(Ordering::SeqCst).to_le_bytes()))
        .expect("write the record");
    let guard = FailCritical::new().expect("guard");
    println!(
        "files: leave-held: INT 24h moved: {}",
        vector() != BEFORE.load(Ordering::SeqCst)
    );
    core::mem::forget(guard);
    println!("files: leave-held: exiting with the guard held");
    0
}

fn check() -> i32 {
    let recorded = read(VEC).map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]));
    println!(
        "files: check: INT 24h is as it was before the guard: {}",
        recorded == Ok(vector())
    );
    wipe();
    0
}
