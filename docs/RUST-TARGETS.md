# Rust targets and std across the application spaces

> **Status:** a tracking document, started 2026-10-22. The hosted DOS target specification (PR #3), the native kernel target specification (PR #6) and the d0 freestanding DOS target (PR #7) exist and are merged; the shared HAL crates (spike H) build for the last two. Everything else here is planned, and every claim marked "to be confirmed" is a target until an experiment says otherwise.

## Purpose and how to update it

This is the single starting point for how Rust is built for each application space and system Desk cares about (see [`DIRECTION.md`](DIRECTION.md) for why there are several). Update it in the same pull request as the work that changes an entry, and mark each entry with one of three states:

- **planned**: nothing exists yet.
- **in a PR**: the work is open on a branch; name the PR.
- **on main**: merged.

Files that live on other branches are written in backticks with their PR number, because they do not exist on this branch: `docs/TOOLCHAIN.md` and `core/dos.json` (PR #3), `docs/S1-DXE3.md` (PR #4), `docs/NATIVE-BOOT.md` and `kernel/i486-desk-none.json` (PR #6), and `docs/D0-RUST-DOS.md` once d0 lands. Turn them into links when they reach `main`.

## The concept

A custom Rust **target specification** (a JSON file) tells rustc and LLVM what the code is compiled for. It is passed with `--target file.json`, or through the cargo `build.target` setting.

rustup ships no precompiled `core` for such a target, so cargo builds `core`, `alloc` and `compiler_builtins` from the `rust-src` component with `-Zbuild-std`. Both `-Zbuild-std` and `-Zjson-target-spec` are unstable. The repository therefore pins the toolchain (1.98.0 in `rust-toolchain.toml`) and sets `RUSTC_BOOTSTRAP=1` for those builds only. Specification fields change between rustc versions, which is why the specs are kept minimal.

### The three library layers

| Layer | What it needs from the target | Notes |
|---|---|---|
| `core` | Nothing | Language primitives, no allocation, no I/O |
| `alloc` | A global allocator | `Vec`, `String`, `Box`, `Rc`; the runtime supplies the allocator |
| `std` | A platform abstraction layer (the `sys` module) | Console, files, time, environment, arguments, threads, locks, and a random seed for `HashMap` |

### The fields of `core/dos.json` (PR #3)

| Fields | What they say, and why |
|---|---|
| A borrowed `i486-pc-windows-gnu` triple and `os: windows` | LLVM has no MS-DOS OS, and DJGPP emits COFF like MinGW. This is also why 32-bit symbols carry a leading underscore and why objects still need `tools/coff2go32.py` |
| `cpu: i486`, SSE and MMX features off | The target machine has no SSE or MMX |
| 32-bit pointers | Protected-mode flat 32-bit code |
| `max-atomic-width: 32` | A 486 has no 64-bit compare-and-swap |
| `panic-strategy: abort` | No unwinding; smaller code |
| Static relocation | No dynamic loading by the OS |
| No stack probes | Frames over 4 KB need an `_alloca` helper |
| `executables: false` | We build static libraries for the C layer to link, not programs |

The native kernel specification (`kernel/i486-desk-none.json`, PR #6) differs: ELF output, OS none, and linking with `rust-lld` or our own `elf2bin.py` instead of DJGPP.

## Matrix of application spaces

| Space | What it is | Target spec and where | Crate type | Layers | Linker and packaging | Runtime or platform layer | Tested by | Status |
|---|---|---|---|---|---|---|---|---|
| (a) Desk hosted on DOS, program | The shell on DOS | `core/dos.json` (PR #3) | `staticlib` | `core`, `alloc` | DJGPP gcc links with a C layer; `coff2go32.py` patches the objects; CWSDPMI. Direction: pure Rust via d0 | C layer and DJGPP libc today | DOSBox Staging, headless | in a PR (PR #3); pure-Rust route planned (d0) |
| (b) Desk hosted on DOS, apps as DXE3 modules | Apps loaded by the core | The same spec (PR #3) | Module objects | `core`, `alloc` | `dxe3gen` | The core's exported symbols | DOSBox Staging | in a PR (PR #4): yes, with limits (`docs/S1-DXE3.md`) |
| (c) Desk native kernel | A bootable kernel | `kernel/i486-desk-none.json` (PR #6) | Kernel binary | `core`, `alloc` | ELF, `rust-lld` or `elf2bin.py` | Our own kernel and drivers | QEMU `-cpu 486`, later 86Box and Bochs | in a PR (PR #6; `docs/NATIVE-BOOT.md`) |
| (d) Desk apps | Apps on the native OS | A target such as `i486-desk-app` | To decide | `core`, `alloc`, and `std` over the SDK or a syscall ABI | To decide | A platform layer over the Desk SDK | To decide | planned |
| (e) Lt. Fizz for DOS, Episode 1 | The existing game, in `ScottMorris/lieutenant-fizz-dos` | Its own `m2/dos.json` | `staticlib` | `core`, `alloc` | DJGPP, `coff2go32.py`, a C layer | C layer and DJGPP libc | Its own CI | exists elsewhere; the origin of this approach and a candidate to migrate to the shared machinery |
| (f) Lt. Fizz, Episode 2 for DOS | A new game | Planned: the shared DOS target | Program | `core`, `alloc` (maybe a thin std-like crate) | Rust-only link and a CWSDPMI packer | The shared runtime and drivers | DOSBox | planned |
| (g) Lt. Fizz on the web | The existing wasm build, in `liminal-hq/lieutenant-fizz` | `wasm32` | Library for the browser | Reference only | Browser | Browser | Reference only | exists elsewhere; reference only |
| (h) Host tools | The font build (Bun and TypeScript), and any Rust host tools such as a packer | The host target | Ordinary crates | `std` | Host toolchain | The host OS | Host test runs, for example a native 32-bit bench as Fizz DOS has | planned for Rust tools; the font build exists |
| (i) Shared HAL and drivers | The HAL traits, the drivers written once against them, and the mock platform (`hal/`, [`HAL.md`](HAL.md)) | The host for tests; `kernel/i486-desk-none.json` and `spikes/d0-rust-dos/dos-elf.json` for the cross builds | `rlib` | `core` (the mock adds `alloc`); a `std` feature for host builds only | Linked into the kernel or a DOS program by their own builds | The platform: `rt` under DOS, the kernel natively, `desk-hal-mock` in tests | `cargo test` on the host, `cargo check` and `clippy` for both targets with `-Zbuild-std` | in a PR (PR #8, spike H): traits, 16550, 8254 and 8259 drivers; no binary uses them yet |
| (j) The shell crates | The shared core's first piece: `desk-gfx` (rectangles, the back buffer, damage, the compositor, frame digests) and the crates that follow it (`shell/`, [`FIRST-WINDOW.md`](FIRST-WINDOW.md)) | The host for tests; `kernel/i486-desk-none.json` and `spikes/d0-rust-dos/dos-elf.json` for the cross builds | `rlib` | `core` and `alloc`; a `std` feature for host builds only | Linked into the kernel or a DOS program by their own builds | The platform, through the `Present` trait and the HAL's upper-edge types | `cargo test` on the host, `cargo check` and `clippy` for both targets with `-Zbuild-std` | in a PR (task T1 of [`FIRST-WINDOW-PLAN.md`](FIRST-WINDOW-PLAN.md)): `desk-gfx` only; no binary uses it yet |
| (k) The shell on DOS | `SHELL.EXE`: the shell's scenario at both depths on the pure-Rust DOS runtime (`shell/dos/`, [`FIRST-WINDOW.md`](FIRST-WINDOW.md)) | `spikes/d0-rust-dos/dos-elf.json` | `bin` (own workspace, `elf2go32.py` behind DJGPP's stub) | `core` and `alloc` | `rt` by path, `rust-lld`, `tools/elf2go32.py`, `CWSDPMI.EXE` | `rt` and `DosDisplay` | DOSBox Staging, headless: the frame buffer and DAC read back and compared with the goldens | in a PR (task T9 of [`FIRST-WINDOW-PLAN.md`](FIRST-WINDOW-PLAN.md)): both depths match the goldens; not run on hardware |

## The std question

`std` is not just a bigger `core`: it needs a platform layer, the `sys` module, to implement console, files, time, environment, arguments, threads, locks and the random seed for `HashMap`. Our targets have no such layer. Options, in rough order of cost:

1. **The "unsupported" stubs.** Tried in spike E (`docs/E-STD-STUBS.md`): with Rust 1.98.0 an invented OS name does **not** build, because four pieces have no fallback (the system allocator, the I/O error mapping, the thread-local keys and the random function). With those supplied, the rest is stubs: `println!` is silent, clocks, `sleep` and `process::exit` abort. Not a usable `std` on their own.
2. **Our own `std` platform layer** over our runtime or the Desk SDK, for a Desk app target, patched into a copy of the sources that cargo reads through `__CARGO_TESTS_ONLY_SRC_ROOT`. **Works** (spike E): 55 added lines in 12 existing files, about 340 lines of new files, and 15 `desk_*` C functions; `println!`, `HashMap`, `Instant`, `sleep`, arguments, environment and `exit` run in DOSBox; a hello is 19.5 KB against 9.7 KB for `no_std` (10.2 against 8.2 KB with `-Cpanic=immediate-abort`); the patch needed hand edits on four of five older releases.
3. **A thin std-like crate of our own** on `core` and `alloc` (`println!`, `File`, `Instant`). The cheap path.
4. **Reusing the unix platform layer over DJGPP's libc.** Probably not viable, because of missing pthreads and other system calls. Unverified.
5. **An upstream Tier 3 target.** A long-term idea only.

Costs to weigh:

- Maintenance on every Rust upgrade, since `sys` internals are not stable.
- Binary size on a floppy. `panic=abort` and `panic_immediate_abort` are the size levers.
- The licence of any `std` fork.

**Recommended split (backed by spike E):** the kernel and the low-level runtime stay `no_std` plus `alloc`; apps are where a real `std` pays off, through the patched platform layer with the `desk_*` functions as the contract. A thin std-like crate stays the answer for size-critical programs.

## Cross-cutting ideas to track

- A prebuilt sysroot published from CI, so contributors need no `build-std`.
- The choice of linker per target.
- Reproducible builds and the toolchain pinning policy.
- The licence of any `std` fork.
- What the shared hardware-driver crates need from each target: the port-I/O and IRQ abstraction, designed in [`HAL.md`](HAL.md) (space (i)).

## Spikes and experiments

| Name | Space | Owner | Branch or PR | Status |
|---|---|---|---|---|
| hosted walking skeleton | Hosted | Scott Morris | PR #3 | done: Rust `no_std` plus a C layer, run headless in DOSBox |
| s1-dxe3 | Hosted | Scott Morris | PR #4 (on PR #3) | done: Rust modules as DXE3 files, yes with limits (`docs/S1-DXE3.md`) |
| native walking skeleton | Native | Scott Morris | PR #6 | done: NASM boot sector and a Rust kernel in QEMU with a 486 CPU (`docs/NATIVE-BOOT.md`) |
| d0-rust-dos | Hosted | Scott Morris | PR #7 (on PR #3) | done: yes, a pure Rust DOS program with no gcc, libc or crt0 (a `rust-lld` link, a 139-line packer and a roughly 1,600-line runtime, `docs/D0-RUST-DOS.md`); next: a freestanding ELF target, `rt` as a top-level crate, Rust drivers |
| g-boot-emulators | Native | Scott Morris | PR #16 | done: the native image boots in Bochs as a 486 (three BIOS images) and in 86Box on an AMIBIOS 486; the 86Box run found a loader bug (no E820 or E801), fixed with an AH=88 fallback (`docs/NATIVE-EMULATORS.md`) |
| H: HAL design | Shared crates | Scott Morris | PR #8 | done: the design ([`HAL.md`](HAL.md)) and `hal/` with host-tested 16550, 8254 and 8259 drivers, built for both targets |
| A: VGA and VBE (s2-vbe) | Hosted, shared driver | Scott Morris | PR #11 (on PR #8) | done in DOSBox: the `desk-vga` driver (mode 13h and 12h by registers, DAC, VBE 2.0 over a `Bios` trait) and `rt::hal::DosDisplay`, run by `spikes/a-vga-vbe` (`docs/A-VGA-VBE.md`); not run on hardware |
| B: DOS input (s3-mouse) | Hosted, shared drivers | Scott Morris | PR #15 | done in DOSBox: `rt::irq` (a stub per IRQ line, locked stack, EOI through `desk-pic`), the `desk-int33` driver over `Bios`, `DosInput` handing the core the native kernel's `KeyEvent` and `MouseEvent`, run by `spikes/b-dos-input` with `AUTOTYPE` and, opt-in, `xdotool` under Xvfb (`docs/B-DOS-INPUT.md`); not run on hardware |
| D: native graphics (n2) | Native, shared driver | Scott Morris | on PR #10 and PR #11 | done in QEMU: VBE asked in the loader, `desk-vga` modes 13h and 12h by registers and a VBE linear frame buffer from the kernel, a software cursor and a window, text restore, screenshot tests (`docs/D-NATIVE-VGA.md`); not run on hardware |
| n3: a real BIOS path (native) | Native, shared driver | Scott Morris | PR #17 (on PR #14) | done in QEMU and Bochs: a real-mode trampoline (one NASM page, `kernel/bios16.asm`, embedded in the kernel by `tools/native-build.sh`) and the Rust around it (`kernel/src/bios.rs`) give the kernel `INT 10h`, `13h`, `15h` and `1Ah` after boot, the HAL's `Bios` trait natively, and a way back to text on every QEMU card (`docs/N3-BIOS-PATH.md`); not run on hardware |
| e-std-stubs | Any | Scott Morris | PR #12 | done: the stubs alone do not build; a patched platform layer over `rt` does and runs in DOSBox (`docs/E-STD-STUBS.md`); next: `fn main` through `lang_start`, a DOS-backed `std::fs`, a CI job that applies the patch to the next toolchain |
| s-desk-pal | Desk apps | Not assigned | None | planned: design the platform layer and the app ABI |
| prebuilt sysroot | All custom targets | Not assigned | None | idea |
| native-app-target | Desk apps | Not assigned | None | planned |

### Open questions

- Answered by spike E: no, not in 1.98.0 (four pieces lack a fallback); with a small patch it does, and a hello is 19.5 KB (10.2 KB with `immediate-abort`).
- Which crate type and loader suit a Desk app on the native OS?
- Can a custom `std` platform layer be kept alive across Rust upgrades at a cost we accept? Spike E: the patch applies to 1.97 and 1.98 and needs hand edits for 1.93 to 1.96; a CI job that applies it to the next release would make that a scheduled task.
- Does one DOS target serve both the hosted Desk and Lt. Fizz Episode 2, or do they diverge?
- Which linker per target, and what does a pure-Rust DOS link look like (d0)?
