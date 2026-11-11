# The Desk target for `std`

How Rust's standard library would reach Desk apps: one target for both tracks, its specification, what the OS must provide to `std`, how `std::sync` gets its locks from the one wait and wake call, what following Rust releases costs, and the staging. It belongs to [`APP-MODEL.md`](APP-MODEL.md#the-desk-target-for-std) and rests on the `std` experiment, [`E-STD-STUBS.md`](E-STD-STUBS.md).

> **Status.** A design, written on 2026-11-11. Nothing here is built beyond experiment E (`spikes/e-std-stubs/`), which ran in headless DOSBox only: there it showed `println!`, `HashMap`, `Instant`, `process::exit`, arguments, the environment and `thread_local!` working over a patched platform layer, and it did not run threads, locks that block, files through `std::fs`, `std`'s own `fn main` start-up, the native kernel, or anything on real hardware. The `std::sync` reading below is of the Rust 1.98.0 sources in the pinned toolchain's `rust-src` on the development machine, not a build.

## Contents

- [One target for both tracks](#one-target-for-both-tracks)
- [The target specification](#the-target-specification)
- [What the OS must provide](#what-the-os-must-provide)
- [`std::sync` and the one wait and wake call](#stdsync-and-the-one-wait-and-wake-call)
- [Maintenance](#maintenance)
- [Staging](#staging)
- [Questions for the owner](#questions-for-the-owner)

## One target for both tracks

**Recommendation: one target, `os: "desk"`, for apps on both tracks.** E's finding is that one platform layer over a list of `desk_*` C functions (15 of them in the experiment) is the whole contract between `std` and the system: on the hosted track a shim implements them over `rt` (`spikes/e-std-stubs/shim/src/desk.rs`, over `rt::dos`, the heap and the PIT), and on the native track the same functions become system calls. A second target would duplicate the patch and its maintenance for no difference an app can see.

**How the functions resolve.** In a `.DMD` or `.DKX` file the platform layer's `extern "C"` declarations (`patch/overlay/library/std/src/sys/pal/desk/abi.rs` in E) become import slots, declared through the guest crate's `import!` instead of plain `extern` blocks, and the loader binds them by name ([`LOADER.md`](LOADER.md#address-space-assumptions)). On the hosted track the core's table maps each name to the shim's function in the same process; on native stage 1 to the kernel's function; from stage 3 to a stub that traps. The app binary is the same file on all three.

## The target specification

**Name:** `i486-unknown-desk`. Today a custom JSON specification, built with `-Zbuild-std=std,panic_abort`, `-Zjson-target-spec` and `RUSTC_BOOTSTRAP=1` on the pinned stable toolchain, with the patched library sources selected by `__CARGO_TESTS_ONLY_SRC_ROOT` as E does; an upstream Tier 3 target much later (R71). E's `spikes/e-std-stubs/targets/desk-app.json` is the starting point.

| Field | `kernel/i486-desk-none.json` | `i486-unknown-desk` | Why |
|---|---|---|---|
| `llvm-target`, `cpu`, `features`, `data-layout` | `i486-unknown-none-elf`, `i486`, no MMX or SSE | The same | The same machine and C ABI as the modules, so one file loads on both tracks |
| `os` | `none` | `desk` | Selects the Desk platform layer in `std`'s `cfg_select!` arms |
| `executables` | `true` | `true` | Apps are linked executables, then packed |
| `relocation-model`, `position-independent-executables` | `static`, `false` | `static`, `false` | The loader relocates ([`LOADER.md`](LOADER.md#evidence-gathered-for-this-design)) |
| `panic-strategy` | `abort` | `abort` | No unwinding machinery; a panic ends the thread's task through the guard or the kernel |
| `max-atomic-width` | 32 | 32 | A 486 has no 64-bit atomic (no `CMPXCHG8B`), so `AtomicU64` does not exist for the app's own code; `std` copes, as E found, and keeps 64-bit state behind locks |
| `has-thread-local` | absent (false) | false at first | `std` then uses thread-local keys, which need no TLS segment in the file; true later, with a TLS segment ([`APP-MODEL.md`](APP-MODEL.md#thread-local-storage)) |
| `linker`, `linker-flavor` | `rust-lld`, `gnu-lld` | `rust-lld`, `gnu-lld` | Then `dmdpack` |

**`restricted_std`.** `library/std/build.rs` lists the OS names with no special requirements, and every JSON target and every other OS gets `cfg(restricted_std)`, which makes a crate using `std` fail with an unstable-feature error until it adds `#![feature(restricted_std)]` ([`E-STD-STUBS.md`](E-STD-STUBS.md#1-does-std-build-for-an-invented-os)). E's patch adds `desk` to that list, so apps need no feature line; an upstream target would make the patch line unnecessary.

## What the OS must provide

What `std` needs, the `desk_*` call that provides it (E's names where E had one), and how it maps to the owner's decisions:

| `std` piece | Provided by | Hosted (E's shim, or new) | Native |
|---|---|---|---|
| The system allocator (`System`) | `desk_alloc`, `desk_alloc_zeroed`, `desk_dealloc`, `desk_realloc`, with size and alignment | `rt`'s heap (E, run) | The task's heap through the ledger; the decided `desk_alloc(size, align)` |
| Standard input, output and error | `desk_read`, `desk_write` | INT 21h through `rt` (E, run) | A console service or the app's log; the window system has no console yet |
| Files (`std::fs`) | New: open, read, write, seek, close, metadata over the file API | Not in E (`Err(Unsupported)`); over `rt::dos` next | Over the filesystem (R75) |
| `Instant`, `SystemTime` | `desk_now_ns`, `desk_unix_ns` | The PIT and the BIOS tick (838 ns), INT 21h for the date (E, run) | The kernel's tick and PIT counter; the RTC |
| `HashMap`'s random seed | `desk_random` | A mixer seeded from the PIT and ticks, not secure (E, run) | The kernel's entropy pool (timer and clock jitter, input timing; [`LOADER.md`](LOADER.md#load-addresses-and-randomisation)) |
| Arguments, environment, exit | `desk_arg_count`, `desk_arg`, `desk_env_entry`, `desk_env_get`, `desk_exit` | The PSP and environment block (E, run) | The task's start block; ending the task |
| `thread::sleep` | `desk_sleep_ns` | A spin on the clock (E, run); a green-thread switch later | `desk_sleep` |
| `std::thread` | New: spawn, join, exit, yield, the current thread's id | Cooperative green threads (not built) | Kernel threads, 1:1 ([`APP-MODEL.md`](APP-MODEL.md#threads)) |
| Thread-local keys (one of E's four missing arms) | New: `desk_tls_alloc`, `desk_tls_get`, `desk_tls_set`, `desk_tls_free` | A per-green-thread array | A per-thread array; E used the `no_threads` arm, which only works without threads |
| `std::sync` | New: `desk_wait`, `desk_wake` | Park and unpark green threads | The kernel's wait table |

The thread pieces are the change from E: E kept `std` on its `no_threads` arms, which are correct only while there is one thread, and a threaded Desk needs real implementations of the thread, thread-local-key and sync arms.

## `std::sync` and the one wait and wake call

**What the Rust 1.98.0 sources show** (`library/std/src/sys/sync/`, read in the pinned toolchain's `rust-src`):

- `mutex`, `condvar`, `rwlock`, `once` and `thread_parking` each choose an implementation with a `cfg_select!` on `target_os`. Each has a `futex.rs` arm, selected by a list of OS names that includes Linux, Android, FreeBSD, OpenBSD, DragonFly, Windows (not Windows 7), Motor and WebAssembly with atomics (the lists differ slightly: Hermit is in most, Fuchsia in some, and Fuchsia's mutex has its own file); the fallbacks are `no_threads` and, for `Once` and `RwLock`, a queue-based one.
- The `futex.rs` implementations use three functions and four types from `crate::sys::futex`, which a platform supplies: `futex_wait(&Atomic<u32>, expected, Option<Duration>) -> bool`, `futex_wake(&Atomic<u32>) -> bool` (one waiter), `futex_wake_all(&Atomic<u32>)`, and the type aliases `Futex`, `Primitive`, `SmallFutex` and `SmallPrimitive` (all `u32`-based in Hermit's `sys/pal/hermit/futex.rs`, the smallest example).
- So the claim holds, with two qualifications. **Holds:** a Desk platform layer that supplies those three functions over `desk_wait` and `desk_wake` (wake one is `desk_wake(addr, 1)`, wake all is `desk_wake(addr, u32::MAX)`) gets `Mutex`, `Condvar`, `RwLock`, `Once` and thread parking from `std`'s own code, with nothing else to write. **Qualification 1:** `desk` has to be added to the five `cfg_select!` futex arms (and `sys/pal/desk` has to export the `futex` module), which is five more hunks for the patch. **Qualification 2:** `std`'s futex locks use `compare_exchange` (the mutex's uncontended lock, `Once`, `RwLock`), which compiles to `CMPXCHG`. Where that instruction runs is **unverified** here. [`D0-RUST-DOS.md`](D0-RUST-DOS.md#what-failed-on-the-way) recorded DOSBox Staging's `cputype = 486` raising an exception on it, and the core's own crates ban compare-and-swap for that reason; that QEMU's 486 model and a real 486 accept it is believed, not tested in this repository. A one-instruction test in DOSBox (`cputype = 486`) and in QEMU's `-cpu 486` settles what is true, and its result is to be recorded here. The mutex also spins 100 times on a load before waiting, which is harmless and useless on one CPU.
- Not checked: whether the overlay compiles with these arms added, and how `std` behaves with them under DOSBox. Nothing was built.

## Maintenance

The patch follows each Rust release: [`E-STD-STUBS.md`](E-STD-STUBS.md#5-maintenance-on-rust-upgrades) measured it applying cleanly to 1.98.0 and 1.97.0 and partly to 1.93.0 to 1.96.1, the drift being in the `cfg_select!` arm lists and in internal items the overlay calls, and estimated one rebase by hand per release (minutes), a rebuild and the checks. The threaded arms add five hunks to the lists that move, so the cost grows a little. E proposed a CI job that applies the patch to the next toolchain, so the drift is seen before a bump; that is still the recommendation. An upstream Tier 3 target (R71) would move the arms into Rust's own sources, where Rust's changes carry them, at the price of upstream review and of keeping the target building in Rust's CI on a best-effort basis.

## Staging

1. **The loader spike (R48) stays `no_std`.** Its modules use `desk-guest`.
2. **The hosted shim path first.** It already builds (E's 12-second `std` build); the steps are the `desk_*` import slots instead of `extern` blocks, `std::fs` over `rt::dos`, `fn main` through `lang_start`, and an app packed as a `.DKX` and run by the hosted runtime. Threads come with the hosted green threads.
3. **Native after tasks and threads exist** (R77, R141): the same file, the functions resolved by the kernel.

## Questions for the owner

1. **One target for both tracks.** Default: yes, `i486-unknown-desk`.
2. **`CMPXCHG` in `std` apps.** Not decided: the owner chose to verify first, with the one-instruction test above. The two answers it chooses between: accept `CMPXCHG` in `std` apps (and run them under DOSBox with another `cputype` if its 486 mode rejects it), or a sixth patch hunk replacing `std`'s mutex with Desk's own lock, which does not cover `Once` and `RwLock`.
3. **`fn main` or `desk_main`.** Default: the file's entry stays `desk_main`, which calls `std`'s `lang_start` and so the app's `fn main`, so `std` initialises as it expects (E's open item).
4. **Thread-local keys or native thread-locals first.** Default accepted by the owner: keys, which need no format change.
5. **A patched sysroot or an upstream target.** Default accepted by the owner: the patched sysroot for years; upstream (R71) only when the target is stable and someone will maintain it there.
