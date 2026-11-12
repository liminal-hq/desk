# Module loader: the build plan

The task-by-task plan for building [`LOADER.md`](LOADER.md), roadmap item R48: the `.DMD` format and its reader, the packer, the app-side runtime and the test modules, the native kernel and a Rust-only DOS host loading the same file, then several modules, unload, leaks, a missing import and a contained panic on both tracks, and the result. Each task is one pull request, sized for one builder (a Sonnet agent or a person) working from this document, the design and the files it names.

> **Status.** A plan, written on 2026-11-11 against `main` at `eaede0f` and revised the same day for the owner's decisions ([`LOADER.md`](LOADER.md#decisions)): the 96-byte header with `kind` and a reserved export table, a second extension for applications, the packer in Rust sharing the format crate, and a test at several load addresses. No task has started. The API sketches are the design's; a builder may change a signature when the code shows a better one, and says so in the pull request and in [`LOADER.md`](LOADER.md) in the same branch. The defaults taken for the owner's questions ([`LOADER.md`](LOADER.md#questions-for-the-owner)) are the ones built unless the owner says otherwise before the task that depends on one.

## Contents

- [Scope](#scope)
- [Order and parallelism](#order-and-parallelism)
- [Rules for every task](#rules-for-every-task)
- [M1. The format crate desk-module](#m1-the-format-crate-desk-module)
- [M2. The packer's ELF reader](#m2-the-packers-elf-reader)
- [M3. Packing, the guest crate and the hello module](#m3-packing-the-guest-crate-and-the-hello-module)
- [M4. Native: the kernel loads HELLO.DMD from a buffer](#m4-native-the-kernel-loads-hellodmd-from-a-buffer)
- [M5. Hosted: DMDRUN.EXE loads HELLO.DMD in DOSBox](#m5-hosted-dmdrunexe-loads-hellodmd-in-dosbox)
- [M6. Several modules, unload, leaks and a missing import, on both tracks](#m6-several-modules-unload-leaks-and-a-missing-import-on-both-tracks)
- [M7. A contained panic on both tracks](#m7-a-contained-panic-on-both-tracks)
- [M8. CPU faults in a module, natively (optional)](#m8-cpu-faults-in-a-module-natively-optional)
- [M9. The result](#m9-the-result)
- [Definition of done](#definition-of-done)

## Scope

R48 is the format, the packer and a loader on each track, for modules (kind 1). It does not build preemption, tasks, threads, wait and wake, priorities, the window system as a task, applications that run, libraries or `std` ([`APP-MODEL.md`](APP-MODEL.md), [`DESK-STD.md`](DESK-STD.md)): those are R77, R78, R79 and R141 to R146. The spike writes and checks the fields those later items need (`kind`, the export table) so the format does not change when they arrive, and refuses what it cannot yet load.

## Order and parallelism

At most two tasks run at once. A wave starts when everything it depends on is merged; each task is a pull request off `main`.

| Wave | Tasks, in parallel | Depends on | Size (new lines, roughly, tests included) |
|---|---|---|---|
| 1 | M1 the format crate ∥ M2 the packer's ELF reader | — | 900 ∥ 800 |
| 2 | M3 packing, the guest crate and `hello` | M1, M2 | 600 |
| 3 | M4 native hello ∥ M5 hosted hello | M3 | 350 ∥ 450 |
| 4 | M6 several modules, unload, leaks, a missing import | M4, M5 | 900 |
| 5 | M7 a contained panic | M6 | 500 |
| 6 | M8 native CPU faults (optional) | M7 | 300 |
| 7 | M9 the result | M7, and M8 if it is built | 250 and documents |

**The first "hello from a loaded program"** is M4, at the end of wave 3: the kernel prints the module's line on serial in QEMU. M5 gives the same line from the same file in DOSBox in the same wave. Nothing before M4 runs code in an emulator; M1 to M3 make sure what M4 loads is right before it runs.

Branches: `feat/module-format` (M1), `feat/module-elf` (M2), `feat/module-pack` (M3), `feat/module-native` (M4), `feat/module-dos` (M5), `feat/module-lifecycle` (M6), `feat/module-panic` (M7), `feat/module-faults` (M8), `docs/module-result` (M9).

Shared files between tasks of one wave: M1 and M2 both create `module/Cargo.toml` (whichever merges second adds its member when it rebases; the first creates the workspace with the `[workspace.package]` table of `shell/Cargo.toml`) and both add to the same `tools/check.sh` step. M4 and M5 both add to `tools/check.sh` (different steps) and both edit [`LOADER.md`](LOADER.md) only if the code chose differently (the second rebases, keeping both changes).

## Rules for every task

- **Read first**: [`AGENTS.md`](../AGENTS.md), [`LOADER.md`](LOADER.md), and the documents the task names. Follow `AGENTS.md`: Canadian English, no hard-wrapped markdown, real em dashes, licence headers on new source files, Conventional Commits ending with the agent co-author trailer, a PR title without a prefix, labels on the PR, merge commits only, and no merging without the owner's go-ahead.
- **Work in a git worktree** under `.claude/worktrees/` on the task's branch off `origin/main`, never in the main checkout.
- **Missing tools run in Docker**, never installed on the host: the CI image `ghcr.io/liminal-hq/ci-dos` (its digest is in `.github/workflows/ci.yml`) has Rust 1.98.0 with `rust-src`, QEMU, NASM, DJGPP, DOSBox Staging and CWSDPMI. Run containers with `--user "$(id -u):$(id -g)"` and a writable `HOME` and `CARGO_HOME`. Do not prune Docker images or volumes.
- **The rules of the crates** ([`LOADER.md`](LOADER.md#where-the-code-lives-and-its-rules)): `desk-module` and `desk-module-host` follow `shell/`'s rules (`no_std` + `alloc`, `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`, no compare-and-swap through a `clippy.toml` copied from `shell/`, no external crates, no clocks or randomness); `dmdpack` is `std` and `#![forbid(unsafe_code)]`; `unsafe` elsewhere carries a one-line `SAFETY:` comment saying what makes it sound.
- **No module is refused for containing `CMPXCHG`** (decided, [`LOADER.md`](LOADER.md#decisions)): the packer does not scan for it and the loaders do not check. The spike's own crates and modules stay under the compare-and-swap ban, so every check here runs under DOSBox's strict normal core; `std` apps, later, run under its dynamic core ([`DESK-STD.md`](DESK-STD.md#cmpxchg-on-the-hosted-track)).
- **The service table stays at four functions** (`desk_log`, `desk_panic`, `desk_alloc`, `desk_free`) plus whatever a test needs to be missing. The SDK is R79's; a task that wants a fifth service stops and asks.
- **The pinned files do not change**: `tests/native/expected.txt` and every existing `tests/native/expected-*.txt`, the shell goldens and their transcripts. Module code prints only in the module test modes. A task that needs to change a pinned file has found a regression and stops to report it.
- **Fix a bug with a check that fails before the fix.** Every rejection path is a test that feeds a crafted input and expects that error, and each new emulator check has a negative run that must fail.
- **Verify before the PR**: the task's acceptance commands, `python3 -I tools/check-docs.py`, `tools/check-headers.sh`, and `tools/check.sh` (in the CI image where the host lacks a tool) where the task touches what it runs. The PR's test plan says exactly what ran and what did not, and that nothing ran on real hardware.
- **The header and `tools/desk.magic` change together**: a task that moves a header field updates the magic file's offsets in the same commit, and M3's `file` check catches one that does not.
- **Update the documents the task changes** in the same branch, and correct [`LOADER.md`](LOADER.md) where the code chose differently.

## M1. The format crate desk-module

**Goal.** The container (`.DMD` modules, `.DKX` applications) in `no_std` Rust, the one definition of the format that the loaders and the packer share: parse and validate a file, write one (for the packer and the tests), and load a parsed module into a byte slice at a given address with every relocation applied and every import bound. No `unsafe`.

**Files.**

- `module/Cargo.toml` (workspace, `members = ["format"]`, `exclude = ["mods", "dos"]`, `[workspace.package]` as `shell/`'s), `module/Cargo.lock`, `module/clippy.toml` (a copy of `shell/clippy.toml`), `module/rustfmt.toml` (as `shell/`'s).
- `module/format/Cargo.toml`, `module/format/src/lib.rs`, `header.rs` (constants, `Header`, `parse_header`), `module.rs` (`Module`, the table checks, `Import`), `load.rs` (`load_into`, `Placed`), `write.rs` (`Builder`), `crc.rs`, `error.rs` (`FormatError`, `LoadError`, both `Display`), `module/format/tests/reject.rs`, `module/format/tests/load.rs`.
- `tools/check.sh`: a "Module loader: host tests and no_std builds" step after the shell's, with the five commands the shell's step has (`fmt --check`, `clippy --all-targets`, `test`, and the cross `clippy` for `kernel/i486-desk-none.json` and `check` for `spikes/d0-rust-dos/dos-elf.json`).
- `AGENTS.md` and `CLAUDE.md`: `module/` in the repository layout and its rules. `docs/RUST-TARGETS.md`: a row for the module crates.

**API.**

```rust
pub const MAGIC: [u8; 4] = *b"DMD1";
pub const HEADER_SIZE: u16 = 96;
pub const LINK_BASE: u32 = 0x0040_0000;
pub const ABI_I486_FLAT: u16 = 1;
pub const KIND_MODULE: u16 = 1;       // .DMD
pub const KIND_APP: u16 = 2;          // .DKX
pub const KIND_LIBRARY: u16 = 3;      // reserved: refused in version 1
pub const MAX_MEMORY: u32 = 16 << 20;
pub const MAX_IMPORTS: u32 = 256;
pub const MAX_NAME: usize = 31;

pub struct Header {
    pub header_size: u16, pub flags: u16, pub abi: u16, pub sdk: u16, pub kind: u16,
    pub link_base: u32, pub align: u32,
    pub image_offset: u32, pub file_size: u32, pub mem_size: u32, pub ro_size: u32,
    pub start: u32, pub event: u32, pub stop: u32,
    pub reloc_offset: u32, pub reloc_count: u32,
    pub import_offset: u32, pub import_count: u32,
    pub export_offset: u32, pub export_count: u32,   // reserved: an empty table in version 1
    pub strings_offset: u32, pub strings_size: u32,
    pub crc: u32,
}

pub struct Module<'a> { /* the header and the file's bytes */ }
pub struct Import<'a> { pub name: &'a str, pub slot: u32 }

impl<'a> Module<'a> {
    /// Checks everything that can be checked without a destination: the header, the layout, the CRC and every table entry.
    pub fn parse(bytes: &'a [u8], core_sdk: u16) -> Result<Module<'a>, FormatError>;
    pub fn header(&self) -> &Header;
    pub fn image(&self) -> &'a [u8];
    pub fn relocations(&self) -> impl Iterator<Item = u32> + 'a;
    pub fn imports(&self) -> impl Iterator<Item = Import<'a>> + 'a;
}

pub struct Placed { pub base: u32, pub end: u32, pub start: u32, pub event: Option<u32>, pub stop: Option<u32> }

/// Copies the image into `dest`, zeroes the tail, relocates it for `dest_addr` and binds every import through `resolve`.
/// `dest` must hold at least `mem_size` bytes and `dest_addr` must be a multiple of `align`; `kind` is what the caller can load
/// (a module loader passes KIND_MODULE). Nothing is called.
pub fn load_into<'a>(m: &Module<'a>, kind: u16, dest: &mut [u8], dest_addr: u32, resolve: &dyn Fn(&str) -> Option<u32>) -> Result<Placed, LoadError<'a>>;

pub struct Builder {
    pub sdk: u16, pub kind: u16, pub align: u32, pub link_base: u32,
    pub image: Vec<u8>, pub mem_size: u32, pub ro_size: u32,
    pub start: u32, pub event: u32, pub stop: u32,
    pub relocs: Vec<u32>, pub imports: Vec<(String, u32)>,
}
impl Builder {
    /// Lays the file out in the order of the specification, sorts nothing (the caller gives sorted relocations), fills in the CRC.
    pub fn write(&self) -> Result<Vec<u8>, FormatError>;
}

pub fn crc32(bytes: &[u8]) -> u32;
```

`FormatError` has one variant per check in [the specification](LOADER.md#format-specification-sketch-dmd) (`TooShort`, `BadMagic`, `UnsupportedVersion(u8)`, `BadHeaderSize(u16)`, `UnknownFlags(u16)`, `UnsupportedAbi(u16)`, `SdkTooNew { needs, have }`, `UnknownKind(u16)`, `KindNotSupported(u16)` (a library), `ExportsNotSupported { count }`, `ExportTableMisplaced { offset }`, `BadAlignment(u32)`, `BadLinkBase(u32)`, `TableOutsideFile { table, offset, len }`, `TablesOverlap`, `TrailingBytes { at }`, `SizesOutOfOrder`, `TooLarge(u32)`, `BadChecksum { stored, computed }`, `EntryOutOfRange { which, offset }`, `RelocOutsideImage { index, offset }`, `RelocsOutOfOrder { index }`, `BadImportName { index }`, `DuplicateImport { index }`, `BadImportSlot { index, slot }`, `TooManyImports(u32)`); `LoadError<'a>` has `KindNotLoadable { kind, wanted }`, `DestTooSmall { need, have }`, `DestMisaligned { addr, align }`, `RelocTargetOutOfRange { index, offset, value }`, `MissingImport { index, name: &'a str }`. Each `Display` is one line a transcript can print, naming the number it concerns.

**Acceptance.**

- `cargo test` in `module/`: a `Builder` round trip (write, parse, read every field back); `crc32(b"123456789") == 0xCBF43926`; `load_into` at the link base leaves the image unchanged apart from the bound slots; at `link base + 0x1000` and at `0x0010_0000` every listed word moves by the delta and no other byte changes; loaded at eight addresses from a fixed-seed generator (each a multiple of the alignment, some below the link base and one near the top of the 32-bit space), each image minus its delta at the listed words equals the stored image, which is the host half of the varied-address test; the `.bss` tail is zeroed even when `dest` held other bytes; slots hold the resolver's addresses.
- The reserved fields: `kind` round-trips for module and application; an application file loaded with `KIND_MODULE` is refused as not loadable here, and the reverse; kind 3 and kind 0 are refused; an export count of 1, and an export offset not equal to the end of the import table, are refused; the header is exactly 96 bytes and `header_size` 80 (the first draft's) is refused.
- One test per rejection, each with a file the `Builder` makes and then damages (the CRC recomputed where the test is not about the CRC): too short, bad magic, `DMD2`, every header field out of its range, a table past the end of the file, two tables overlapping, trailing bytes, a single flipped bit anywhere (bad checksum), every prefix of a valid file shorter than it (truncated: refused, and never a panic), relocations out of order, two relocations 2 bytes apart, a relocation in the `.bss` tail, a relocation whose word points outside the image (refused by `load_into`), an import name with a capital or 32 bytes long, a duplicate name, a slot in the read-only part, a slot not aligned, a slot that is also a relocation, a slot not zero in the file, a missing import (the error names it), a destination one byte too small, a misaligned destination.
- No input makes `parse` or `load_into` panic: a fixed-seed integer generator mutates a valid file 10,000 times and every result is `Ok` or an error (all arithmetic on offsets is checked).
- The cross builds in `tools/check.sh` pass for both targets.

**Depends on**: nothing. **Parallel with**: M2. **Labels**: `enhancement`, `rust`, `apps`, `sdk`, `spike`.

**Risks.** Offsets are `u32` from the file and sizes `usize` on the host: every addition is `checked_add`, and the mutation test is what keeps it honest. The `Display` lines become transcript lines in M4 to M7, so keep them stable once M4 pins them.

## M2. The packer's ELF reader

**Goal.** The `dmdpack` host crate with a strict ELF32 reader that turns a module linked by `rust-lld` into everything a `.DMD` needs, and refuses every input the design lists. No packing yet (M3 joins it to `desk-module`).

**Files.** `module/pack/Cargo.toml` (a `bin` and a `lib`, `std`, no dependencies yet), `module/pack/src/lib.rs`, `module/pack/src/elf.rs`, `module/pack/src/main.rs` (only `dmdpack elf <file>`, which prints what the reader found, for debugging), `module/pack/tests/elf.rs`, `module/pack/tests/elfgen/mod.rs` (a small writer of ELF32 files for the tests: header, program headers, sections, a symbol table, relocation sections). `module/Cargo.toml`: add `pack` (or create the workspace as M1 describes).

**API.**

```rust
pub struct Extract {
    pub link_base: u32,
    pub image: Vec<u8>,          // .text, .rodata and .data at their offsets from the link base, gaps zero-filled
    pub mem_size: u32,           // image plus .bss
    pub ro_size: u32,            // the read-only segments' extent from the link base
    pub relocs: Vec<u32>,        // image offsets of the R_386_32 words, sorted, unique
    pub imports: Vec<(String, u32)>, // from the `__desk_imp_<name>` symbols in `.desk.imp`, by slot
    pub start: u32, pub event: Option<u32>, pub stop: Option<u32>,   // `app_start` etc. for a module; `desk_main` alone for an app
}

pub fn read(bytes: &[u8], link_base: u32, kind: u16) -> Result<Extract, ElfError>;
```

`ElfError` has one variant per refusal in [`LOADER.md`](LOADER.md#building-a-module), each with the index or offset it concerns.

**Acceptance.**

- A file written by `elfgen` with three segments (R E, R, RW plus `.bss`), `R_386_32` and `R_386_PC32` relocations in `.rel.text` and `.rel.rodata`, two import symbols and the three entry symbols gives exactly the `Extract` the test states.
- `R_386_PC32` relocations produce nothing; `R_386_32` relocations in a non-allocated section (`.rel.debug_info`) are ignored; `R_386_NONE` is ignored.
- Refused, each with its own error: not ELF, 64-bit, big-endian, not `ET_EXEC`, not `EM_386`, program headers outside the file, a section header table outside the file, a `PT_LOAD` whose file bytes run past the end, `p_filesz > p_memsz` (the two probes of the analysis's section 6), overlapping segments, segments out of order, `.bss` before the end, a writable segment before a read-only one, a link base other than the one asked for, a relocation type other than the three (for example `R_386_GOT32`), an `R_386_32` against an `SHN_ABS` or undefined symbol, a relocation into a section outside the image, an import symbol of size 2, an import in `.data` instead of `.desk.imp`, an import name that is not valid, no `app_start` (for a module) or no `desk_main` (for an application), an application that also defines `app_event` or `app_stop`, a `.tdata` or `.tbss` section, an `R_386_TLS_*` relocation.
- The linked ELF of the design's experiment (re-made by M3, not committed here) is not needed: the tests make every file they read.

**Depends on**: nothing. **Parallel with**: M1. **Labels**: `enhancement`, `rust`, `build`, `spike`.

**Risks.** The section-symbol relocations (`R_386_32` against `.rodata`) carry their addend in the word itself (`REL`, not `RELA`): the reader must not add the symbol value twice. A section-to-segment mapping that `rust-lld` writes differently from `elfgen` is the likeliest surprise; M3's real modules are the check, so keep the reader driven by program headers for the image and by section headers only for relocations and symbols.

## M3. Packing, the guest crate and the hello module

**Goal.** `dmdpack pack`, `info` and `cross`, the Rust packer over the shared `desk-module` crate (it may later replace `tools/elf2go32.py`, which is not in this task); the app-side runtime; the module workspace with its linker script; `hello` built, packed and checked on the host, including relocation proved by linking at two bases.

**Files.**

- `module/pack/Cargo.toml` (a path dependency on `desk-module`), `module/pack/src/main.rs`: `dmdpack pack [--kind module|app] [--link-base ADDR] [--sdk N] <in.elf> <out>`, `dmdpack info <file>` (one line per header field, the kind by name, the counts, the import names, the CRC), `dmdpack cross <a.dmd> <b.dmd>` (loads `a` at `b`'s link base with every import bound to 0 and requires the result to equal `b`'s image and the two relocation lists to be equal). Exit 0 on success, 1 on a refused input (with the error's line), 2 on a usage error.
- `module/guest/Cargo.toml`, `module/guest/src/lib.rs`: `import!`, `desk_app!`, `log`, the panic handler (calls `desk_panic`), behind a `panic-handler` feature on by default.
- `module/mods/Cargo.toml` (its own workspace, `members = ["hello"]`), `module/mods/.cargo/config.toml` (target `../../kernel/i486-desk-none.json`, `build-std = ["core", "alloc", "compiler_builtins"]`, `compiler-builtins-mem`, rustflags `-Tmodule.ld`, `-nostdlib`, `--emit-relocs`, `-zmax-page-size=0x1000`), `module/mods/module.ld` (base from the symbol `__desk_link_base`, default `0x0040_0000`; `.text`, `.rodata`, then, from a page boundary, `.desk.imp : ALIGN(4) { KEEP(*(.desk.imp)) }` **as an output section of its own** (the packer finds an import slot by the section its symbol is in, so slots merged into `.data` are refused), then `.data`, then `.bss`, with `ENTRY(app_start)` and `EXTERN(app_event)` and `EXTERN(app_stop)` so `--gc-sections` keeps the optional entry points; the layout is the one `module/pack/tests/real/module.ld` links in the packer's real-linker test, which M3's script starts from; discards `.eh_frame*`, `.comment`, `.note*`), `module/mods/rustfmt.toml`, `module/mods/hello/Cargo.toml`, `module/mods/hello/src/lib.rs`, and `module/mods/appmain/` (a `desk_main` that only logs a line, packed with `--kind app` into `APPMAIN.DKX`; it is never run in the spike, only refused).
- `tools/desk.magic` (already in the repository from the design): the `file(1)` magic for the container. M3 checks it against the packer's real output and, if the header changed while building M1, updates its offsets in the same commit. `tests/module/file-magic.txt`: the expected lines.
- `tools/build-modules.sh`: builds `dmdpack` (host, release), builds `module/mods` for the module target, packs each module into `dist/modules/<NAME>.DMD` and each application into `dist/modules/<NAME>.DKX` (upper case, 8.3) and copies the ELF to `dist/modules/<name>.elf`; `--link-base ADDR --out DIR` for the cross check.
- `tools/check.sh`: in the module step, `tools/build-modules.sh`, `dmdpack info dist/modules/HELLO.DMD` compared with `tests/module/hello-info.txt` (sizes as ranges, so the comparison uses `tools/native-check.py`), the cross check of `HELLO.DMD` against a build linked at `0x0080_0000`, and the negative runs below.

**API.**

```rust
// desk-guest
#[macro_export]
macro_rules! import {
    // import!(fn desk_log(text: *const u8, len: usize));
    // expands to a zero `#[no_mangle] #[used] #[link_section = ".desk.imp"] static __desk_imp_desk_log: AtomicUsize`
    // and an `unsafe fn desk_log(text: *const u8, len: usize)` that loads the slot and calls it.
}
#[macro_export]
macro_rules! desk_app {
    // desk_app!(start, event, stop); defines `#[no_mangle] extern "C" fn app_start`, `app_event`, `app_stop`
    // over three plain Rust functions; `event` and `stop` may be omitted.
}
pub fn log(text: &str);             // through desk_log
pub struct Lines;                   // a `core::fmt::Write` that sends each line to desk_log, with a 160-byte buffer
```

`hello` prints two lines from `app_start`: `hello from a loaded program`, then `hello: app=<app> count=<n>` from a `static` counter that starts at 7 and is incremented, and returns 0. It defines no `app_event` and an `app_stop` that prints `hello: stop app=<app>`.

**Acceptance.**

- `tools/build-modules.sh` from clean writes `HELLO.DMD` and `hello.elf`; `dmdpack info` matches `tests/module/hello-info.txt`; the PR states the sizes (file, image, relocations, imports) beside the design's estimate.
- `dmdpack cross` passes for `HELLO.DMD` against the `0x0080_0000` build, and fails (exit 1, naming the first differing offset) against a copy of the second build with one relocated word changed, and against a `HELLO.DMD` packed from an ELF linked without `--emit-relocs`: such an ELF has no relocation sections, the packer cannot tell that from a module with no absolute words, and the cross check is what catches it.
- `dmdpack pack` refuses a link at `0x0080_0000` without `--link-base`, and refuses an object file (`ET_REL`).
- `file -b -m tools/desk.magic` names the packer's first outputs exactly, so the magic file stays right when the header changes: `dist/modules/HELLO.DMD` gives `Desk loadable image, version 1, module (.DMD), i486 flat, SDK 1, 96-byte header`, `dist/modules/APPMAIN.DKX` gives `Desk loadable image, version 1, application (.DKX), i486 flat, SDK 1, 96-byte header`, and a copy of `HELLO.DMD` with its first byte changed gives `data`. The lines are in `tests/module/file-magic.txt` and the check compares them with `diff`. `file` is in the CI image (`file-5.45`, checked on 2026-11-11); on a host without it the check runs in the image, as the other Docker steps do, and no host install is needed.
- `module/guest` and `module/mods` pass `cargo fmt --check` and `clippy -D warnings` for the module target; the disassembly of `HELLO.DMD`'s ELF has no `cmpxchg` and no `cli` or `sti` (apps never mask interrupts: [`APP-MODEL.md`](APP-MODEL.md#decisions)) (checked once by `llvm-objdump` or `objdump` in the CI image, stated in the PR).

**Depends on**: M1, M2. **Parallel with**: nothing (wave 2). **Labels**: `enhancement`, `rust`, `build`, `apps`, `sdk`, `spike`.

**Risks.** A second link at another base rebuilds the module (the rustflags change): use a separate `--target-dir` so the normal build stays warm. If `rust-lld` lays the `0x0080_0000` link out differently (it should not; only the base moves), `cross` reports it and the task stops to look. `#[used]` and `KEEP` must both hold, or LTO drops a slot that only the loader writes.

## M4. Native: the kernel loads HELLO.DMD from a buffer

**Goal.** The first "hello from a loaded program": the kernel, in a new `module` test mode, takes `HELLO.DMD` embedded in its image, gives it frames, loads it with `desk_module::load_into`, binds `desk_log`, calls `app_start` and `app_stop`, frees the frames, and prints the module's lines on serial, in QEMU as a 486.

**Files.**

- `kernel/Cargo.toml`, `kernel/Cargo.lock`: a path dependency on `desk-module`.
- `kernel/src/module.rs`: `static HELLO: &[u8] = include_bytes!("../../dist/modules/HELLO.DMD");`, the image memory (`memory::alloc_contiguous` for `ceil(mem_size / 4096)` frames, freed frame by frame), the service table (`desk_log` writes `  log> ` and the text and a newline through `println!`), and `run(variant)`.
- `kernel/src/main.rs`: `TestMode::Module = 16` and `ModuleBad = 17`; after the FPU demo, `module::run` for those modes, then `boot complete` and success.
- `tools/native-build.sh`: runs `tools/build-modules.sh` before the kernel's `cargo build`. `tools/native-image.sh`: `--test-mode module` and `module-bad`. `tools/native-test.sh`: a "Module loader" section after the BIOS one. `tests/native/expected-module.txt`, `tests/native/expected-module-bad.txt`.
- `docs/NATIVE-BOOT.md`: the two test modes and the build order; `kernel/README.md`: `include_bytes!` needs `dist/modules/HELLO.DMD`, which `tools/native-build.sh` makes.

**API.**

```rust
// kernel/src/module.rs
extern "C" fn svc_log(text: *const u8, len: usize);
static SERVICES: [(&str, u32); 1];      // sorted by name; desk_log only in M4
pub fn run(bad: bool);
```

The transcript lines (after the normal boot's, which stay as they are):

```text
module: HELLO.DMD {N:1000..20000} bytes, image {N:1000..20000}, {N:1..2000} relocations, 1 import
module: HELLO loaded away from its link base: yes
  log> hello from a loaded program
  log> hello: app=1 count=7
module: HELLO app_start returned 0
  log> hello: stop app=1
module: HELLO unloaded, memory as before: yes
```

The kernel also embeds `APPMAIN.DKX` and the `module` mode ends with `module: refused APPMAIN.DKX: an application (kind 2) is not loadable here`, because an application needs a task (R77).

`module-bad` copies the buffer into a heap `Vec`, flips one bit of the image and must print `module: refused HELLO.DMD: bad checksum (stored {HEX}, computed {HEX})`, then `boot complete` with exit status 0. "Memory as before" compares `memory::free_frames()` and `heap::free_bytes()` before the load and after the unload.

**Acceptance.**

- `tools/native-test.sh`: the `module` run matches `expected-module.txt` and the `module-bad` run matches `expected-module-bad.txt`, both with exit 0, on `-cpu 486`.
- Negative runs that must fail: `expected-module.txt` with the `log>` line changed; the `module-bad` transcript against `expected-module.txt`.
- Every existing native check is unchanged (`git diff origin/main -- tests/native/expected*.txt` shows only the two new files).
- `kernel.bin`'s size before and after is in the PR, and stays under the image tool's 1,000-sector limit.
- Optional, reported if tried: the `module` image under Bochs (`NATIVE_EXTRA=1`).

**Depends on**: M3. **Parallel with**: M5. **Labels**: `enhancement`, `rust`, `apps`, `spike`.

**Risks.** `include_bytes!` makes the kernel's build depend on `tools/build-modules.sh` having run; a person running `cargo build` in `kernel/` by hand sees a missing-file error, which the README names. The frames are identity-mapped, so the `&mut [u8]` over them is the one `unsafe` step (`SAFETY:` the frames were just taken from the allocator and nothing else refers to them); calling `start` through `core::mem::transmute` is the other. Test modes 16 and 17 must not collide with another open branch's.

## M5. Hosted: DMDRUN.EXE loads HELLO.DMD in DOSBox

**Goal.** The same `HELLO.DMD` loaded from a DOS file by a Rust-only DOS host on `rt`, under CWSDPMI in headless DOSBox Staging, printing the same module lines as M4.

**Files.**

- `module/dos/Cargo.toml` (its own `[workspace]`, as `shell/dos/`), `module/dos/Cargo.lock`, `module/dos/.cargo/config.toml` (as `shell/dos/.cargo/config.toml`), `module/dos/rustfmt.toml`, `module/dos/src/main.rs`, `module/dos/build.sh` (into `dist/module-dos/`: `DMDRUN.EXE` by `tools/elf2go32.py` behind the stub, `CWSDPMI.EXE`, and the `.DMD` files from `dist/modules/`), `module/dos/check.sh`, `tests/module/expected-dos-hello.txt`, `tests/module/expected-dos-bad.txt`, `tests/module/expected-dos-truncated.txt`.
- `tools/check.sh`: a "Module loader on DOS" step after "Shell on DOS".

**What `main` does.** First `widen_code_segment()`: DPMI 0008h on `CS` with limit `0xFFFF_FFFF`, then reload `CS` with a far return, and print `dmdrun: code segment covers 4 GiB`. Then read the file named on the command line with `rt::dos::File::read_to_vec`, allocate the image with `alloc::alloc::alloc` at the header's alignment, load it with `desk_module::load_into` (the address is `rt::dpmi::ptr_to_linear`'s offset form, which is what a near pointer is), bind `desk_log` (printing through `rt::println!`), call `app_start` and `app_stop`, free the image, and print the same lines as M4 (`memory as before` compares `rt`'s heap free bytes). Arguments: `DMDRUN.EXE HELLO.DMD`, and `nowiden HELLO.DMD`, which skips the widening.

**Acceptance.**

- `module/dos/check.sh` in the CI image: format and lints, build, `DMDRUN.EXE HELLO.DMD` with a transcript equal to `expected-dos-hello.txt` and exit 0; the module lines of that transcript equal the module lines of M4's (`grep -E '^(module|  log>)'` of both, compared by `diff`), so the same file gave the same output on both tracks.
- Negative runs that must fail or refuse: `DMDRUN.EXE NOFILE.DMD` (DOS error 2, exit 1); `BAD.DMD`, a copy with one bit flipped, refused with the bad-checksum line (exit 1); `TRUNC.DMD`, the first 100 bytes, refused as truncated (exit 1); `APPMAIN.DKX`, refused as an application not loadable here (exit 1); `nowiden HELLO.DMD`, which must not print the module's `log>` lines and must exit non-zero, showing that the widening is what makes the call possible.
- The program size is in the PR.
- Opt-in, outside `tools/check.sh`, reported if tried: `DMDRUN.EXE HELLO.DMD` on FreeDOS in QEMU, as `spikes/s1-dxe3/qemu-faults.sh` runs `HOST.EXE`.

**Depends on**: M3. **Parallel with**: M4. **Labels**: `enhancement`, `dos`, `rust`, `apps`, `spike`.

**Risks.** The code-segment limit is the one hosted fact the design inferred rather than observed ([`LOADER.md`](LOADER.md#address-space-assumptions)). If DOSBox does not enforce the `CS` limit on instruction fetch, `nowiden` succeeds in DOSBox and the negative run cannot show the fault there; then the task records that, keeps the widening, makes the FreeDOS run in QEMU the evidence, and changes the `nowiden` check to expect what DOSBox does. If CWSDPMI refuses 0008h on `CS`, the fallback is a 256 KiB module arena in the host's `.bss`, which the stub's `CS` already covers, and the task says so in [`LOADER.md`](LOADER.md). The module is built for the kernel's target specification and the host for `dos-elf.json`; the matching module lines are the check that their C ABI is the same.

## M6. Several modules, unload, leaks and a missing import, on both tracks

**Goal.** The core's side of the loader as one shared crate, so both tracks run the same scenarios and print the same lines: instances with their own statics, the per-instance ledger behind `desk_alloc` and `desk_free`, a missing import refused by name with the core carrying on, and 200 load and unload rounds with memory unchanged.

**Files.**

- `module/host/Cargo.toml`, `module/host/src/lib.rs`, `table.rs` (the sorted service table and `resolve`), `ledger.rs`, `instance.rs`, `scenario.rs`, `module/host/tests/scenario.rs` (every scenario run on the host against a `Platform` that runs no code: it records the calls and answers them from a script). `module/Cargo.toml`: add `host`.
- `module/mods/count/`, `module/mods/needs/` (imports `desk_nothing`), `module/mods/alloc/` (a `Vec<String>` built with `format!` through the guest allocator, and one block kept on purpose at stop). `module/guest/src/lib.rs`: the global allocator over `desk_alloc` and `desk_free`, behind a feature `alloc`.
- `kernel/src/module.rs` and `module/dos/src/main.rs`: each a `Platform` implementation and the four services' `extern "C"` shims; the M4 and M5 code paths become the `hello` scenario. `kernel/src/main.rs`: the `module` mode runs every scenario in order. `tools/native-test.sh`, `module/dos/check.sh`: the transcripts.
- `tests/module/scenarios.txt`: the shared expected module lines of every scenario, which both tracks' transcripts must contain exactly (the cross-track check); `tests/native/expected-module.txt` and `tests/module/expected-dos-*.txt` updated.
- `tools/check.sh`: after the native step, the cross-track comparison of the module lines.

**API.**

```rust
pub trait Platform {
    /// `len` bytes of memory at a multiple of `align`, for an image; its address as the module will see it.
    fn alloc_image(&mut self, len: u32, align: u32) -> Option<ImageMem>;
    fn free_image(&mut self, mem: ImageMem);
    /// The platform's count of free memory, for "memory as before" (frames and heap natively, the heap on DOS).
    fn free_memory(&self) -> u64;
    /// Calls the function at `entry` with three arguments (the guard arrives in M7).
    fn call(&mut self, entry: u32, args: [u32; 3]) -> Call;
    fn line(&mut self, text: &str);
}
pub struct ImageMem { pub addr: u32, pub bytes: &'static mut [u8] }  // frames natively, a leaked heap block on DOS, until free_image
pub enum Call { Returned(u32), Crashed }  // Crashed only from M7

pub struct Host { /* service table, instances, ledger */ }
impl Host {
    pub fn new(services: &'static [(&'static str, u32)], sdk: u16) -> Host;
    pub fn load(&mut self, p: &mut impl Platform, name: &str, file: &[u8]) -> Result<InstanceId, Refused>;
    pub fn start(&mut self, p: &mut impl Platform, id: InstanceId, args: &[u8]) -> Result<i32, Crashed>;
    pub fn stop_and_unload(&mut self, p: &mut impl Platform, id: InstanceId) -> Unloaded; // blocks reclaimed, bytes
    pub fn current(&self) -> Option<InstanceId>;        // which instance a service call comes from
    pub fn ledger_alloc(&mut self, ptr: u32, size: u32, align: u32);
    pub fn ledger_free(&mut self, ptr: u32, size: u32, align: u32) -> Result<(), LedgerError>;
}
pub fn run_scenarios(host: &mut Host, p: &mut impl Platform, files: &Files) -> bool;
```

The platform keeps its `Host` in a `static` that only its service shims and its `run` touch (single processor, no service is called from an interrupt), which is the one place outside the format crate that needs `unsafe` for state, with its `SAFETY:` line.

**Scenarios and their lines.** `life` (load `hello`, start, stop, unload, twice; `count=7` both times); `two` (`count` loaded twice: `count[1]: 7`, `count[1]: 8`, `count[2]: 7`, interleaved, each its own statics); `needs` (refused: `module: refused NEEDS.DMD: missing import desk_nothing`, then `hello` loads and runs); `alloc` (`alloc: kept 3 strings`, then at unload `reclaimed 1 block, N bytes`, with the numbers pinned by the first run and reviewed in the pull request); `places` (`hello` loaded at four different addresses, with spacer allocations of 1, 3 and 7 pages taken between the loads and freed after, so each image lands somewhere new: the module's lines must be identical at every address, and the scenario prints `module: HELLO at 4 distinct addresses, same lines: yes`; this is the behaviour a randomising loader would rely on, [`LOADER.md`](LOADER.md#load-addresses-and-randomisation)); `leak` (200 rounds of `hello` and of `alloc`, then `memory as before after 200 rounds: yes`; a control round that leaks on purpose in the host must report `no`).

**Acceptance.**

- `cargo test` in `module/`: every scenario on the scripted platform; the ledger refuses a free of a block it does not hold and a free with the wrong size; an instance's blocks are all reclaimed at unload.
- Native and DOS: every scenario's lines equal `tests/module/scenarios.txt`; the cross-track comparison passes; with one line of `scenarios.txt` changed, both tracks' comparisons fail.
- The deliberate-leak control is reported as a leak on both tracks (the measure can see one).
- `places`: on both tracks the four addresses are distinct (the scenario fails if two coincide, so a platform that kept reusing one block would not pass silently) and the lines are identical.

**Depends on**: M4, M5. **Parallel with**: nothing (wave 4). **Labels**: `enhancement`, `rust`, `apps`, `sdk`, `dos`, `spike`.

**Risks.** `rt`'s heap never returns blocks to the host, and the kernel's heap merges neighbours: "memory as before" compares the allocator's own free count, which is right for both, and not the host's figures (S1 compared DPMI figures because its allocator was DJGPP's). The scripted platform must answer calls in the order the real ones happen; when the two disagree, the emulator is right and the script is fixed. Keep `Host` free of platform types, or each track needs a shim.

## M7. A contained panic on both tracks

**Goal.** A panic in a module ends that instance, frees what it held, and leaves the core and the other modules working, on both tracks, through the guard in [`LOADER.md`](LOADER.md#containing-a-crash).

**Files.**

- `module/guard/Cargo.toml`, `module/guard/src/lib.rs`: `GuardFrame`, `desk_guard_call`, `desk_guard_escape` in `global_asm!`, with the rules of what may be abandoned as the crate's documentation. `module/Cargo.toml`: add `guard` (built for both targets by the cross step; there is nothing to run on the host).
- `module/mods/panicky/` (holds five blocks, then panics in `app_start` on its first load and in `app_event` on its second).
- `kernel/src/module.rs`, `module/dos/src/main.rs`: `Platform::call` through the guard; the `desk_panic` service copies the message into a static buffer and escapes; after an escape, `FNINIT` and the core's control word. `module/host/src/scenario.rs`: the `panic` and `panic-event` scenarios. `tests/module/scenarios.txt`: their lines.

**API.**

```rust
#[repr(C)]
pub struct GuardFrame { ebx: u32, esi: u32, edi: u32, ebp: u32, esp: u32, eflags: u32, eip: u32 }
pub const RETURNED: u32 = 0;
pub const CRASHED: u32 = 1;
extern "C" {
    /// Calls `entry(a0, a1, a2)`; returns (RETURNED, its result) or (CRASHED, 0) if `desk_guard_escape` was called meanwhile.
    pub fn desk_guard_call(frame: *mut GuardFrame, entry: u32, a0: u32, a1: u32, a2: u32, result: *mut u32) -> u32;
    pub fn desk_guard_escape(frame: *mut GuardFrame, status: u32) -> !;
}
```

**Scenarios and their lines.** `panic`: `hello` started, `panicky` started, `module: PANICKY crashed: panic: deliberate panic number 1 (src/lib.rs:LINE)`, `module: PANICKY unloaded, reclaimed 5 blocks, N bytes`, `hello` receives a stop and its line, a second `panicky` crashes in `app_event` with number 1 again (fresh statics), and `module: the core kept running`. `panic-event`: an event delivered to three instances in turn, the middle one `panicky`; the third still gets it.

**Acceptance.**

- Both tracks: the `panic` and `panic-event` lines equal `scenarios.txt`; memory as before after the scenario.
- Negative: with the guard bypassed (`noguard` on DOS, test mode `module-noguard` = 18 natively), `desk_panic` finds no active guard, prints `module: panic with no guard: <message>` and ends the run (exit status 3 on DOS, 1 natively); the checks require both.
- The x87 control word after an escape equals the one before (printed and compared).

**Depends on**: M6. **Parallel with**: nothing (wave 5). **Labels**: `enhancement`, `rust`, `apps`, `dos`, `spike`.

**Risks.** The guard is the sharpest code in the spike: the saved `ESP` must be the caller's after the call's arguments are pushed, and `EBP`-based frames in the caller must survive. Write it against the cdecl rules, test it first with a module that only escapes (no panic machinery) and then with `panicky`. The analysis's [section 4](RUST-DOS-INTEGRATION-ANALYSIS.md#4-panic-recovery-is-narrower-than-app-isolation) is the review checklist: the PR lists every service and why its frame holds nothing when an escape can pass it.

## M8. CPU faults in a module, natively (optional)

**Goal.** A divide error, an invalid opcode and a general-protection fault in a module are turned into the same unload as a panic in the native kernel; without a guard they stop the machine as today.

**Files.** `kernel/src/idt.rs` (the handlers for vectors 0, 6 and 13 ask `module::fault(eip)` first), `kernel/src/module.rs` (`fault` escapes when a guard is active and `eip` lies in the running instance's image), `module/mods/faulty/` (three entry arguments select the fault), `module/host/src/scenario.rs` (`faults`, native only), `tests/native/expected-module.txt`.

**Acceptance.** The three faults each print `module: FAULTY faulted: divide error at image offset {HEX}` (and the other two), the instance is unloaded with its blocks, and `hello` runs after; a fault in kernel code during the module test mode (the existing `div0` mode, unchanged) still stops the machine with exit status 1; memory as before.

**Depends on**: M7. **Labels**: `enhancement`, `rust`, `apps`, `spike`.

**Risks.** The exception frame in ring 0 has no saved `ESP`; the escape restores the guard's `ESP`, which abandons the frame correctly only because no privilege change happened. A fault inside a service called by the module has its `EIP` in the kernel, so it is not contained, which is the safe answer; the PR says so.

## M9. The result

**Goal.** The spike's written result and the documents brought up to date with what was built.

**Files.** `docs/LOADER.md`: a "Result" section (what was built, the sizes measured against the estimates, the transcripts of both tracks, what differed from the design, what was not verified), and corrections where the code chose differently. `docs/FORMATS.md`: the `.DMD` section as built; `APP.INF`'s `module=`. `docs/SDK.md`, `docs/SPEC.md`, `docs/DIRECTION.md`, `docs/RUST-TARGETS.md`, `docs/HAL.md`, as [`LOADER.md`](LOADER.md#what-changes-in-other-documents) lists. `docs/ROADMAP.md` and `tools/progress-report/roadmap.json`: R48 to `done` only after the result's pull request is merged; the result's pull request sets R48's evidence and leaves the status for the merge.

**Acceptance.** `tools/check.sh` passes in full in the CI image; the documents pass `tools/check-docs.py`; the "not verified" list is written first and covers at least everything in [What the spike will not prove](LOADER.md#what-the-spike-will-and-will-not-prove).

**Depends on**: M7, and M8 if it is built. **Labels**: `documentation`, `design-docs`, `spike`, `rust`.

**Risks.** None new; this is where an unverified claim is most tempting.

## Definition of done

The spike is done when, on `main`:

- `tools/check.sh` runs the format's and the host crate's tests, builds and packs the test modules, runs the cross check, loads them in the native kernel in QEMU and in `DMDRUN.EXE` in DOSBox, and compares the two tracks' module lines, all passing in the CI image;
- every rejection path has a host test, and the emulator runs refuse a damaged, a truncated and a mismatched module;
- the same `HELLO.DMD` prints the same lines on both tracks and at four different addresses on each; an application file, a library kind and an export table are refused; two instances keep their own state; 200 rounds leave memory as before; a missing import is named and refused; a panic is contained on both tracks;
- [`LOADER.md`](LOADER.md) has its result section and the documents it lists are updated.
