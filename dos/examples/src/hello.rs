// The test program: output, arguments, environment, allocation, floating point, files, DPMI and the exit status.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use rt::{dos, dpmi, env, fpu, println};

pub fn main() -> i32 {
    println!("D0-HELLO ok");
    let args = env::args();
    println!("args: {} {:?}", args.len(), args);
    if args.first().map(String::as_str) == Some("exit") {
        let code = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        println!("exiting with status {}", code);
        return code;
    }

    println!("comspec: {}", env::var("COMSPEC").is_some());
    println!(
        "path: {}",
        env::program_path().is_some_and(|p| p.to_ascii_uppercase().ends_with(".EXE"))
    );
    dos::print_dollar(b"console: AH=09h ok\r\n");

    // The heap.
    let squares: Vec<u32> = (1..=10).map(|n| n * n).collect();
    let sum: u32 = squares.iter().sum();
    let text = format!("squares={} sum={} big={}", squares.len(), sum, sum as u64 * 1_000_003);
    println!("alloc: {}", text);
    let mut v: Vec<u8> = Vec::new();
    for i in 0..1_000_000u32 {
        v.push(i as u8);
    }
    println!("alloc: 1 MB vec of {} bytes, byte 999999 = {}", v.len(), v[999_999]);
    let mut many: Vec<Vec<u8>> = Vec::new();
    for i in 0..100 {
        many.push(alloc::vec![i as u8; 1000 + i * 37]);
    }
    for i in (0..100).step_by(2) {
        many[i] = Vec::new();
    }
    let kept: usize = many.iter().map(|m| m.len()).sum();
    println!("alloc: churn kept {} bytes", kept);
    drop(many);
    drop(v);

    // The x87.
    let r = fpu::sqrt(385.0);
    println!(
        "float: sqrt(385)={:.6} sin(1)={:.6} cos(1)={:.6} atan2(1,1)*4={:.6}",
        r,
        fpu::sin(1.0),
        fpu::cos(1.0),
        fpu::atan2(1.0, 1.0) * 4.0
    );
    println!("float: {} {:e} {:?}", 1.0f64 / 3.0, 6.02214076e23f64, 0.1f32 + 0.2f32);
    println!("float: control word {:#06x}", fpu::control_word());

    // DPMI.
    let (maj, min, _flags, _cpu, _m, _s) = dpmi::version();
    println!("dpmi: version {}.{}", maj, min);
    let b = dpmi::alloc_block(1 << 20).expect("0501h");
    let p = dpmi::linear_to_ptr::<u32>(b.linear);
    // SAFETY: the block is 1 MiB of committed memory at `p`.
    let ok = unsafe {
        for i in 0..(1 << 18) {
            p.add(i).write(i as u32 ^ 0xA5A5_A5A5);
        }
        (0..(1 << 18)).all(|i| p.add(i).read() == i as u32 ^ 0xA5A5_A5A5)
    };
    dpmi::free_block(b.handle).expect("0502h");
    println!("dpmi: 1 MiB block write/read/free {}", if ok { "ok" } else { "FAILED" });

    dpmi_more();
    files();
    let (cases, bad) = mem_check();
    println!("mem: memcpy and memset against a byte loop: {} cases, {} wrong", cases, bad);
    if args.first().map(String::as_str) == Some("oom") {
        // More than the machine has: the allocator fails, and the failure is a panic.
        let v: Vec<u8> = Vec::with_capacity(900 << 20);
        println!("unreachable: {}", v.capacity());
    }
    0
}

fn dpmi_more() {
    // Real-mode vectors: set a spare one, read it back, put it back.
    let spare = 0x60u8;
    let old = dpmi::get_rm_vector(spare).unwrap();
    dpmi::set_rm_vector(spare, 0x1234, 0x5678).unwrap();
    let new = dpmi::get_rm_vector(spare).unwrap();
    dpmi::set_rm_vector(spare, old.0, old.1).unwrap();
    println!(
        "dpmi: real-mode vector 0201h/0200h round trip {}",
        new == (0x1234, 0x5678) && dpmi::get_rm_vector(spare).unwrap() == old
    );
    // Protected-mode vectors: the same, with a selector and offset that are never called.
    let old = dpmi::get_pm_vector(spare).unwrap();
    dpmi::set_pm_vector(spare, old.0, 0x1111).unwrap();
    let new = dpmi::get_pm_vector(spare).unwrap();
    dpmi::set_pm_vector(spare, old.0, old.1).unwrap();
    println!(
        "dpmi: protected-mode vector 0205h/0204h round trip {}",
        new == (old.0, 0x1111) && dpmi::get_pm_vector(spare).unwrap() == old
    );
    // Locking a region of the stack.
    let x = [0u8; 64];
    let lin = dpmi::ptr_to_linear(x.as_ptr());
    println!(
        "dpmi: lock/unlock {}",
        dpmi::lock(lin, 64).is_ok() && dpmi::unlock(lin, 64).is_ok()
    );
    // A real-mode call by hand: INT 10h AH=0Fh (video mode) must agree with the protected-mode call.
    let mut rr = dpmi::RealRegs {
        eax: 0x0F00,
        ..dpmi::RealRegs::default()
    };
    dpmi::simulate_int(0x10, &mut rr).unwrap();
    println!(
        "dpmi: simulated real-mode INT 10h reports mode {:#04x}, as the direct call does: {}",
        rr.eax as u8,
        rr.eax as u8 == rt::vga::get_mode()
    );
    // What a protected-mode INT 21h does with a pointer: nothing useful. DOS writes from wherever real-mode DS:DX points.
    let name = "D0DIRECT.BIN";
    let f = dos::File::open(name, dos::Mode::Create).unwrap();
    let wrote = dos::write_direct_unsafe(f.handle(), b"ABCD").unwrap();
    drop(f);
    let mut f = dos::File::open(name, dos::Mode::Read).unwrap();
    let back = f.read_to_vec(0).unwrap();
    drop(f);
    dos::remove(name).unwrap();
    println!(
        "dpmi: direct INT 21h AH=40h with a pointer: DOS reports {} bytes, file holds the buffer: {}",
        wrote,
        back == b"ABCD"
    );
}

fn files() {
    // A file larger than the host's transfer buffer, in one call and in pieces.
    let n = 300_000usize;
    let data: Vec<u8> = (0..n).map(|i| (i.wrapping_mul(7) ^ (i >> 8)) as u8).collect();
    let name = "D0TEST.BIN";
    {
        let mut f = dos::File::open(name, dos::Mode::Create).expect("create");
        f.write_all(&data).expect("write");
    }
    let mut f = dos::File::open(name, dos::Mode::Read).expect("open");
    println!("file: wrote {} bytes, length {}", n, f.len().unwrap());
    let one = f.read_to_vec(0).expect("read");
    println!("file: one read call: {} bytes, match {}", one.len(), one == data);
    f.seek_start(0).unwrap();
    let pieces = f.read_to_vec(4096).expect("read");
    println!("file: 4 KiB reads: {} bytes, match {}", pieces.len(), pieces == data);
    drop(f);
    dos::remove(name).expect("remove");
    println!(
        "file: missing file gives {:?}",
        dos::File::open("NOSUCH.FIL", dos::Mode::Read).err()
    );
}

/// Compares the runtime's `memcpy` and `memset`, called through the C symbols, with a plain byte loop (volatile, so the compiler cannot
/// turn the loop back into a call to them) over destination and source offsets 0 to 7 and lengths 0 to 40, and a few larger ones.
fn mem_check() -> (usize, usize) {
    use core::ffi::c_void;
    extern "C" {
        fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
        fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void;
    }
    const PAD: usize = 16;
    let lengths: Vec<usize> = (0..=40).chain([63, 64, 65, 255, 256, 257, 1000, 4099]).collect();
    let mut cases = 0;
    let mut bad = 0;
    for &n in &lengths {
        for doff in 0..8usize {
            for soff in 0..8usize {
                let total = PAD + 8 + n + PAD;
                let src: Vec<u8> = (0..total).map(|i| (i as u8).wrapping_mul(31).wrapping_add(7)).collect();
                let mut got: Vec<u8> = (0..total).map(|i| (i as u8) ^ 0x5A).collect();
                let mut want = got.clone();
                // SAFETY: both ranges lie inside the vectors (offset + length + padding), and the vectors do not overlap.
                unsafe {
                    for i in 0..n {
                        let b = core::ptr::read_volatile(src.as_ptr().add(PAD + soff + i));
                        core::ptr::write_volatile(want.as_mut_ptr().add(PAD + doff + i), b);
                    }
                    let r = memcpy(
                        got.as_mut_ptr().add(PAD + doff).cast(),
                        src.as_ptr().add(PAD + soff).cast(),
                        n,
                    );
                    if r != got.as_mut_ptr().add(PAD + doff).cast() {
                        bad += 1;
                    }
                }
                cases += 1;
                if got != want {
                    bad += 1;
                }
                // memset, with a fill byte whose sign bit is set so a wrong widening shows.
                let fill = 0xA7u8;
                let mut got: Vec<u8> = (0..total).map(|i| (i as u8) ^ 0x5A).collect();
                let mut want = got.clone();
                // SAFETY: the range lies inside the vector.
                unsafe {
                    for i in 0..n {
                        core::ptr::write_volatile(want.as_mut_ptr().add(PAD + doff + i), fill);
                    }
                    let r = memset(got.as_mut_ptr().add(PAD + doff).cast(), i32::from(fill), n);
                    if r != got.as_mut_ptr().add(PAD + doff).cast() {
                        bad += 1;
                    }
                }
                cases += 1;
                if got != want {
                    bad += 1;
                }
            }
        }
    }
    (cases, bad)
}
