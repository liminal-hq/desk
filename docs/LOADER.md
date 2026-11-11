# Module loader: one executable format for both tracks

The design for roadmap item R48, the executable-format and loader spike: what a Desk app is on disk, and how a program gets from a file into the running system, on the native kernel and on the hosted DOS track, with the same file on both. It is the first step on the roadmap's path to the first OS milestone (R80, a separate app loaded from disk). The build plan is [`LOADER-PLAN.md`](LOADER-PLAN.md). What a program is on Desk beyond the file (tasks, threads, scheduling, the window system as a task, libraries) is designed in [`APP-MODEL.md`](APP-MODEL.md); this document is the format and the loader.

> **Status.** A design, written on 2026-11-11 against `main` at `eaede0f` and revised the same day with the owner's decisions ([Decisions](#decisions)). Nothing in it is built. The one experiment behind it (an app-shaped Rust module linked by `rust-lld` and read with `readelf`, under [Evidence](#evidence-gathered-for-this-design)) ran on the development machine in a scratch directory and is not committed; every other number is a target or an estimate and says so. Nothing here has run on real hardware. Where this document says "the loader" it means the module loader; the boot stage that loads the kernel (`boot/stage2.asm`), which [`NATIVE-BOOT.md`](NATIVE-BOOT.md) also calls the loader, is unrelated.

## Contents

- [The question](#the-question)
- [What exists today](#what-exists-today)
- [Constraints](#constraints)
- [Evidence gathered for this design](#evidence-gathered-for-this-design)
- [The candidate formats](#the-candidate-formats)
- [Address-space assumptions](#address-space-assumptions)
- [The hosted track: DXE3, the C layer and FreeDOS](#the-hosted-track-dxe3-the-c-layer-and-freedos)
- [Recommendation](#recommendation)
- [Decisions](#decisions)
- [Format specification sketch: `.DMD`](#format-specification-sketch-dmd)
- [Building a module](#building-a-module)
- [Where the code lives and its rules](#where-the-code-lives-and-its-rules)
- [Containing a crash](#containing-a-crash)
- [Load addresses and randomisation](#load-addresses-and-randomisation)
- [Libraries, `std` and data files](#libraries-std-and-data-files)
- [What the spike will and will not prove](#what-the-spike-will-and-will-not-prove)
- [Risks](#risks)
- [What changes in other documents](#what-changes-in-other-documents)
- [Questions for the owner](#questions-for-the-owner)

## The question

Until now every Rust program in the repository is linked whole: the kernel is one flat image at 1 MiB, and `SHELL.EXE` is one go32 image behind DJGPP's stub. An operating system needs a program that is built separately, stored as a file, and brought into the running system later. Four questions follow:

1. **What is an app on disk?** The bytes, the header, what they promise and how a damaged or foreign file is refused.
2. **How does it get into memory?** Where the image goes, how its absolute addresses are fixed for that place, and how it is called.
3. **How does it reach the Desk?** How a call to `desk_log` in the app becomes a call into the core, and what happens when the core does not have the function.
4. **How does it leave?** Normal unload, a panic, a CPU fault, and what memory comes back.

The answer has to hold on the **native** kernel (ring 0, flat segments, no paging yet) and on the **hosted** track (a DPMI client under CWSDPMI, where the Rust-only programs have no dynamic loader at all), and ideally with the same file on both, so an app is built once.

## What exists today

| Piece | What it does | Limit for this question |
|---|---|---|
| [`S1-DXE3.md`](S1-DXE3.md) | Rust `no_std` modules as DJGPP DXE3 files, loaded with `dlopen` by a host in C; imports by name from a table the host registers with `dlregsym` (route a); panic recovery through `setjmp` in C; 200 load and unload rounds with memory unchanged | Needs DJGPP's C library at run time, `coff2go32.py` and `dxe3gen` at build time, and LTO; one file is one instance; a stack overflow is unrecoverable; hosted only |
| [`D0-RUST-DOS.md`](D0-RUST-DOS.md), `spikes/d0-rust-dos/` | A Rust-only DOS program: `rt` (start-up, DPMI, INT 21h, heap, panic), `dos-elf.json`, `link.ld`, and [`tools/elf2go32.py`](../tools/elf2go32.py), which packs a linked ELF behind the 2 KiB stub | A program, not a module: the stub loads one image at a fixed offset, with no relocations and no imports |
| `kernel/` ([`NATIVE-BOOT.md`](NATIVE-BOOT.md)) | A ring-0 kernel linked at `0x100000` (`kernel/link.ld`), flattened by [`tools/elf2bin.py`](../tools/elf2bin.py); a frame allocator and a 4 MiB first-fit heap, identity-mapped, no paging; `include_bytes!` already embeds the BIOS trampoline and the fonts | Nothing in it loads code; there is no task, no ring 3 and no system call |
| [`RUST-DOS-INTEGRATION-ANALYSIS.md`](RUST-DOS-INTEGRATION-ANALYSIS.md) | Names the gap: "no equivalent loader in the Rust-only executable"; asks for one app slice with a real SDK call, unload and a survived failure before the C path is retired; warns that `elf2go32.py`'s ELF parser accepts malformed input (section 6) and that `desk_alloc` has two signatures (section 5) | A list of contracts, not a design |
| [`SDK.md`](SDK.md), [`FORMATS.md`](FORMATS.md) | The app ABI shape (`extern "C"` `desk_*`, integers, pointers and lengths, `app_start`, `app_event`, `app_stop`) and `APP.INF` with `module=APP.DXE` | Design only, written for DXE3; R79 makes the SDK real |
| [`HAL.md`](HAL.md#apps-and-modules) | Apps use the SDK, never the HAL; the loader is part of the core on each track and only its memory comes from the platform | — |

## Constraints

- **The core and the HAL.** The format reader and the load algorithm are OS-independent: they must not call DOS, DJGPP or the BIOS. Each track supplies the memory for the image and the services the app imports; nothing else crosses ([`AGENTS.md`](../AGENTS.md), [`HAL.md`](HAL.md#apps-and-modules)).
- **`no_std` + `alloc`, integer only, no compare-and-swap, no external crates.** The reader and the load algorithm follow the rules of `shell/`, including `#![forbid(unsafe_code)]`: everything they do is reading a byte slice and writing words into another byte slice, which needs no `unsafe`. The packer is a host tool and may use `std`, but no external crates either.
- **Where `unsafe` is allowed.** Three places, each with its `SAFETY:` line: turning freshly allocated memory into a `&mut [u8]` (the kernel's frames, the DOS host's heap), calling the entry point through a pointer made from an address, and the few instructions of assembly that leave a crashed module ([Containing a crash](#containing-a-crash)). The app-side runtime calls through import slots and so also has `unsafe`, behind a safe API.
- **486.** No instruction past the 486 in the loader or in a module (the targets already say `cpu: i486`), and no `CMPXCHG`, which DOSBox's `cputype = 486` rejects ([`D0-RUST-DOS.md`](D0-RUST-DOS.md#what-failed-on-the-way)).
- **Determinism where it matters.** The load is a pure function of the file's bytes, the address it is loaded at and the service table: the same three inputs give the same image, byte for byte, which is what the host tests check. Addresses differ between runs and tracks, so transcripts print them through the `{HEX}` pattern of `tools/native-check.py` or not at all.
- **The app model is design only.** R79 makes the SDK real and R77 adds the system-call boundary; the spike uses the smallest service table that proves the mechanism (`desk_log`, `desk_panic`, `desk_alloc`, `desk_free`) and does not settle the SDK.
- **The shell and apps are Rust.** A format is judged partly by whether it keeps the build a Rust build: `cargo`, `rust-lld` and a small tool of our own, with no C compiler, binutils or DJGPP in the path.
- **Floppy-sized.** A module should be small on disk: no symbol tables, no page-aligned file offsets, no debug sections in the shipped file.

## Evidence gathered for this design

One experiment, to settle whether `rust-lld` can hand a packer what a relocating loader needs without position-independent code. It used the kernel's target (`kernel/i486-desk-none.json`: `relocation-model: static`, i486, x87), `-Zbuild-std=core,compiler_builtins`, the kernel's release profile (`opt-level = "s"`, LTO, one codegen unit, `panic = "abort"`), a linker script that places `.text`, `.rodata` and `.data` from `0x400000`, and `-Clink-arg=--emit-relocs`. The module had two import slots (`#[no_mangle] #[link_section = ".desk.imp"] static __imp_desk_log: AtomicUsize`), an `extern "C" fn app_start`, a `static` counter, a `match` with seven arms, and `core::fmt` formatting of integers into a stack buffer. Rust 1.98.0 on the development machine; read with `readelf` and a few lines of Python.

| Observation | Value |
|---|---|
| File type | `ET_EXEC`, three `PT_LOAD` segments (R E, R, RW) whose addresses are contiguous from `0x400000`; file offsets page-aligned |
| Relocations kept by `--emit-relocs` | `.rel.text` and `.rel.rodata`: 32 `R_386_32` and 21 `R_386_PC32`, nothing else |
| What they point at | Every `R_386_32` names a section symbol (`.rodata`, `.data`) or an import slot; every `R_386_PC32` is a call or jump inside `.text` |
| Import slots | Present in `.symtab` as `GLOBAL OBJECT` symbols in `.data`, with their names; code reaches them with `R_386_32` (an absolute load of the slot, then an indirect call) |
| Image | 3,228 bytes from the first byte of `.text` to the end of `.data` |
| The same module packed (estimate) | About 3,490 bytes: a 96-byte header, the image, 32 four-byte relocations, two import records and their names |
| The ELF as linked | 9,892 bytes (symbols, section headers, page-aligned offsets) |
| The same code as a PIE (`-pie -z notext`, static code) | Links; 32 `R_386_RELATIVE` in `.rel.dyn`; 10,064 bytes, 4,408 stripped and linked with `-n` |

What it settles: with static code, the relocations of the final link are exactly the absolute words (`R_386_32`), each of which a loader adjusts by one addition, and the PC-relative ones need nothing when the image moves as a whole. No GOT, no PLT, no position-independent code. Import slots by name come for free from the symbol table. What it does not settle: a module with `alloc`, a large `.bss` (the test's 4 KiB array was optimised away), and a run of anything; those are the spike's first tasks.

## The candidate formats

| | Loader on both tracks | Packer and tooling | Size on disk (the sample) | Imports | Debugging | Keeps the build Rust | Verdict |
|---|---|---|---|---|---|---|---|
| **A. Plain ELF, static, fixed address** | Copy the `PT_LOAD` segments; nothing to relocate | None (`rust-lld`) | 9,892 as linked; less stripped | None unless the loader reads `.symtab` | Best: `gdb` reads the file as is | Yes | Rejected: one fixed address per app, which the kernel's heap and the DOS host's heap cannot promise |
| **B. ELF PIE, static code (`-pie -z notext`)** | Parse the ELF header, program headers and `PT_DYNAMIC`, apply `R_386_RELATIVE`; for imports, also `.dynsym`, `.dynstr` and `R_386_32` or `GLOB_DAT` against undefined symbols | None | 4,408 stripped and `-n` | Through the dynamic symbol table, with lld's rules for undefined symbols | Good | Yes | Rejected for now: the largest validation surface, no place for an SDK version or a checksum, and a parser that is easy to make permissive (the analysis's section 6) |
| **C. Flat binary, small header, relocation table** | Copy, zero the tail, add a delta to listed words (about 100 lines, estimated) | A packer that reads the ELF once | About 3,450 | None in the file: a table of function pointers passed to `app_start` (S1's route b) | The ELF kept beside it, loaded in `gdb` at an offset | Yes | A close second; it is D without imports |
| **D. Our own container: C plus named import slots, versions and a CRC** | C plus a lookup per import and a word written per slot (about 300 lines with every check, estimated) | The same packer, which also reads `.symtab` for slot names | About 3,490 | By name, resolved at load, refused by name when missing (S1's route a) | As C | Yes | **Recommended** |
| **E. DJGPP go32 COFF, and DXE3** | The stub loads a go32 program at a fixed offset; DXE3 needs DJGPP's `dlopen` in C at run time; the kernel would need a DXE3 reader of its own | `coff2go32.py`, `dxe3gen`, DJGPP binutils | 7,734 (`HELLO.DXE` in S1, a different program) | By name through `dlregsym` | DJGPP's tools | No: C and binutils in the path | Rejected for the product; S1 stays the hosted precedent |
| **F. PE or LE** | PE: base-relocation blocks and an import table by name or ordinal; LE: per-page fixup records and an object table | A COFF or PE link, which is the MinGW-flavoured path the skeleton had to repair (`coff2go32.py`) | — | Yes | Some tools | Not without fighting the toolchain | Rejected as formats; D borrows PE's ideas (a preferred base, base relocations, an import address table) |
| **G. Position-independent code in a single address space** | Little, in theory | Needs `relocation-model: pic` | — | Through the GOT | — | Yes | Rejected: on x86-32 PIC keeps a GOT pointer in a register (one of seven), and Rust's `static` tables of pointers (`&str` arrays, vtables) still need run-time relocation, which the sample's `.rel.rodata` shows |

**A. Plain ELF at a fixed address.** The simplest loader and the best debugging, and the right answer once each task has its own address space (R78), where every app can live at the same virtual address. Today the kernel hands out physical frames and the DOS host gets heap blocks from DPMI; neither can promise an address, and two apps could not both have it. A could come back as a fast path of D: a module loaded at its link base needs no relocation.

**B. ELF PIE.** A real option: lld links non-PIC code as a PIE when text relocations are allowed, and the loader then applies `R_386_RELATIVE`. It saves writing a packer. It costs a loader that has to parse and validate ELF's dynamic structures in the kernel and in the DOS host, imports that depend on how lld treats undefined symbols in a PIE, and a file with nowhere to put Desk's own fields (the SDK version, a checksum for a floppy) without inventing note sections. It is the fallback if the packer proves fragile.

**C and D.** Both need a packer, because the relocation list lives in non-allocated ELF sections that a loader should not have to read. Once the packer exists, imports by name cost one more pass over `.symtab`, eight bytes per import and a lookup at load; in exchange the core can add functions without disturbing old modules, a missing function is refused at load with its name (S1's finding for route a), and the same slots can later point at system-call stubs instead of kernel functions without changing the format ([Address-space assumptions](#address-space-assumptions)). A table passed to `app_start` (route b) remains possible inside D: nothing stops an app from receiving one.

**E. DXE3.** It works today on the hosted track and it is the recorded precedent for the lifecycle, but it brings DJGPP's C library, `dlopen` and `setjmp` into every hosted app's path and has no native counterpart.

**F. PE and LE.** Mature, documented and heavier than needed. Producing PE from Rust would mean the MinGW-flavoured COFF target that the skeleton needed `coff2go32.py` to repair. LE (the DOS/4GW executables) is more complex still.

**G. Position independence.** It does not remove relocations from Rust code, and it costs a register on a CPU that has few.

## Address-space assumptions

**Flat, no paging, one address space.** On the native track a module runs in ring 0 with the kernel's flat 4 GiB segments (`kernel/src/gdt.rs`); on the hosted track it runs in the DPMI client's segments, where a pointer is an offset from the image block's base (`rt::dpmi`). In both, a module can read and write anything the core can. It is trusted code, exactly as a DXE3 module is ([`S1-DXE3.md`](S1-DXE3.md#5-panics-and-crashes)), and the unload on a crash is not a sandbox.

**Relocation, with a preferred base.** Every module is linked at the same base, `0x0040_0000` (4 MiB, page-aligned; not 0, so no function or static can sit at the null address that `Option<fn>` and null checks rely on). The loader puts the image wherever the platform gives it memory, at the alignment the header asks for, and adds `load address − link base` (wrapping) to every word in the relocation table. The arithmetic is the same on both tracks because both use 32-bit near pointers: on DOS the "address" is the offset in the data segment, which is what Rust's pointers already are under `rt`.

**The hosted code segment.** Under `rt`, `DS` has a 4 GiB limit but `CS` keeps the stub's limit, the size of the image's block ([`D0-RUST-DOS.md`](D0-RUST-DOS.md#2-the-runtime-contract); `rt::dpmi::init` lifts only `DS`). Memory from the heap lies outside that block, so a jump into a module loaded there would fault on the code-segment limit. This is inferred from the recorded contract, not observed. The hosted loader therefore raises the limit of `CS` to 4 GiB with DPMI function 0008h and reloads `CS` with a far return before the first call, which is how DJGPP's near-pointer mode is understood to work (not checked here); the plan's first hosted step shows the fault without it and the call with it. The fallback, if a host refuses, is a fixed arena for modules inside the host's own `.bss`, which `CS` already covers.

**Imports map to a table the core registers.** The core keeps one sorted table of `(name, address)` pairs, which is what `dlregsym` was for DXE3. The module carries one 32-bit slot per import in its writable data, and a record naming the slot; the loader finds each name in the table and writes the address into the slot. The module calls through the slot. One global table in the spike; a table per app (for permissions) is a change to the caller of the loader, not to the format.

**Calling convention.** `extern "C"` (cdecl) on the i486 targets, with only integers, pointers and lengths across ([`SDK.md`](SDK.md)). The module is built for the kernel's target specification and loaded into the DOS host too; the two specifications describe the same LLVM target (`i486-unknown-none-elf`), the same data layout and the same C ABI, and the spike checks that by running the same file on both. A module may assume the direction flag is clear and the x87 is initialised to the core's control word, as Rust code does.

**Stack.** A module runs on its caller's stack: the kernel's 64 KiB stack natively, `rt`'s 256 KiB stack on DOS. Neither has a guard page, so a stack overflow in a module corrupts what lies below, as S1 found; apps keep the SDK rule of bounded recursion and small frames.

**One file, many instances.** Each load is a separate copy with its own statics, so the same file loaded twice is two instances. This is a difference from DXE3, where a second `dlopen` of a loaded file returned the same instance ([`S1-DXE3.md`](S1-DXE3.md#4-rust-runtime-in-the-module)); whether the Desk wants one instance per app is the app model's choice (R79), not the format's.

**Unload.** The core frees the image and everything the module allocated through `desk_alloc`, which it tracks per instance in a ledger (S1's mechanism, carried over). Static data starts over on the next load, because the next load is a fresh copy of the file.

**What per-task address spaces (R78) change later.** With paging, each task can have its own space with the image mapped at its link base: the relocation step becomes a no-op (the delta is zero), the read-only part of the image (`ro_size` in the header) can be mapped read-only, and a page below each stack becomes a guard, which turns a stack overflow into a clean page fault. With a system-call boundary (R77) and ring 3, the core resolves the same import names to small user-mode stubs that trap into the kernel, instead of to kernel functions. The format does not change for either: that is the reason to choose named slots and a preferred base now.

## The hosted track: DXE3, the C layer and FreeDOS

**DXE3 stays the precedent, not the product path.** The hosted Desk is a Rust-only program on `rt` (`SHELL.EXE`); the format lets it load apps with a loader written in Rust and the same file the native kernel loads, without `dlopen`, `libc` or `dxe3gen`. `APP.INF`'s `module=APP.DXE` becomes `module=APP.DMD` once the spike's result is in ([What changes](#what-changes-in-other-documents)).

**The C-layer retirement question.** The analysis's advice is to retire each C service only when a replacement passes the same scenario ([section 1](RUST-DOS-INTEGRATION-ANALYSIS.md#1-runtime-migration-lacks-an-integration-acceptance-gate)). What DXE3 needed C for, and what replaces it here:

| Service | DXE3 path (C) | This design (Rust) | Replaced by the spike? |
|---|---|---|---|
| Loading, relocating, binding imports | DJGPP's `dlopen`, `dlregsym` | `desk-module`'s `load_into` and the core's table | Yes |
| Panic recovery | `setjmp` and `longjmp` in the host | A guard of a few instructions in assembly ([Containing a crash](#containing-a-crash)) | Yes, for panics |
| CPU faults (divide error, null write) | DJGPP `signal` handlers around every call | DPMI exception handlers (function 0203h) in `rt`, which `rt` does not have yet | No: hosted fault recovery stays a DXE3-only capability until `rt` gains exception handlers |
| Building | `coff2go32.py`, `dxe3gen`, `libgcc.a` | `cargo`, `rust-lld`, `dmdpack` | Yes |

So the spike removes the loader and the panic path from the reasons to keep C, and leaves one: CPU-fault recovery on the hosted track. Promoting `rt` (R53, R139) is where the exception handlers belong, and D0 already recommended them.

**FreeDOS (R49).** On the native OS, DOS programs run under bundled FreeDOS, in a V86 task or by booting into it ([`DIRECTION.md`](DIRECTION.md#dos-programs-bundle-freedos)); they are not Desk modules and this format has nothing to do with them. On the hosted track FreeDOS is one more DOS under which the DOS host runs: the owner has run `SHELL.EXE` on FreeDOS in QEMU, and S1's fault runs used FreeDOS 1.4. The plan includes an opt-in run of the DOS host on FreeDOS in QEMU, outside `tools/check.sh` as S1's `qemu-faults.sh` is. How a DPMI program such as the hosted Desk would run under the native kernel's V86 monitor is R49's question.

## Recommendation

**An own container: one image linked at `0x0040_0000` with static code, a table of absolute words to relocate, named import slots resolved against a table the core registers, three entry offsets, a `kind` (module, app, library), a reserved export table, a format version, an SDK version and a CRC-32; one container with two extensions, `.DMD` for a module (and later a library) and `.DKX` for an application (decided, [Decisions](#decisions)); packed from the linked ELF by a Rust host tool; loaded by one `no_std` Rust function that does no `unsafe`, called by a few lines of platform code in the kernel and in a Rust-only DOS host.** The same file runs on both tracks.

Why: it is the smallest format that relocates (so the platform chooses the address), refuses a damaged or mismatched file before any of it runs, names a missing service, and survives the move to paging and system calls unchanged. It keeps the build to `cargo`, `rust-lld` and a tool of our own, and it keeps the load algorithm host-testable, because loading is writing words into a byte slice.

Rejected, in short: plain ELF at a fixed address (no address to fix on today's memory managers; it returns with R78 as a fast path); ELF PIE (a larger loader and validation surface, no room for Desk's fields; the fallback if the packer proves fragile); a flat binary without imports (a close second, kept inside D as the possibility of passing a table); DXE3 and go32 COFF (C and DJGPP in the path, hosted only); PE and LE (heavier, and the wrong toolchain); position-independent code (does not remove relocations from Rust, costs a register).

## Format specification sketch: `.DMD`

To be added to [`FORMATS.md`](FORMATS.md) once the spike's result confirms it. All numbers little-endian. The sections follow each other in this order, each starting at a multiple of 4: header, image, relocations, imports, exports (empty in version 1), strings. The file ends exactly where the strings end.

**Header, 96 bytes in version 1.**

| Offset | Size | Field | Rule |
|---|---|---|---|
| 0 | 4 | magic `DMD1` | The digit is the major version; `DMD` with another digit is refused as an unsupported version, anything else as not a module. The magic is the same for every kind and both extensions |
| 4 | 2 | header size | 96 in version 1; a later minor revision may grow it, and a loader reads only the fields it knows |
| 6 | 2 | flags | 0; a loader refuses any bit it does not know |
| 8 | 2 | ABI | 1 = i486, flat 32-bit near pointers, cdecl, x87 present |
| 10 | 2 | SDK | The version of the service table the file needs; refused if newer than the core's (the role of `sdk=` in `APP.INF`) |
| 12 | 2 | kind | 1 = module (loaded into the caller's address space; `.DMD`), 2 = application (becomes a new task; `.DKX`), 3 = library (reserved: refused in version 1); any other value is refused |
| 14 | 2 | reserved | 0 |
| 16 | 4 | link base | The address the image was linked for: `0x0040_0000` for every file the build ships; a loader accepts any non-zero multiple of 4096, which the packer's cross check uses |
| 20 | 4 | alignment | A power of two from 16 to 4096; the load address must be a multiple |
| 24 | 4 | image offset | Where the image starts in the file; at least the header size |
| 28 | 4 | file size | Bytes of image stored in the file |
| 32 | 4 | memory size | At least the file size; the bytes past the file size are zeroed at load (`.bss`); at most 16 MiB in version 1 |
| 36 | 4 | read-only size | At most the file size: the leading part of the image that is code and read-only data, which paging may later protect |
| 40 | 4 | start | Image offset of the entry: `app_start` for a module, `desk_main` for an application; must lie in the read-only part |
| 44 | 4 | event | Image offset of `app_event`, or 0 for none; always 0 for an application |
| 48 | 4 | stop | Image offset of `app_stop`, or 0 for none; always 0 for an application |
| 52 | 4 | relocation offset | File offset of the relocation table |
| 56 | 4 | relocation count | Entries in it |
| 60 | 4 | import offset | File offset of the import table |
| 64 | 4 | import count | At most 256 in version 1 |
| 68 | 4 | export offset | File offset of the export table: reserved, so in version 1 it equals the end of the import table |
| 72 | 4 | export count | Reserved: 0 in version 1, for every kind |
| 76 | 4 | strings offset | File offset of the string table |
| 80 | 4 | strings size | Bytes in it |
| 84 | 4 | CRC-32 | Of the whole file with these four bytes taken as zero (the IEEE polynomial, the same CRC as `zlib.crc32` and `desk-gfx`'s `crc32`) |
| 88 | 8 | reserved | 0 |

**Image.** The bytes of `.text`, `.rodata` and `.data` laid out at their link addresses relative to the link base, with any gap between segments filled with zeros. Import slots are words in the writable part, stored as zero.

**Relocations.** `relocation count` entries of 4 bytes, each the image offset of a 32-bit word to which the loader adds `load address − link base`. Strictly ascending, at least 4 apart (no two words overlap), each word inside the file-backed part of the image (`offset + 4 ≤ file size`), and none at an import slot. At load, every word's stored value must lie within `[link base, link base + memory size]` (one past the end is a valid Rust pointer); any other value is an out-of-range relocation.

**Imports.** `import count` entries of 8 bytes: the offset of the name in the string table, then the image offset of the slot. Slots are 4-aligned, in the writable part (`ro_size ≤ slot`, `slot + 4 ≤ file size`), zero in the file and unique. Names are NUL-terminated in the string table, 1 to 31 bytes of `a` to `z`, `0` to `9` and `_`, and unique within the module.

**Exports.** Reserved for the `library` kind ([Libraries](#libraries-std-and-data-files)): 8-byte records of a name offset in the string table and an image offset, sorted by name. Version 1 requires the table to be empty and in its place; a file with exports, or of kind library, is refused as unsupported, so a version-1 loader never half-understands one.

**Strings.** The import names, each followed by a NUL. Nothing else in version 1.

**Loading.** Given the file, a destination of at least `memory size` bytes at an address that is a multiple of `alignment`, and a function from a name to an address: check the header, the layout and the CRC; check every relocation and import record; resolve every import (refusing the module with the first missing name); then copy the image, zero the tail, add the delta to each relocation, and write each import's address into its slot. The result is the absolute address of each entry point and the image's range. Nothing is called by the loader.

**Errors.** Every check above is its own error, with the index or offset it concerns, so a refused module says why: too short, bad magic, unsupported version, bad header size, unknown flags, unsupported ABI, SDK too new, unknown kind, kind not supported (a library), exports not supported, kind not loadable here (an application given to a module loader, or the reverse), bad alignment, a table truncated or outside the file, tables overlapping or out of order, trailing bytes, sizes out of order (file size above memory size, read-only size above file size), too large, bad checksum, entry out of range, relocation outside the image, relocations out of order or overlapping, relocation target out of range, bad import name, duplicate import, bad import slot, missing import, destination too small, destination misaligned.

**Versioning.** The major version is in the magic and changes only for an incompatible layout. Compatible additions grow the header and set a flag bit that old loaders refuse when the addition matters, or leave the flags alone when it does not. The SDK version is separate and belongs to the service table, not to the layout. The ABI field changes only for another machine model (for example ring 3 with system calls, if the stubs ever need a different calling convention).

**Entry points.** A module has the callback shape of [`SDK.md`](SDK.md#an-apps-entry-points), without the leading underscore, which belonged to COFF: `app_start(app: u32, args: *const u8, args_len: u32) -> i32` (0 means started), `app_event(app: u32, event: *const DeskEvent)`, `app_stop(app: u32)`. An application has one entry, `desk_main(args: *const u8, args_len: u32) -> i32`, run as the first thread of its new task; it loops on a blocking `next_event()`, and a small runtime library offers the callback shape on top of that loop ([`APP-MODEL.md`](APP-MODEL.md#the-shape-of-an-app)). The spike loads modules only; an application needs a task (R77), so the spike's loaders refuse kind 2 with "not loadable here", and the format tests cover it.

**Kinds and policy.** The kinds differ only in what the loader's caller does with the result: a module is placed in the caller's address space and called; an application becomes a new task, whose imports resolve to the same names as small stubs that become system calls when ring 3 arrives; a library (later) is placed once and satisfies other files' imports. The layout, the relocation and the binding are the same code for all three.

## Building a module

```bash
# A module crate: no_std, no_main, panic = "abort", built for the kernel's target specification with the module linker script.
(cd module/mods && RUSTC_BOOTSTRAP=1 cargo build --release --locked)
# rustflags in module/mods/.cargo/config.toml: -Tmodule.ld -nostdlib --emit-relocs -zmax-page-size=0x1000
module/pack/target/release/dmdpack pack module/mods/target/i486-desk-none/release/hello dist/modules/HELLO.DMD
module/pack/target/release/dmdpack info dist/modules/HELLO.DMD    # the header, the counts and the import names
```

`file -b -m tools/desk.magic dist/modules/HELLO.DMD` names the result without installing anything: `Desk loadable image, version 1, module (.DMD), i486 flat, SDK 1, 96-byte header` (and `Desk loadable image, version 1, application (.DKX), i486 flat, SDK 1, 96-byte header` for an application). The first phrase is the same for every kind on purpose, because modules, applications and libraries are one container, so the family is named by a neutral "loadable image" and the kind follows; the magic file reads the version-1 offsets (4 header size, 8 ABI, 10 SDK, 12 kind) and changes whenever the header table does. No MIME type is given: none is registered, and inventing one would mislead.

`tools/build-modules.sh` runs these for every module and keeps each ELF beside its `.DMD` (`dist/modules/hello.elf`) for debugging: the loader prints where it put a module, and `gdb`'s `add-symbol-file hello.elf -o <load address − 0x400000>` gives symbols for it under QEMU's `-s`.

The packer is Rust (decided: [Decisions](#decisions)), a host-side tool in the `module/` Cargo workspace that shares `desk-module` with the loaders, so the format has one definition; once it is proven it may also take over `tools/elf2go32.py`'s job. `dmdpack pack --kind app` writes an application.

What the packer refuses, so the linker's output is validated rather than trusted (the analysis's section 6): not a 32-bit little-endian x86 `ET_EXEC`; program headers or section headers outside the file; a `PT_LOAD` whose file part lies outside the file, whose file size exceeds its memory size, that overlaps another or that is not in address order; a `.bss` that is not at the end of the last segment; a writable segment before a read-only one; a link base other than `0x0040_0000` unless `--link-base` names it; any relocation type other than `R_386_32`, `R_386_PC32` and `R_386_NONE`; an `R_386_32` against an absolute or undefined symbol; a relocation in an allocated section the image does not contain; an import symbol that is not a 4-byte object in `.desk.imp`, or whose name is not valid; a missing `app_start` for a module or `desk_main` for an application; and thread-local data (`.tdata`, `.tbss` or any `R_386_TLS_*` relocation), because version 1 has no thread-local segment ([`APP-MODEL.md`](APP-MODEL.md#thread-local-storage)).

## Where the code lives and its rules

| Path | Crate | What | Rules |
|---|---|---|---|
| `module/format/` | `desk-module` | The format: `Header`, `parse`, `Module`, `write` (used by the packer and the tests), `load_into`, the CRC, `LoadError` | `no_std` + `alloc`, `#![forbid(unsafe_code)]`, no float, no compare-and-swap, no external crates; host-tested; built for both 486 targets |
| `module/pack/` | `dmdpack` | The host tool: a strict ELF32 reader, `pack`, `info`, `cross` | `std`, `#![forbid(unsafe_code)]`, no external crates |
| `module/guest/` | `desk-guest` | The app side: `import!`, `desk_app!`, the panic handler that calls `desk_panic`, the allocator that forwards to `desk_alloc` | `no_std` + `alloc`; `unsafe` only to call through a slot, each with its `SAFETY:` line; the seed of R79's SDK crate, not the SDK |
| `module/mods/` | `hello`, `appmain`, `count`, `needs`, `alloc`, `panicky`, `faulty` | The test modules, one workspace with `module.ld` and the target by path | As `desk-guest` |
| `module/guard/` | `desk-module-guard` | The crash guard in assembly, shared by both tracks | `no_std`; `unsafe` with `SAFETY:` lines |
| `module/host/` | `desk-module-host` | The core's side, shared by both tracks: the service table, instances, the ledger and the scripted scenarios both transcripts come from | As `desk-module`; the calls into a module go through a `Platform` trait the tracks implement |
| `kernel/src/module.rs` | `desk-kernel` | Frames for images, the services, the `Platform` for the kernel, the `module` test modes | The kernel's |
| `module/dos/` | `desk-module-dos` | `DMDRUN.EXE`, the DOS host on `rt`: reads `.DMD` files, the `CS` limit, the `Platform` for DOS | As `shell/dos/` |

`module/` is a Cargo workspace like `hal/` and `shell/` (members `format`, `pack`, `guest`, `guard`, `host`; `mods` and `dos` have their own workspaces because their default target is not the host's). The no-external-crates rule needs no change: nothing here wants an ELF crate. AGENTS.md's repository layout gains `module/` when the first task creates it, not before.

## Containing a crash

The mechanism, carried over from S1 and written for Rust.

- **The guard.** `desk_guard_call(frame, entry, a0, a1, a2, result) -> status` saves `EBX`, `ESI`, `EDI`, `EBP`, `ESP`, `EFLAGS` and its return address in a `GuardFrame`, calls `entry` with three arguments, stores what it returns in `result` and returns "returned". `desk_guard_escape(frame, status) -> !` restores the saved registers and returns from `desk_guard_call` with that status ("crashed"). About twenty instructions in `global_asm!`; one active frame at a time, because the core never calls a module from inside a service.
- **A panic.** The module's panic handler (in `desk-guest`) formats `message (file:line)` into a 160-byte stack buffer and calls `desk_panic(ptr, len)`. The core's `desk_panic` copies the text into a static buffer and escapes. The instance is marked crashed, its ledger is freed, its image is freed, and the next module runs.
- **What may be abandoned.** Only the module's frames and the `desk_panic` service's own frame, which holds nothing when it escapes (no lock, no borrow, no value with a destructor). The rules the core keeps, answering the analysis's [section 4](RUST-DOS-INTEGRATION-ANALYSIS.md#4-panic-recovery-is-narrower-than-app-isolation): a service never calls back into a module; no borrow of the module table is held across a call into a module (the entry address is copied out first); `desk_panic` is the only service that escapes; the x87 is reset to the core's control word after an escape (`FNINIT`, then the control word), because a module can panic with values on the x87 stack.
- **A CPU fault, natively.** The kernel's exception handlers for `#DE`, `#UD` and `#GP` ask whether a guard is active and the faulting `EIP` lies in that module's image; if so they escape with "faulted" instead of stopping the machine. Ring 0 takes the exception on the same stack, so the escape simply abandons the exception frame. This is an optional task of the plan.
- **A CPU fault, hosted.** Not in the spike; it needs DPMI exception handlers in `rt` ([above](#the-hosted-track-dxe3-the-c-layer-and-freedos)).
- **Not recoverable in the spike, on both tracks:** a stack overflow, a wild write inside the shared address space, an infinite loop (the spike has no preemption and no watchdog), and anything that damages the core before it faults. These are S1's limits. The native track's staged path ([`APP-MODEL.md`](APP-MODEL.md#the-staged-path-on-the-native-track)) removes them in order: preemption (R77) stops a runaway loop, and paging with ring 3 (R78) keeps a wild write or a stack overflow inside its task. The hosted track stays cooperative and keeps them.

## Load addresses and randomisation

The platform chooses where an image goes; the format only says how to fix it up for that place. So randomised load addresses (the idea behind ASLR) are loader policy, not format, and the design is ready for them. The plan holds the property that makes them safe: the same module loaded at several different addresses must behave identically, which the M6 `places` scenario checks on both tracks and the M1 host tests check byte for byte. What randomising would need, and the spike does not do: **entropy**, which a 486 has no hardware source of, so the kernel would gather it from the jitter between the PIT and the RTC, from keyboard and mouse timing, and from disk completion times, mixed into a small pool; and **protection**, because in one flat ring-0 address space a module that can write anywhere gains nothing from not knowing where things are. Randomisation is worth turning on after paging and ring 3 (R78), and not before.

## Libraries, `std` and data files

- **Libraries.** The `library` kind and the export table are reserved in version 1 and nothing builds them; [`APP-MODEL.md`](APP-MODEL.md#shared-libraries-documented-not-built) explains the idea, why static linking is Rust's natural default, and what would trigger building it.
- **`std` for apps** is the later direction, through the patched platform layer of [`E-STD-STUBS.md`](E-STD-STUBS.md) in a sysroot of our own; the spike is `no_std`. `std`'s platform layer calls `desk_*` functions; in a `.DMD` or `.DKX` those are import slots like any other, declared through the guest crate's `import!` and bound by name at load, so `std` adds imports and nothing to the format. Whether an app starts at `std`'s `fn main` (through `lang_start`) or at `desk_main` is an open item from that experiment, carried in [`APP-MODEL.md`](APP-MODEL.md#questions-for-the-owner).
- **Data files.** The OS is files, as modern systems are: fonts, emoji and an app's own data stay plain files, and how an app packs its own data is its own business. No shared asset-pack container (such as Lieutenant Fizz 2's `.FAP`) is part of the OS.

## What the spike will and will not prove

**Will prove, in emulators and on the host:**

- The format, the packer and the loader exist, and every rejection path refuses with its own error (host tests, each with a crafted file).
- A module built once by `cargo` and `rust-lld` and packed by `dmdpack` runs from the same `HELLO.DMD` on the native kernel (QEMU as a 486, from a buffer embedded in the kernel) and on the hosted track (DOSBox Staging, from a file, under CWSDPMI), and prints the same line.
- The same module loaded at several different addresses behaves identically (the `places` scenario), which is what a randomising loader would rely on.
- The header's `kind` and the reserved export table are written and checked: a library, an export, an unknown kind and an application given to a module loader are each refused.
- Relocation is right: a module linked at a second base and packed is byte for byte what the loader makes of the first one loaded there (host check), and the emulator runs load at addresses other than the link base.
- Imports by name work, and a module that imports a service the core does not have is refused with that name on both tracks, with the core carrying on.
- Two instances of one file keep their own statics; 200 load and unload rounds leave the kernel's free frames and heap, and the DOS host's heap, where they were; a module's allocations are freed with it.
- A panic in a module is contained on both tracks, with the module's allocations reclaimed and another module working afterwards; optionally, a divide error and an invalid opcode in a module are contained natively.
- The two tracks' transcripts of the same scenario are equal, line for line, once addresses are masked.

**Will not prove:**

- Anything on real hardware or a real 486, or under a DPMI host other than CWSDPMI r7 (and FreeDOS in QEMU only if the opt-in run is done).
- Loading from a disk through a filesystem (R74, R75, R76): the native module comes from a buffer in the kernel, the hosted one from a DOS file.
- Preemption, threads, the wait and wake primitive, priorities or the window system as a task ([`APP-MODEL.md`](APP-MODEL.md)); they are R77 and R141 to R146, not R48, which stays small. Applications (`.DKX`) are written and refused, never run.
- Ring 3, system calls, tasks or scheduling (R77), per-task address spaces (R78), or any isolation: modules are trusted code in one address space.
- The SDK (R79): four services, chosen to prove the mechanism.
- CPU-fault containment on the hosted track; recovery from a stack overflow, a wild write or an infinite loop on either track.
- How long a load takes on a 486: emulator timings are not hardware timings ([`TOOLCHAIN.md`](TOOLCHAIN.md)).
- Apps that use `std`, apps in C, or modules that depend on each other.

## Risks

- **`--emit-relocs` fidelity.** The packer trusts that the emitted relocations describe the final image. That held in the experiment for static code; linker relaxations or a new relocation type after a toolchain bump would break it. The packer refuses any type it does not know, and the cross check (the same module linked at two bases) catches a wrong or missing relocation, so a toolchain change fails loudly. The fallback is to derive the relocations by diffing two links at different bases, a known technique that needs no relocation sections at all.
- **The hosted code segment.** If CWSDPMI refused to widen `CS`, or the reload misbehaved, the fallback is a module arena inside the host's `.bss`. The first hosted step tests this before anything else.
- **ABI drift between the two targets.** The module is built for the kernel's specification and runs in a DOS host built for `dos-elf.json`. If the two specifications ever differ in their C ABI (a `regparm`, a different stack alignment), the hosted run fails in confusing ways. The plan pins one module target for both tracks and the check runs the same file on each.
- **Abandoning frames.** The guard leaves frames behind; the rules above keep host frames out of them, but a service that breaks a rule (holds a borrow, calls back) turns a contained panic into corrupted host state. Each service is reviewed against the rules, and a test panics a module during an `app_event` to show the host's state intact.
- **Duplicated runtime.** Each module carries its own `core`, `alloc` and `compiler_builtins` (as each DXE3 did). A module that formats is a few kilobytes larger; not a problem at these sizes, and not solved here.
- **Two ABIs for `desk_alloc`.** The SDK says `desk_alloc(size)`; the `std` experiment says `desk_alloc(size, align)` ([section 5 of the analysis](RUST-DOS-INTEGRATION-ANALYSIS.md#5-the-two-desk_-interfaces-are-not-yet-one-abi)). The spike takes the layout-aware form (decision 5, [Decisions](#decisions)) and records it for R79.
- **Scope creep into the SDK.** The temptation is to build the app model here. The service table stays at four functions and the plan says so in every task.

## What changes in other documents

When the spike's result is in (the plan's last task):

- [`FORMATS.md`](FORMATS.md): one section for the container with the layout above as built, naming both extensions (`.DMD` modules, `.DKX` applications) and the `kind` values; `APP.INF`'s `module=` names a `.DMD`, and an application's entry names its `.DKX`.
- [`SDK.md`](SDK.md): imports are slots the loader binds by name, not `dlregsym`; the entry points lose the COFF underscore; `desk_alloc` and `desk_free` as built; the building section names `dmdpack`.
- [`SPEC.md`](SPEC.md): a Desk app is a `.DMD` module on both tracks; DXE3 becomes the recorded precedent.
- [`DIRECTION.md`](DIRECTION.md): "The module format may become our own" becomes what was decided; native milestone 6 is half done (the loader, not the disk).
- [`RUST-TARGETS.md`](RUST-TARGETS.md): a row for modules (space (b) and (d)), and the open question "Which crate type and loader suit a Desk app on the native OS?" answered.
- [`HAL.md`](HAL.md): the open question about the loader and the HAL answered as designed.
- [`ROADMAP.md`](ROADMAP.md): R48's outcome, and R76's and R79's notes.
- [`AGENTS.md`](../AGENTS.md) and [`CLAUDE.md`](../CLAUDE.md): `module/` in the repository layout and its rules; the sentence about `apps/` and `.DXE` modules.

## Decisions

Recorded by the owner on 2026-11-11 after reading the first version of this design. Each was a question here (decision 13 was answered after the others); the one-line reason is the owner's or the design's.

1. **Our own container, packed from the ELF.** It is the smallest format that relocates, names missing services and has room for Desk's fields; ELF PIE remains the fallback if the packer proves fragile.
2. **One format on both tracks, the same file byte for byte.** An app is built once.
3. **Relocatable, every file linked at `0x0040_0000`.** Nothing can promise a fixed address before paging, and a loader with per-task spaces can skip the relocations.
4. **Imports by name, into slots in the file's data.** A missing service is named at load, and the same names can later resolve to system-call stubs.
5. **`desk_alloc(size, align)` and `desk_free(ptr, size, align)`.** They fit Rust's allocator and let the ledger check what is freed; `SDK.md` changes with R79.
6. **`.DMD`, magic `DMD1`, for modules; one container, two extensions.** Applications get their own extension on the same container, and version 1 reserves `kind` and an export table so libraries can follow without a new major version.
7. **The packer is Rust**, a host-side Cargo workspace sharing the format crate with the loaders: one definition of the format.
8. **`std` for apps is the later direction**, through the patched platform layer in our own sysroot; the spike is `no_std`.
9. **The first native module comes from a buffer** in the kernel, not the boot floppy: the disk path belongs to R74 and R75.
10. **Native CPU-fault containment stays optional** in the spike: it does not block the result.
11. **No OS asset-pack container.** The OS is files; data packing is the apps' own business.
12. **Native is preemptive, hosted is cooperative; threads, wait and wake, configurable priorities, the window system as a task and the main-loop app shape** are decided in [`APP-MODEL.md`](APP-MODEL.md#decisions), outside R48's scope.
13. **Applications are `.DKX`** (a Desk executable), on the same container with kind 2; `.DMD` stays the module extension. It is 8.3, reads as an executable, and avoids `.DXE` (DJGPP's), `.DEX` (Android's Dalvik), `.DAP` (Desk's app package) and `.APP`. The magic is `DMD1` for every kind, so a renamed file is still recognised by its `kind`.

## Questions for the owner

None remain for the format: the application extension was the last, and it is decided (decision 13). The open items of the app model are in [`APP-MODEL.md`](APP-MODEL.md#questions-for-the-owner) and of the `std` target in [`DESK-STD.md`](DESK-STD.md#questions-for-the-owner).
