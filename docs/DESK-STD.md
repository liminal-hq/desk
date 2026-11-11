# The Desk target for `std`

How Rust's standard library would reach Desk apps: one target for both tracks, its specification, what the OS must provide to `std`, how `std::sync` gets its locks from the one wait and wake call, what following Rust releases costs, and the staging. It belongs to [`APP-MODEL.md`](APP-MODEL.md#the-desk-target-for-std) and rests on the `std` experiment, [`E-STD-STUBS.md`](E-STD-STUBS.md).

> **Status.** A design, written on 2026-11-11. Nothing here is built beyond experiment E (`spikes/e-std-stubs/`), which ran in headless DOSBox only: there it showed `println!`, `HashMap`, `Instant`, `process::exit`, arguments, the environment and `thread_local!` working over a patched platform layer, and it did not run threads, locks that block, files through `std::fs`, `std`'s own `fn main` start-up, the native kernel, or anything on real hardware. The `std::sync` reading below is of the Rust 1.98.0 sources in the pinned toolchain's `rust-src` on the development machine, not a build.

## Contents

- [One target for both tracks](#one-target-for-both-tracks)
- [The target specification](#the-target-specification)
- [What the OS must provide](#what-the-os-must-provide)
- [`std::sync` and the one wait and wake call](#stdsync-and-the-one-wait-and-wake-call)
- [`CMPXCHG` on the hosted track](#cmpxchg-on-the-hosted-track)
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
- So the claim holds, with two qualifications. **Holds:** a Desk platform layer that supplies those three functions over `desk_wait` and `desk_wake` (wake one is `desk_wake(addr, 1)`, wake all is `desk_wake(addr, u32::MAX)`) gets `Mutex`, `Condvar`, `RwLock`, `Once` and thread parking from `std`'s own code, with nothing else to write. **Qualification 1:** `desk` has to be added to the five `cfg_select!` futex arms (and `sys/pal/desk` has to export the `futex` module), which is five more hunks for the patch. **Qualification 2:** `std`'s futex locks use `compare_exchange` (the mutex's uncontended lock, `Once`, `RwLock`), which compiles to `CMPXCHG`, and DOSBox's `486` mode rejects that instruction. The test and the owner's decision are in [the next section](#cmpxchg-on-the-hosted-track).
- Not checked: whether the overlay compiles with these arms added, and how `std` behaves with them under DOSBox. Nothing was built.

## `CMPXCHG` on the hosted track

**The test** (run by the coordinator on 2026-11-11 with a NASM `.COM`; DOSBox Staging 0.83.0, QEMU 8.2.2, NASM 2.16.01; emulators only, nothing on real silicon):

| Instruction | DOSBox `486` and `486_prefetch`, `normal` and `simple` cores | DOSBox `dynamic` core | DOSBox `pentium`, `auto` | DOSBox `386` modes | QEMU `486`, `486-v1`, `pentium`, `qemu32` |
|---|---|---|---|---|---|
| Dword `CMPXCHG` (`0F B1`, with or without `lock`) | Invalid opcode | Runs | Runs | Invalid | Runs |
| Byte `CMPXCHG` (`0F B0`), `XADD`, `BSWAP` | Run | Run | Run | Invalid | Run |

Where it ran, the succeeding and failing cases gave the right values and `ZF`. `tools/dosrun.sh` uses `core = normal` and `cputype = 486`, the failing combination. The early A-step encoding (`0F A6`/`A7`) was reported invalid everywhere; that is from memory and unverified, and nothing here says what any real 486 stepping does.

**The decision, in two steps.** The owner first chose an interrupt-masking lock fallback: the locks' compare-and-swap through an imported helper and a trap for everything else, option (a) below. A fourth experiment then showed DOSBox's dynamic core running the instruction while staying strict about the rest, and the owner simplified the decision (below).

**A fourth experiment: DOSBox's cores** (an agent; DOSBox Staging 0.83.0, DOSBox-X 2026.10.01 built from source, the CI image, rustc 1.98.0; 240 single-instruction runs under `cputype` 386, 486, 486_prefetch, pentium and auto with the normal, simple and dynamic cores, then the repository's own DOSBox checks; emulators only):

| Finding | Result |
|---|---|
| Staging, `cputype = 486`, `core = dynamic` against `core = normal` | The same faults for every Pentium-and-later instruction tried (`CMOVcc`, `CMPXCHG8B`, `RDMSR`, `SYSENTER`, `UD2`, `RDTSC`, `CPUID`, `EMMS` invalid) **except dword `CMPXCHG`, which dynamic accepts and normal rejects**; a real 486 has the instruction |
| `BSWAP`, `XADD`, byte `CMPXCHG`, `INVD`, `WBINVD` | Run at 486 on every core; `386` rejects them all; nothing crashed or hung |
| The repository's checks under both cores | Pass identically, with the same digests and goldens: `shell/dos/check.sh` (64 s normal, 53 s dynamic), `spikes/d0-rust-dos/check.sh` (26 s, 22 s), `spikes/b-dos-input/check.sh` (79 s, 80 s); most of the time is Rust builds, and DOSBox ran the shell scenario in 1.9 s on normal and 0.76 s on dynamic |
| Differences | `RETSTART.EXE` (a program returning from `_start`) aborts under normal ("Pagefault didn't correct table", status 125) and exits 1 under dynamic, and the check needs only non-zero; the live runs' key counts varied (21, 23 or 24) under both cores, an existing flakiness the checks tolerate |
| DOSBox-X | More lenient: its `486` accepts `CPUID` and dword `CMPXCHG` and rejects `RDTSC`, `CMOV`, `CMPXCHG8B`, `EMMS`; `486_prefetch` needs the normal core; `SHELL.EXE depth8 scenario dump` matched `tests/shell/expected-dos-8.txt` in six runs. `tools/dosrun.sh` stays on Staging for the strict check |

**The codegen experiment** (in the CI image, `ci-dos@sha256:6b8c6308`, Rust 1.98.0, `cargo rustc --release -Zbuild-std=core -Zjson-target-spec -- --emit=asm` on a `no_std` crate with `AtomicU32::compare_exchange`, `fetch_add` and `swap`; not committed):

| Target variant | `compare_exchange` | `fetch_add` | `swap` |
|---|---|---|---|
| `kernel/i486-desk-none.json` as is | `lock cmpxchgl` | `lock xaddl` | `xchgl` |
| The same with `"cpu": "i386"` and `llvm-target` `i386-unknown-none-elf` | `lock cmpxchgl` | `lock xaddl` | `xchgl` |
| With `"singlethread": true` (on i486 and on i386) | `lock cmpxchgl` | `lock xaddl` | `xchgl` |
| With `"atomic-cas": false` | does not compile: `compare_exchange`, `fetch_add` and `swap` do not exist | — | — |
| With `"max-atomic-width"` 0 or 16 | does not compile: `AtomicU32` does not exist | — | — |

No helper call (`__sync_*`, `__atomic_*`) appeared in any variant, and `compiler_builtins` 1.98 provides `__sync_*` helpers only for ARM (`src/sync/arm_linux.rs`, `thumbv6k.rs`), none for x86. So **no target-specification setting found makes LLVM lower 32-bit x86 atomics to library calls**: with 32-bit atomics enabled, `compare_exchange` is inline `lock cmpxchg` even for an i386 CPU, and the switches that remove it remove the atomic types as well. (Other LLVM options were not searched exhaustively; none is known to the author.)

**What that means for one file on both tracks.** A `.DMD` or `.DKX` is the same bytes on both tracks, so an inline `lock cmpxchg` anywhere in it faults under `tools/dosrun.sh`'s DOSBox. Locks are only part of it: in the 1.98 sources `compare_exchange` is also used by `alloc`'s `Arc` (five sites in `alloc/src/sync.rs`, `Weak::upgrade` among them), `std::sync::mpsc`'s channels, the thread id counter, the panic count and `process::exit`, and by any crate an app uses.

**What each atomic operation compiles to** (the same experiment, `kernel/i486-desk-none.json`, `-Zbuild-std=core,alloc`):

| Rust operation (`AtomicU32`) | i486 code | Runs in DOSBox's 486 mode (by the test table) |
|---|---|---|
| `load`, `store` | plain `mov` | Yes |
| `swap` | `xchgl` | Yes (the runtime table) |
| `fetch_add` with the result used | `lock xaddl` | Yes |
| `fetch_sub` or `fetch_add` with the result unused | `lock decl` or `lock incl` (or `lock addl`) | Yes (the runtime table) |
| `fetch_or` with the result unused | `lock orl` | Yes (the runtime table) |
| `fetch_or`, `fetch_and` with the result used | a `lock cmpxchgl` loop | No |
| `fetch_max`, `fetch_update` | a `lock cmpxchgl` loop | No |
| `compare_exchange`, `compare_exchange_weak` | `lock cmpxchgl` | No |
| `alloc`'s `Arc::clone` and drop | `lock incl`, `lock decl` | Yes |
| `alloc`'s `Arc::downgrade`, `Weak::upgrade`, `Arc::get_mut` | `lock cmpxchgl` | No |


**A runtime run of the same operations** (an agent's pure-Rust DOS program on `rt`, one test per run; the CI image, rustc 1.98.0, DOSBox Staging 0.83.0, QEMU 8.2.2 with FreeDOS; nothing on real hardware). Builds: `a` the repository's `dos-elf.json` (`i486`, atomic width 32); `b1` `cpu: i386`; `c1` `i386` with `-cx8,-cmov`; `b2` `b1` with `atomic-cas: false`; `b4` `i486` with `atomic-cas: false`; `b3` `max-atomic-width: 0`. `a`, `b1` and `c1` compile to the same code, with no `__sync_*` or `__atomic_*` call, and LLVM has no feature for 32-bit `cmpxchg` (only `cx8` and `cx16`).

| Operation | Emitted | DOSBox `cputype = 486`, `core = normal` | DOSBox `pentium` | QEMU `-cpu 486` |
|---|---|---|---|---|
| `compare_exchange`, `compare_exchange_weak`, `fetch_or` (result used), `fetch_and`, `fetch_xor`, `fetch_max` | `lock cmpxchg` | Invalid opcode (no output, exit status 1) | OK | OK |
| `fetch_add`, `fetch_sub` | `lock xadd` | OK | OK | OK |
| `fetch_add` and `fetch_or` with the result unused, `fence` | `lock add`, `lock or` | OK | OK | OK |
| `swap`, `store` (sequentially consistent), `load` | `xchg`, `mov` | OK | OK | OK |
| `AtomicU8::compare_exchange` | byte `lock cmpxchg` | OK | OK | OK |
| A test-and-set spinlock from `swap`, a ticket lock from `fetch_add` | as above | OK | OK | OK |
| Builds `b2` and `b4` (`atomic-cas: false`) | — | Only `load`, `store` and `fence` compile; `swap` and `fetch_add` fail with "no method named … found" | | |
| Build `b3` (`max-atomic-width: 0`) | — | All 16 tests fail to compile (`unresolved imports AtomicU32, AtomicU8`) | | |

So `desk-hal` itself would not build with `atomic-cas: false` (it uses `swap` in `ring.rs` and `fetch_add` in `time.rs`), and the repository's `shell/` and `hal/` already avoid compare-and-swap, which is why the first window runs in DOSBox's 486 core.

**The options considered**, kept as the record (the owner first chose (a) with the trap below, then simplified):

- **(a) One `std` build, `std`'s own futex locks, the compare-and-swap through a helper per track**: In `std`'s futex-based `Mutex`, `Condvar`, `RwLock`, `Once` and `Parker` the atomic word's type is the platform's `futex::Futex` and `futex::SmallFutex` (the five files name only those aliases; read in the sources, not built). The Desk platform layer can define them as its own `u32` wrapper whose `compare_exchange` and `compare_exchange_weak` call an imported `desk_cas32(addr, expected, new) -> previous` and whose `load`, `store`, `swap`, `fetch_add` and `fetch_sub` stay inline (`xchg` and `lock xadd` run in DOSBox's 486 mode). The five `sync` files then compile unchanged. On native the kernel binds `desk_cas32` to a `lock cmpxchg`; on hosted to a compare and store, which is atomic there because green threads switch only at calls and `rt`'s interrupt handlers never touch an app's lock words, with the virtual interrupt flag cleared around it (DPMI 0900h and 0901h) as the owner's masking form for any word an interrupt might share. The cost is a call per lock acquire, tens of cycles against an inline instruction (an estimate). Plus the trap below for everything else.
- **(b) A cooperative `sys::sync` selection on the hosted layer**: locks with no atomics, correct because green threads switch only at calls. The selection is made when `std` is compiled, so it means a second build of every `std` app.
- **(c) Locks written with `swap` and `fetch_add` only**, so `std`'s locks emit no `CMPXCHG` on either track. From the sources: `Condvar` already uses only `fetch_add` and `load`; `Mutex` uses `compare_exchange` only for its uncontended acquire and `try_lock`, and an exchange-only mutex is a known design (lock: `swap(1)`, then `swap(2)` and wait while the old value is not 0; unlock: `swap(0)`, wake if it was 2), whose `try_lock` needs care (after a `swap(1)` that saw 2, a `swap(2)` either takes the lock or restores the waiters' state), so it is straightforward but wants its own tests; the `Parker` uses one `compare_exchange` that a `swap` can replace; `RwLock` packs readers and writer bits into one word and updates it with `compare_exchange` loops, and `Once` moves between states with `compare_exchange_weak`, so both need a redesign, most simply over the exchange-only mutex and a condition variable, slower than `std`'s. What (c) cannot do, measured: remove `CMPXCHG` from `alloc`'s `Arc` (`downgrade`, `upgrade`, `get_mut`), from `std`'s channels and thread ids, or from any crate. The target-level halves the owner asked about do not help either: `"atomic-cas": false` removes `swap` and `fetch_add` as well as `compare_exchange` (so (c)'s own locks would not compile, and `alloc::sync` needs compare-and-swap; the latter inferred, not built), and an `i386` CPU setting still emits inline `lock cmpxchg`, with no `__sync_*` or `__atomic_*` call and no such helper in `compiler_builtins` for x86.

| | (a) Helper | (b) Hosted selection | (c) Swap and add only |
|---|---|---|---|
| Files | One for both tracks | Two builds | One for both tracks |
| `std`'s locks | Unchanged (a wrapper type) | Replaced on hosted | Replaced on both: `Mutex` and `Parker` rewritten, `RwLock` and `Once` redesigned |
| `CMPXCHG` elsewhere (`Arc`, channels, crates) | Emulated by `rt` | Still there | Still there: emulated by `rt` |
| Cost on the lock path | A call to `desk_cas32` per acquire (tens of cycles, estimated) | None | None |
| Patch to maintain per Rust release | Arm lists and the wrapper | A hosted arm and lock code | Arm lists and four lock implementations of our own |
| One file, both tracks | Allowed | Not allowed | Allowed |

**Decided (simplified): `std` apps keep inline `lock cmpxchg`.** A real 486 has the instruction, QEMU's `486` model and DOSBox's dynamic core run it, so `std` apps and their crates are built as Rust builds them, with `std`'s own futex-based locks unchanged, and there is no `desk_cas32`: it is not in the `desk_*` import set, the SDK version or the platform contract, and the header needs no flag or ABI number (no helper is imported and a loader has nothing to check). The rule that goes with it: **`std` apps' DOSBox runs use `core = dynamic`; the strict `core = normal` runs stay for `shell/`, `hal/` and every existing check**, which already avoid compare-and-swap. The target-level result stands: no Rust setting avoids `cmpxchg` (an `i386` CPU still emits it, and `atomic-cas: false` would remove `swap` and `fetch_add` and so break `desk-hal`).

**Residual risks, plainly.** The dynamic core accepts a dword `CMPXCHG` that the normal core rejects, so a `std` build that passes under dynamic fails under normal: hence the rule, and a `std` app is never run under the normal core. The patch then needs only `desk` in the five futex arm lists and a `sys/pal/desk/futex.rs` with the four types as plain `Atomic<u32>` and `futex_wait`, `futex_wake` and `futex_wake_all` over `desk_wait` and `desk_wake`. The early i486 A-step is said to have used a different `CMPXCHG` encoding (`0F A6`/`A7`) from later steppings; that is unverified here and nothing is claimed about it. QEMU's `486` model accepts instructions a real 486 lacks (it is not a strict model), and DOSBox's dynamic core is strict about everything tried except dword `CMPXCHG`; neither is silicon.

**Refusing a file that contains `CMPXCHG`.** No: nothing refuses it, because the instruction is valid on the target. A packer check would be fragile anyway: the bytes `0F B1` can occur inside other instructions' immediates and displacements, so a byte scan has false positives, and a sound answer needs an instruction-length decoder sweeping each function from its symbol, a few hundred lines that the packer does not otherwise need. If a report is wanted, `llvm-objdump -d` of the kept ELF in a CI step is the cheap form, as a warning.

### Verified option, not planned: the hosted `CMPXCHG` trap

Kept for the day a need appears (a strict-486 CI requirement, or a real-hardware finding about some stepping): `rt` can emulate dword `CMPXCHG` from an invalid-opcode handler, and an experiment showed it working in emulators. `rt` installs a DPMI exception handler (function 0203h) for the invalid opcode; when the faulting bytes are `0F B1 /r`, with or without an `F0` prefix, it performs the compare and exchange, sets the flags and `EAX` as the instruction would, and resumes after it. A fault-delivery experiment (a pure-Rust DOS program on `rt`, `spikes/d0-rust-dos/udt` in a scratch copy, not committed; the CI image, rustc 1.98.0, DOSBox Staging 0.83.0, QEMU 8.2.2, CWSDPMI r7; emulators only) settled it:

| Question | Result |
|---|---|
| Does exception 6 reach a 0203h handler under CWSDPMI in DOSBox's `486` and `486_prefetch` modes with `core = normal`? | Yes; the host passes it to the handler and does not end the program first |
| What does the handler see? | Frame dwords in order: return `EIP`, return `CS`, error code (6), `EIP`, `CS`, `EFLAGS`, `ESP`, `SS`; the faulting `EIP` is the start of the instruction (bytes seen `f0 0f b1 4c 24 30`, `lock cmpxchg [esp+0x30], ecx`); the general registers as at the fault |
| How does it resume? | The handler returns with `retf` to a trampoline that restores `EAX` and the flags and jumps to the next instruction; without the trampoline `EAX` is lost |
| Is the emulation right? | Yes in 12 forms (with and without `lock`; `[reg]`, `disp8`, `disp32`, absolute, SIB, `[esp]`, register to register); the flags match `cmp` exactly; `compare_exchange`, `compare_exchange_weak` and a lock loop of 5,000 acquisitions pass |
| What does it cost? | About 25 µs per emulated instruction (10,000 in about 0.25 s) against about 1 µs for a `swap`, roughly 20 times slower; rough, because the timer is noisy and DOSBox ran at `cycles = max` |
| Elsewhere? | QEMU `-cpu 486` under FreeDOS: never invoked (the instruction runs); DOSBox `pentium`: runs natively |

Gotchas: DPMI 0202h (get the previous handler) returns 0:0 under CWSDPMI, and a jump to it crashed DOSBox ("JMP Illegal descriptor"), so there is **no chaining**: a fault the handler does not emulate is reported and the program exits (the experiment used status 4). DOSBox's 486 mode faults only the dword form `0F B1`; the byte form `0F B0` runs, though DOSBox's flags for it lack `PF`. `push ds` in `global_asm!` pushes only 2 bytes, which shifted the first frame offsets. Not covered: a fault inside the handler, interrupts during it, locking the handler's memory (with paging active under a DPMI host), and the `66` operand-size prefix (a 16-bit `cmpxchg`). The handler runs on a flat-data stack of its own with one static context, so it is not re-entrant. That is enough for the cooperative hosted runtime, threads included, because a green thread never switches inside the handler and `rt`'s interrupt handlers never use dword `CMPXCHG`; a preemptive or interrupt-time use would need a context and stack per nesting level. Because of the cost it would be a correctness fallback for code outside the locks, not a fast path.

What building it would take, had it stayed planned (it was the plan's task M10 for a day): `rt::ud::install()` over DPMI 0203h with the trampoline, registered on the clean-up list; a module that calls `compare_exchange` directly, run in DOSBox's normal 486 core through the trap and in QEMU natively with equal lines; negative runs without the trap (non-zero exit) and with an instruction the handler does not emulate (`ud2`, or a `66`-prefixed `cmpxchg`: the report and status 4).

**Measured and not.** Measured: the instruction table (the coordinator's test), the two codegen tables (the CI image), the runtime table, the fault-delivery table and the cores table (agents' runs in DOSBox, DOSBox-X and QEMU). Read, not built: the `sync` sources' use of compare-and-swap. Inferred: that `alloc::sync` needs compare-and-swap support in the target. The fault-delivery table (an agent's runs). Not: a `std` app built for the Desk target at all; the trap in the repository's `rt` (the experiment's copy is not committed) and the cases it did not cover; whether the five `sync` files compile against a wrapper type; CWSDPMI's delivery of the exception under DOSBox; real hardware; and whether QEMU's 486 model matches any real 486 stepping.

## Maintenance

The patch follows each Rust release: [`E-STD-STUBS.md`](E-STD-STUBS.md#5-maintenance-on-rust-upgrades) measured it applying cleanly to 1.98.0 and 1.97.0 and partly to 1.93.0 to 1.96.1, the drift being in the `cfg_select!` arm lists and in internal items the overlay calls, and estimated one rebase by hand per release (minutes), a rebuild and the checks. The threaded arms add five hunks to the lists that move, so the cost grows a little. E proposed a CI job that applies the patch to the next toolchain, so the drift is seen before a bump; that is still the recommendation. An upstream Tier 3 target (R71) would move the arms into Rust's own sources, where Rust's changes carry them, at the price of upstream review and of keeping the target building in Rust's CI on a best-effort basis.

## Staging

1. **The loader spike (R48) stays `no_std`.** Its modules use `desk-guest`.
2. **The hosted shim path first.** It already builds (E's 12-second `std` build); the steps are the `desk_*` import slots instead of `extern` blocks, `std::fs` over `rt::dos`, `fn main` through `lang_start`, and an app packed as a `.DKX` and run by the hosted runtime. Threads come with the hosted green threads.
3. **Native after tasks and threads exist** (R77, R141): the same file, the functions resolved by the kernel.
4. **A planned small task with the hosted step:** `tools/dosrun.sh` takes the DOSBox core from a variable (`DOS_CORE`, default `normal`, so nothing changes for the existing checks), the `std` apps' checks set `DOS_CORE=dynamic`, and the "Headless DOSBox" section of [`TOOLCHAIN.md`](TOOLCHAIN.md#headless-dosbox) states the rule. The matching wording for `AGENTS.md`'s testing section is a proposed edit for the owner, not made here.

## Questions for the owner

1. **One target for both tracks.** Default: yes, `i486-unknown-desk`.
2. **Decided: inline `lock cmpxchg` in `std` apps** ([above](#cmpxchg-on-the-hosted-track)): no helper; `std` apps run under DOSBox's dynamic core and the strict normal core stays for `shell/`, `hal/` and the existing checks.
3. **`fn main` or `desk_main`.** Default: the file's entry stays `desk_main`, which calls `std`'s `lang_start` and so the app's `fn main`, so `std` initialises as it expects (E's open item).
4. **Thread-local keys or native thread-locals first.** Default accepted by the owner: keys, which need no format change.
5. **A patched sysroot or an upstream target.** Default accepted by the owner: the patched sysroot for years; upstream (R71) only when the target is stable and someone will maintain it there.
