---
id: bios
n: 17
title: The BIOS comes back
date: 2026-10-23
summary: Spike n3 gives the protected-mode kernel the BIOS back, one call at a time, through a trampoline that drops the CPU to real mode and returns, and with it the way out of a VBE mode that spike D could not find.
prs: [17]
---
Chapter 12 ended on a problem. The native kernel could ask the BIOS for a VBE mode only before the switch to protected mode, and after the switch it could not leave that mode again on any card but QEMU's standard one: on `-vga cirrus` the text screen came back garbled. Spike n3 (pull request #17, {{pr17.state}}, branch `spike/n3-native-bios`, written on top of the native graphics spike) is the answer the archive said it was waiting for: a real-mode trampoline, a one-page blob that drops the CPU back to real mode, runs one `INT n` with a register block and a buffer below 1 MiB, and returns, plus the Rust around it that keeps the kernel's interrupts, timer and keyboard whole.

**The options.** The kernel needs the BIOS for video, for disks until it has its own drivers, and for system information. The document sets four ways beside one another: a real-mode trampoline; V86 mode, a virtual 8086 task under the kernel with a monitor that emulates the sensitive instructions; an x86 emulator running the BIOS's code; and doing everything in the loader, which is what spike D did. The recommendation is the trampoline first and V86 later: it is the smallest thing that gives every BIOS service, it can be tested in QEMU on day one, and it leaves the decision about V86 for the day there is a need, which is calls that must not turn interrupts off, or FreeDOS.

**The round trip.** Fourteen steps, in three modes, with interrupts off and the non-maskable interrupt masked for the whole of it. The kernel checks the preconditions, saves the controller masks and puts both 8259s on the BIOS's vector bases; the blob saves the processor's tables and control registers, loads a 64 KiB byte-granular 16-bit descriptor into every segment register, clears paging and protection, jumps into real mode, and calls the BIOS handler through the real vector table as an `INT` would; and the way back reverses it.

<!-- explainer biscall -->

The 16-bit descriptors are the careful part. Loading one into every data segment, and jumping through a 16-bit code descriptor before clearing the protection bit, leaves the hidden parts of the segment registers with the limit real mode keeps using. A code or data segment left with a 4 GiB limit would be "big real mode", which works on most processors and is not what a BIOS or a 486 manual promises.

**Why the controllers must be put back and forth.** The BIOS's handlers expect IRQ 0 to arrive as `INT 08h` and IRQ 1 as `INT 09h`; the kernel's arrive at `0x20` and `0x21`. So the call reprograms both 8259s to the BIOS bases and back afterwards, with the mask open only for the lines whose BIOS handlers a call can depend on: the timer, the cascade, the floppy, the real-time clock and the IDE lines. The keyboard and the mouse are closed on purpose, because the BIOS's keyboard handler would swallow the scancode into its own buffer and the kernel's driver would never see it.

**The hard part.** That has a price. A byte that arrives from the keyboard during a call stays in the 8042's output buffer; the controller raised its interrupt edge while the line was masked, the 8259 discarded the request when it was reprogrammed, and the 8042 will not raise another edge until the byte is read. The fix is to poll after the call: while the 8042 reports a waiting byte, run the registered handler for the line by hand, as the interrupt would have. The test sends a key during a one-second wait inside `INT 15h AH=86h` and checks that the driver delivers it. The timer needed the same care: a BIOS tick counted in the BIOS data area is exactly one kernel tick at 100 Hz, so after a call the difference is added back to the kernel's clock, and a tick raised but not yet delivered when the controllers were reset is added too. A test makes a call every tick across four seconds of the real-time clock and checks that the kernel counts a hundred ticks to each.

**What it fixes.** Through the trampoline the kernel runs the same `desk-vga` VBE layer it used on DOS, unchanged, after boot: it sets 640 by 480 and 800 by 600 in 256 colours, draws the picture and reads back the DOSBox checksum `0x5bc65936`, and returns to text with `INT 10h` function 0003h, twice in a row, on QEMU's standard, Cirrus and VMware cards alike. Spike D's failing case is the Cirrus one, and it now works. The floppy the kernel booted from reads through `INT 13h`, and with an IDE disk attached the LBA extensions work too; the memory map read again after boot equals the loader's; and `INT 1Ah` agrees with the CMOS read directly.

<!-- table n3-results.md -->

**What it costs.** Each call reprograms two interrupt controllers, which is about sixty port accesses, and switches modes twice. Measured in an emulator, a trivial call takes tens of microseconds, and the document guesses about 100 microseconds on a 486, unmeasured.

<!-- figure bios-cost -->

**What it does not do.** It cannot be called from an interrupt handler, and refuses with an error if it is; the kernel is blind for the length of a call; a BIOS that never returns takes the kernel with it; and with paging it needs the low megabyte identity-mapped, which the kernel does not have yet. The V86 upgrade the document sketches (a task-state segment, a monitor for `int`, `pushf`, `cli` and port access, and a fault for every sensitive instruction on a 486) would add calls without turning interrupts off and the path towards FreeDOS, behind the same `Bios` trait. It is designed in the document and not built.

One more thing to know about how the pieces fit. This branch's loader needed the memory map from E820 or E801, and 86Box's 1993 BIOS has neither, so the kernel of this pull request, on its own branch, never started on 86Box: it needed the AH=88h fallback of spike G (chapter 16), which was in a separate pull request. Both are merged now, and `boot/stage2.asm` on `main` holds the fallback; I have not run the merged image on 86Box, so that the two work together there is unverified.

> **From the stacks — Why a trampoline.** The Intel 80286 could not return from protected mode to real mode on its own, according to Wikipedia: switching back needed a hardware reset, and IBM's PC/AT did it through added circuitry, code in the BIOS and the 8042 keyboard microcontroller, at what the article calls a huge performance penalty. The 8042 article says software could command that reset, which the 80386 and later processors did not need. That is why a 486 kernel can drop to real mode for one call by clearing a bit and jumping, which is the whole of this spike's trick.
> Source: [Intel 80286, Wikipedia](https://en.wikipedia.org/wiki/Intel_80286)

```figures
id: biscall
kind: explainer
track: native
title: One BIOS call from the protected-mode kernel
alt: A step-through of the fourteen-step round trip from the protected-mode kernel into real mode and back, showing the CPU mode, the interrupt controller's vector bases and the control register bits at each step.
caption: The switching sequence of the real-mode trampoline, step by step.
at: explainer biscall
thumb: steps

id: bios-cost
kind: chart
track: native
title: What a BIOS call costs
alt: Bars of the time of one BIOS call from the kernel in three emulator setups, about 47, 32 and 160 microseconds, emulated speeds and not a 486.
caption: Tens of microseconds in an emulator, from the document's table.
at: figure bios-cost
thumb: svg
```

## The shelves

### Catalogued
- In QEMU 11.1.2 as a 486 the kernel makes the BIOS calls in `tests/native/expected-bios.txt` and the screenshots match their reference checksums: VBE set, draw and restore twice on the standard, Cirrus and VMware cards; the floppy read through `INT 13h`; LBA reads on an IDE disk; the memory map re-read; and `INT 1Ah` against the CMOS. The same transcript and screenshots result on QEMU 8.2.2, which is the CI image's version. CI is `{{pr17.ci}}` on the merge commit `{{pr17.head}}` of pull request #17, which is {{pr17.state}}.
- A thousand calls leave the stack, GDT, IDT, segment registers, `CR0` and the interrupt controller masks identical after each; the kernel keeps a hundred ticks to the real-time-clock second while it calls every tick; a key sent during a call is not lost; and an unsupported function, a missing drive, a vector with no handler and a call from an interrupt handler are each refused or answered with an error, after which the next call works.
- The same image, without key injection, ran on Bochs 2.7 built as a 486 with two of its BIOSes, with the same results; with SeaBIOS 1.13 under Bochs there is no VBE, which the kernel reports and survives.

### Uncatalogued
- Any real BIOS or real hardware: a real 486 BIOS, a real VGA BIOS for an S3 or Cirrus card, a real floppy controller and a real 8042 and 8259. The re-initialisation of the interrupt controllers is the thing most likely to differ on a chipset with an integrated controller.
- 86Box, as above: the kernel of this branch never started there without the fallback for a BIOS without E820 or E801. That fallback is merged on `main`, and nobody has run the merged image on 86Box.
- The Bochs runs have no key injection and no screenshots: they prove the calls and the VBE sequence, not the pixels of the text screen or the lost-key path.
- Paging, and V86 mode: described in the document and not built, so the identity-mapping requirement and the V86 monitor are untested design.
- The cost on a 486, which is a guess; the figures are emulated.
- SMM, APM and laptop BIOSes, and a BIOS that hangs: there is no watchdog.

### Wish list
- A V86 monitor behind the same `Bios` trait, for calls that cannot tolerate a blind kernel and for FreeDOS.
- A trait method for disk calls, because a track does not fit the 512-byte buffer the trait carries.
- Allowing a call with paging on, once the low megabyte is identity-mapped.
- The kernel's timer rate and the BIOS tick conversion, if the rate ever changes from 100 Hz.
