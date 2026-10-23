# E: `std` for an invented OS

Spike `e-std-stubs`: since we write our own target specifications, can Rust's `std` be built for a target whose `os` is a name we invent, what does the "unsupported" platform give us, what does it cost, and what is the cheapest path to a useful `std` for DOS programs and for future Desk apps? The code is in [`../spikes/e-std-stubs/`](../spikes/e-std-stubs/); it builds on the pure-Rust DOS runtime of [`D0-RUST-DOS.md`](D0-RUST-DOS.md) (`rt`) and its packer. Everything below was run with Rust 1.98.0 (the pinned toolchain), headless DOSBox Staging 0.83.0 and CWSDPMI r7 through [`../tools/dosrun.sh`](../tools/dosrun.sh), inside the `ci-dos` image. Nothing has run on real hardware.

## Contents

- [Verdict and recommendation](#verdict-and-recommendation)
- [Method and the commands that work](#method-and-the-commands-that-work)
- [1. Does std build for an invented OS?](#1-does-std-build-for-an-invented-os)
- [2. What the stubs give you](#2-what-the-stubs-give-you)
- [3. The cheapest useful std](#3-the-cheapest-useful-std)
- [4. Sizes and times](#4-sizes-and-times)
- [5. Maintenance on Rust upgrades](#5-maintenance-on-rust-upgrades)
- [What failed on the way](#what-failed-on-the-way)
- [What was and was not verified](#what-was-and-was-not-verified)

## Verdict and recommendation

**Verdict.** The stubs alone are not a `std`. In Rust 1.98.0 an unknown `os` name does not get the "unsupported" platform in full: `std` fails to build (21 errors) because four pieces have no fallback arm — the system allocator, the I/O error mapping, the thread-local keys and the random-data function. Once those four exist, everything else falls back to stubs that compile and link, and the result is poor but predictable: `println!` is silent, clocks and `sleep` abort the process, `thread::current()` aborts when the system allocator is a stub, and `process::exit` aborts. **A small patched platform layer works.** A patch of 55 added lines in 12 existing files of `library/std/src/sys` plus about 340 lines of new files select a "desk" platform (also reachable as os "dos") that calls 14 `desk_*` C functions; a 450-line shim implements them over the d0 runtime. With it `println!`, `eprintln!`, `stdin`, `Vec`, `HashMap` (with a real seed), `Instant` (microsecond resolution from the PIT), `SystemTime`, `thread::sleep`, `env::args`, `env::var`/`vars` and `process::exit(7)` all work in DOSBox. A hello program is 19.5 KB against 9.7 KB for the `no_std` + `alloc` version, or 10.2 KB against 8.2 KB with `-Cpanic=immediate-abort`.

**Recommendation.**

- **Kernel, runtime, drivers and the Desk shell:** stay `no_std` + `alloc`, as the matrix in [`RUST-TARGETS.md`](RUST-TARGETS.md) says. `std` doubles a plain hello program and adds machinery (panic hooks, thread handles, `fmt` for `Duration`) these programs do not need.
- **Desk apps (matrix row d) and DOS programs that want the crates ecosystem:** use a **patched `std` platform layer in a copied sysroot**, with the `desk_*` C functions as the contract between `std` and the runtime. It is cheaper than it looked: the layer is small, builds in 12 seconds, needs no precompiled sysroot, and the same layer serves the hosted runtime (the shim here) and, later, the native Desk (syscalls behind the same 14 functions). Do not rely on the unmodified stubs, and do not rely on borrowing another OS's layer (below) beyond experiments.
- **A thin `desk_std` crate on `core` + `alloc`** is still the right answer for size-critical programs (floppy-sized games, the first-stage tools), and a good complement: it needs no patch and no tracking. It is not a substitute for `std` when a dependency needs `std::io`, `std::collections::HashMap` or `std::time`.
- **An upstream Tier 3 target** remains a long-term idea only. The patch is small enough to propose, but nothing here depends on it.
- **Next:** (1) `fn main` as the entry — let `std`'s `lang_start` run (a C `main` called from `_start`) so the main thread has a name, the arguments are passed in and `std::rt` initialises, instead of `#![no_main]` + `rt::entry!`; (2) `std::fs` over the `rt::dos` file calls; (3) track Rust releases with the patch (a CI job that applies it to the next toolchain); (4) a real-hardware run of the clock.

## Method and the commands that work

```bash
tools/setup.sh                            # once
spikes/e-std-stubs/build.sh               # builds every flavour into dist/e/{base,stub,trusty,desk,dos}
spikes/e-std-stubs/check.sh               # format, lints, builds, negative builds, and every probe run in headless DOSBox
spikes/e-std-stubs/measure.sh             # sizes under the size flags, and cold -Zbuild-std times (not part of the checks)
spikes/e-std-stubs/breakdown.py ELF       # where the bytes of a program go, by crate and by symbol
tools/dosrun.sh dist/e/desk PROBE.EXE all
```

`check.sh` is a step of [`../tools/check.sh`](../tools/check.sh). The build, in short:

```bash
# an unmodified std, for an OS name the layer knows (trusty): -Zbuild-std=std,panic_abort
RUSTC_BOOTSTRAP=1 cargo build --release --target targets/dos-trusty.json -p probe --features trusty

# a patched std: copy rust-src, patch it, point cargo at the copy
spikes/e-std-stubs/patch/mkstd.sh spikes/e-std-stubs/target/desk-src
__CARGO_TESTS_ONLY_SRC_ROOT=$PWD/spikes/e-std-stubs/target/desk-src/library RUSTC_BOOTSTRAP=1 \
  cargo build --release --target targets/desk-app.json -p probe --features desk

# the baseline, without std
RUSTC_BOOTSTRAP=1 cargo build --release --target targets/dos-std.json -p base -Zbuild-std=core,alloc,compiler_builtins
```

[`.cargo/config.toml`](../spikes/e-std-stubs/.cargo/config.toml) sets `build-std = ["std", "panic_abort"]`, `build-std-features = ["compiler-builtins-mem"]` and `json-target-spec = true`; the profile is `opt-level = "s"`, LTO, one codegen unit and `panic = "abort"`. Each program is linked by `rust-lld` with the d0 linker script and packed by [`../tools/elf2go32.py`](../tools/elf2go32.py). Cargo downloads the crates that `std`'s sources depend on, so the build needs network access and a writable `CARGO_HOME`.

How the pieces fit:

| Piece | What it is |
|---|---|
| [`targets/`](../spikes/e-std-stubs/targets/) | Four copies of d0's `i486-unknown-none-elf` specification that differ only in `os`: `dos` (DOS program flavour), `desk` (Desk app flavour), `stub` (the minimal-patch stub platform) and `trusty` (a borrowed platform) |
| [`probe/`](../spikes/e-std-stubs/probe/) | A `#![no_main]` std program that exercises each API and reports through `rt::println!` (straight to DOS), so a report is visible whatever `std`'s own stdout does; plus `hello_std` and `hello_map` for the size comparison |
| [`base/`](../spikes/e-std-stubs/base/) | The same hello program on `core` + `alloc` and `rt` |
| [`shim/`](../spikes/e-std-stubs/shim/) | The symbols a platform layer needs, over `rt`: libc-style (`malloc`, `writev`, ...) for the borrowed Trusty layer, and `desk_*` for the patched layer; the clock (PIT and BIOS ticks, INT 21h date) |
| [`patch/`](../spikes/e-std-stubs/patch/) | `desk-std.patch` (arms in `sys/*/mod.rs`), `overlay/` (the new files) and `mkstd.sh` |

d0's `rt` gained one cargo feature, `panic-handler` (on by default): `std` defines the panic handler lang item itself, so a std program turns it off. Nothing else in d0 changed.

## 1. Does std build for an invented OS?

`std` picks its platform layer in `library/std/src/sys/*/mod.rs` with `cfg_select!` arms on `target_os`, `target_family`, `target_vendor` and `target_env`. Most modules end in `_ => { mod unsupported; ... }`. The result for the specification of `dos-std.json` with different `os` values (all with `-Zbuild-std=std,panic_abort -Zjson-target-spec`, `RUSTC_BOOTSTRAP=1`, `panic-strategy: abort`, ELF, no `libc`):

| `os` | Result |
|---|---|
| `unknown` | Refused by rustc before any build: ``error loading target specification: `unknown` os can only be used on particular targets; use `none` for bare-metal targets`` |
| `none`, `desk`, `dos` | `std` fails: **21 errors**, the same for all three |
| `desk` with `"has-thread-local": true` | 18 errors (the thread-local key errors go; the other three gaps remain) |
| `desk` with `"target-family": ["unix"]` | Fails earlier, in the `libc` crate (``unresolved import `unistd```): `libc` does not know the OS |
| `desk` with `"target-family": ["wasm"]` | Fails in `core`: 281 errors, ``the feature named `..` is not valid for this target`` |
| `uefi` | The specification is refused: ``is_like_windows must be set if and only if os is windows, uefi or cygwin`` (needs a PE target) |
| `xous`, `vexos` | `std` fails (`dlmalloc` for `xous`; unknown token in inline assembly for `vexos`) |
| `zkvm`, `trusty` | **`std` builds unmodified** in about 11 s |

The 21 errors reduce to four missing pieces, none of which has an "unsupported" fallback:

| Missing | Where | Why it fails |
|---|---|---|
| `System: GlobalAlloc` | `sys/alloc/mod.rs` | `cfg_select!` has no `_` arm: ``none of the predicates in this `cfg_select` evaluated to true``, then 14 errors for the missing impl |
| `decode_error_kind`, `errno`, `error_string`, `is_interrupted` | `sys/io/error/mod.rs` | no `_` arm |
| `thread_local::key::{Key, LazyKey, get, set}` | `sys/thread_local/mod.rs` | without `target_thread_local` the fallback is the OS-key implementation, which exists only for unix, windows, sgx, xous and motor; the `no_threads` implementation is selected by an OS list (wasm without atomics, uefi, zkvm, trusty, vexos) |
| `sys::fill_bytes` | `sys/random/mod.rs` | no `_` arm (there is one for wasm-unknown, xous and vexos that supplies `hashmap_random_keys` too) |

Everything else — `pal`, `args`, `env`, `fs`, `stdio`, `thread`, `time`, `process`, `os_str`, `paths`, `sync` — does fall back to `unsupported`. Other facts about the build:

- **`restricted_std`.** `library/std/build.rs` lists the OSes with no special requirements; any other `os`, and every JSON target, gets `cfg(restricted_std)`, and a crate that uses `std` then fails with ``error[E0658]: use of unstable library feature `restricted_std` `` until it adds `#![feature(restricted_std)]` (which `RUSTC_BOOTSTRAP=1` allows). The `stub` flavour keeps it (the probe has `cfg_attr(target_os = "stub", feature(restricted_std))`); the patch adds `desk` and `dos` to the list.
- **`panic_immediate_abort`** is no longer a feature of `core`: with `build-std-features = [".., panic_immediate_abort"]` the build stops with ``panic_immediate_abort is now a real panic strategy``. It is `-Zunstable-options -Cpanic=immediate-abort` now, and it works with a specification that says `panic-strategy: abort`.
- **No `unwind`, `libc` or linker errors** arose with `panic_abort`, `rust-lld` and a freestanding ELF: `std`'s `unwind` and `libc` crates compile for an unknown OS (the `libc` crate only fails for `target-family: unix`). `max-atomic-width: 32` is why `AtomicU64` is unavailable to our own code (the shim keeps 64-bit state in a `Cell` wrapper); `std` itself copes.
- **`std` build time:** 11.9 s cold for `-Zbuild-std=std,panic_abort` plus the probe, against 8.7 s for `core,alloc,compiler_builtins` plus the baseline, on a 16-thread machine.

## 2. What the stubs give you

Three platform layers were run through the same probe. **stub** is the minimum patch that makes an unknown OS name build (`os` `stub`: a system allocator that always returns null, the generic error mapping, `no_threads` thread-locals, and the unsupported random functions), so everything else is the stock unsupported layer. **trusty** is stock `std` with `os` set to `trusty`, whose layer is the unsupported one plus `libc` calls for allocation (`malloc` and friends), standard output (`writev`) and the random seed (`trusty_rng_secure_rand`); the shim defines those symbols over `rt`. **desk** is the patched layer of section 3. The program has no `std` start-up (no `lang_start`), so it is `#![no_main]` and enters through `rt::entry!`; the global allocator is `rt`'s DPMI-backed heap in all three. Standard error is written to standard output by the shim in the harness (DOS standard error is the console, which `>` cannot capture); a `process::exit` or panic status is the process's own. Exit status 1 for the stub and trusty aborts is CWSDPMI's exception exit (the abort is `ud2`); the patched layer exits 134 deliberately.

| API | stub (unsupported) | trusty (borrowed) | desk (patched) |
|---|---|---|---|
| `println!`, `eprintln!` | compiles, links, runs, **prints nothing** | prints (`writev`) | prints |
| `stdout().write_all`, `flush` | `Ok(())`, discards | writes | writes |
| `stdin().read_line` | `Ok(0)` | `Ok(0)` | reads handle 0 (`Ok(0)` at `NUL`) |
| `Vec`, `String`, `format!`, `Box` | works (the program's own `#[global_allocator]`) | works | works |
| `System.alloc` | **returns null** | non-null (`malloc`) | non-null (`desk_alloc`) |
| `HashMap` | works; the seed comes from stack and heap addresses, so it repeats on every run | works (seed from the shim) | works (seed from the shim) |
| `BTreeMap` | works | works | works |
| `Instant::now`, `elapsed` | **panics** (`time not implemented on this platform`), process aborts | same | works, microsecond steps |
| `SystemTime::now` | panics, aborts | same | works (DOS date and time, 10 ms) |
| `thread::Builder::spawn` | **aborts** (see below) | `Err(Unsupported)` | `Err(Unsupported)` |
| `thread::spawn` | aborts | panics: `failed to spawn thread: Unsupported`, aborts | same, exit 134 |
| `thread::sleep` | panics (`can't sleep`), aborts | same | works (busy-wait; 1000 ms is 18 BIOS ticks) |
| `thread::current()`, `available_parallelism`, `yield_now` | `current()` **aborts**; `available_parallelism` is `Err(NotFound)`; `yield_now` returns | `ThreadId(1)`, `Err(NotFound)`, returns | same as trusty |
| `Mutex`, `RwLock`, `Once`, `OnceLock`, atomics | work (no-contention implementations) | work | work |
| `env::args` | empty | empty | the command tail (`["all"]`); no program name, as in d0 |
| `env::var` | `Err(NotPresent)` | `Err(NotPresent)` | works (`COMSPEC`); `vars()` works |
| `env::vars()` | panics (`not supported on this platform`) | panics | works |
| `fs::File::open`, `fs::write` | `Err(Unsupported)` | `Err(Unsupported)` | `Err(Unsupported)` (not implemented) |
| `process::exit(7)` | aborts, status 1 | aborts, status 1 | exits 7 |
| `panic!` | message lost (`panic_output` is `None`), aborts | message on standard error, aborts | message on standard error, status 134 |
| `panic::set_hook` | the hook receives the message (`panicked at ..:\nboom 42`) | same | same |
| `process::abort` | `ud2`, status 1 | same | exit 134 |
| `thread_local!` (`Cell` and `RefCell<Vec>`) | works (plain statics) | works | works |

The `thread::current()` abort is the surprising one. The `Thread` handle is allocated with `System`, not the global allocator, so a null `System` makes `thread::current()` and `thread::spawn` fail. Every panic path that names the thread uses it as well. A platform's `System` allocator therefore has to work even when the program installs its own `#[global_allocator]`; a null stub cannot be the answer.

## 3. The cheapest useful std

Two cheaper paths were tried, then the patch.

**Borrowing an existing OS's layer (no patch).** Set `os` to `trusty` and supply `malloc`, `calloc`, `realloc`, `free`, `memalign`, `posix_memalign`, `writev` and `trusty_rng_secure_rand` from the runtime. Unmodified `std` then links and `println!`, `HashMap` and the heap work (table in section 2, 120 lines of shim). It is the cheapest thing that runs, and it is a trap as a basis: the layer is chosen by an OS name that happens to be convenient, time, arguments, environment, `exit` and `sleep` stay stubs, and a future Trusty change moves the contract under us. `zkvm` also builds unmodified but wants its own `sys_*` ABI; it was not linked or run.

**A patched copy of the sources.** `cargo -Zbuild-std` reads the library workspace from the sysroot's `rust-src`; the environment variable **`__CARGO_TESTS_ONLY_SRC_ROOT`** overrides that, and its value is the path of the **`library` directory itself** (``failed to read `<root>/Cargo.toml` `` otherwise). `patch/mkstd.sh` copies the toolchain's `library` to `target/desk-src/`, applies `desk-std.patch` and copies `overlay/` over it. No `--sysroot`, copied toolchain or `RUSTFLAGS` is needed, the pinned toolchain stays untouched, and the std build is 12 s, so there is nothing to prebuild. The patch adds, per module:

| Module | Change |
|---|---|
| `sys/pal` | `any(target_os = "desk", target_os = "dos") => mod desk`: the unsupported `common` plus `abort_internal` that calls `desk_exit(134)`, and `abi.rs`, the 14 `extern "C"` declarations |
| `sys/alloc` | `System: GlobalAlloc` over `desk_alloc`, `desk_alloc_zeroed`, `desk_dealloc` and `desk_realloc` |
| `sys/stdio` | `Stdin` over `desk_read`; `Stdout` and `Stderr` over `desk_write`; `panic_output` is standard error |
| `sys/random` | `fill_bytes` over `desk_random` (`HashMap` seeds itself from it) |
| `sys/args`, `sys/env` | `desk_arg_count` and `desk_arg`; `desk_env_entry` and `desk_env_get` |
| `sys/time` | `Instant` from `desk_now_ns` (monotonic), `SystemTime` from `desk_unix_ns` |
| `sys/thread` | `sleep` over `desk_sleep_ns`; everything else stays unsupported |
| `sys/exit` | `process::exit` over `desk_exit` |
| `sys/io/error`, `sys/thread_local` | `desk` and `dos` (and `stub`) added to the existing `generic` and `no_threads` arms |
| `std/build.rs` | `desk` and `dos` added to the `restricted_std` list |

`shim/src/desk.rs` implements the 14 functions over `rt`:

- **Memory:** `alloc::alloc`, i.e. `rt`'s heap.
- **Output and input:** `rt::dos::write_all` and `read`.
- **Arguments and environment:** read from the PSP and the environment block once, then kept.
- **Random:** a 64-bit mixer seeded from the PIT, the BIOS tick count and a counter. Not secure, and a 486 has no cycle counter.
- **Monotonic clock:** the BIOS tick count at `0040:006C` with the PIT's channel 0. The PIT is a square wave, so its counter runs twice per tick and the output pin (read with the read-back command, `0xC2`) tells the halves apart; reads retry when a tick lands between them and the result never goes backwards. Resolution is 838 ns.
- **Wall clock:** INT 21h `AH=2Ah` and `2Ch` through the DPMI real-mode call, 10 ms steps, the local clock taken as UTC. A run printed an epoch value matching the day of the run.
- **Sleep:** a spin on the monotonic clock. `hlt` is a privileged instruction under a DPMI host.

Both flavours — os `desk` (`desk-app.json`) and os `dos` (`dos-std.json`) — select this layer and pass the same checks. Everything in the table of section 2 that says "works" for desk was run in DOSBox. `HELLOMAP.EXE` prints a sorted `HashMap` and the elapsed time of an `Instant`.

## 4. Sizes and times

The executable sizes (bytes, the 2 KiB stub included) of the same hello program (a `Vec`, `format!` and one line of output) on each platform layer, with `opt-level = "s"`, LTO and `panic = "abort"` in every row. The last column adds a `HashMap<String, u32>` of ten entries, a sort, `{:?}` of the pairs and, for desk only, an `Instant`. `imm` is `-Cpanic=immediate-abort`; `imm+loc` adds `-Zlocation-detail=none -Zfmt-debug=none`. The reference from d0 is its full test program `HELLO.EXE` at roughly 49 KB, which does far more than print one line.

| Flags | `no_std` + alloc | std, stub | std, trusty | std, desk | desk + `HashMap` |
|---|---|---|---|---|---|
| default | 9,728 | 14,848 | 20,992 | 19,456 | 41,472 |
| imm | 8,192 | 9,216 | 11,264 | 10,240 | 30,720 |
| imm+loc | 8,192 | 9,216 | 11,264 | 10,240 | 21,504 |

- **What `std` adds to a hello:** about 9.7 KB by default (+100%), about 2 KB with `immediate-abort` (+25%). The default panic path is most of it (the hook, the message formatting, `Location` strings that carry the build path).
- **`HashMap` and friends are not free:** the last column is not the map alone (it includes a sort and `Debug` for tuples), but the breakdown of the default desk build points at `core` (sorting, unicode tables used by `Debug` of strings, `Duration` formatting), `hashbrown` (about 2 KB) and SipHash (about 1 KB). The `imm+loc` row shows the lever: 41 KB to 21 KB.
- **Where the bytes go** (the default desk hello, `breakdown.py`, symbols only): `std` 4.2 KB (`panicking::panic_with_hook` 0.9 KB, `StdoutLock::write_all` 0.7 KB), `core` 2.6 KB (`Formatter::pad`, `pad_integral`, `fmt::write`), `alloc` 1.4 KB, `rt` 1.4 KB, `compiler_builtins` 0.7 KB. The 256 KiB `RT_STACK` is `.bss`, not file size. The stub layer is 4.6 KB smaller than the desk layer because its stdio, time, args and env are empty.
- **Build times** (cold, this machine, 16 threads, includes downloading nothing already cached): `core,alloc,compiler_builtins` plus the baseline 8.7 s; `std,panic_abort` plus the probe 11.9 s. A warm rebuild of the probe is under a second. `check.sh` as a whole takes about 90 seconds in the `ci-dos` image on this machine, most of it three failing `std` builds (about 10 s each) and the cold builds; it runs about 60 headless DOSBox runs at 0.4 s each.
- **Patch size:** 55 added lines, none removed, in 12 existing files (about 40 lines of `cfg_select!` arms); about 340 lines of overlay; the `desk_*` shim is 330 lines (with the clock), the borrowed-Trusty shim 120.

## 5. Maintenance on Rust upgrades

What must be tracked is the `cfg_select!` structure of `library/std/src/sys/*/mod.rs` and the signatures the overlay files implement. The patch was dry-run (`patch --dry-run`) against the `rust-src` of other stable releases, installed into a throwaway `RUSTUP_HOME`:

| Toolchain | Patch applies | Failed hunks |
|---|---|---|
| 1.98.0 | yes | none |
| 1.97.0 | yes (with offsets) | none |
| 1.96.1 | partly | `sys/io/error/mod.rs` (the arm list moved) |
| 1.95.0 | partly | `sys/io/error/mod.rs` |
| 1.94.0 | partly | `sys/io/error/mod.rs`, `sys/env/mod.rs`, `sys/thread_local/mod.rs` |
| 1.93.0 | partly | `sys/env/mod.rs`, `sys/thread_local/mod.rs` |

Only patch application was tested on the older releases; whether the overlay files compile against them was not. The patch is the pinned toolchain's, so this cost arises on each bump of `rust-toolchain.toml`, not on every nightly. The drift is in the arm lists (an OS added to an `any(...)`, a module split); the overlay files copy the shape of the zkvm and trusty layers, and call internal items (`sys::io::DEFAULT_BUF_SIZE`, `env::common::Env::new`, `thread::local_impl`) that have moved before. The `abi.rs` function list is stable by construction: it is ours. Expect, per Rust release, one rebase of the patch by hand (minutes), a rebuild and the checks of `check.sh`; `check.sh` already fails if the patch does not apply or the platform no longer builds. A CI job that applies the patch to the next release's `rust-src` ahead of the bump would turn the surprise into a scheduled task.

The other costs the matrix in [`RUST-TARGETS.md`](RUST-TARGETS.md) listed: size is addressed above, and the licence is the `std` licence (Apache-2.0 OR MIT, the same as the repository); the patch and overlay are small derivative files of `std` and carry the repository's header.

## What failed on the way

- `os` `unknown`: refused by rustc.
- `std` for `none`, `desk`, `dos`: four gaps, 21 errors (section 1).
- `build-std-features = ["panic_immediate_abort"]`: removed from `core`; use `-Cpanic=immediate-abort`.
- `HELLO_STD.EXE` and `HELLO_MAP.EXE`: DOS says "Illegal command" for a base name longer than eight characters; renamed `HELLOSTD` and `HELLOMAP`.
- A cargo `--config target.<t>.rustflags` is appended to the config file's flags, so the linker script was given twice and `rust-lld` rejected the overlap; setting `RUSTFLAGS` replaces them instead.
- `AtomicU64` does not exist for `max-atomic-width: 32` (the shim's 64-bit state).
- `__CARGO_TESTS_ONLY_SRC_ROOT` pointing at the `rust` directory instead of `rust/library`.
- A blocking `stdin().read_line` hangs the headless run (no input, no end of file): the probe runs it with `< NUL`.
- `bc` is not in the image; the timings use Python.

## What was and was not verified

Verified, by running (and by `check.sh`, which asserts each line of the table in section 2): the four-gap failure; the unmodified Trusty layer, linked with the shim; the stub and desk layers; both flavours; every API in the table, in headless DOSBox with `cputype = 486`; the sizes and times above; the patch against the 1.93 to 1.98 sources (application only).

Not verified:

- **Real hardware and other hosts.** Nothing ran on a 486, under EMM386, QEMM or a Windows 9x DOS box. The PIT read-back command (`0xC2`) is a 8254 feature; DOSBox emulates it and the clock was only exercised there. The BIOS tick count restarts at midnight, which the clock does not handle. The wall clock takes the local time as UTC.
- **Threads.** There are none: spawn is `Unsupported`, locks do not block, and `thread_local!` destructors were not tested.
- **`std::fs`, `net`, `process::Command`:** unsupported, as on the stock stubs. A DOS-backed `fs` was not tried.
- **The ordinary `fn main` entry.** Every program is `#![no_main]`; `std`'s own start-up (`lang_start`, the main thread's name, argument passing) never ran, so `thread::current().name()` is `None` and a panic message names the thread `<unnamed>`.
- **The native Desk.** The `desk_*` contract was only implemented over DOS; a native implementation is the point of the design and is untested. The layer is single-threaded by assumption.
- **Other Rust releases** beyond patch application, and `zkvm`, which builds but was not linked.
- **Panic-path size.** The `imm` rows remove the panic message machinery, so a program built that way cannot report a panic at all.
