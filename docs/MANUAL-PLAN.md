# The Manual: a plan

A plan for a second group of pages in the Desk Progress Report: a technical manual and textbook of what has been built, beside the running log. The log ([`tools/progress-report/chapters/`](../tools/progress-report/chapters/)) tells the story in the order it happened; the anatomy pages draw one picture each of how a program stands on the system; the manual explains each layer on its own terms, from the processor upward, and says how it works, why it is this way, where the code is and what has been verified.

> **Status.** A plan, written on 2026-10-23 against `main` at `44f0265`. Nothing in it is built. It records decisions with defaults (see [Questions for the owner](#questions-for-the-owner)); a builder may change a detail when the code shows a better one, and says so in the pull request and in this document in the same branch.

## Contents

- [What the manual is](#what-the-manual-is)
- [Two paths through the same material](#two-paths-through-the-same-material)
- [The chapters](#the-chapters)
- [The chapter template](#the-chapter-template)
- [How it plugs into the generator](#how-it-plugs-into-the-generator)
- [Diagrams and explainers](#diagrams-and-explainers)
- [The writing plan](#the-writing-plan)
- [Risks](#risks)
- [Out of scope](#out-of-scope)
- [Questions for the owner](#questions-for-the-owner)

## What the manual is

| | The log | The anatomy pages | The manual |
|---|---|---|---|
| Question it answers | What happened, when, and what it proved | How does one program stand on the system? | How does this layer work, and why? |
| Order | Chronological, one chapter per stretch of work | One diagram per kind of program | By layer, from the hardware upward |
| Unit | A pull request or a batch of them | A box in a diagram | A layer, a crate, a mechanism |
| Changes when | A new stretch of work is written up | A box is promoted | The code of its layer changes |
| Evidence | Pull requests, CI, transcripts, the three shelves | `status` and `evidence` per box | A state tag per mechanism, each linked to evidence |
| Voice | Gregory, the archivist, in the first person | Gregory | Plain textbook voice; Gregory edits, writes the preface and the "From the stacks" sidebars |

The manual does not replace the design documents in `docs/`. They stay the source of truth for design and for results; the manual teaches, summarises and points into them and into the code, and never restates a table it could link. When the manual and the code disagree, the code wins, as everywhere else in this repository ([`AGENTS.md`](../AGENTS.md)).

## Two paths through the same material

The same chapters are read two ways. Neither path has pages of its own; each is an order, held in one data file, plus a page that presents it.

- **The textbook path, "Learn it".** Bottom-up, prerequisites first: the map of the system, then the processor and the devices a 486 PC has, then the boot from power-on to `kmain`, the kernel, the HAL and its drivers, the shared core, the two platforms, and last the tools and the evidence method. A reader who knows nothing about x86 should be able to read it in order. This is the chapter numbering, and the pager follows it.
- **The manual path, "Find it".** Top-down and task-shaped, for a contributor who wants to change something. It starts from the anatomy pages and walks down from the shell to the machine, and it has a task index: "change the loader" leads to M4 and M5, "add a kernel test mode" to M5 and M21, "add a driver" to M9 and M10, "add a widget" to M14, "add a scenario and its goldens" to M15 and M21, "change a target" to M19, "add a glyph" to M18. Each task names the chapter anchors to read and the checks to run.

Every chapter is written to serve both: the first half teaches (what it is, how it works, why), the second half is reference (the code, the commands, the state). The landing page (`manual.html`) shows both paths side by side; each chapter ends with a "Where next" box that gives the next chapter on each path.

## The chapters

Twenty-two chapters in eight parts, and four appendices. The numbers are M-numbers so that they never collide with the log's chapter numbers. Ids start with `m-` because the log already uses `boot`, `hal`, `input`, `bios` and `pixels`. "Log" names the log chapters whose pull requests proved the layer; "Anatomy" names the boxes that the chapter explains (`desk:` for `anatomy-desk.json`, `dos:` for `anatomy-dos.json`). Sizes are words of prose, not counting tables and figures.

### Part I. Foundations

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M1 | `m-map` | The map | The two tracks, the four kinds of code (HAL, driver, platform, core), which directory is which layer, what is built and what is not, how to read state tags | `docs/DIRECTION.md`, `docs/HAL.md` ("The four kinds of code"), `docs/FIRST-WINDOW.md` ("Where it sits"), the two anatomy pages | `pivot`, `hal`, `firstwindow` | all layers of `desk:` | 1,500 |
| M2 | `m-cpu` | The processor: real mode, protected mode and the A20 gate | Segments and offsets, the 1 MiB limit and its wrap-around, the A20 gate and its three switches, descriptors and selectors, the GDT, CR0.PE and the far jump, privilege levels (and that the kernel uses only ring 0), interrupts and exceptions in both modes, the x87 and the CR0 bits that govern it, what a 486 lacks (CMOV, CPUID use, SSE) | `docs/NATIVE-BOOT.md` (boot flow, FPU, the kernel target), `boot/stage2.asm`, `kernel/src/gdt.rs`, `kernel/src/fpu.rs`, `docs/N3-BIOS-PATH.md` ("The switching sequence") | `boot`, `bios` | `desk:boot`, `desk:kernel` | 3,000 |
| M3 | `m-devices` | The devices of a 486 PC | The ISA bus and port I/O, the two 8259 PICs and IRQ lines, the 8254 PIT, the 8042 controller and PS/2 keyboard and mouse, the 16550 UART, VGA (text mode, mode 13h, planar mode 12h, the DAC), VBE 2.0 and the linear frame buffer, the floppy controller and DMA (as the BIOS sees them), the BIOS services the project calls (INT 10h, 13h, 15h, 1Ah, 33h) and what E820 is | `docs/HAL.md` ("The surfaces"), `docs/C-INPUT.md` ("The 8042 controller"), `docs/A-VGA-VBE.md`, `docs/D-NATIVE-VGA.md`, `docs/N3-BIOS-PATH.md` | `input`, `pixels`, `bios` | `desk:emu`, `dos:vga`, `dos:lfb`, `dos:kbd` | 3,000 |

### Part II. Booting

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M4 | `m-loader` | The boot sector and the loader | The 1.44 MB image and its layout, the 512-byte boot sector and its signature, CHS reads by track, the 64 KiB DMA boundary and the retries, stage 2's serial and screen output, the A20 test and its fallbacks, E820 then E801 then AH=88, the boot information block, the VBE request byte, the three-entry GDT and the switch, the copy to 1 MiB, the values patched in after the build | `boot/stage1.asm`, `boot/stage2.asm`, `boot/README.md`, `tools/native-image.sh`, `docs/NATIVE-BOOT.md` ("Boot flow", "Image layout"), `docs/D-NATIVE-VGA.md` ("The boot information block"), `docs/NATIVE-EMULATORS.md` (the loader bug) | `boot`, `pixels`, `emulators` | `desk:boot` | 3,000 |
| M5 | `m-boot` | The boot, end to end | Power-on to `boot complete`, one transcript line at a time: the BIOS, stage 1, stage 2, `_start` and `.bss`, `kmain` and its order (serial, screen, GDT, IDT, PIC, PIT, memory, heap, x87, BIOS path, input), the test-mode byte and every test mode, how a run ends through `isa-debug-exit` and what the exit statuses mean, the watchdog, and what each negative check makes the kernel print | `kernel/src/main.rs`, `kernel/src/exit.rs`, `tests/native/expected*.txt`, `tools/native-run.sh`, `tools/native-check.py`, `tools/progress-report/media/native-serial.txt`, `docs/NATIVE-BOOT.md` ("The checks", "Running it and the CPU model") | `boot`, `emulators` | `desk:boot`, `desk:kernel`, `desk:emu` | 2,500 |

### Part III. The native kernel

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M6 | `m-interrupts` | The kernel: tables, interrupts and the timer | The kernel's own GDT, the 256-gate IDT and its assembly stubs, the exception dump, remapping the PICs to `0x20` and `0x28`, masking, the IRQ registry in the native platform, end-of-interrupt and spurious IRQ 7 and 15, the PIT at 100 Hz (divisor 11,932), the bounded first-tick spin and why `hlt` comes after it, "interrupts off" as the lock on one processor | `kernel/src/gdt.rs`, `kernel/src/idt.rs`, `kernel/src/pic.rs`, `kernel/src/pit.rs`, `kernel/src/hal.rs`, `kernel/src/sync.rs`, `docs/NATIVE-BOOT.md` ("Interrupts and the timer"), `docs/HAL.md` ("Interrupts", "Concurrency") | `boot`, `hal` | `desk:kernel` | 2,500 |
| M7 | `m-memory` | The kernel: memory and the x87 | The memory map as the loader leaves it, the one-bit-per-frame bitmap over 256 MiB, rounding usable regions inward and others outward, what stays reserved, first-fit frames, the 4 MiB heap as an address-ordered free list with merging behind `#[global_allocator]`, the arithmetic of the usable-RAM line, the x87 set-up and its checks, and what is missing (paging, protection) | `kernel/src/memory.rs`, `kernel/src/heap.rs`, `kernel/src/fpu.rs`, `docs/NATIVE-BOOT.md` ("Memory", "The FPU"), `docs/N3-BIOS-PATH.md` ("Paging") | `boot`, `emulators` | `desk:kernel` | 2,500 |
| M8 | `m-bioscalls` | The kernel: calling the BIOS from protected mode | Why the kernel needs the BIOS at all, the options weighed (trampoline, V86, an emulator, the loader), the low-memory map, the switching sequence and its descriptors, putting the PICs back on the BIOS vectors and returning them, NMI, the BIOS's timer ticks, the 8042 byte hand-off, the `Bios` trait and `NativeBios`, VBE and the return to text on three QEMU cards, INT 13h, E820 and the RTC through it, the measured cost, what V86 would add | `kernel/bios16.asm`, `kernel/src/bios.rs`, `kernel/src/biostest.rs`, `hal/hal/src/bios.rs`, `tests/native/expected-bios.txt`, `docs/N3-BIOS-PATH.md` | `bios`, `emulators` | `desk:kernel` | 3,000 |

### Part IV. Hardware abstraction

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M9 | `m-hal` | The HAL | The contract crate `desk-hal`: port I/O and why the `unsafe` sits on holding the bus, memory-mapped regions, interrupts and the ring, time, video and input types, errors, `no_std` and features, the alternatives rejected, the mock platform and host tests, the migration steps done and not done | `hal/hal/src/*.rs`, `hal/platform/mock/src/*.rs`, `hal/README.md`, `docs/HAL.md` | `hal` | `desk:hal`, `dos:rthal` | 3,000 |
| M10 | `m-sysdrivers` | Drivers I: serial, interrupt controllers and the timer | `desk-serial` (the 16550), `desk-pic` (two 8259s), `desk-pit` (the 8254 and `PitClock`): their register sequences, their tests against the mock bus, and how the kernel and the DOS runtime each use them | `hal/drivers/serial`, `hal/drivers/pic`, `hal/drivers/pit`, `docs/HAL.md` | `hal` | `desk:drivers` | 2,000 |
| M11 | `m-input` | Drivers II: keyboard and mouse | The 8042 and its set-up sequences, scancode set 1 and the decoder, the PS/2 mouse packet, the event contract shared by both tracks, the interrupt hand-off through lock-free rings, the kernel's `input.rs`, the DOS side (a protected-mode IRQ 1 handler and INT 33h), the injection tests, and what real hardware may do differently | `hal/drivers/i8042`, `hal/drivers/kbd`, `hal/drivers/int33`, `hal/hal/src/input.rs`, `hal/hal/src/ring.rs`, `kernel/src/input.rs`, `docs/C-INPUT.md`, `docs/B-DOS-INPUT.md` | `input`, `oneevent` | `desk:drivers`, `desk:live`, `dos:kbd`, `dos:mouse` | 3,000 |
| M12 | `m-graphics` | Drivers III: VGA and VBE | `desk-vga`: registers, mode 13h and 12h by register programming, planes, the DAC and fades, text mode and the way back to it, the VBE layer over the `Bios` trait, the linear frame buffer and its mapping on each track, the software cursor, the kernel's `video.rs` checks and `gfx` tour, what QEMU hides compared with real cards | `hal/drivers/vga/src/*.rs`, `kernel/src/video.rs`, `kernel/src/gfx.rs`, `kernel/src/vga.rs`, `docs/A-VGA-VBE.md`, `docs/D-NATIVE-VGA.md`, `docs/S5-JSDOS.md` (VBE in the browser) | `pixels`, `browser`, `bios` | `desk:drivers`, `dos:vga`, `dos:lfb` | 3,000 |

### Part V. The shared core

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M13 | `m-pixels` | The core I: surfaces, damage and presenting | The 8-bit back buffer and the pixel model, `Rect`, `Surface`, `Sprite`, dirty rectangles and how damage merges, the `Present` trait and the compositor, frame digests, `desk-present`'s linear and planar presenters and the read-back for captures | `shell/gfx/src/*.rs`, `shell/present/src/*.rs`, `docs/FIRST-WINDOW.md` ("The pixel model", "desk-gfx", "desk-present") | `firstwindow` | `desk:gfx`, `desk:present`, `dos:gfx`, `dos:present` | 3,000 |
| M14 | `m-text` | The core II: text, themes and window chrome | `.DFN` reading in `desk-font`, glyph lookup and measuring, `desk-ui`'s roles, slots and palettes, text drawing, the window chrome and its stepped halo, icons, the first widgets | `shell/font/src/lib.rs`, `shell/ui/src/*.rs`, `docs/FIRST-WINDOW.md` ("desk-font", "desk-ui"), `docs/PALETTE.md`, `docs/HIG.md` | `firstwindow`, `faces` | `desk:ui`, `dos:ui` | 2,500 |
| M15 | `m-shell` | The core III: windows, events and scenarios | `desk-wm` (windows, z-order, focus, hit testing, dragging, the damage each change makes), `desk-shell` (`Shell::handle` and `Shell::frame`, the cursor, the frame loop), the scenario format and runner, and the seam the platforms call | `shell/wm/src/*.rs`, `shell/shell/src/*.rs`, `tests/shell/first-window.scn`, `docs/FIRST-WINDOW.md` ("desk-wm", "desk-shell", "The scenario format") | `firstwindow` | `desk:wm`, `desk:shell`, `desk:seam` | 3,000 |

### Part VI. The platforms

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M16 | `m-platforms` | Two platforms, one core | What a platform is, side by side: who owns the loop, how a display and input are opened, how damage reaches the screen, how a run ends; the native `shell` test mode and `SHELL.EXE` compared line for line; why there is no `Platform` trait yet | `kernel/src/shell.rs`, `kernel/src/hal.rs`, `shell/dos/src/main.rs`, `docs/FIRST-WINDOW.md` ("The platform wirings"), `docs/HAL.md` ("The platforms", "A platform as one value") | `firstwindow`, `oneevent` | `desk:native`, `desk:dos`, `desk:seam`, `dos:main` | 2,500 |
| M17 | `m-rt` | The hosted runtime and the build to SHELL.EXE | What a DOS extender and a DPMI host are, the runtime contract, `rt`'s `_start`, panic handler and clean-up list, the heap over DPMI 0501h, INT 21h and the 32 KiB transfer buffer, the IRQ stubs, `DosBios` and `DosDisplay`; then the build: `dos-elf.json`, `-Zbuild-std`, `rust-lld` and `link.ld`, `elf2go32.py`, the stub from `stubify -g`, CWSDPMI beside it; and, as legacy, the first hosted skeleton (`core/`, `c/`, `coff2go32.py`) and the DXE3 module result | `spikes/d0-rust-dos/rt/src/*.rs`, `spikes/d0-rust-dos/dos-elf.json`, `spikes/d0-rust-dos/link.ld`, `tools/elf2go32.py`, `shell/dos/build.sh`, `shell/dos/check.sh`, `docs/D0-RUST-DOS.md`, `docs/TOOLCHAIN.md`, `docs/S1-DXE3.md` | `wheels`, `module`, `purerust`, `firstwindow` | `dos:start`, `dos:heap`, `dos:dosw`, `dos:dpmi`, `dos:irq`, `dos:rthal`, `dos:target`, `dos:lld`, `dos:pack`, `dos:stubf`, `dos:exe`, `dos:cws` | 3,500 |

### Part VII. Making and checking

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M18 | `m-fonts` | Fonts and formats | The four bitmap faces and their grid, `.DFN` byte by byte, how `fonts/build.ts` makes `.DFN`, TTF and OTF from one source, the Windows 3.1-era TrueType constraints, the outline faces, the emoji set, its tiers and `.DEM`; the other formats in `docs/FORMATS.md` named and tagged planned | `fonts/build.ts`, `fonts/generated/*.dfn`, `fonts/generated/emoji/*.dem`, `docs/FONTS.md`, `docs/EMOJI.md`, `docs/FORMATS.md` (".DFN", ".DEM") | `faces`, `pictures` | `dos:assets` | 2,500 |
| M19 | `m-toolchain` | The toolchain and the custom targets | `rust-toolchain.toml` and why a pinned stable with `RUSTC_BOOTSTRAP=1`, the three target files compared field by field (`kernel/i486-desk-none.json`, `spikes/d0-rust-dos/dos-elf.json`, `core/dos.json`), `-Zbuild-std` and `compiler-builtins-mem`, `rust-lld`, `elf2bin.py` instead of `objcopy`, NASM, the Docker images for the tools that are not installed, `tools/setup.sh` and `fetch-tools.sh`, the `std` question | `rust-toolchain.toml`, `kernel/.cargo/config.toml`, `tools/elf2bin.py`, `tools/native-build.sh`, `tools/docker/`, `docs/RUST-TARGETS.md`, `docs/TOOLCHAIN.md`, `docs/E-STD-STUBS.md` | `scaffold`, `wheels`, `std` | `dos:target`, `dos:lld` | 2,500 |
| M20 | `m-emulators` | The machines we run on | QEMU with `-cpu 486` and what it does and does not reject, Bochs as a 486 and its BIOS images, 86Box's AMIBIOS board, DOSBox Staging headless and why it cannot boot the native image, js-dos in the browser, how each run is driven (serial files, the debug-exit port, the monitor socket, `AUTOTYPE`, Xvfb), and the standing caveat that no real machine has run anything | `tools/native-run*.sh`, `tools/native-test*.sh`, `tools/dosrun.sh`, `tools/86box/`, `docs/NATIVE-EMULATORS.md`, `docs/S5-JSDOS.md`, `docs/NATIVE-BOOT.md` | `emulators`, `browser`, `boot` | `desk:emu`, `dos:dosbox`, `desk:real` | 2,500 |
| M21 | `m-evidence` | Testing and the evidence method | The rule that nothing is promoted without evidence; host tests against the mock bus; expected transcripts and the pattern language of `native-check.py`; negative checks (each deliberate failure and what it must print); key injection; golden frames, digests and `frames.py`; comparing the two tracks; CI and `tools/check.sh`; the three shelves and the state tags; what "verified" never means (real hardware) | `tools/check.sh`, `tools/native-check.py`, `tools/native-inject.py`, `tools/frames.py`, `tools/shell-compare.sh`, `tools/shell-goldens.sh`, `tests/native/`, `tests/shell/`, `hal/platform/mock/` | `boot`, `hal`, `input`, `firstwindow`, `archive` | none | 3,000 |

### Part VIII. The frontier

| No. | Id | Title | Covers | Primary sources | Log | Anatomy | Size |
|---|---|---|---|---|---|---|---|
| M22 | `m-frontier` | What is not built | Every planned box, said plainly: the app model, the SDK ABI, system calls, a loader and executable format, processes, window ownership, paging and protection, storage and a filesystem, sound, FreeDOS; what each would need from the layers below, and the decisions that are open | `docs/DIRECTION.md` ("Open questions", "Milestones"), `docs/SDK.md`, `docs/S1-DXE3.md`, `docs/HAL.md` ("Sound", "Storage", "Apps and modules"), `docs/OPEN-QUESTIONS.md` | `decisions`, `path` | every `planned` box | 2,000 |

### Appendices

| Id | Title | What it holds |
|---|---|---|
| `m-glossary` | A. Glossary | Every term the chapters define (A20, BIOS data area, CHS, CR0, DAC, descriptor, DPMI, E820, EOI, frame, GDT, IDT, IRQ, LFB, mode 13h, PIC, PIT, planar, selector, stub, VBE, and so on), each with a one-line definition and a link to the chapter that teaches it. Generated from the chapters' `glossary` blocks |
| `m-maps` | B. Maps and numbers | Reference tables in one place: physical memory while booting and after, I/O ports the code touches, IRQ lines and interrupt vectors, the BIOS calls made and by whom, the exit statuses of every runner, the test modes. Each table cites the source file or document it was read from; the ones a script can read (exit statuses, test modes, vectors) are generated |
| `m-commands` | C. Commands | Every "Try it yourself" command in the manual, grouped by chapter, with what it needs (host, Docker, DJGPP) and what it prints. Generated from the chapters' `try` blocks |
| `m-sources` | D. Sources | Every source a sidebar cites and every document a chapter leans on. Generated from the chapters |

## The chapter template

Every chapter is a file in `tools/progress-report/manual/` with this shape. Sections may be short, and the primers (M2, M3) have more "how it works" and fewer code pointers, but none is skipped; the tests enforce the ones marked *(checked)*.

1. **Front matter** *(checked)*. `id`, `n` (the M-number), `part`, `title`, `summary`, `date` (first written), `reviewed` (the commit of `main` the chapter was last checked against), `watches` (the paths whose changes may make it stale), `log` (log chapter ids), `anatomy` (box references).
2. **What it is.** Two or three paragraphs: the layer's job, where it sits on the map (a link to M1 and to the anatomy box), and what a reader will be able to do after the chapter.
3. **How it works.** The mechanism, in order, with the hardware facts it needs (linked to M2 or M3 instead of repeated). Numbered steps where there is a sequence. A figure or explainer where it explains something the prose cannot.
4. **Why it is this way.** The decisions and the alternatives rejected, each with where it was decided (a document section or a pull request). Not opinions: what the documents and results say.
5. **The code.** A table of the files and the symbols that implement the layer, as `sym:` and `repo:` links *(checked: every path and symbol exists)*, with one line each on what it holds; then the design documents for depth. No line numbers, because they go stale.
6. **Worked example.** One real case followed through, using real values: the usable-RAM sum from the six E820 lines of the captured transcript (639 KiB plus 15,232 KiB is 15,871 KiB), the PIT divisor for 100 Hz (1,193,182 ÷ 100 rounded is 11,932), the three GDT descriptors decoded byte by byte, one glyph read out of `fonts/generated/ui.dfn`, one frame's damage rectangles from `tests/native/expected-shell-8.txt`. Never an invented value.
7. **State of this layer** *(checked)*. A table generated from the chapter's `status` block: each mechanism with its tag, built-verified, built-unverified or planned, and its evidence. "Verified" always means in an emulator or on the host, and the table says so. Inline mentions use the same tags through placeholders, so prose and table cannot disagree.
8. **From the stacks.** Zero to two sidebars of history that explain where the mechanism came from (why the A20 gate exists, why the PIC's default vectors collide with exceptions, why a boot sector is 512 bytes and ends in `55AA`, what VBE 2.0 added). Checkable facts only: each ends with a `Source:` line to a page the writer has read, uncertain details are left out or hedged, and a sidebar is never evidence *(checked: source line present, five sentences at most)*.
9. **Try it yourself** *(checked)*. A fenced `try` block of commands that exist in the repository, each followed by what it should print (taken from a real transcript or test output), and what it needs (host tools, Docker, DJGPP). Plus one or two exercises that ask the reader to predict and then check, for example "change the PIT divisor in a scratch branch, predict the tick count, run the boot test".
10. **Proved in the log.** Generated from `log`: the log chapters and their pull requests, with their merge state. The reverse link appears on the log pages without editing them.
11. **Where next.** Generated: the next chapter on the textbook path, the next on the manual path, and the tasks in the index that lead here.
12. **Glossary block.** A fenced `glossary` block of the terms the chapter defines (`term`, `def`), aggregated into appendix A.
13. **Figures block.** The existing `figures` block, for the gallery.
14. **Revisions.** Dated entries, newest last: what changed, against which commit, and which status rows moved.

## How it plugs into the generator

### Files

```
tools/progress-report/
├── manual/
│   ├── plan.json              the parts, every chapter id, title and M-number (written or not), the two path orders, the task index
│   ├── MNN-id.md              one file per chapter or appendix, the template above
│   └── explainers/NAME.py     one module per manual explainer, each with render(D, ctx) and its own validation; discovered, not registered by hand
├── manual.py                  loading, the status and glossary blocks, the link checks, the backlinks, staleness
└── tests/test_report.py       the new tests (below), and a manual chapter in tests/fixture/
```

Keeping a chapter's status, glossary terms, try commands and figures inside its own file, and each explainer in its own module, means two writers working in parallel never edit the same file.

### The group in the contents

- `load_chapters` reads `manual/` as well as `chapters/`, sets `section: manual` on every manual page, and refuses an id used in both. The log keeps its order, numbers and pager; manual pages get a pager in textbook order.
- The contents page gains a section "The manual" after "The chapters": a table by part with each chapter's M-number, title, summary, a summary of its state rows (for example "9 verified, 2 not verified, 1 planned") and a "may be out of date" pill when it applies. Chapters listed in `plan.json` and not yet written show as "coming" rows, as `pages/coming.json` does for the log.
- The site header gains "Manual", linking to `manual.html`: the two paths, the task index, the parts, and links to the appendices.
- Manual pages do not have the three shelves and are left out of the front page's shelf counts, so nothing is counted twice; their honesty lives in the state table.
- The gallery tiles of manual figures carry a "Manual" group, and a figure reused from the log (below) gets no second tile.

### Cross-links in both directions

- **Manual to log.** The `log` front-matter list renders "Proved in the log". The existing `chapter:ID` links work across both groups.
- **Log to manual.** Every log chapter named in some manual chapter's `log` list gets an "In the manual" aside after its evidence table, computed at build time. No log chapter file changes.
- **Manual and anatomy.** The `anatomy` list (`desk:kernel`, `dos:rt` and so on) renders "On the anatomy pages" with links to the boxes; the anatomy pages' text tables gain a "Manual" column computed from the same lists. Box ids are anchors on the anatomy pages.
- **Glossary.** A `term:ID` link scheme points to the glossary entry; the first use of a term in a chapter may link it.

### State that stays current

Each chapter's fenced `status` block holds records in the `figures` block's format:

```status
id: kernel.heap
label: The heap: a first-fit free list behind #[global_allocator]
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::heap, sym:kernel/src/heap.rs::Heap
```

- `state` is `built-verified`, `built-unverified` or `planned`, plus `external` for what is not ours (SeaBIOS, CWSDPMI, DOS). The words match the anatomy pages' `verified`, `unverified` and `planned`, and the manual displays them the same way.
- Evidence kinds: `pr:N`, `commit:HASH`, `repo:PATH`, `sym:PATH::NAME` (the name appears in the file), `test:PATH::NAME` (a test function or an expected-transcript line containing NAME), `doc:PATH#ANCHOR` (the heading exists).
- A `built-*` record must have evidence, and a `built-verified` record must have at least one kind that shows something ran (`pr` merged with CI passing, `test`). As the roadmap already does, the generator shows a `built-verified` record whose pull request is not merged with CI passing as built-unverified, with the reason. A `planned` record must cite where it is planned (`doc:` or `repo:docs/...`).
- Inline, `{{state.kernel.heap}}` renders the same tag as the table, so the prose cannot claim more than the data.

### Staleness

`reviewed` and `watches` make staleness visible without blocking work. At build time `manual.py` runs `git log --oneline REVIEWED..HEAD -- WATCHES` for each chapter; if anything changed, the chapter shows "May be out of date: N commits since it was reviewed touched `kernel/src/`" with the pull requests, and the contents table shows the pill. In a shallow clone, where the history is not there, it says it could not tell. `build.py --manual-report` prints the same list as text for the update pass.

### Tests

New tests in `tests/test_report.py`, each with a negative twin that must fail:

- Every manual chapter has the front matter above, a `status` block, at least one row in "The code", and a `reviewed` commit that is a hexadecimal hash (and an ancestor of `HEAD` when the history is available).
- Every `repo:` path exists in the tree and every `sym:` name appears in its file; a renamed function fails the test.
- Every status record has one of the four states, a `built-*` record has evidence, a `built-verified` one has run evidence, and every evidence item resolves.
- Every `log` id is a log chapter, every `anatomy` reference is a box, every `term:` link is defined, each term is defined once and used at least once.
- Every command in a `try` block starts with a script that exists in the repository or a known tool (`cargo`, `python3`, `docker`, `bun`).
- Every id in `plan.json` is unique, every manual file is in it, and both path orders list every written chapter exactly once.
- A fixture build with one manual chapter: the contents section, the header link, the manual pager, the backlink on a log page, the "Manual" column on an anatomy table, and `check_site` passes.
- The existing sidebar rules (a source line, five sentences at most) apply unchanged.

These run in `tools/check.sh` with the rest of the generator's tests, so a code pull request that renames a symbol or moves a file the manual points at fails CI until the pointer is fixed in the same branch. That is the house rule ("when code and a document disagree, the code wins and the document is corrected in the same branch") applied to the manual; the staleness banner, which is softer, covers changes in behaviour that a pointer cannot see.

### When a pull request lands

1. **In the pull request.** If CI fails on a manual pointer, the author fixes the pointer (a path or a name), nothing more. Prose is not required in a code pull request.
2. **After the merge, in the archive pass.** Run `build.py --manual-report`. For each chapter it lists: the commits and pull requests that touched its watched paths since `reviewed`, and status records whose evidence changed state (a pull request merged, a test added or removed).
3. **Update the chapter.** Change the prose that the change made wrong; promote or add status records with their evidence (never promote without it); add the new log chapter to `log`; add a Revisions entry; set `reviewed` to the merge commit. If the anatomy diagram changed, the box and the chapter change in the same branch.
4. **Check and publish.** `python3 -I tools/progress-report/tests/test_report.py`, `python3 tools/progress-report/build.py --check`, then the publish recipe in [`tools/progress-report/README.md`](../tools/progress-report/README.md#publishing).

The README of the tool gains a "Updating the manual" section with these steps, beside "Updating the anatomy pages".

## Diagrams and explainers

### Reused from the log

A manual chapter can draw any existing directive. Its `figures` record says `reuse: FIGURE-ID` instead of describing a new figure, so the gallery keeps one tile and the registry rules still see every drawing claimed.

| Explainer or figure (log chapter) | Driven by | Used in |
|---|---|---|
| `explainer boot` (`boot`) | The QEMU serial transcript and `tests/native/expected.txt` | M5, M4 |
| `figure qemu` (`boot`) | The captured screen | M5 |
| `explainer memchain` (`emulators`) | Captured E820 chains | M7, M4 |
| `explainer biscall` and `figure bios-cost` (`bios`) | The n3 round trip and its measurements | M8 |
| `explainer pit` (`hal`) | The driver's own test code | M10, M6 |
| `figure hal-layers` (`hal`) | The HAL design | M9 |
| `explainer input` (`input`) | A recorded transcript replayed through a replica of the decoder | M11 |
| `explainer pipelines` (`oneevent`) | Both tracks' input paths | M11, M16 |
| `explainer vga13`, `figure bootinfo`, the `shots` (`pixels`) | The register writes of mode 13h, the boot information layout, real screendumps | M12, M4 |
| `explainer vbeattrs` (`browser`) | VBE mode attributes captured in the browser | M12 |
| `explainer firstwindow`, `figure fw-digests` (`firstwindow`) | The sums, the damage lines and the result table | M13, M15, M21 |
| `explainer pipeline` (`purerust`) | The sizes in `docs/D0-RUST-DOS.md` | M17 |
| `explainer modules` (`module`) | The s1-dxe3 transcripts | M17 |
| `figure std-sizes` (`std`) | The measured sizes | M19 |
| `figure font-matrix`, `figure emoji-coverage` (`faces`, `pictures`) | The font tables and `.dem` headers | M18 |
| `figure arch` (`pivot`), `figure anatomy-desk`, `figure anatomy-dos` | `docs/DIRECTION.md` and the anatomy JSON | M1, M16, M22 |

### New, in order of value

Each is driven by files in the repository, refuses data that disagrees with the source it replays (as the hardware explainers already do), reads without script as an ordered list or a table, has keyboard controls and a live region when it steps, and turns its transitions off under `prefers-reduced-motion`. Inline SVG and the existing `site.js` stepper only.

| # | Explainer | Chapter | Driven by | Refuses |
|---|---|---|---|---|
| X1 | **Address translation and the A20 wrap.** Pick a `segment:offset` (default the loader's own test, `0000:0500` against `FFFF:0510`) and see the linear address with the gate open and closed; then the same for a protected-mode selector through its descriptor | M2 | The A20 test addresses read from `boot/stage2.asm` | A source whose test addresses no longer alias |
| X2 | **The GDT, decoded.** The three descriptors of the loader and the kernel's own, bit by bit: base, limit, granularity, D/B, the access byte | M2, M4, M6 | The descriptor bytes parsed from `boot/stage2.asm` and `kernel/src/gdt.rs` | Descriptors that do not decode to a flat 4 GiB code and data pair at `0x08` and `0x10` |
| X3 | **The floppy image and its reads.** The 2,880 sectors as a strip, stage 1, stage 2 and the kernel on it; step through the reads by track, with LBA to CHS and the 64 KiB boundary marked | M4 | The kernel's sector count from `snapshots/measured.json`, the geometry and layout constants from `boot/stage1.asm` and `tools/native-image.sh` | A layout whose regions overlap or a read that crosses a 64 KiB boundary |
| X4 | **Frames and the heap.** The six E820 entries of the captured transcript replayed through a replica of the bitmap's rounding (usable inward, reserved outward), then the 4 MiB heap carved out | M7 | `media/native-serial.txt` and the rounding rules of `kernel/src/memory.rs` | A total that does not equal the usable-RAM line of the transcript |
| X5 | **An interrupt, from line to handler.** IRQ 0 raised, the remapped vector `0x20`, the IDT stub, the registry, the handler, the end-of-interrupt; and an exception's path to the dump, using a real negative-check transcript | M6 | The remap constants in `kernel/src/pic.rs`, the vector names in `kernel/src/idt.rs`, `tests/native/expected-div0.txt` | A transcript that does not match its expected file |
| X6 | **The crate graph.** Every crate in `hal/` and `shell/` and their path dependencies, coloured by kind (HAL, driver, platform, core), each node linked to its chapter | M1, M9, M13 | The `Cargo.toml` files | A cycle, or a core crate that depends on a driver's register-level API or on `desk-present` |
| X7 | **A glyph out of a `.DFN`.** The header of `fonts/generated/ui.dfn` field by field, then one glyph's bitmap row by row | M14, M18 | The file itself, read with a Python replica of the parser in `shell/font/src/lib.rs` | A header that disagrees with `docs/FORMATS.md` or a glyph count that disagrees with the font tables |
| X8 | **Three targets compared.** The fields of the three target files side by side, with the differences marked and why | M19 | The JSON files | Nothing to refuse; the table is the files |
| X9 | **The negative checks.** Every deliberate failure (the `TestMode` values `DivideError`, `GeneralProtection`, `InvalidOpcode`, `Panic`, `Hang`, `NoTick`, `InputMasked` and `ShellCorrupt` in `kernel/src/main.rs`, and the deliberately wrong `tests/native/wrong-expected.txt`) with the test mode that causes it and the lines that must appear | M5, M21 | `tests/native/expected-*.txt` and `tools/native-test.sh` | An expected file with no test mode that runs it |

X1 to X5 serve the textbook path most and come first. A wish list, not planned: a v86 boot of the real image in the page, which would need a large dependency and a decision about hosting a disk image.

## The writing plan

Each task is one pull request into `main` (the archive's chapters land the same way today), written by one Sonnet agent from the sources it lists, and reviewed with the `code-review` skill before merge. At most two tasks run at once; a wave starts when everything it depends on is merged.

### Rules for every writing task

- Read the template above, the generator README, `AGENTS.md` (Canadian English, unwrapped markdown, real em dashes) and the sources listed. Read the code, not only the documents: every claim in "How it works" is checked against the file it names, and a disagreement between a document and the code is reported in the pull request, not papered over.
- Use real values from the repository for every example. Run the cheap commands (`cargo test` in `hal/` and `shell/`, the generator's tests) and put their real output in the try blocks; for commands that need QEMU or DOSBox, use Docker when the tool is not installed (`tools/native-run.sh` falls back to it), never install host packages, and say in the pull request which commands were run and which were copied from a committed transcript.
- Sidebars: fetch the source before citing it; no quotations, dates or names that the source does not state.
- Promote nothing without evidence. When unsure, `built-unverified`.
- Acceptance for every task: `python3 -I tools/progress-report/tests/test_report.py`, `python3 -I tools/check-docs.py`, `python3 tools/progress-report/gather.py --offline --out dist/progress-report-data.json` then `python3 tools/progress-report/build.py --data dist/progress-report-data.json --check`, and a look at each new page served locally at 400 and 1280 px, light and dark.

### Order and parallelism

| Wave | Tasks, in parallel | Depends on |
|---|---|---|
| 1 | W1 | — |
| 2 | W2, W3 | W1 |
| 3 | W4, W5 | W2, W3 |
| 4 | W6, W7 | W5 |
| 5 | W8, W9 | W7 |
| 6 | W10, W11 | W9 |
| 7 | W12, W13 | W10, W11 |
| 8 | X-a, X-b | the chapters they draw into |
| 9 | X-c, W14 | all chapters |

### W1. The generator: the Manual group, its data and its tests

- **Files.** `tools/progress-report/manual.py` (new), `build.py`, `mdlite.py` (the `sym:` and `term:` schemes, the `{{state.ID}}` placeholders, the `status`, `glossary` and `try` blocks), `figures.py` (`reuse`), `templates/site.css` and `site.js` (the state tags, the "Where next" box, the pill), `manual/plan.json` (all 22 chapters and four appendices, both orders, the task index), `manual/explainers/__init__.py` (discovery), `tests/test_report.py` and `tests/fixture/` (a manual chapter), `README.md` ("Updating the manual"), and the "How this archive is made" chapter (one paragraph on the manual).
- **Builds.** Everything in [How it plugs into the generator](#how-it-plugs-into-the-generator): loading, the contents section with coming rows, the header link, `manual.html`, the glossary, maps, commands and sources pages as generated shells (empty until chapters exist), the manual pager, the backlinks on log and anatomy pages, the state tables and downgrades, staleness and `--manual-report`.
- **Acceptance.** All the tests listed under [Tests](#tests) pass, and each negative twin fails for the stated reason; the real site builds with `--check` and shows 26 "coming" rows; no log page changes except the computed "In the manual" aside (none yet, as no chapter is written); `tools/check.sh`'s generator step passes.
- **Read.** `tools/progress-report/README.md`, `build.py`, `mdlite.py`, `figures.py`, `explainers_fw.py` (the anatomy validation to copy), `tests/test_report.py`.

### W2. M1 The map, and M2 The processor

- **Files.** `manual/M01-map.md`, `manual/M02-cpu.md`.
- **Read.** `docs/DIRECTION.md`, `docs/HAL.md` ("Decisions", "The four kinds of code"), `docs/FIRST-WINDOW.md` ("Where it sits"), both anatomy chapters and their JSON; `docs/NATIVE-BOOT.md`, `boot/stage2.asm`, `kernel/src/gdt.rs`, `kernel/src/fpu.rs`, `docs/N3-BIOS-PATH.md` ("The switching sequence"); for sidebars, a primary or encyclopaedic source on the 8086 wrap-around and the AT's A20 gate.
- **Acceptance.** The common checks; M1's status block covers every layer of `anatomy-desk.json` with the same state; M2's worked example uses the loader's own A20 test addresses; each glossary term M2 introduces (segment, offset, linear address, A20, descriptor, selector, GDT, CR0, protected mode, real mode, ring) is in its glossary block.

### W3. M4 The boot sector and loader, and M5 The boot end to end

- **Files.** `manual/M04-loader.md`, `manual/M05-boot.md`.
- **Read.** `boot/stage1.asm`, `boot/stage2.asm`, `boot/README.md`, `tools/native-image.sh`, `tools/native-build.sh`, `tools/native-run.sh`, `tools/native-check.py`, `kernel/src/main.rs`, `kernel/src/exit.rs`, `tests/native/expected*.txt`, `tools/progress-report/media/native-serial.txt`, `snapshots/measured.json`, `docs/NATIVE-BOOT.md`, `docs/D-NATIVE-VGA.md` ("The loader", "The boot information block"), `docs/NATIVE-EMULATORS.md` ("What the emulators exposed"), and the log chapters `boot` and `emulators`.
- **Acceptance.** The common checks; M5 walks every line of the captured transcript (it may group them) and lists every test mode in `kernel/src/main.rs`, with the expected file that checks it; the exit statuses match `tools/native-run.sh`; both chapters reuse the `boot` explainer once between them, not twice. This pair is the exemplar for the rest: the owner's review of it settles the template before wave 3.

### W4. M3 The devices of a 486 PC

- **Files.** `manual/M03-devices.md`.
- **Read.** `docs/HAL.md` ("The surfaces"), `docs/C-INPUT.md` ("The 8042 controller", "The mouse packet"), `docs/A-VGA-VBE.md`, `docs/D-NATIVE-VGA.md`, `docs/N3-BIOS-PATH.md` ("Interrupts, the PICs, the timer and the keyboard"), `hal/drivers/*/src/lib.rs` for the register names actually used.
- **Acceptance.** The common checks; every port and IRQ number named is one the code uses (a test of the appendix B table comes later, in W14); the chapter introduces devices and links forward to M10 to M12 for how the drivers drive them, without duplicating them.

### W5. M6 Tables, interrupts and the timer

- **Files.** `manual/M06-interrupts.md`.
- **Read.** `kernel/src/gdt.rs`, `idt.rs`, `pic.rs`, `pit.rs`, `hal.rs`, `sync.rs`, `main.rs`; `hal/hal/src/irq.rs`; `docs/NATIVE-BOOT.md` ("Interrupts and the timer"), `docs/HAL.md` ("Interrupts", "Concurrency", "The unsafe policy"); `tests/native/expected-div0.txt`, `expected-gp.txt`, `expected-ud.txt`, `expected-notick.txt`.
- **Acceptance.** The common checks; the worked example derives the PIT divisor and the tick count the transcript prints; each exception the negative checks raise is named with the expected file that proves it.

### W6. M7 Memory and the x87, and M8 Calling the BIOS

- **Files.** `manual/M07-memory.md`, `manual/M08-bioscalls.md`.
- **Read.** `kernel/src/memory.rs`, `heap.rs`, `fpu.rs`, `bios.rs`, `biostest.rs`, `kernel/bios16.asm`, `hal/hal/src/bios.rs`, `tests/native/expected-bios.txt`, `bios.inject`, `bios.sums`; `docs/NATIVE-BOOT.md` ("Memory", "The FPU", "The BIOS path"), `docs/N3-BIOS-PATH.md` (all of it), the log chapters `emulators` and `bios`.
- **Acceptance.** The common checks; M7's usable-RAM arithmetic matches the transcript; M8 states the measured cost exactly as `docs/N3-BIOS-PATH.md` records it, with its conditions.

### W7. M9 The HAL, and M10 Drivers I

- **Files.** `manual/M09-hal.md`, `manual/M10-sysdrivers.md`.
- **Read.** `hal/README.md`, `hal/hal/src/*.rs`, `hal/platform/mock/src/*.rs`, `hal/drivers/serial`, `pic`, `pit`; `docs/HAL.md` (all of it); the log chapter `hal`.
- **Acceptance.** The common checks; the migration steps are tagged as the code shows them now (step 1 done, step 2 not), not as `docs/HAL.md` first planned them; the try block runs `cargo test` in `hal/` and quotes its real summary line.

### W8. M11 Keyboard and mouse, and M12 VGA and VBE

- **Files.** `manual/M11-input.md`, `manual/M12-graphics.md`.
- **Read.** `hal/drivers/i8042`, `kbd`, `int33`, `vga`; `hal/hal/src/input.rs`, `ring.rs`, `video.rs`; `kernel/src/input.rs`, `video.rs`, `gfx.rs`, `vga.rs`; `spikes/d0-rust-dos/rt/src/irq.rs`, `vga.rs`, `hal.rs`; `docs/C-INPUT.md`, `docs/B-DOS-INPUT.md`, `docs/A-VGA-VBE.md`, `docs/D-NATIVE-VGA.md`, `docs/S5-JSDOS.md`.
- **Acceptance.** The common checks; the mouse path on DOS is tagged as the anatomy page tags it (not verified in the first-window run); every "QEMU hides" statement cites the document that records it.

### W9. M13 Surfaces and damage, and M14 Text and chrome

- **Files.** `manual/M13-pixels.md`, `manual/M14-text.md`.
- **Read.** `shell/gfx/src/*.rs`, `shell/present/src/*.rs`, `shell/font/src/lib.rs`, `shell/ui/src/*.rs`, their tests; `docs/FIRST-WINDOW.md` (the sections on each crate, "Goldens and frame digests"), `docs/PALETTE.md`, `docs/HIG.md`; the log chapter `firstwindow`.
- **Acceptance.** The common checks; M13's worked example takes one frame's damage lines from `tests/native/expected-shell-8.txt` and shows how they were merged; the try block runs `cargo test` in `shell/` and quotes the real summary.

### W10. M15 Windows and the shell, and M16 Two platforms

- **Files.** `manual/M15-shell.md`, `manual/M16-platforms.md`.
- **Read.** `shell/wm/src/*.rs`, `shell/shell/src/*.rs`, `tests/shell/first-window.scn`, `tests/shell/README.md`, `kernel/src/shell.rs`, `kernel/src/hal.rs`, `shell/dos/src/main.rs`; `docs/FIRST-WINDOW.md` ("desk-wm", "desk-shell", "The scenario format", "The platform wirings"), `docs/HAL.md` ("The platforms").
- **Acceptance.** The common checks; M16 has a side-by-side table of the two wirings in which every row cites a symbol on each side; nothing claims a `Platform` trait exists.

### W11. M17 The hosted runtime, and M19 The toolchain

- **Files.** `manual/M17-rt.md`, `manual/M19-toolchain.md`.
- **Read.** `spikes/d0-rust-dos/` (the `rt` sources, `dos-elf.json`, `link.ld`), `tools/elf2go32.py`, `tools/elf2bin.py`, `tools/coff2go32.py`, `shell/dos/build.sh`, `check.sh`, `core/dos.json`, `kernel/i486-desk-none.json`, `kernel/.cargo/config.toml`, `rust-toolchain.toml`, `tools/setup.sh`, `tools/fetch-tools.sh`, `tools/docker/`; `docs/D0-RUST-DOS.md`, `docs/TOOLCHAIN.md`, `docs/RUST-TARGETS.md`, `docs/E-STD-STUBS.md`, `docs/S1-DXE3.md`; the log chapters `wheels`, `module`, `purerust`, `std`.
- **Acceptance.** The common checks; the legacy hosted skeleton is one clearly marked section, not the main story; every size quoted is the one a document records, with the document named.

### W12. M18 Fonts and formats, and M20 The machines we run on

- **Files.** `manual/M18-fonts.md`, `manual/M20-emulators.md`.
- **Read.** `fonts/README.md`, `fonts/build.ts`, `docs/FONTS.md`, `docs/EMOJI.md`, `docs/FORMATS.md`; `tools/native-run*.sh`, `tools/native-test*.sh`, `tools/dosrun.sh`, `tools/86box/`, `docs/NATIVE-EMULATORS.md`, `docs/S5-JSDOS.md`; the log chapters `faces`, `pictures`, `emulators`, `browser`.
- **Acceptance.** The common checks; formats in `docs/FORMATS.md` that nothing reads or writes yet are tagged planned; the emulator table's results are copied from `docs/NATIVE-EMULATORS.md` with that citation.

### W13. M21 Testing and evidence, and M22 What is not built

- **Files.** `manual/M21-evidence.md`, `manual/M22-frontier.md`.
- **Read.** `tools/check.sh`, `tools/native-check.py`, `tools/native-inject.py`, `tools/frames.py`, `tools/shell-compare.sh`, `tools/shell-goldens.sh`, `tests/`, `.github/workflows/`; the log chapter `archive`; `docs/DIRECTION.md`, `docs/SDK.md`, `docs/OPEN-QUESTIONS.md`, `anatomy-desk.json`.
- **Acceptance.** The common checks; M22's status block has one `planned` record for every planned box in `anatomy-desk.json`, with the same evidence, and a test asserts that they stay in step.

### X-a. Explainers X1, X2 and X3

- **Files.** `manual/explainers/address.py`, `gdt.py`, `image.py`, their tests, and the directives and `figures` records in M2, M4 and M6.
- **Acceptance.** Each refuses the bad input named in its row; each reads without script; keyboard and reduced-motion checks pass by hand at 400 and 1280 px.

### X-b. Explainers X4, X5 and X9

- **Files.** `manual/explainers/frames.py`, `irq.py`, `negatives.py`, tests, directives in M5, M6, M7 and M21.
- **Acceptance.** As X-a.

### X-c. Explainers X6, X7 and X8

- **Files.** `manual/explainers/crates.py`, `dfn.py`, `targets.py`, tests, directives in M1, M9, M13, M14, M18 and M19.
- **Acceptance.** As X-a; X7's parser agrees with `desk-font` on every glyph count of the four faces.

### W14. The appendices and the editorial pass

- **Files.** `manual/A1-glossary.md` to `A4-sources.md` (short framing prose; the bodies are generated), the generated tables of appendix B and the test that each table's numbers appear in the source it cites, and fixes across chapters for consistency of terms and tags.
- **Acceptance.** Every glossary term is used; appendix B's exit statuses, test modes and vectors are generated, not typed; every chapter's `reviewed` is the same recent commit of `main`; the whole site builds with `--check`; publishing (the owner's sign-in) follows the recipe.

## Risks

- **Staleness.** A manual describes code, and code moves. Mitigations: pointers to paths and symbols that CI checks, never line numbers; `reviewed` and `watches` with a visible banner; the state table generated from evidence; the update steps after every merge. What remains: a behaviour change behind an unchanged name, which only the banner and the archive pass catch.
- **CI coupling.** A code pull request that renames a symbol now fails until the manual pointer is fixed. The fix is one line, and it is the repository's rule already; if it proves a nuisance, the pointer check can become a warning (see the questions).
- **Technical accuracy.** Writers summarising documents can be wrong in small ways that read convincingly. Mitigations: every mechanism names the file that implements it, writers read the code, the worked examples use values that tests or transcripts contain, and the review of each pull request checks claims against code. The exemplar wave (W3) is reviewed by the owner before the rest are written.
- **Accuracy of history.** Sidebars are where invented detail creeps in. The rules: the writer fetches and reads the source, cites it, states nothing the source does not, hedges or drops what is uncertain, and keeps sidebars apart from evidence. The tests check form, not truth; the review checks truth.
- **Duplication with `docs/`.** The manual could become a second copy of the design documents that drifts from them. It teaches and links; it does not copy tables, and where it summarises a result it names the document section.
- **Size.** About 26 pages and 60,000 words, plus new figures. The publish limits are 255 files and 64 MB per publish; the report has about 50 media files and 25 pages now, so the manual fits in one publish but the margin narrows. The README already allows a larger set to go up over several publishes to the same address. Each page stays under 16 MB easily.
- **Writer consistency.** Twelve writing tasks by different agents could produce twelve styles. The template, the exemplar pair and the editorial pass hold it together.
- **Voice.** A textbook in the archivist's first person would read oddly; a manual with none of his care would lose what makes the archive trustworthy. The default splits them (see the questions).

## Out of scope

- Claims about real hardware. Nothing has run on a real PC, and the manual says so wherever it matters.
- Rewriting or restructuring `docs/`; changing log chapters by hand (their manual asides are computed).
- API reference generated from the code (`rustdoc`); the manual links to sources instead.
- A manual for apps or the SDK, which do not exist; M22 says what is planned.
- A separate site, search, print or PDF output, translations.
- Running the "Try it yourself" commands in CI; the tests check that they name real scripts, not that they still print the quoted output.
- Building a browser emulator (v86) into the pages.

## Questions for the owner

Each has a default that the plan follows unless the owner says otherwise.

1. **Where the manual lives.** Default: in the same artifact, as a second group beside the log, with its own landing page. The alternative is a separate artifact, which would lose the shared gallery and the two-way links.
2. **Numbering and ids.** Default: M-numbers (M1 to M22) and `m-` ids, so they never collide with the log. The alternative is parts and chapters ("II.4").
3. **Voice.** Default: the chapters in a plain textbook voice, with Gregory as editor writing the preface on `manual.html` and the "From the stacks" sidebars in his voice. The alternative is Gregory's first person throughout.
4. **Broken pointers in code pull requests.** Default: CI fails, so the pointer is fixed in the same branch. The alternative is a warning, with the archive pass fixing it later.
5. **The pager.** Default: textbook order, with the manual path offered in each chapter's "Where next" box. The alternative is two pagers.
6. **The legacy hosted skeleton.** Default: one marked section in M17 (`core/`, `c/`, `coff2go32.py`, DXE3), not a chapter of its own, since the owner's decision on retiring the C layer is open.
7. **Planned formats.** Default: M18 names the formats in `docs/FORMATS.md` that are not built and tags them planned, in a short table, without teaching them.
8. **Order of writing.** Default: the boot pair (W3) is the exemplar and is reviewed before wave 3. The alternative is the kernel chapters first, if the owner wants the "kernel" chapter he asked for soonest.
