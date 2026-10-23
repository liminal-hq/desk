---
id: m-frontier
n: M22
part: VIII
title: What is not built
summary: Every planned box said plainly: apps and the SDK, window ownership, system calls, a loader and executable format, processes and protection, storage, sound, FreeDOS and a real machine; what each would need from the layers below, and the decisions that are open.
date: 2026-10-23
reviewed: 44f0265
watches: [docs/DIRECTION.md, docs/SDK.md, docs/OPEN-QUESTIONS.md, tools/progress-report/anatomy-desk.json, kernel/src/main.rs, kernel/src/gdt.rs, kernel/src/idt.rs]
log: [decisions, path]
anatomy: [desk:appmodel, desk:sdk, desk:winown, desk:syscalls, desk:loader, desk:process, desk:freedos, desk:real]
---
## What it is

The rest of this manual describes what exists. This chapter describes what does not, because a manual that stopped at the edge of the code would leave the most important sentence unsaid: **there is no operating system yet**. What exists is a kernel that boots and checks itself, a hosted program on DOS, a set of shared drivers, and the first piece of a shared core that draws two windows, text and a cursor on both tracks. It has no apps, no system calls, no loader, no processes, no storage and no sound. Everything in this chapter is a plan, and the chapter's job is to say which parts of each plan are written down, which are open, and what the built layers would have to provide before the plan could start.

On the map ([M1](chapter:m-map)) these are the boxes drawn dashed: eight of them on [the anatomy page](chapter:anatomydesk), at the top, bottom and in the middle of the stack. Nothing in the state table below is built; every record is tagged `planned`, and each cites the document that plans it, so a reader can follow the claim to where it was made. The chapter adds four more records for things the documents sketch without a box of their own: paging, storage, sound and the V86 task that FreeDOS might need. It invents nothing: where a document leaves a decision open the chapter says it is open, and where the chapter explains what a thing would need it says what the built code offers, not what a design would choose.

<!-- figure anatomy-desk -->

## How it works

### What "not built" means here

A box is `planned` when no code implements it. A design document may describe it in detail (the SDK document specifies a C interface of more than sixty functions that no app calls), and a spike may have proved a piece of it on one track (a Rust module was loaded and unloaded on the hosted track, and the result is in [`S1-DXE3.md`](repo:docs/S1-DXE3.md)), but until code on the native track does the work the box stays dashed. The kernel today is one flat program: [`gdt.rs`](repo:kernel/src/gdt.rs) has the null descriptor and two ring 0 segments, [`idt.rs`](repo:kernel/src/idt.rs) holds the 32 exception gates and the gates of the hardware lines that drivers register, and [`main.rs`](repo:kernel/src/main.rs) runs the whole bring-up, the demonstrations and the shell scenario as a single thread of control that never leaves ring 0.

### Apps and the SDK

The documents sketch an app (the [app model](term:app-model) is the open question of what an app is and how it runs) as a module behind a C-compatible interface. `docs/SDK.md` is that interface: every function `extern "C"`, taking and returning only integers, pointers and lengths; handles are 32-bit with zero never valid; strings are UTF-8 with a length. An app exports three entry points, `app_start`, `app_event` and `app_stop`, and the core calls them. Its functions cover memory (`desk_alloc`, `desk_free`), logging, windows (`desk_window_new` and the rest), drawing, widgets, files, the clipboard, sound, settings, timers and more. [`HAL.md`](doc:docs/HAL.md#apps-and-modules) adds a rule that the SDK is the app's only door: an app never holds a port, never registers an interrupt handler and never sees a frame buffer; the core owns the screen and the palette, and the HAL is internal.

What is built: on the hosted track, the DXE3 spike proved the route with five functions (`desk_log`, `desk_alloc`, `desk_free`, a stub `desk_window_new` and `desk_panic`), a three-function lifecycle, two modules side by side and 200 load and unload cycles with memory unchanged. Every other function in the document is design. On the native track there is nothing: no module format, no table of exported functions, no call from the kernel into code it did not link. The box is tagged as an open design question in the anatomy data, and what an app *is* (a module in one shared address space, or a process) depends on the open decisions below.

To start, an app model would need from the layers below: memory from the heap ([M7](chapter:m-memory)), the event contract the two tracks share ([M11](chapter:m-input)), drawing through the core ([M13](chapter:m-pixels), [M14](chapter:m-text)), windows ([M15](chapter:m-shell)) and a loader. Of these, only the last is missing outright.

### Windows owned by apps

In the first window the scenario, which the shell runs, opens both windows: no app opens one. The window manager ([M15](chapter:m-shell)) keeps windows, a z-order and a focus, and decides the damage each change makes, but a window has no owner that would receive its events, close it when it crashes or count its memory. In the SDK's sketch the core unloads a crashed app, closes its windows and shows a notice; that needs the owner relation, which nothing records yet.

### System calls and a boundary

A [system call](term:system-call) is how a program running with less privilege asks the kernel for something. The kernel has no such boundary today. The 486 offers privilege levels, and the kernel uses only the most privileged ([M2](chapter:m-cpu)): there is no ring 3 descriptor in its table, no task-state segment (the structure the processor consults to find the kernel's stack on a change of level) and no gate that a less privileged program could use to enter the kernel. All code is one program in one address space, and a call from the shell into a driver is an ordinary function call.

> **From the stacks — Calls across a boundary.** According to the Wikipedia article on system calls, a system call is the programmatic way in which a program requests a service from the operating system, and such calls are often initiated through interrupts, which put the processor into an elevated privilege level and pass control to the kernel. For x86 the article lists a software interrupt, a call gate (which it calls an older mechanism, unpopular on x86) and the SYSENTER and SYSCALL instructions. It adds that on embedded systems system calls typically do not change the privilege mode of the processor. Which of these the Desk would use, or whether it would cross a boundary at all, is open.
> Source: [System call, Wikipedia](https://en.wikipedia.org/wiki/System_call)

The last sentence of that note is the point: whether there is a boundary at all depends on the isolation decision below. If apps are trusted modules in the core's address space, as on the hosted track, a "system call" is a function call through a table, and nothing in the processor changes.

### A loader and an executable format

A loader reads a file in some [executable format](term:executable-format) and puts the program in memory. The hosted track proved loading a Rust module with DJGPP's DXE3 format; its limits are in the spike's result: link-time optimisation is mandatory, a stack overflow in a module cannot be recovered, a page fault could not be tested under DOSBox and one file is one instance ([`S1-DXE3.md`](doc:docs/S1-DXE3.md#verdict-and-recommendation)). A native loader would need a format that it can load without DJGPP's runtime, and `docs/DIRECTION.md` and the anatomy data list that as an open question, planned after live input. The repository has two packers that show the shape of the problem without solving it: `tools/elf2go32.py` makes a DOS executable out of a Rust ELF, and `tools/elf2bin.py` flattens the kernel's ELF into the image ([M17](chapter:m-rt), [M19](chapter:m-toolchain)). Neither is a loader, and neither loads anything at run time.

### Tasks, scheduling, protection and paging

`docs/DIRECTION.md` lists these as open: **scheduling** (cooperative in one address space, as the hosted shell is, or preemptive processes), **memory protection** (flat and shared, or per-app address spaces on the 486's paging unit) and **app isolation** (what an app may touch, and whether a module is trusted code or a sandboxed process). The built layers decide none of it, but they constrain it. The kernel has no paging: the frame allocator hands out physical frames and the heap is a flat region ([M7](chapter:m-memory)). Its "lock" is turning interrupts off, which is sound on one processor and with one flow of control and would need revisiting if the kernel became preemptive; the HAL's rings are single-consumer for the same reason (`docs/HAL.md`, "Open questions"). A preemptive design would also need a timer-driven switch (the timer exists, [M6](chapter:m-interrupts)), a way to save and restore a task (a task-state segment or code that does it by hand) and an answer to what a crashed task leaves behind.

### Storage and a filesystem

Nothing on the native track reads a file. The loader reads the floppy by BIOS calls ([M4](chapter:m-loader)), and the kernel can call the BIOS from protected mode to read sectors in a test ([M8](chapter:m-bioscalls)), but there is no driver for the floppy controller or an IDE disk and no filesystem. `docs/HAL.md` sketches a `BlockDevice` trait (read and write 512-byte blocks by 32-bit LBA, blocking at first) and says the native FAT driver would sit on it and offer the same filesystem service that the hosted track gets from DOS. [`docs/DIRECTION.md`](doc:docs/DIRECTION.md#open-questions) leaves the filesystem (FAT12 and FAT16 for floppies and for sharing with FreeDOS, and perhaps one of our own for the system volume), the boot media and the choice between BIOS calls and own drivers for early disk access as open.

### Sound

Sound is specified on paper twice, in the SDK (`desk_opl_write`, sampled playback) and in the HAL, which sketches `Opl` and `PcmOut` traits over the OPL chip, the Sound Blaster's DSP and the DMA controller, with the platform's only parts being the DMA buffer, the interrupt and the configuration. None of it is built. Audio also cannot be checked headless (the DOSBox runs of the checks have sound off), so the first test of a sound driver would be a test of its register sequence against the mock, and nothing would show that a speaker made a noise. Detecting a card without `BLASTER` and without DOS is an open question in `docs/DIRECTION.md`.

### FreeDOS, and a real machine

DOS programs on the native OS are planned by bundling FreeDOS, either booting into it or running it in a V86 task (the design is later) instead of reimplementing DOS. `docs/DIRECTION.md` says what that implies and what is open: how the Desk's state is saved and restored when a DOS program takes over, how the screen and sound are shared, what ships and under what licences (FreeDOS is mostly GPL, and the project's licence is Apache-2.0 or MIT), and that a V86 task needs a monitor that fakes the hardware the program expects, which `docs/SPEC.md` calls a much larger project. What the built layers offer is the trampoline of [M8](chapter:m-bioscalls), which drops to real mode for one BIOS call and is the cheaper predecessor of a V86 monitor, not a substitute for one.

The last box is a real 486 PC. The anatomy data says "planned, never tried", and the claim holds for the whole manual: nothing in the repository has run on a real machine, a real floppy or a real card ([M20](chapter:m-emulators), [M21](chapter:m-evidence)).

### The decisions that are open

These come from `docs/DIRECTION.md` ("Open questions"), where each is stated with its alternatives; the third column says what the built code already settles or constrains.

| Decision | Alternatives the documents name | What exists |
|---|---|---|
| Scheduling | Cooperative in one address space; preemptive processes | A single flow of control; the timer tick is built |
| Memory protection | Flat and shared; per-app address spaces on the 486's MMU | No paging; flat segments |
| App isolation | Trusted module; sandboxed process | Nothing |
| Filesystem | FAT12 and FAT16; perhaps a system volume of our own | None |
| Boot media | Floppy; IDE disk image; both; our loader or an existing one | A floppy loader of our own |
| Early disk access | BIOS calls in the loader; own drivers in the kernel | BIOS calls in the loader and, from the kernel, through a trampoline |
| Real-mode switching | Never; a trampoline; a V86 monitor | A trampoline, measured; V86 left as the later upgrade (`docs/N3-BIOS-PATH.md`) |
| Graphics driver strategy | VBE before the switch; V86 calls; native drivers per card | VBE before the switch and register modes |
| Sound detection | Probe for a Sound Blaster, OPL or MPU-401 without `BLASTER` | Nothing |
| Input | Own PS/2 drivers; whether USB is ever in scope (the document says not for the 486 target) | Own PS/2 drivers |
| Launching DOS programs | Boot into FreeDOS; a V86 task; both | Nothing |
| Licensing of FreeDOS components | What is bundled, and how | Nothing |

## Why it is this way

- **Planned boxes are drawn, not hidden.** The anatomy page draws the planned boxes dashed so that the picture of a layered system is not mistaken for a picture of a built one. A manual that omitted them would let the reader assume they exist.
- **The app interface was written before the app model.** The SDK document came from the design brainstorm, written without access to code, and `AGENTS.md` says its numbers and formats are targets, not measurements; when the code disagrees, the code wins and the document is corrected.
- **Open means open.** The documents record each open question with its alternatives and, where a spike answered one for now, the answer and the spike (the real-mode trampoline answered "does the kernel ever go back to real mode" with "for one call, by trampoline, V86 later"). This chapter keeps that: it does not pick.
- **The order is recorded, not argued here.** The anatomy data plans the native loader after live input, and `docs/DIRECTION.md` lists the milestones of each track in order; the log's [path](chapter:path) page tracks what has been done of them.

## The code

This chapter points at documents and data, because there is no code, and at the code the future work would sit on.

| Where | What it holds |
|---|---|
| [`docs/DIRECTION.md`](repo:docs/DIRECTION.md) | The two tracks, the FreeDOS decision, the open questions and the milestones |
| [`docs/SDK.md`](repo:docs/SDK.md) | The C interface that apps would use, mostly design |
| [`docs/S1-DXE3.md`](repo:docs/S1-DXE3.md) | The result of loading Rust modules on the hosted track |
| [`docs/HAL.md`](repo:docs/HAL.md) | The sketches for sound, storage and a platform as one value |
| [`docs/OPEN-QUESTIONS.md`](repo:docs/OPEN-QUESTIONS.md) | What is decided and what is still open |
| [`tools/progress-report/anatomy-desk.json`](repo:tools/progress-report/anatomy-desk.json) | The eight planned boxes, with their evidence; the status block below follows it, and a test checks that it does |
| [`kernel/src/gdt.rs`](repo:kernel/src/gdt.rs) | The kernel's table: no ring 3 descriptor and no task-state segment |
| [`kernel/src/idt.rs`](repo:kernel/src/idt.rs) | The only gates: exceptions and hardware lines |
| [`kernel/src/main.rs`](repo:kernel/src/main.rs) | The single flow of control, which never leaves ring 0 |
| [`kernel/src/memory.rs`](repo:kernel/src/memory.rs) | The frame allocator, with no paging above it |

## Worked example

**What crossing the boundary is, today and in the sketch.** Take the one call the hosted spike proved end to end, `desk_log(text, len)`. In the SDK document it is `void desk_log(const char *text, size_t len)`: integers and pointers only, one line to the core's log. On the hosted track the core registers the function in a table by name (`_desk_log`, with the underscore a COFF symbol carries) and the module's import is resolved against that table by `dlopen`; a module with an import not in the table is refused, naming the first missing symbol. A call is then an ordinary call instruction to an address in the same address space, and nothing in the processor changes: no privilege level, no stack switch, no copying of arguments. A crash in the module is turned into an unload by signal handlers the core installs, which is why the document says plainly that the unload on a crash "is not a sandbox".

A native system call across a privilege boundary would be a different animal, and the documents do not design one: it would need a ring 3 descriptor pair in the table, a task-state segment so that the processor can find the kernel's stack, a gate or a software-interrupt vector in the IDT that user code may invoke, and a rule for validating the pointers a caller passes (the SDK's integers, pointers and lengths would have to be checked against the caller's own address space). None of those four exists. This example is a sketch of what would have to be added, labelled as such: it is not a measurement and not a decision.

## State of this layer

Each record below is one planned box of the anatomy page with the same evidence the box cites, or one of the four things the documents sketch beyond it.

```status
id: frontier.appmodel
label: An app model: what an app is, and how it runs
state: planned
evidence: repo:docs/DIRECTION.md, repo:docs/SDK.md
note: An open design question. The documents sketch apps as modules behind a C-compatible SDK; nothing implements it on the native track.

id: frontier.sdk
label: The SDK ABI (desk_*): integers, pointers and lengths
state: planned
evidence: repo:docs/SDK.md
note: Specified in a document; five functions were proved on the hosted track and no app calls the rest.

id: frontier.winown
label: Windows owned by apps
state: planned
evidence: repo:docs/FIRST-WINDOW.md
note: The scenario, run by the shell, opens both windows today. No app owns a window.

id: frontier.syscalls
label: System calls
state: planned
evidence: repo:docs/DIRECTION.md
note: The kernel has no user mode, no task-state segment and no system-call gate.

id: frontier.loader
label: An executable format and a loader
state: planned
evidence: repo:docs/DIRECTION.md, repo:docs/S1-DXE3.md
note: DXE3 was proved on the hosted track only. A native loader needs a format it can load without DJGPP's runtime; planned after live input.

id: frontier.process
label: Tasks, scheduling and memory protection
state: planned
evidence: repo:docs/DIRECTION.md
note: Cooperative in one address space or preemptive processes is open, and so is paging.

id: frontier.freedos
label: FreeDOS for DOS programs
state: planned
evidence: repo:docs/DIRECTION.md
note: A later milestone: booting into it or running it in a V86 task.

id: frontier.real
label: A real 486 PC
state: planned
evidence: repo:docs/FIRST-WINDOW.md
note: No real hardware has been used.

id: frontier.paging
label: Paging and memory protection in the kernel
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#paging, doc:docs/DIRECTION.md#open-questions

id: frontier.storage
label: A block device driver and a filesystem on the native track
state: planned
evidence: doc:docs/HAL.md#storage-sketch-the-native-storage-milestone-builds-it, doc:docs/DIRECTION.md#open-questions

id: frontier.sound
label: Sound: the OPL, the Sound Blaster DSP and DMA
state: planned
evidence: doc:docs/HAL.md#sound-sketch-the-sound-milestone-builds-it, repo:docs/SDK.md

id: frontier.v86
label: A V86 monitor for DOS programs and calls that must not turn interrupts off
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#hal-feedback, repo:docs/DIRECTION.md
```

The chapter's one claim, that none of this is built, is the claim the whole archive rests on staying true. When a box moves, its anatomy entry, its record here and its chapter change together, in the same branch, and the test that compares them fails until they do.

## Try it yourself

The commands below look for the things this chapter says do not exist, in the code that is there, and list the planned boxes of the anatomy data. They need only a shell and Python 3. Output is from a run on 2026-10-23.

```try
needs: host: grep, python3
ran: 2026-10-23 on the host
$ grep -n "static GDT" kernel/src/gdt.rs
13:static GDT: [u64; 3] = [0, 0x00CF_9A00_0000_FFFF, 0x00CF_9200_0000_FFFF];
$ grep -rn -i "tss\|task-state\|ring 3" kernel/src/*.rs
kernel/src/idt.rs:157:    "invalid TSS",
$ grep -rn -i "syscall\|sysenter\|call gate" kernel/src/ | wc -l
0
$ python3 -I -c "import json; d=json.load(open('tools/progress-report/anatomy-desk.json')); print(' '.join(b['id'] for b in d['boxes'] if b['status']=='planned'))"
appmodel sdk winown syscalls loader process freedos real
```

The first command shows the whole table the kernel loads: the null descriptor and the two ring 0 segments. The second finds the only mention of a task-state segment or of ring 3 anywhere in the kernel, which is the name of exception 10, "invalid TSS", in the table of exception names. The third finds nothing that names a system-call instruction or a call gate. The fourth lists the eight planned boxes whose records are in the table above.

**Exercises.** (1) Using the open-decisions table, say which of the decisions would have to be made before a first app could draw a window, and which could wait. (2) The loader record says "planned after live input". Find in the log what live input is and say what it would give a loader. (3) Pick one planned record and write down, from the chapters of the layers below, the three things it would need that already exist and the one that does not.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: App model
def: The answer to what an app is on the native Desk and how it runs: a module in one shared address space, or a process of its own. Open; the documents sketch modules behind the SDK.

term: System call
def: How a program that runs with less privilege asks the kernel for a service. The Desk's kernel has none: all code runs at the most privileged level in one address space.

term: Executable format
def: The layout of a file that a loader can read to put a program in memory and start it. The hosted track proved DXE3; a native format is an open question.
```

```figures
id: m22-anatomy
reuse: anatomy-desk
at: figure anatomy-desk
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every planned box of `anatomy-desk.json` has a record in the status block with the box's evidence, and a test keeps the two in step. The `try` commands were run on the host. No disagreement was found between the documents and the code for this layer.
