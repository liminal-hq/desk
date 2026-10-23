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

## The std question

`std` is not just a bigger `core`: it needs a platform layer, the `sys` module, to implement console, files, time, environment, arguments, threads, locks and the random seed for `HashMap`. Our targets have no such layer. Options, in rough order of cost:

1. **An experiment with the "unsupported" stubs.** For an unknown OS name, `std` is expected to build with stubs that make I/O and threads return errors. To be confirmed by an experiment; measure what builds and the size cost.
2. **Our own `std` platform layer** over our runtime or the Desk SDK, for a Desk app target, patched into a custom sysroot with a pinned toolchain. Feasibility is to be confirmed by a spike, not assumed.
3. **A thin std-like crate of our own** on `core` and `alloc` (`println!`, `File`, `Instant`). The cheap path.
4. **Reusing the unix platform layer over DJGPP's libc.** Probably not viable, because of missing pthreads and other system calls. Unverified.
5. **An upstream Tier 3 target.** A long-term idea only.

Costs to weigh:

- Maintenance on every Rust upgrade, since `sys` internals are not stable.
- Binary size on a floppy. `panic=abort` and `panic_immediate_abort` are the size levers.
- The licence of any `std` fork.

**Recommended split (a target, not a decision):** the kernel and the low-level runtime stay `no_std` plus `alloc`; apps are where a real `std` pays off.

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
| H: HAL design | Shared crates | Scott Morris | PR #8 | done: the design ([`HAL.md`](HAL.md)) and `hal/` with host-tested 16550, 8254 and 8259 drivers, built for both targets |
| s-std-stubs | Any | Not assigned | None | planned: build `std` for an unknown OS name and measure |
| s-desk-pal | Desk apps | Not assigned | None | planned: design the platform layer and the app ABI |
| prebuilt sysroot | All custom targets | Not assigned | None | idea |
| native-app-target | Desk apps | Not assigned | None | planned |

### Open questions

- Does `std` build for an unknown OS name with the stubs, and how large is the result?
- Which crate type and loader suit a Desk app on the native OS?
- Can a custom `std` platform layer be kept alive across Rust upgrades at a cost we accept?
- Does one DOS target serve both the hosted Desk and Lt. Fizz Episode 2, or do they diverge?
- Which linker per target, and what does a pure-Rust DOS link look like (d0)?
