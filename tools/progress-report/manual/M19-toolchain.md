---
id: m-toolchain
n: M19
part: VII
title: The toolchain and the custom targets
summary: The pinned Rust, the three target specification files compared field by field, build-std, rust-lld and the linker scripts, the two packers that replace objcopy and a linker, NASM, the Docker images and the setup scripts, and the std question.
date: 2026-10-23
reviewed: 44f0265
watches: [rust-toolchain.toml, kernel/.cargo/config.toml, kernel/i486-desk-none.json, kernel/link.ld, spikes/d0-rust-dos/dos-elf.json, spikes/d0-rust-dos/link.ld, spikes/d0-rust-dos/.cargo/config.toml, shell/dos/.cargo/config.toml, core/dos.json, core/.cargo/config.toml, tools/elf2bin.py, tools/native-build.sh, tools/setup.sh, tools/fetch-tools.sh, tools/docker/]
log: [scaffold, wheels, std]
anatomy: [dos:target, dos:lld]
---
## What it is

Rust has no target for a 486 PC that runs no operating system, nor for a program that runs under DOS, so the Desk writes its own. This chapter is about the tools and files that make that possible: the compiler version the repository pins, the three JSON files that describe a machine to the compiler, the options that build the standard pieces of Rust for them, the linker that joins the objects, the two small Python programs that turn a linked file into something a machine can load, and the scripts and container images that supply whatever the host does not have. It is the chapter to read before changing a target, before bumping the compiler, or before asking why the build needs a variable named `RUSTC_BOOTSTRAP`.

On the map of the system ([M1](chapter:m-map)) it sits under both tracks. The [hosted track](term:hosted-track) uses two of the files: `core/dos.json`, the first target, now only the legacy skeleton's, and `spikes/d0-rust-dos/dos-elf.json`, which builds `SHELL.EXE` ([M17](chapter:m-rt)). The [native track](term:native-track) uses `kernel/i486-desk-none.json` ([M4](chapter:m-loader) and [M5](chapter:m-boot) follow what it produces). On the anatomy page of the hosted program the boxes `dos-elf.json` and `rust-lld` are this chapter's. After it you should be able to read any of the three files and say what each field does, say which differences between them are deliberate and which are accidents of history, build either track from a clean machine, and find the Docker image that supplies a tool you have not installed.

## How it works

### The pin, and why a stable compiler needs a variable

`rust-toolchain.toml` names one release, `1.98.0`, and three components: `rustfmt`, `clippy` and `rust-src`. The last is the important one. Rust ships a precompiled `core` for each target it supports, and for none of ours, so the compiler's own sources have to be present for cargo to compile them. That is [build-std](term:build-std). Cargo's reference describes the option as one that "enables Cargo to compile the standard library itself as part of a crate graph compilation", needing the `rust-src` component, and lists a nightly toolchain as a requirement. The repository does not use a nightly. It pins a stable release and sets `RUSTC_BOOTSTRAP=1` in the command that needs the unstable options, which makes the stable compiler behave as if it were a nightly for that command and nothing else (`docs/RUST-TARGETS.md`, "The concept"). The scripts set it per invocation: `tools/native-build.sh` for the kernel, `spikes/d0-rust-dos/build.sh` and `shell/dos/build.sh` for the DOS programs.

The reason to pin is that the unstable options are exactly the ones that change between releases. `docs/RUST-TARGETS.md` says "specification fields change between rustc versions, which is why the specs are kept minimal", and spike E measured the same fragility in `std`'s own sources (below). A new compiler is therefore a deliberate change made in the CI image first and arriving here as a pull request that changes the image's digest (`AGENTS.md` and the log chapter on the scaffold). `tools/setup.sh` reads the channel out of `rust-toolchain.toml` and refuses to continue if `rustc --version` reports anything else.

What the variable does is visible by leaving it out. Cargo reads the `[unstable]` table of a `.cargo/config.toml` only when it believes it is a nightly, so a plain stable run ignores the table and meets the target file unprepared. This is the output of `cargo build --release --locked` in `shell/dos/`, and the same message comes from `kernel/`:

```text
error: `.json` target specs require -Zjson-target-spec to be added to the cargo invocation
```

With `RUSTC_BOOTSTRAP=1` the same command builds. Nothing in the repository tests that the variable is needed beyond this; the claim is that the build works with it and does not without it, on 1.98.0.

> **From the stacks — The standard library, built on demand.** The Cargo reference says that `build-std` makes Cargo compile the standard library as part of the build, using the same profile as the main build. It says the source must be available through the `rust-src` component, and that it must be used with a nightly Cargo and compiler. It describes `-Z json-target-spec` as the flag that allows custom target specification files, and says it usually has to be combined with `build-std`.
> Source: [Unstable features, The Cargo Book](https://doc.rust-lang.org/cargo/reference/unstable.html)

### A target specification, three times

A [target specification](term:target-specification) is a JSON file that tells rustc and LLVM what the code is compiled for: the triple LLVM uses, the processor, which instruction-set extensions it may use, how big a pointer is, whether a panic unwinds, how the linker is called. cargo passes it with `--target`, and the `[build] target` setting in `.cargo/config.toml` makes it the default for everything below that directory, which is why `core/`, `kernel/`, `spikes/d0-rust-dos/` and `shell/dos/` are each their own workspace (`docs/TOOLCHAIN.md` says so for `core/`). The file's name matters twice: rustc rejects a specification it cannot read, and cargo names the output directory after the file's stem, so the three builds land in `core/target/dos/`, `spikes/d0-rust-dos/target/dos-elf/` and `kernel/target/i486-desk-none/`.

The table lists every field that appears in at least one of the three files, 26 in all, read from the files on this branch. A dash means the file does not set the field. Where a document records the reason for a field, the reason column says which; where none does, it says so. The explainer that will draw this comparison is still to be written, so for now the table is the figure.

| Field | `kernel/i486-desk-none.json` | `spikes/d0-rust-dos/dos-elf.json` | `core/dos.json` | Differs, and why |
|---|---|---|---|---|
| `arch` | `x86` | `x86` | `x86` | Same. The 32-bit x86 backend |
| `cpu` | `i486` | `i486` | `i486` | Same. The lowest processor the code may assume: no `cmov`, no `cmpxchg8b`, no `rdtsc` (`docs/TOOLCHAIN.md`; `docs/NATIVE-BOOT.md`) |
| `llvm-target` | `i486-unknown-none-elf` | `i486-unknown-none-elf` | `i486-pc-windows-gnu` | Differs. The two ELF files describe freestanding ELF; `core/dos.json` borrows the 32-bit MinGW triple because LLVM has no DOS and DJGPP's objects are COFF (`docs/RUST-TARGETS.md`) |
| `os` | `none` | `none` | `windows` | Differs. `windows` is a borrowed name; `none` is the freestanding one. For a custom `os` name that is not `none`, `std` does not build without a patch ([E-STD-STUBS](doc:docs/E-STD-STUBS.md#1-does-std-build-for-an-invented-os)) |
| `vendor` | — | `unknown` | `pc` | Differs. No document records why the kernel file omits it |
| `env` | — | `""` | `gnu` | Differs. `gnu` is the MinGW flavour; the empty string and the omission mean the same thing here, and no document says why both exist |
| `binary-format` | — | — | `coff` | Differs. The ELF files take ELF from the triple |
| `is-like-windows` | — | — | `true` | Differs. Selects the `.text$name` section-per-function layout and the Windows-style stack probe calls (`docs/TOOLCHAIN.md`) |
| `features` | `-mmx,-sse,-sse2` | `-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx` | `-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx` | Differs in length only. Every list switches the SIMD extensions off so that floating point uses the x87 and nothing faults on a 486. No document says why the kernel's list is shorter; an i486 has none of the missing names either way |
| `data-layout` | `e-m:e-p:32:32-…-f64:32:64-f80:32-n8:16:32-S128` | the same string as the kernel | `e-m:x-p:32:32-…-i64:64-…-f80:32-n8:16:32-a:0:32-S32` | Differs for `core/dos.json`. `m:x` is x86 COFF name mangling (the leading underscore) and `S32` a 4-byte stack alignment; the ELF pair use `m:e` and `S128`. rustc checks the string against LLVM's and refuses a mismatch (`docs/TOOLCHAIN.md`) |
| `target-endian` | — | `little` | `little` | Differs by omission only; x86 is little-endian |
| `target-pointer-width` | `32` | `32` | `32` | Same. Flat 32-bit protected mode |
| `target-c-int-width` | — | `32` | `32` | Differs by omission only. ILP32, as DJGPP's C (`docs/TOOLCHAIN.md`) |
| `max-atomic-width` | `32` | `32` | `32` | Same. A 486 has a 32-bit `cmpxchg` and no 64-bit one |
| `panic-strategy` | `abort` | `abort` | `abort` | Same. No unwinder; a panic goes to the `#[panic_handler]` |
| `relocation-model` | `static` | `static` | `static` | Same. Programs are linked at a fixed address and nothing relocates them |
| `position-independent-executables` | `false` | `false` | — | Differs by omission; `core/dos.json` builds a static library, which has no executable to be position independent |
| `disable-redzone` | `true` | `true` | `true` | Same. Harmless on x86-32, kept for safety (`docs/TOOLCHAIN.md`; `docs/NATIVE-BOOT.md`) |
| `frame-pointer` | — | `may-omit` | `may-omit` | Differs by omission. `may-omit` keeps `ebp` free on a register-starved processor (`docs/TOOLCHAIN.md`) |
| `stack-probes` | — | `{"kind": "none"}` | `{"kind": "none"}` | Differs by omission. No inline probes; on `core/dos.json` frames over 4 KB still call `_alloca` (`docs/TOOLCHAIN.md`, "Known limits") |
| `default-uwtable` | — | `false` | — | Differs. Only `dos-elf.json` says it; no document records why |
| `dynamic-linking` | `false` | `false` | `false` | Same |
| `executables` | `true` | `true` | `false` | Differs. The ELF pair build programs that rustc links; `core/dos.json` builds only a library, because DJGPP's `gcc` links the program (`docs/TOOLCHAIN.md`) |
| `linker` | `rust-lld` | `rust-lld` | — | Differs. The linker that ships with rustc, so no GCC or binutils is needed (`docs/NATIVE-BOOT.md`; `docs/D0-RUST-DOS.md`) |
| `linker-flavor` | `gnu-lld` | `ld.lld` | — | Differs. Both name the ELF flavour of LLD under different spellings. No document records why they differ, and I did not test whether the spelling changes the command line |
| `metadata` | a record: description, `host_tools: false`, `std: false`, `tier: 3` | — | — | Differs. Only the kernel's file describes itself; the record is documentation for tools that read specifications |

Eight fields are the same in all three files: `arch`, `cpu`, `max-atomic-width`, `panic-strategy`, `relocation-model`, `target-pointer-width`, `disable-redzone` and `dynamic-linking`. They are the facts about the machine and the policy that do not depend on the output format. Everything else follows from one choice: whether the output is ELF or the MinGW-flavoured COFF. The kernel file and `dos-elf.json` are close relatives, with the same triple, the same data layout and the same linker, and `docs/RUST-TARGETS.md` calls the second simpler than the COFF target "in every way that mattered": no section folding, no relocation patching, no weak-external objects to drop, no `_alloca`. `core/dos.json` is the odd one, and the rest of the toolchain's history is the story of moving off it.

### Building `core` and `alloc`: `-Zbuild-std` and the memory functions

All three builds ask cargo for `build-std = ["core", "alloc", "compiler_builtins"]` and `json-target-spec = true`. Two of them add `build-std-features = ["compiler-builtins-mem"]`: `kernel/.cargo/config.toml` and `spikes/d0-rust-dos/.cargo/config.toml`, which `shell/dos` repeats. The feature makes the `compiler_builtins` crate provide `memcpy`, `memmove`, `memset` and `memcmp`, which the compiler calls for copies and comparisons and which nothing else supplies when there is no C library (the comment in the d0 config says so). `core/.cargo/config.toml` does not set it, because on that path DJGPP's C library supplies them; and a fourth, quite different, request, `["std", "panic_abort"]`, belongs to spike E. The same crate supplies the helpers the 32-bit target has no instruction for, such as 64-bit division. On the legacy path the converter drops the crate's objects, because of their weak externals, and DJGPP's `libc` and `libgcc` supply those functions instead ([M17](chapter:m-rt), legacy section).

### Linking: `rust-lld` and the linker scripts

`rust-lld` is LLVM's linker, which rustup installs inside the toolchain (on this host under `lib/rustlib/x86_64-unknown-linux-gnu/bin/`); naming it as `linker` in the specification is what lets the build need no `gcc`, no `ld` and no binutils. The linker joins the objects into one executable and needs to be told where the pieces go. That is the job of a [linker script](term:linker-script): a file that names the output sections, the address each starts at, the order of the input sections inside them, and symbols such as `__image_start` that the program can read. Both ELF targets pass one through `rustflags` as `-C link-arg=-Tlink.ld` together with `-nostdlib`, and the path is relative, which is why `kernel/` and `spikes/d0-rust-dos/` must be the current directory when cargo runs (`shell/dos` spells the path as `../../spikes/d0-rust-dos/link.ld`).

The two scripts answer different questions. `kernel/link.ld` places `.text`, `.rodata` and `.data` from `0x100000`, 1 MiB, where the loader copies the flat image, and marks `.bss` `NOLOAD`. `spikes/d0-rust-dos/link.ld` starts at `0x1000`, leaves the first page empty "which gives a null pointer a little room before real code", aligns each section on 512 bytes "because the stub reads the file in sectors", and uses two program headers only, one for text and read-only data, one for data and `.bss`. Those limits are not arbitrary: they are what `tools/elf2go32.py` accepts. The linker's work is finished by then; the rest of the toolchain only rearranges bytes.

### Packing: two small programs instead of `objcopy`

A linked ELF is not yet something a machine can start. The kernel needs a flat run of bytes at a known address, which the loader copies to 1 MiB. `objcopy -O binary` does that, and it belongs to binutils. `tools/elf2bin.py`, about thirty lines of Python, replaces it ("so the native build needs no binutils", in its own header): it reads the program headers, keeps the segments of type `PT_LOAD` that have file contents, lays them out from the lowest physical address with the gaps zero-filled, and writes the result. A `NOLOAD` section like the kernel's `.bss` is not in the file, which is why the kernel clears it itself before any Rust runs. A DOS program needs a different shape: DJGPP's stub loads a go32 image, a COFF with exactly three section headers. `tools/elf2go32.py` builds that from the two-segment ELF and puts the stub in front. Its refusals are the shape of the requirement: not an executable, not a 386, not one or two loadable segments, a text segment with a `.bss` tail, an entry point outside the text. [M17](chapter:m-rt) walks through what it writes.

The two tools are the same idea at different heights, and neither is a linker. `docs/RUST-TARGETS.md` once describes `elf2bin.py` as linking "instead of DJGPP", which is loose; `docs/NATIVE-BOOT.md` has it right, as a stand-in for `objcopy`.

### NASM, and the machines that do not have it

The boot sectors are not Rust. `tools/native-build.sh` assembles `boot/stage1.asm`, `kernel/bios16.asm` and `boot/stage2.asm` with `nasm -f bin -Werror`, in that order, before it runs cargo, because the kernel embeds the assembled `bios16`. NASM comes from the host, or, when only Docker is available, from the image `desk-native-qemu:1`; with neither the script stops with status 3.

### The Docker images, and what each is for

The repository's rule is that a missing tool runs in a container and is never installed on the host (`AGENTS.md`: do not install system packages to get a build working without asking). The images are small and each exists for one job.

| Image | Where it is defined | What it supplies |
|---|---|---|
| `ghcr.io/liminal-hq/ci-dos` | `docker/ci/Dockerfile` in the `liminal-hq/.github` repository, pinned by digest in `.github/workflows/ci.yml` | Rust 1.98.0 with `rust-src`, DJGPP, DOSBox Staging and CWSDPMI under `/opt/dos-toolchain`: everything the CI needs |
| `desk-native-qemu:1` | `tools/docker/Dockerfile.native-qemu`, built by `tools/native-run.sh` | QEMU and NASM on Ubuntu 24.04, for machines without them |
| `desk-native-bochs:1` | `tools/docker/Dockerfile.native-bochs` | Bochs 2.7 built from source as a 486, because the distribution package starts at a later processor |
| `desk-native-86box:1` | `tools/docker/Dockerfile.native-86box` | 86Box and its ROM set with a virtual X server, built from the upstream releases on the user's machine |
| `desk-dos-xvfb:1` | `tools/docker/Dockerfile.dos-xvfb` | A virtual X server and `xdotool`, to drive DOSBox's window with a pointer |

The first is not in this repository, so its contents are known here only through `tools/setup.sh`, `tools/fetch-tools.sh` and `docs/TOOLCHAIN.md`, which say what the CI relies on it for. The other four are the emulators' ([M20](chapter:m-emulators)).

### `tools/setup.sh` and `tools/fetch-tools.sh`

`tools/setup.sh` is run once. It checks for `rustup`, `python3`, `curl`, `sha256sum`, `timeout`, `tar`, `bzip2` and `xz`, and for any that is missing prints the package that provides it and stops: it never installs one. It asks rustup for the pinned toolchain and its components, then confirms by looking that `rustc` reports the channel and that `rust-src` is installed. For the DOS tools it has two paths. If `DOS_TOOLCHAIN` is set, as it is in the CI image, it links `vendor/djgpp`, `vendor/dosbox` and `vendor/cwsdpmi` to the directories beneath it and downloads nothing. Otherwise it runs `tools/fetch-tools.sh`, which downloads DJGPP (build-djgpp v3.4, GCC 12.2.0), DOSBox Staging 0.83.0 and CWSDPMI r7 into `vendor/dl`, checks each against a SHA-256 sum before unpacking, and unpacks them into the same layout. The versions and sums are copied from the CI image's Dockerfile so that a local build and a CI build use the same bytes; a change is made in the Dockerfile first. Last, `setup.sh` runs DJGPP's `gcc` once, so that a missing `libfl.so.2` is found now and not in the middle of a build.

### The `std` question

Everything built so far is `no_std`. Spike E asked whether Rust's `std` can be built for an operating system whose name the project invents, and `docs/E-STD-STUBS.md` answers in three parts. First, it does not build as is: in 1.98.0 an unknown `os` name fails with 21 errors, because four pieces have no fallback arm (the system allocator, the I/O error mapping, the thread-local keys and the random function). Second, supplying those four gives a `std` whose `println!` is silent and whose clocks and `sleep` abort. Third, a patch of 55 added lines in 12 existing files and about 340 lines of new files, applied to a copy of the library sources that cargo reads through `__CARGO_TESTS_ONLY_SRC_ROOT`, gives a working layer over 15 `desk_*` C functions, so the pinned toolchain stays untouched. The price in bytes, from the same spike, is below.

<!-- figure std-sizes -->

The document's recommendation, which `docs/RUST-TARGETS.md` repeats, is a split: the kernel, runtime, drivers and the Desk shell stay `no_std` with `alloc`; programs that want the crates ecosystem use the patched layer; a thin `std`-like crate stays the answer for size-critical programs; an upstream Tier 3 target is a long-term idea only. The cost of the patch is a hand rebase per compiler bump: it applied to 1.98.0 and, with offsets, to 1.97.0, and only partly to 1.96.1 and older. Only application was tested on the older releases, not compilation.

## Why it is this way

- **Pin a stable release and use the bootstrap variable.** Decided in the walking skeleton (`wheels`, PR #3; `docs/TOOLCHAIN.md`, "The pipeline"): the options are unstable, the release must not move under the build, and a nightly would move every day. The variable is confined to the commands that need it, so ordinary `cargo fmt` and `cargo clippy` runs on the host crates are not affected.
- **Write the target files by hand and keep them small.** Decided in PR #3 and repeated for each new file (`docs/RUST-TARGETS.md`). The skeleton's file dropped the linker, flavour, position-independence and TLS entries that Lieutenant Fizz DOS carried because "the generated machine code is identical" (`docs/TOOLCHAIN.md`, "Differences from Fizz DOS").
- **Leave the borrowed COFF target behind.** The MinGW triple was a means to an end, and its cost was a converter (`tools/coff2go32.py`) with three necessary repairs. Spike d0 (`purerust`, PR #7) showed a freestanding ELF target, `rust-lld` and a 139-line packer need none of them, and `docs/D0-RUST-DOS.md` recommends retiring the C layer and the converter "when the Desk's hardware code has moved over". That retirement is still an owner decision.
- **Replace `objcopy` with a Python script.** Decided with the native kernel (PR #6; `docs/NATIVE-BOOT.md`, "The linker script and the flat binary"): the whole build then needs only NASM, cargo and Python. The same reasoning kept `elf2go32.py` in Python (`docs/D0-RUST-DOS.md`: "it is small and has no dependencies beyond Python, which the repository already needs"), to be ported only if Python leaves the build.
- **Use the linker that ships with the compiler.** `rust-lld` is already there; `ld` is not, on a machine without DJGPP or binutils (`docs/NATIVE-BOOT.md`; `docs/D0-RUST-DOS.md`, "No C, no gcc, no libc").
- **Keep `std` out of the base.** `docs/E-STD-STUBS.md` ("Verdict and recommendation"): `std` doubles a plain hello program and adds machinery that kernels and drivers do not need, so it stays an option for applications.
- **No host packages.** `tools/setup.sh` reports and does not install; Docker supplies the rest (`AGENTS.md`; the scaffold, `scaffold`).
- **Known flaws in the documents.** `docs/RUST-TARGETS.md` still says in its header that `core/dos.json` and `docs/TOOLCHAIN.md` "live on other branches", and its matrix still marks several rows "in a PR"; the files are on this branch. `docs/E-STD-STUBS.md` gives the `desk_*` shim as 450 lines in its verdict and 330 in its sizes section; the log chapter takes 330. Neither is a mistake in a build, and both are for the document's owner.

## The code

| Where | What it holds |
|---|---|
| [`rust-toolchain.toml`](repo:rust-toolchain.toml) | The pin: channel `1.98.0` and the components `rustfmt`, `clippy`, `rust-src` |
| [`kernel/i486-desk-none.json`](repo:kernel/i486-desk-none.json) | The native kernel's target specification |
| [`kernel/.cargo/config.toml`](repo:kernel/.cargo/config.toml) | The kernel's default target, link arguments and `build-std` options |
| [`kernel/link.ld`](repo:kernel/link.ld) | The kernel's linker script: load address `0x100000` |
| [`spikes/d0-rust-dos/dos-elf.json`](repo:spikes/d0-rust-dos/dos-elf.json) | The freestanding DOS target used by `rt`, the spike's programs and `SHELL.EXE` |
| [`spikes/d0-rust-dos/.cargo/config.toml`](repo:spikes/d0-rust-dos/.cargo/config.toml) | `build-std`, `compiler-builtins-mem` and the link arguments of the DOS programs |
| [`spikes/d0-rust-dos/link.ld`](repo:spikes/d0-rust-dos/link.ld) | The DOS linker script: two loadable segments from `0x1000` |
| [`shell/dos/.cargo/config.toml`](repo:shell/dos/.cargo/config.toml) | The same target and script, reached by relative paths |
| [`core/dos.json`](repo:core/dos.json) | The first, legacy, target: borrowed MinGW triple, COFF, library only |
| [`tools/elf2bin.py`](repo:tools/elf2bin.py) | [`main`](sym:tools/elf2bin.py::main) flattens the kernel ELF into a raw binary |
| [`tools/elf2go32.py`](repo:tools/elf2go32.py) | [`parse_elf`](sym:tools/elf2go32.py::parse_elf) reads the program headers and writes the go32 image behind a stub |
| [`tools/native-build.sh`](repo:tools/native-build.sh) | NASM for the three assembly files, then cargo with `RUSTC_BOOTSTRAP=1`, then `elf2bin.py` |
| [`tools/setup.sh`](repo:tools/setup.sh) | Host checks, the pinned Rust, and `vendor/` linked to `DOS_TOOLCHAIN` or fetched |
| [`tools/fetch-tools.sh`](repo:tools/fetch-tools.sh) | DJGPP, DOSBox Staging and CWSDPMI downloaded with SHA-256 sums |
| [`tools/docker/`](repo:tools/docker/) | The four Dockerfiles of the emulator and input images |

For depth: [`docs/RUST-TARGETS.md`](doc:docs/RUST-TARGETS.md#the-concept) (the concept and the matrix of application spaces), [`docs/TOOLCHAIN.md`](doc:docs/TOOLCHAIN.md#the-target-specification) (the first target, field by field), [`docs/E-STD-STUBS.md`](doc:docs/E-STD-STUBS.md#verdict-and-recommendation) (the `std` spike) and [`docs/NATIVE-BOOT.md`](doc:docs/NATIVE-BOOT.md#the-kernel-target) (the kernel's).

## Worked example

**One field followed to a file name: `executables`.** `core/dos.json` says `"executables": false`. Its build can therefore only produce a library, and the one it makes is a static archive that DJGPP's `gcc` links with the C layer; its output directory is named after the file, `dos`, as `spikes/d0-rust-dos/build-stage1.sh` and `docs/D0-RUST-DOS.md` use it (`target/dos/release/libstage1.a`). The two ELF files say `"executables": true` and name `rust-lld`, so rustc itself runs the linker with the arguments from `rustflags`. The consequence is traceable on the files of `SHELL.EXE`. `shell/dos/.cargo/config.toml` sets the target to `../../spikes/d0-rust-dos/dos-elf.json`; the stem is `dos-elf`; and `shell/dos/build.sh` reads the result from `shell/dos/target/dos-elf/release/shell`. That file, built in the CI image on 2026-10-23 for this chapter, is a 127,232-byte ELF executable with two loadable segments, which is what `link.ld` asks for:

```text
  Type  Offset    VirtAddr   FileSiz  MemSiz   Flg Align
  LOAD  0x001000  0x00001000 0x18200  0x18200  R E 0x1000
  LOAD  0x019200  0x00019200 0x00200  0x42600  RW  0x1000
```

The first is `.text` (98,816 bytes), the second `.data` of 512 bytes with a `.bss` tail: 0x42600 − 0x200 = 0x42400, or 271,360 bytes, of which the runtime's stack is 262,144 (`STACK_SIZE` in `rt/src/start.rs`, 256 KiB). `tools/elf2go32.py --info` then reports `stub 2048 + coff 99840 = 101888 bytes` and `block 384 KiB`: 99,840 is the 512-byte header area plus the two sections padded to 512, and 384 KiB is the end of `.bss`, 0x5B800, rounded up to a multiple of 64 KiB. For the kernel the same flag, `executables: true`, gives `kernel/target/i486-desk-none/release/desk-kernel`, an ELF of 795,956 bytes in that build with three loadable segments, which `elf2go32.py` would refuse (it accepts one or two) and `elf2bin.py` accepts: it keeps the file-backed parts and reports `dist/native/kernel.bin: 173968 bytes, load address 0x100000`. The sizes are this build's. The documents record others (`SHELL.EXE` at 102,400 bytes in `docs/FIRST-WINDOW.md`, `kernel.bin` at 173,344 in the same Result section, 174,032 in the log chapter on the first window), which are measurements of earlier trees.

What the example does not show is the effect of a field when it is removed. I did not remove any, so the table's reasons are the documents' and not mine.

## State of this layer

```status
id: toolchain.pin
label: The pinned stable Rust 1.98.0 with rust-src, and setup.sh's check that rustc matches it
state: built-verified
evidence: sym:rust-toolchain.toml::channel, test:tools/setup.sh::rustc is not $channel, test:.github/workflows/ci.yml::tools/setup.sh
note: Run by me in the CI image on 2026-10-23: setup.sh passed and reported rustc 1.98.0. A newer compiler is untested by design.

id: toolchain.bootstrap
label: RUSTC_BOOTSTRAP=1 lets the stable compiler use build-std and json-target-spec
state: built-verified
evidence: test:shell/dos/build.sh::RUSTC_BOOTSTRAP=1, test:tools/native-build.sh::RUSTC_BOOTSTRAP=1
note: Without it cargo 1.98.0 stops with the json-target-spec error quoted above; I ran both ways.

id: toolchain.target-dos-elf
label: dos-elf.json links with rust-lld and link.ld into a program the stub loads
state: built-verified
evidence: sym:spikes/d0-rust-dos/dos-elf.json::i486-unknown-none-elf, test:shell/dos/check.sh::All shell on DOS checks passed, test:spikes/d0-rust-dos/check.sh::PANICKY.EXE
note: In headless DOSBox in the CI image. I ran shell/dos/check.sh and it passed; I did not rerun the whole d0 check.

id: toolchain.target-kernel
label: i486-desk-none.json builds the kernel that boots in QEMU as a 486
state: built-verified
evidence: sym:kernel/i486-desk-none.json::gnu-lld, test:tests/native/expected.txt::boot complete
note: I built the kernel on the host (tools/native-build.sh) and did not boot it for this chapter; the transcript is the committed one that M5 reruns.

id: toolchain.target-legacy
label: core/dos.json builds the legacy Rust core that DJGPP's gcc links
state: built-verified
evidence: sym:core/dos.json::i486-pc-windows-gnu, test:tools/check.sh::DESK-SKELETON ok
note: Rests on the committed check in tools/check.sh, which I did not rerun.

id: toolchain.buildstd
label: build-std with compiler-builtins-mem supplies core, alloc and the memory functions for both ELF targets
state: built-verified
evidence: sym:kernel/.cargo/config.toml::compiler-builtins-mem, sym:spikes/d0-rust-dos/.cargo/config.toml::compiler-builtins-mem, test:tests/native/expected.txt::alloc: Vec sum 385
note: Both builds ran for this chapter; the alloc line is the kernel's.

id: toolchain.elf2bin
label: elf2bin.py flattens the kernel ELF in place of objcopy
state: built-verified
evidence: sym:tools/elf2bin.py::main, test:tests/native/expected.txt::boot complete
note: I ran it on the host and it printed the size and load address quoted above.

id: toolchain.nasm
label: NASM assembles the loader stages and the BIOS trampoline, from the host or the desk-native-qemu image
state: built-unverified
evidence: sym:tools/native-build.sh::run_nasm, repo:tools/docker/Dockerfile.native-qemu
note: Host NASM ran for me. The Docker fallback was not built here.

id: toolchain.images
label: The Bochs, 86Box and Xvfb images
state: built-unverified
evidence: repo:tools/docker/Dockerfile.native-bochs, repo:tools/docker/Dockerfile.native-86box, repo:tools/docker/Dockerfile.dos-xvfb
note: Not built for this chapter. M20 says what was recorded of the emulator runs.

id: toolchain.fetch
label: fetch-tools.sh downloads DJGPP, DOSBox Staging and CWSDPMI and checks their sums
state: built-unverified
evidence: sym:tools/fetch-tools.sh::DJGPP_SHA256, doc:docs/TOOLCHAIN.md#the-pieces
note: I ran setup.sh by its DOS_TOOLCHAIN path in the CI image, which skips this script. The download path was not run.

id: toolchain.std
label: A patched std platform layer over rt, with 15 desk_* functions
state: built-unverified
evidence: repo:spikes/e-std-stubs/patch, doc:docs/E-STD-STUBS.md#verdict-and-recommendation
note: A committed check (spikes/e-std-stubs/check.sh) is a step of tools/check.sh; I did not run it. The sizes in the figure are the document's.

id: toolchain.prebuilt
label: A prebuilt sysroot published from CI, and an app target for Desk programs
state: planned
evidence: doc:docs/RUST-TARGETS.md#cross-cutting-ideas-to-track
```

The two ELF targets are {{state.toolchain.target-dos-elf}} and {{state.toolchain.target-kernel}} in emulators; the legacy target is {{state.toolchain.target-legacy}} on a committed check; the patched `std` is {{state.toolchain.std}} here; and a prebuilt sysroot is {{state.toolchain.prebuilt}}. Nothing ran on real hardware.

## Try it yourself

The first command builds the kernel the way the native track does (NASM and cargo on the host) and shows the packer's output. The second compares the three target files from the shell. The third and fourth are the error without the bootstrap variable, and the shape of `elf2go32.py`'s refusal. The DOS build itself needs the CI image; the commands for it are in [M17](chapter:m-rt), and the pinned Rust inside the image is the same 1.98.0 as on this host.

```try
needs: host: bash, nasm, cargo (Rust 1.98.0 with rust-src), python3
ran: 2026-10-23 on the host, NASM 3.02, rustc 1.98.0, and in ghcr.io/liminal-hq/ci-dos by its digest in .github/workflows/ci.yml (Docker, no host packages installed)
$ tools/native-build.sh 2>&1 | tail -1
dist/native/kernel.bin: 173968 bytes, load address 0x100000
$ python3 -I -c "import json;f=['kernel/i486-desk-none.json','spikes/d0-rust-dos/dos-elf.json','core/dos.json'];d=[json.load(open(x)) for x in f];k=sorted(set().union(*d));s=[x for x in k if len({json.dumps(y.get(x)) for y in d})==1];print(len(k),'fields;',len(s),'the same in all three:',', '.join(s))"
26 fields; 8 the same in all three: arch, cpu, disable-redzone, dynamic-linking, max-atomic-width, panic-strategy, relocation-model, target-pointer-width
$ cd shell/dos && cargo build --release --locked 2>&1 | head -1
error: `.json` target specs require -Zjson-target-spec to be added to the cargo invocation
$ cd shell/dos && RUSTC_BOOTSTRAP=1 cargo build --release --locked 2>&1 | tail -1
    Finished `release` profile [optimized] target(s) in 7.73s
```

(The time includes compiling the standard pieces for the target.) The packer's refusal of anything that is not an ELF, run with the stub standing in as the "ELF", needs the DOS tools in `vendor/`, so it is in M17 and not here; I ran it there.

**Exercises.** (1) Without running it, predict which of the 26 fields you could delete from `kernel/i486-desk-none.json` with no change to the kernel's behaviour, and which would make rustc refuse the file; then try one in a scratch branch and say whether the documents agreed. (2) Make `dos-elf.json` use `"linker-flavor": "gnu-lld"` and check whether `SHELL.EXE` changes by a byte (the answer is not recorded anywhere). (3) Work out from the table why `core/dos.json` could not simply gain `"executables": true`: what would be missing at the link?

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Target specification
def: A JSON file that tells rustc and LLVM what code is compiled for: the triple, the processor and its features, the pointer width, how a panic ends, how to link. The Desk has three, and cargo passes one with --target or the build.target setting.

term: Build-std
def: The cargo option that compiles core, alloc and compiler_builtins from the rust-src component for a target Rust ships no standard pieces for. It is unstable, so the repository sets RUSTC_BOOTSTRAP=1 for the commands that use it.

term: Linker script
def: A file that tells the linker which output sections to make, where each starts, which input sections go into it, and which symbols to define. The kernel's puts the image at 1 MiB; the DOS programs' at offset 0x1000 of the block the stub allocates.
```

```figures
id: m19-std-sizes
reuse: std-sizes
at: figure std-sizes
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The target table was computed from the three files; the build, the packer output and the bootstrap error were run on the host and in the CI image. Found while writing: `docs/RUST-TARGETS.md` calls `elf2bin.py` a linker and still lists the first target's files as living on other branches; `docs/E-STD-STUBS.md` gives the shim as 450 lines in one place and 330 in another. The target-comparison explainer (`m-targets`) is to be added in place of the table when its module lands.
