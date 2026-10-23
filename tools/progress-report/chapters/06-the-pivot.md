---
id: pivot
n: 6
title: The pivot
date: 2026-10-22
summary: The owner decided that the Desk is an operating system and not a shell, and by 17:31 the documents said so.
prs: [5]
---
This is the chapter where the project changed what it was. Until that afternoon the documents described the Desk as a graphical shell for MS-DOS. At 17:31:59 on 22 October the commit [`e84aed1`](commit:e84aed1) added `docs/DIRECTION.md`, and its message records "the 2026-10-22 decision that Desk is an operating system in Rust for old hardware, built as a hosted track on DOS and a native track in parallel". PR #5 states whose decision it was: it "records the owner's decision that it is an OS, with the shell as its product layer". I have no transcript of the conversation, so I will not quote it; the commits and the pull request are the record.

What the decision changed is written down in four lines of `docs/OPEN-QUESTIONS.md`. The Desk is **an operating system, written in Rust, for old hardware** (486-class PCs with VGA or VBE graphics, Sound Blaster and OPL sound, PS/2 input, floppy and IDE storage). It is built **both ways in parallel**: a *hosted* track that runs on DOS, which gives fast iteration, and a *native* track, a bootable Rust kernel tested in QEMU, 86Box and Bochs. The two share an OS-independent core behind a small hardware abstraction layer (HAL), and the core must never call DOS or DJGPP directly. DOS programs on the native OS are to run by **bundling FreeDOS** instead of reimplementing DOS. And since DOSBox and js-dos emulate DOS, not PC hardware, they can only ever test the hosted track.

The diagram below is the architecture that follows from it, with the solid boxes being work that has since merged and the dashed boxes being plans.

<!-- figure arch -->

Two companions arrived with it. `docs/RUST-TARGETS.md` tracks how Rust is built for each application space, and the new spike **d0** ("a Rust-only DOS program with its own `_start`, DPMI wrappers and INT 21h output") was written into `docs/DIRECTION.md` as planned. You will meet its result in chapter 8. PR #5 touched the README, `AGENTS.md`, `CLAUDE.md`, the SPEC and the banner too, and warned that it would conflict with #1, #3 and #4. It did, and chapter 9 tells how those conflicts were resolved.

## The shelves

### Catalogued
- The decision and its consequences are recorded in `docs/DIRECTION.md`, `docs/RUST-TARGETS.md` and `docs/OPEN-QUESTIONS.md`, merged as PR #5 at `{{pr5.merge}}`, with CI green. That catalogues the *record*, not the claims inside it.
- The banner text was rendered in headless Chromium and checked to fit (PR #5 test plan).

### Uncatalogued
- Every number and claim in `docs/DIRECTION.md` is a target: the document says "nothing here has been built, booted or measured" about the native track, and only the two skeletons have since given it any code.
- FreeDOS licensing: the bundle is "mostly the GPL" per the document, and `NOTICE` does not list it yet. The suggested repository description and topics in PR #5 were not applied.

### Wish list
- Bundling FreeDOS, booting into it or running it in a V86 task (design later).
- The HAL: `docs/DIRECTION.md` says its surface "is not designed yet" and should be written against the first two consumers.
- The ordered milestones for each track, from "n0 boot" to a real 486.

> **From the stacks — FreeDOS.** As the story is usually told, the FreeDOS project began on 29 June 1994, started by Jim Hall, then a student, after Microsoft announced it would no longer sell or support MS-DOS; within weeks others joined. The Desk's direction document plans to bundle it rather than reimplement DOS, and notes that its parts carry different licences, mostly the GPL, which must be checked first.
> Source: [FreeDOS, Wikipedia](https://en.wikipedia.org/wiki/FreeDOS)
