// A std program for DOS: what each part of std does on a target whose platform layer is stubbed, borrowed or patched.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// std has no start-up code here (no `lang_start`), so the program is `#![no_main]` and enters through the d0 runtime's `rt::entry!`.
// Every report goes through `rt::println!`, which writes to DOS directly, so a report is visible whatever std's own stdout does.
// The first argument names one test. `all` runs the ones that return on every platform; the others end the process (a panic, an abort, an exit) or wait
// for the keyboard, and are run one at a time by `check.sh`.
#![no_main]
#![cfg_attr(target_os = "stub", feature(restricted_std))]

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::sync::{Mutex, Once, OnceLock};

#[cfg(any(feature = "trusty", feature = "desk"))]
extern crate shim;

macro_rules! say {
    ($($arg:tt)*) => { rt::println!($($arg)*) };
}

rt::entry!(main);

fn main() -> i32 {
    let args = rt::env::args();
    let which = args.first().map(String::as_str).unwrap_or("all");
    say!("PROBE {}", which);
    if which == "all" {
        for (name, f) in SAFE {
            run(name, *f);
        }
    } else if let Some((name, f)) = SAFE.iter().chain(FATAL.iter()).find(|(n, _)| *n == which) {
        run(name, *f);
    } else {
        say!("unknown test {}", which);
        return 2;
    }
    say!("PROBE {} returned", which);
    0
}

fn run(name: &str, f: fn()) {
    say!("T {}: start", name);
    f();
    say!("T {}: end", name);
}

const SAFE: &[(&str, fn())] = &[
    ("println", t_println),
    ("eprintln", t_eprintln),
    ("write_all", t_write_all),
    ("alloc", t_alloc),
    ("hashmap", t_hashmap),
    ("btreemap", t_btreemap),
    ("sync", t_sync),
    ("args", t_args),
    ("var", t_var),
    ("fs", t_fs),
    ("tls", t_tls),
    ("hook", t_hook),
];

const FATAL: &[(&str, fn())] = &[
    ("thread_info", t_thread_info),
    ("thread_spawn", t_thread_spawn),
    ("stdin", t_stdin),
    ("vars", t_vars),
    ("instant", t_instant),
    ("systemtime", t_systemtime),
    ("sleep", t_sleep),
    ("spawn_unwrap", t_spawn_unwrap),
    ("exit", t_exit),
    ("panic", t_panic),
    ("panic_hook", t_panic_hook),
    ("abort", t_abort),
];

fn t_println() {
    println!("STD-PRINTLN visible");
}

fn t_eprintln() {
    eprintln!("STD-EPRINTLN visible");
}

fn t_write_all() {
    let mut out = std::io::stdout();
    let r = out.write_all(b"STD-WRITE_ALL visible\r\n");
    say!("stdout().write_all -> {:?}", r);
    let f = out.flush();
    say!("stdout().flush -> {:?}", f);
}

fn t_alloc() {
    let v: Vec<u32> = (1..=10).map(|n| n * n).collect();
    let s = format!("squares={} sum={}", v.len(), v.iter().sum::<u32>());
    let b = Box::new([7u8; 64]);
    let mut big = Vec::new();
    for i in 0..100_000u32 {
        big.push(i as u8);
    }
    say!("alloc: {} box={} big={}", s, b[63], big.len());
    use std::alloc::{GlobalAlloc, Layout, System};
    // SAFETY: a non-zero size and a valid alignment; the block is freed with the same layout.
    unsafe {
        let l = Layout::from_size_align(64, 8).unwrap();
        let p = System.alloc(l);
        say!("System.alloc(64) null? {}", p.is_null());
        if !p.is_null() {
            System.dealloc(p, l);
        }
    }
}

fn t_hashmap() {
    let mut m = HashMap::new();
    for i in 0..100u32 {
        m.insert(i, i * 2);
    }
    let rs = std::hash::RandomState::new();
    let (k0, k1) = {
        use std::hash::BuildHasher;
        (rs.hash_one(1u32), rs.hash_one(1u32))
    };
    say!(
        "hashmap: len={} get(42)={:?} hash_one stable={}",
        m.len(),
        m.get(&42),
        k0 == k1
    );
    let a = {
        use std::hash::BuildHasher;
        std::hash::RandomState::new().hash_one(1u32)
    };
    let b = {
        use std::hash::BuildHasher;
        std::hash::RandomState::new().hash_one(1u32)
    };
    say!(
        "hashmap: two RandomState hashes of 1 {} ({:#x} {:#x})",
        if a == b { "are equal" } else { "differ" },
        a,
        b
    );
}

fn t_btreemap() {
    let mut m = BTreeMap::new();
    for i in (0..50u32).rev() {
        m.insert(i, i.to_string());
    }
    say!("btreemap: len={} first={:?} get(7)={:?}", m.len(), m.iter().next(), m.get(&7));
}

fn t_thread_spawn() {
    let r = std::thread::Builder::new().spawn(|| 5);
    match r {
        Ok(_) => say!("thread: Builder::spawn -> Ok"),
        Err(e) => say!("thread: Builder::spawn -> Err({:?}) kind={:?}", e.to_string(), e.kind()),
    }
}

fn t_thread_info() {
    say!(
        "thread: available_parallelism -> {:?}",
        std::thread::available_parallelism().map_err(|e| e.kind())
    );
    std::thread::yield_now();
    say!("thread: yield_now returned");
    say!("thread: current().id -> {:?}", std::thread::current().id());
    say!("thread: current().name -> {:?}", std::thread::current().name());
}

static M: Mutex<u32> = Mutex::new(0);
static ONCE: Once = Once::new();
static CELL: OnceLock<String> = OnceLock::new();

fn t_sync() {
    *M.lock().unwrap() += 5;
    let mut n = 0;
    ONCE.call_once(|| n += 1);
    ONCE.call_once(|| n += 1);
    let s = CELL.get_or_init(|| String::from("init"));
    let rw = std::sync::RwLock::new(3);
    let c = std::sync::atomic::AtomicU32::new(1);
    c.fetch_add(2, std::sync::atomic::Ordering::SeqCst);
    let held = M.lock().unwrap();
    let relock = M.try_lock().is_err();
    drop(held);
    say!(
        "sync: mutex={} once ran {} time(s) oncelock={} rwlock={} atomic={} try_lock while held fails={}",
        *M.lock().unwrap(),
        n,
        s,
        *rw.read().unwrap(),
        c.load(std::sync::atomic::Ordering::SeqCst),
        relock
    );
}

fn t_args() {
    let a: Vec<String> = std::env::args().collect();
    say!("env::args: {} {:?}", a.len(), a);
    say!("env::current_exe -> {:?}", std::env::current_exe().map_err(|e| e.kind()));
}

fn t_var() {
    say!("env::var(COMSPEC) -> {:?}", std::env::var("COMSPEC"));
}

fn t_vars() {
    say!("env::vars().count() -> {}", std::env::vars().count());
}

fn t_fs() {
    match std::fs::File::open("HELLO.EXE") {
        Ok(_) => say!("fs: File::open(HELLO.EXE) -> Ok"),
        Err(e) => say!("fs: File::open(HELLO.EXE) -> Err kind={:?} ({})", e.kind(), e),
    }
    say!("fs: write -> {:?}", std::fs::write("E.TXT", b"x").map_err(|e| e.kind()));
}

thread_local! {
    static COUNTER: std::cell::Cell<u32> = const { std::cell::Cell::new(10) };
    static NAMES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn t_tls() {
    COUNTER.with(|c| c.set(c.get() + 1));
    NAMES.with(|n| n.borrow_mut().push("a".to_string()));
    say!(
        "thread_local: counter={} names={}",
        COUNTER.with(|c| c.get()),
        NAMES.with(|n| n.borrow().len())
    );
}

fn t_hook() {
    std::panic::set_hook(Box::new(|info| say!("hook saw: {}", info)));
    let _ = std::panic::take_hook();
    say!("panic: set_hook and take_hook returned");
}

fn t_stdin() {
    let mut s = String::new();
    let r = std::io::stdin().read_line(&mut s);
    say!("stdin().read_line -> {:?} {:?}", r.map_err(|e| e.kind()), s);
}

fn t_instant() {
    let a = std::time::Instant::now();
    let b = std::time::Instant::now();
    say!("Instant::now twice: elapsed {:?}", b.duration_since(a));
    say!("Instant::elapsed -> {:?}", a.elapsed());
}

fn t_systemtime() {
    let t = std::time::SystemTime::now();
    say!("SystemTime::now -> since epoch {:?}", t.duration_since(std::time::UNIX_EPOCH));
}

fn t_sleep() {
    let before = rt::timer::bios_ticks();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let after = rt::timer::bios_ticks();
    say!(
        "sleep(1000 ms) returned after {} BIOS ticks (18.2 per second)",
        after - before
    );
}

fn t_spawn_unwrap() {
    let h = std::thread::spawn(|| 1 + 1);
    say!("thread::spawn -> joined {:?}", h.join().is_ok());
}

fn t_exit() {
    say!("calling process::exit(7)");
    std::process::exit(7);
}

fn t_panic() {
    let v: Vec<u8> = Vec::new();
    say!("about to panic");
    let _ = v[std::hint::black_box(3)];
    say!("not reached");
}

fn t_panic_hook() {
    std::panic::set_hook(Box::new(|info| say!("hook saw: {}", info)));
    panic!("boom {}", 42);
}

fn t_abort() {
    say!("calling process::abort()");
    std::process::abort();
}
