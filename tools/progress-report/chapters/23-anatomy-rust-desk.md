---
id: anatomydesk
n:
kicker: Anatomy
title: Anatomy of a Rust Desk application
date: 2026-10-23
summary: What stands under a Desk application today, and what does not exist yet: an honest diagram that grows as the app model is built, with a dated list of revisions.
prs: [19, 29, 30]
section: reference
---
The honest first sentence of this page is that **there is no Desk application yet**. There is no app model, no executable format a native loader could read, no system calls, no process or task, and no window that an app owns: the two windows of the first window are opened by a script that the shell runs. This chapter is the start of a page that will be revised as that changes. What it records today is what stands *under* where an application would sit, so that each later piece has somewhere to go.

<!-- figure anatomy-desk -->

The diagram is drawn from `tools/progress-report/anatomy-desk.json`, where every box has a status (built and verified, built and not verified, or planned) and evidence. A box is promoted by changing its status and evidence in that file; the generator refuses a built box with no evidence or a status that is not one of the three. The README of the tool says how to update the page when something lands.

**What exists.** The shared core, `shell/`, is six `no_std` crates that turn events into frames. Between the core and a platform is a narrow seam: the platform owns the loop, calls `Shell::handle` and `Shell::frame`, and presents the damage through the `Present` trait (`docs/FIRST-WINDOW.md`, "Where it sits"). Under the seam is the HAL (`docs/HAL.md`) and its drivers. Two platforms implement the loop: the native kernel's `shell` test mode in `kernel/src/shell.rs`, booted by `boot/stage1.asm` and `stage2.asm` (`docs/NATIVE-BOOT.md`) and run in QEMU, and the hosted `SHELL.EXE` on DOS ([the other anatomy page](chapter:anatomydos)). Both run the same scenario to the same pixels ([chapter 18](chapter:firstwindow)).

**What does not exist.** The kernel has no user mode: a search of `kernel/src` finds no task-state segment, no system-call gate and nothing that runs code at a lower privilege. The documents sketch the pieces (`docs/SDK.md`, `docs/DIRECTION.md`, `docs/S1-DXE3.md`), and the hosted track proved that a Rust module can be loaded with DJGPP's DXE3 format, with limits; but a native loader needs a format it can load without DJGPP's runtime, and that is a question, not a result. The owner has placed the executable-format spike after live input.

## Open questions

Each of these is on the wish list. The page will answer them as they are built, in the order the project takes them.

- **The app model.** What an app is: a module in the core's address space, as the hosted track does, or something with its own protection. `docs/DIRECTION.md` lists scheduling and app isolation among the open questions. Wish list.
- **The executable format and loader.** A format that both tracks could share, with symbol import, per-module state, panic containment and unload (`docs/DIRECTION.md`; lessons from `docs/S1-DXE3.md`). Planned after live input. Wish list.
- **System calls.** How an app calls the Desk on the native OS (`docs/RUST-TARGETS.md`, space (d)). The kernel has no gate for it today. Wish list.
- **Window ownership.** Who creates, owns and closes a window once apps exist: today the shell does, from a scenario. Wish list.

## The shelves

### Catalogued
- The layers drawn solid in the diagram are the ones that exist and were run: the six shell crates (161 host tests passed when run for this page), the HAL and its drivers, the native kernel's shell mode in QEMU ({{pr29.state}}, #29) and `SHELL.EXE` in DOSBox ({{pr30.state}}, #30), the boot sector and loader, and the kernel's services.
- The statement that the kernel has no user mode or system-call gate is a search of `kernel/src` for task-state, system-call and privilege-level code, which found none; it is not a claim about the documents.

### Uncatalogued
- Live keys and the mouse reaching the shell: the drivers are built and the first-window runs use scripted events.
- Real hardware of any kind; the emulators here are QEMU and DOSBox Staging.

### Wish list
- An app model, an executable format and loader, system calls and window ownership, as in the open questions above.
- Running DOS programs on the native OS by bundling FreeDOS (`docs/DIRECTION.md`).

## Revisions

**2026-10-23, the first entry.** Drawn from `main` at {{head}} after the first window landed (#31, merge commit {{pr31.merge}}). Everything under the shell is drawn as built; the app model, system calls, executable format and loader, and window ownership are drawn dashed as planned. Nothing was promoted that was not verified.

```figures
id: anatomy-desk
kind: diagram
track: native
title: Anatomy of a Rust Desk application
alt: A layered diagram of what stands under a Desk application today, from planned apps, system calls and loader at the top, through the built shell core, the seam, the HAL and the two platforms, to boot and the emulated machine, with a planned real PC; a text table of every box follows it.
caption: What exists is solid; what does not exist is dashed and labelled as planned.
at: figure anatomy-desk
thumb: svg
state: none
```
