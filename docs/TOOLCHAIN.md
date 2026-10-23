# Toolchain

How Desk is built for DOS and checked without a DOS machine. This describes what exists and was run: the walking skeleton, a small program in Rust and C that `tools/check.sh` builds and runs under headless DOSBox Staging, in CI and on a developer's machine. The skeleton is the seed of the real Desk: Rust `no_std` with `alloc` for the logic, a C layer (DJGPP) for the hardware and the C runtime, linked into one 32-bit protected-mode `.EXE` that runs under CWSDPMI. It borrows lessons from [`ScottMorris/lieutenant-fizz-dos`](https://github.com/ScottMorris/lieutenant-fizz-dos) ("Fizz DOS"), whose `docs/RUST_ON_DOS.md` is the longer guide; the code here is written for this project.

## Contents

- [The pieces](#the-pieces)
- [Running it](#running-it)
- [The pipeline](#the-pipeline)
- [The target specification](#the-target-specification)
- [The boundary between Rust and C](#the-boundary-between-rust-and-c)
- [The runtime glue](#the-runtime-glue)
- [Converting the objects](#converting-the-objects)
- [Floating point and the x87](#floating-point-and-the-x87)
- [Headless DOSBox](#headless-dosbox)
- [What the checks cover](#what-the-checks-cover)
- [Differences from Fizz DOS](#differences-from-fizz-dos)
- [Known limits](#known-limits)
- [What was and was not verified](#what-was-and-was-not-verified)

## The pieces

| Piece | Version | Where it comes from |
|---|---|---|
| Rust | 1.98.0 with `rust-src` | `rust-toolchain.toml`; `rust-src` is what `-Zbuild-std` compiles `core` and `alloc` from |
| DJGPP | build-djgpp v3.4, GCC 12.2.0 (`i586-pc-msdosdjgpp-gcc`) | `vendor/djgpp/`, or `/opt/dos-toolchain/djgpp/` in the CI image |
| DOSBox Staging | 0.83.0 | `vendor/dosbox/`, or `/opt/dos-toolchain/dosbox/` |
| CWSDPMI | r7 (`csdpmi7b.zip`) | `vendor/cwsdpmi/`, or `/opt/dos-toolchain/cwsdpmi/` |
| Python 3 | any recent | runs `tools/coff2go32.py` and `tools/check-docs.py` |

The versions and SHA-256 sums of the three DOS tools are those of the `ci-dos` stage in `docker/ci/Dockerfile` in `liminal-hq/.github`, which builds the image the CI runs in (`ghcr.io/liminal-hq/ci-dos`, pinned by digest in `.github/workflows/ci.yml`). `tools/fetch-tools.sh` repeats them so a local build uses the same bytes; a version change is made in the Dockerfile first and copied here. The image already carries Rust 1.98.0 with `rust-src`, `python3` and `libfl2` (which DJGPP's binaries load), so the CI needed no change to the image.

## Running it

```bash
tools/setup.sh     # once: checks host tools, installs the pinned Rust pieces, links or downloads vendor/
tools/check.sh     # headers, docs, Rust format and lints, build, headless run, panic run
```

Individual steps:

```bash
tools/build.sh                          # dist/DESK.EXE and dist/CWSDPMI.EXE
tools/dosrun.sh dist DESK.EXE           # run it headless; prints its output, returns its exit status
tools/dosrun.sh dist DESK.EXE panic     # the deliberate-panic path: exit status 3
```

`tools/setup.sh` installs nothing system-wide and never installs a host package; it names the package a missing tool needs. When `DOS_TOOLCHAIN` is set (the CI image sets it) it links `vendor/` to that directory instead of downloading. A clean build takes about 10 seconds including `-Zbuild-std`, and a rebuild plus the run about one second.

## The pipeline

```
core/ (Rust, no_std)        cargo build --release, -Zbuild-std, target core/dos.json
        │  core/target/dos/release/libdesk_core.a    PE-flavoured i386 COFF
        ▼
tools/coff2go32.py          fix REL32 addends, fold sections, drop compiler_builtins
        │  dist/obj/desk_core_go32.a                 go32 COFF
        ▼
i586-pc-msdosdjgpp-gcc      c/main.c c/out.c c/panic.c + the archive
        │
        ▼
dist/DESK.EXE  +  dist/CWSDPMI.EXE
```

`tools/build.sh` is the reference and is short. The cargo options live in `core/.cargo/config.toml` (`build-std = ["core", "alloc", "compiler_builtins"]`, `json-target-spec = true`, `target = "dos.json"`), and the script sets `RUSTC_BOOTSTRAP=1` for that one command so the pinned stable toolchain accepts them. `core/` is its own cargo workspace because that config makes `dos.json` the default target for everything beneath it. The profile is `opt-level = "s"`, LTO and one codegen unit, so the static library holds one object for all the Rust code.

Cargo downloads a few crates (`libc`, `gimli` and others) because the standard library sources have dependencies of their own; the build needs network access the first time, and `Cargo.lock` is committed and used with `--locked`.

## The target specification

Rust has no DOS target, so `core/dos.json` describes one. It borrows LLVM's 32-bit MinGW triple (`i486-pc-windows-gnu`): LLVM then writes i386 COFF objects whose C symbols carry a leading underscore, which is DJGPP's convention too (Rust's `malloc` links to DJGPP's `_malloc`). The objects are close to what DJGPP's linker reads but not identical; see [Converting the objects](#converting-the-objects).

| Field | Value | Why |
|---|---|---|
| `llvm-target`, `arch`, `os`, `vendor`, `env`, `binary-format`, `is-like-windows` | `i486-pc-windows-gnu`, `x86`, `windows`, `pc`, `gnu`, `coff`, `true` | The MinGW flavour of 32-bit x86 COFF; `is-like-windows` also selects the `.text$name` section-per-function layout and Windows-style stack probe calls |
| `cpu` | `i486` | The lowest CPU the code may assume: no `cmov`, no `cmpxchg8b`, no `rdtsc` |
| `features` | `-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx` | Turns every SIMD extension off so floating point goes through the x87 and nothing faults on a 486. The FPU exists on the target, so soft float is not enabled (`rustc-abi` is left at its default) |
| `data-layout` | `e-m:x-p:32:32-…-f80:32-n8:16:32-a:0:32-S32` | The i686 Windows layout. `m:x` is x86 COFF name mangling; `S32` a 4-byte stack alignment. rustc checks it against LLVM's own and refuses a mismatch |
| `target-pointer-width`, `target-c-int-width` | `32`, `32` | ILP32, as DJGPP's C |
| `max-atomic-width` | `32` | A 486 has a 32-bit `cmpxchg` but no 64-bit one |
| `panic-strategy` | `abort` | There is no unwinder; a panic goes to the `#[panic_handler]` |
| `relocation-model` | `static` | DJGPP programs are not position independent |
| `disable-redzone`, `frame-pointer` | `true`, `may-omit` | No red zone below the stack pointer (harmless on x86-32, kept for safety); `ebp` stays free for the register-starved CPU |
| `stack-probes` | `{"kind": "none"}` | No inline probes. Large frames still call `_alloca` (see [Known limits](#known-limits)) |
| `executables`, `dynamic-linking` | `false`, `false` | The target only builds libraries; DJGPP's gcc links the program, so rustc's linker fields were dropped |

Cargo names the output directory after the file stem, so the library lands in `core/target/dos/release/`.

## The boundary between Rust and C

Only integers, pointers and lengths cross it (`AGENTS.md`, `SDK.md`): no structs by value, because struct passing is where the Windows-flavoured LLVM and DJGPP's gcc are most likely to disagree. Both use cdecl on i386.

- C calls Rust: `desk_main(char *out, size_t capacity)` writes the result line into C's buffer and returns its length (or -1 if it does not fit), and `desk_selftest_panic(void)` panics on purpose. C declares them `extern`; the Rust side is `#[no_mangle] extern "C"`. `desk_main` is `unsafe` in Rust because it writes through a pointer.
- Rust calls C: `malloc`, `free`, `realloc` and `desk_panic(unsigned line)`, declared in an `extern "C"` block.
- There is no Rust entry point. DJGPP's start-up code runs C's `main`, which calls Rust; any one-time Rust set-up would go in the first function C calls.

## The runtime glue

The Rust object leaves seven symbols for C to define. `i586-pc-msdosdjgpp-nm -u` on the converted archive lists them:

| Symbol | Provided by | Why it is needed |
|---|---|---|
| `_malloc`, `_free`, `_realloc` | DJGPP's libc | The global allocator in `core/src/lib.rs`. `malloc` returns 8-byte aligned blocks, so an allocation needing more alignment is refused with null. Never allocate from an interrupt handler: DJGPP's `malloc` is not reentrant |
| `_memcpy`, `_memset` | DJGPP's libc | `compiler_builtins` does not export them on a Windows-flavoured target, and the objects that would are dropped (see below), so libc's are used |
| `___udivdi3` | DJGPP's libgcc | 64-bit division, from the float formatter. The 32-bit target has no instruction for it |
| `_desk_panic` | `c/panic.c` | The panic handler's exit. Rust passes the source line only: formatting the message would pull `core::fmt` into the path that must not fail. It prints the report through `desk_emit`, then `exit(3)`. Once something hooks interrupts or sets a video mode, restoring them belongs here |

`c/out.c` (`desk_emit`) prints a line to stdout and appends it to `OUT.TXT` in the current directory. The file exists because capturing a DOS program's stdout through DOSBox is fiddly; the checks read both and require them to agree. Anything an interrupt handler touches must be `volatile` and no handler may allocate (`AGENTS.md`); nothing is hooked yet.

Nothing else was needed. In particular there is no `_alloca`, `-lm` or `-fno-toplevel-reorder`: see [Differences from Fizz DOS](#differences-from-fizz-dos).

## Converting the objects

DJGPP's `ld` reads only `coff-go32`. rustc's COFF for a `*-windows-gnu` target is PE-flavoured, which `ld` opens without complaint and links without a single error, and the program is then wrong. `tools/coff2go32.py` repairs two things and leaves one object out. It was checked against Rust 1.98.0 by turning each part off in turn:

| Part | Without it |
|---|---|
| The REL32 addend patch (a PC-relative `call` or `jmp` field must hold the addend minus the field's offset plus 4 in go32; PE stores just the addend) | A page fault inside DOSBox, even with the sections folded |
| Folding `.text$name`, `.rdata`, `.xdata` and the rest into `.text`, `.data` and `.bss` | The program links, runs and prints nothing at all |
| Dropping the `compiler_builtins` objects (their weak externals are not understood by coff-go32; libc and libgcc supply the same helpers) | A flood of linker warnings ("local symbol has no section"); not tested on its own, since the program already crashes without the other two fixes |

So the converter is still needed with the pinned toolchain. The script reads and writes the archive with DJGPP's `ar`, so it needs no host `ar`. If a later Rust release changes what it emits, repeat that experiment before assuming the script still holds.

## Floating point and the x87

`desk_main` computes `fsqrt` of an integer sum with inline assembly, since `core` has no `sqrt` (the float methods live in `std`), and prints it with `{:.6}`, which runs core's float formatter. Rust does not allow an x87 register as an inline-assembly operand, only as a clobber, so the value goes through memory (`fld`, `fsqrt`, `fstp`).

**The precision-control question from Fizz DOS does not matter here.** The x87 computes in an 80-bit register with a 64-bit mantissa unless the control word says otherwise, so an `f64` expression can round differently from IEEE on WebAssembly or SSE2. Fizz DOS had to match another build bit for bit and set 53-bit precision (control word `0x027F`) before the first float operation. This skeleton prints six decimals of one square root, which is the same either way, and has no second build to agree with, so it does not set the control word. The Desk needs it set only if some float result must match another build exactly; integer and fixed-point code is not affected. If that day comes, set it once in the first function C calls and keep a comparison check, as Fizz DOS does. Not verified: which precision the control word has when DOSBox and CWSDPMI hand over control.

## Headless DOSBox

`tools/dosrun.sh <dir> <command…>` generates a DOSBox configuration, mounts `<dir>` as `C:`, runs the command with its output redirected to a scratch file, and exits. Things it settles:

- **No display is needed.** `SDL_VIDEODRIVER=dummy` and `SDL_AUDIODRIVER=dummy` give SDL a driver that draws and plays nothing. DOSBox Staging 0.83.0 starts and runs normally in the CI container, which has no X server. The script also unsets `DISPLAY` and `WAYLAND_DISPLAY`.
- **`output = texture`.** The old `surface` is deprecated, and the default tries OpenGL, which the dummy driver cannot provide (DOSBox falls back, with a logged error). `texture` goes straight to SDL's software renderer.
- **`core = normal`, `cputype = 486`, `cpu_cycles = max`.** The interpreter is the slowest core and the most predictable. With `cputype = 486` DOSBox raises an exception for an instruction a 486 lacks: a program using `rdtsc` exits with status 255 there and runs on `pentium`. A build that needs a Pentium therefore fails here. `cycles` is deprecated in 0.83; with `cpu_cycles = max`, `cpu_cycles_protected` must be `auto`.
- **Sound, MIDI, the Sound Blaster and the speaker are off.** Audio cannot be checked headless.
- **Exit status.** DOS cannot hand a status to its host and DOSBox Staging does not expand `%ERRORLEVEL%`, so the generated batch file sets `RC` with 255 lines of `if errorlevel n set RC=n`. `IF ERRORLEVEL n` is true for every n up to the status, so the last match is the status. Two statuses are the script's own: 124 for a timeout (`DOS_TIMEOUT`, default 60 seconds) and 125 for a DOSBox failure, whose log is then printed.
- **Output.** Stdout is redirected inside DOS to a file on a second mounted scratch drive (`D:`), so the program's directory only gets what the program writes; the script prints it with the DOS line ends removed.

## What the checks cover

`tools/check.sh` runs, in order: licence headers, markdown style and links, `cargo fmt --check` and `cargo clippy -D warnings` on `core/`, the build, then two runs under DOSBox.

- `DESK.EXE` must exit 0 and print exactly `DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417`, both on stdout and in `OUT.TXT`. The numbers exercise a `Vec` and a `String` through the C heap, a 64-bit multiply, and an x87 square root with core's float formatting.
- `DESK.EXE panic` must exit 3 and report `DESK-SKELETON PANIC at line N` in both places, which proves the panic handler reports through C and a non-zero status survives DOSBox.

Changing the expected line, or breaking the arithmetic in `core/src/lib.rs`, makes `tools/check.sh` fail with the expected and actual lines.

## Differences from Fizz DOS

- **A smaller target specification.** The linker, linker flavour, position-independence and `dll-tls-export` entries are gone; the generated machine code is identical.
- **No `chkstk.c`.** Fizz DOS needed an `_alloca` helper because its Rust code had frames over 4 KB. Nothing here does, so none is carried; see [Known limits](#known-limits).
- **No `libm` crate, no `-lm`.** The one maths operation is an `fsqrt`, and the C side does no maths.
- **No `-fno-toplevel-reorder`.** It keeps C functions in source order so an interrupt handler can be locked as an address range. The first handler brings it back.
- **A converter with one tool.** `tools/coff2go32.py` uses DJGPP's `ar` for both reading and writing.
- **Exit status and `OUT.TXT`.** Fizz DOS captured stdout only. Here the status comes back through the `IF ERRORLEVEL` ladder, and the program writes its own result file.
- **`cputype = 486`** in DOSBox, so a stray Pentium instruction fails the check.
- **`cargo fmt`, `clippy` and `--locked`** are part of the check.

Surprises worth knowing: a link with `-o DESK.EXE` leaves a second `DESK.exe` beside it (DJGPP's gcc adds the lower-case extension), so the build links to `dist/obj/desk` and copies `desk.exe`; Rust refuses an x87 register as an asm operand; and clippy's `not_unsafe_ptr_arg_deref` is why `desk_main` is `unsafe`.

## Known limits

- **Large stack frames.** `"stack-probes": {"kind": "none"}` removes Rust's inline probes, but code generation for a Windows-flavoured target still calls `_alloca` before a frame larger than 4 KB, and DJGPP defines no such helper. The first function that needs it fails the link with `undefined reference to '_alloca'`; the fix, as in Fizz DOS, is a small assembly routine in `c/` that touches each page on the way down. Not exercised here, so not verified.
- **Every `extern "C"` symbol must exist.** With LTO there is one Rust object and the linker takes all of it, so an unused import still has to be defined.
- **DOSBox speed is not 486 speed.** It counts instructions, not clocks, and hides the cost of the x87 and of divisions. `cpu_cycles = max` is for correctness runs only; timing needs a fixed cycle count and an estimate.
- **A crash shows little.** DJGPP's register dump goes to stderr, which the harness does not capture; the exit status (255 for an exception) and `OUT.TXT` are what survive.
- **The DJGPP and DOSBox builds are x86-64 Linux only**, so the CI image and these scripts are too.

## What was and was not verified

Verified:

- `tools/check.sh` passes on the author's Linux machine (Arch, host Rust 1.98.0 via `rustup`, tools in `vendor/`) and in `ghcr.io/liminal-hq/ci-dos` by the digest in `ci.yml`, from a clean copy of the tree with `tools/setup.sh` first.
- The program's output, exit statuses 0 and 3, and the failures when the expected line or the arithmetic is broken.
- DOSBox Staging runs headless with no X server, in the container; each part of `tools/coff2go32.py` is necessary; a Pentium-only instruction faults under `cputype = 486`; a hung program is ended with status 124.

Not verified:

- Anything on real hardware, on a real 486, on an older DOS version, or with a DPMI host other than CWSDPMI.
- The GitHub Actions workflow itself, which runs when the branch is pushed.
- The x87 precision-control state at program start.
- Stack frames over 4 KB, interrupt handlers, video modes, and everything else the skeleton does not do.
