---
id: m-bioscalls
n: M8
part: III
title: The kernel: calling the BIOS from protected mode
summary: Why the kernel needs the BIOS, the switching sequence, the Bios trait, what was measured, and what V86 would add.
date: 2026-10-23
reviewed: 44f0265
watches: [kernel/bios16.asm, kernel/src/bios.rs, kernel/src/biostest.rs, hal/hal/src/bios.rs, tests/native/expected-bios.txt, docs/N3-BIOS-PATH.md]
log: [bios, emulators]
anatomy: [desk:kernel]
---
## What it is

After the loader switches the processor to [protected mode](term:protected-mode), the BIOS is gone: its routines are 16-bit code that expect [real mode](term:real-mode), and the kernel runs in 32-bit flat protected mode with its own tables. [M4](chapter:m-loader) showed the loader's answer, which is to ask the BIOS every question the kernel will want answered before the switch and leave the answers in memory. This chapter is about the other answer, built in a later spike (`docs/N3-BIOS-PATH.md`, "spike n3"): a *real-mode [trampoline](term:trampoline)*. The kernel can, at any time and from ordinary code, ask for one BIOS software interrupt to be run. A small page of 16-bit code drops the processor back to real mode, runs the `INT`, and brings the processor back to protected mode, with the kernel's tables, timer and keyboard as it left them.

It is the kernel's `bios` module (`kernel/src/bios.rs`), the assembly page it calls (`kernel/bios16.asm`) and the scenarios that test both (`kernel/src/biostest.rs`), together with the HAL's `Bios` trait that makes the call look the same to the shared code as it does under DOS (`hal/hal/src/bios.rs`; [M9](chapter:m-hal) is the HAL). On the map of the system ([M1](chapter:m-map)) these are in the `kernel` box of the native track, and the box on the [anatomy page](chapter:anatomydesk#box-desk-kernel) is where the manual links them. It is the one place where the kernel steps backwards in the history of the processor on purpose.

After this chapter you should be able to say why a kernel written in 2026 for a 486 would want a firmware interface from 1981, follow the fourteen steps of a call, decode the two 16-bit [descriptors](term:descriptor) of the trampoline by hand, say what the call does to the [interrupt controllers](term:pic), the timer and the keyboard and why each is needed, and quote what a call costs together with the conditions under which that was measured. The processor facts it relies on ([segments](term:segment), [selectors](term:selector), [CR0](term:cr0), the switch) are in [M2](chapter:m-cpu); the controllers and the 8042 in [M3](chapter:m-devices); the interrupt tables in [M6](chapter:m-interrupts).

## How it works

### Why the kernel needs the BIOS at all

The native kernel has its own drivers for the serial port, the timer, the keyboard and the mouse. It has none for a graphics card, a floppy controller or a hard disk. The document names three things the kernel needs the BIOS for, in the order of how soon: video (leave a [VBE](term:vbe) mode and return to text, or set another resolution), disk (read sectors before the kernel has drivers of its own) and system information (the memory map, the time). Spike D, the native graphics spike, found what the loader-only answer cost: the kernel could not leave a VBE mode on any card but QEMU's Bochs-derived one, could not change resolution, and could not read a disk (`docs/N3-BIOS-PATH.md`, intro). The trampoline closes the first of those: through it the kernel returns to text on all three cards QEMU emulates.

### The options weighed

The document compares four ways to get the BIOS back, and chooses the first.

| Option | What it is | What it costs |
|---|---|---|
| (a) Real-mode trampoline | Switch the CPU back to real mode for one call, switch forward again | About 200 lines of assembly (most of them comments) and 450 of Rust; a fixed cost per call; the kernel is blind for the duration |
| (b) [V86 mode](term:v86-mode) | A virtual-8086 task under the kernel, with a monitor that handles the faults of sensitive instructions | A TSS, an IDT that routes the faults, a monitor: some 600 to 1,000 lines and a real test matrix; every sensitive instruction faults on a 486 |
| (c) An x86 emulator (as `vbetool` does) | Run the BIOS's code in an interpreter | Some 10,000 lines of C or a port; slow; port I/O and IRQ waits must be forwarded; good for video only |
| (d) Everything in the loader | Ask before protected mode | Free and robust, but no resolution change, no way back to text on some cards, no disk after boot (this is what the project already had) |

The recommendation is "(a) first, (b) later": the trampoline is the smallest thing that gives the kernel every BIOS service, and it can be tested under QEMU on day one. The numbers in the first row are the document's, and the files agree with them: `bios16.asm` is 220 lines and `bios.rs` is 461.

### The low-memory map

The kernel is linked at 1 MiB, which real mode cannot execute from, so the trampoline lives below 1 MiB. It uses the staging area where the loader first put the kernel (`0x10000`), free once the loader has copied the kernel up. The frame allocator never hands out the first megabyte ([M7](chapter:m-memory)), so the kernel does not need to ask for it.

| Physical | Size | What |
|---|---|---|
| `0x00000` | 1 KiB | Real-mode vector table. Never written by the kernel; the blob loads the real-mode `IDTR` with base 0 and limit `0x3FF` |
| `0x00400` | 256 B | BIOS data area: read for the tick count (`0x46C`), the midnight flag (`0x470`) and the video mode (`0x449`) |
| `0x00500` to `0x00820` | | The loader's boot information and map; untouched |
| `0x02000` | 4 KiB | The BIOS font the loader copied |
| `0x07C00`, `0x07E00` | 512 B, 8.5 KiB | Stage 1 and stage 2, still in memory (a scenario compares the first with the disk) |
| `0x10000` | 4 KiB | **The trampoline blob**: control block at offsets `0x000` to `0x02F`, the entry at `0x100`, the code, its tables and the 16-bit GDT |
| `0x11000` | 8 KiB | **The [bounce buffer](term:bounce-buffer)**, for buffers the BIOS reads and writes |
| `0x13000` | 4 KiB | **The real-mode stack**: `SS = 0x1000`, `SP` starts at `0x4000` |

The three regions sit inside the one 64 KiB block `0x10000` to `0x1FFFF`, so a floppy transfer into the bounce buffer can neither cross a [DMA](term:dma) boundary nor go out of the DMA controller's reach. The kernel prints the map once per boot: `bios: real-mode trampoline at 0x10000, bounce buffer 0x11000 (8192 bytes), stack 0x13000`, which `kernel/src/main.rs` builds from the constants in `bios.rs`. The *bounce buffer* is needed because the BIOS cannot see the kernel's memory above 1 MiB: a caller writes the input into it with [`write_low`](sym:kernel/src/bios.rs::Bounce), points `ES:DI` (or `DS:SI`) at its real-mode segment `0x1100`, makes the call, and reads the output back.

### The switching sequence

Everything between "the kernel calls" and "the kernel is back" runs with interrupts off. The sequence is fourteen steps; the first three and the last three are Rust, the rest is `bios16.asm`.

1. **Checks.** [`call`](sym:kernel/src/bios.rs::call) refuses if the trampoline is not set up (`NotReady`), if another call is in progress (`Busy`; the guard is a flag taken with an `xchg`), if an 8259 in-service bit is set (`InInterrupt`), if `CR0.PG` is set (`PagingEnabled`) or if the vector table entry for the interrupt is `0000:0000` (`NoHandler`). It runs the rest inside `without_interrupts`.
2. **Save.** [`bios_int`](sym:kernel/src/bios.rs::bios_int) saves the controllers' masks, reads the BIOS tick count and whether a timer request is waiting, and masks [NMI](term:nmi).
3. **Reprogram.** It puts both 8259s on the BIOS's vectors with the mask below and writes the interrupt number, the flags and the registers into the control block at `0x10000`.
4. **Entry** at `0x10100`: `pushfd`, `pushad`, `cli`; the blob saves `ESP`, the descriptor-table registers `GDTR` and `IDTR`, `CR0`, `CR3` and the five data-segment selectors in its own data.
5. **To 16-bit protected mode.** `lgdt` its own table; load selector `0x20` into `DS`, `ES`, `FS`, `GS` and `SS`; `ESP = 0x4000`; far jump through selector `0x18`.
6. **Real mode.** In 16-bit code: clear `CR0.PG` and `CR0.PE`, and far jump to `0x1000:rm_entry` to reload `CS` with a real-mode segment.
7. **Set up.** `DS = SS = 0x1000`, `SP = 0x4000`, `ES`, `FS`, `GS` zero; `lidt` with limit `0x3FF` and base 0; read the handler's far address from the vector table.
8. **The call.** Load the caller's flags and push a copy; load `ES`, `FS`, `GS`, `EBP`, `ESI`, `EDI`, `EDX`, `ECX`, `EBX`, `EAX` and last `DS` from the block; `call far` to the handler, which returns with `iret` as it would from an `INT`.
9. **Store.** Save the registers and the flags the handler returned; `cli`, `cld`; `lgdt`; set `CR0.PE`; far jump through selector `0x08` to `pm32_back`.
10. **Back in 32 bits.** Load the flat data selector everywhere; restore `ESP`, `GDTR`, `IDTR`; reload the saved selectors (`SS` last, so the stack switch is the last change); restore `CR3` and then `CR0` (so paging would come back last, if it were on).
11. **Return.** `popad`, `popfd`, `ret` to Rust.
12. **Controllers back.** Put the PICs back on `0x20` and `0x28` with the saved masks, and restore NMI.
13. **Clock.** Add the ticks the BIOS counted, and any undelivered timer tick, to the kernel's clock.
14. **Input.** Hand a keyboard or mouse byte left in the 8042 to the driver by hand, and count the call.

<!-- explainer biscall -->

The figure steps through the same fourteen steps with the mode, the PIC vector bases and the `CR0` bits at each. The sequence is the one in the document and the one in the code; the figure is drawn from the document, not captured from a run.

**The descriptors.** The blob's table, `tramp_gdt`, has five entries. The first three are the kernel's own, at the same selector numbers, so the kernel's selectors keep their meaning: null, `0x08` the flat 32-bit code `0x00CF9A000000FFFF` and `0x10` the flat data `0x00CF92000000FFFF` (decoded in [M4](chapter:m-loader)). The two new ones are 16-bit with a base at the blob and byte granularity: `0x18` is code, `0x00009A010000FFFF`, and `0x20` is data, `0x000092010000FFFF`. Written least significant byte first, `0x18` is `FF FF 00 00 01 9A 00 00`: the limit is `0xFFFF` (64 KiB); the base is bytes `00 00 01`, which is `0x010000`; the access byte `9A` means present, [ring](term:ring) 0, code, readable; and the flags byte `00` means byte granularity and a 16-bit default size. The data descriptor differs only in the access byte, `92`, writable data.

Loading `0x20` into every data segment, and jumping through `0x18` before `PE` is cleared, is what leaves the *hidden* part of each segment register (the processor's cached copy of the descriptor) with a 64 KiB byte-granular limit, which real mode keeps using after the switch. A segment left with a 4 GiB limit would be what is called "big real mode", which works on most processors and is not what a BIOS or a 486 manual promises. The base `0x10000` is what real mode computes for segment `0x1000`, so the first real-mode instruction agrees with the cached state.

**Calling through the vector table.** The blob does not patch an `int n` instruction into its own code, which a 486's prefetch queue could execute as the old opcode; it reads the handler's far address from the real vector table and does a far call with a flags word on the stack, which is what an `INT` leaves (except that an `INT` also clears `IF` and `TF`, and interrupts are already off).

### Putting the controllers on the BIOS vectors, and back

The BIOS's handlers expect the timer to arrive as `INT 08h` and the keyboard as `INT 09h`; the kernel's arrive at `0x20` and `0x21` ([M6](chapter:m-interrupts)). So a call reprograms both 8259s to the BIOS bases, `BIOS_BASES` = (`0x08`, `0x70`), and afterwards to the kernel's, `REMAPPED_BASES` = (`0x20`, `0x28`), both from the shared driver in `hal/drivers/pic`. Re-initialising a controller also clears its request register and resets its mask, so the kernel restores its own masks, saved in step 2.

During the call the mask [`BIOS_PIC_MASK`](sym:kernel/src/bios.rs::BIOS_PIC_MASK) leaves open only the lines whose BIOS handlers a call can depend on: IRQ 0 (the timer tick and the floppy motor timeout), IRQ 2 (the cascade to the slave), IRQ 6 (the floppy: `INT 13h` waits for its interrupt), IRQ 8 (the real-time clock, used by `INT 15h` waits) and IRQ 14 and 15 (IDE). Everything else is closed, *above all IRQ 1 and IRQ 12*, the keyboard and the mouse. If they were open the BIOS's `INT 09h` handler would read the [scancode](term:scancode) from the keyboard controller into the BIOS's own buffer, and the kernel's driver would never see it. A call refuses to start if any in-service bit is set, because the reprogramming would clear it under a handler that has not yet sent its [end-of-interrupt](term:eoi). The kernel's handlers send the end-of-interrupt before they return, so a call from the main line or the idle loop always passes and a call from a handler never does.

### NMI

The non-maskable interrupt (the [NMI](term:nmi)) cannot be turned off with `cli`. In protected mode it is delivered through the kernel's IDT and in real mode through the vector table, and there is a window in a call where the processor is in real mode but the kernel's IDT is still the one in memory. An NMI in that window would fetch from the wrong table. So the call masks NMI for its whole length, by writing `0x80` to the CMOS index port `0x70` (bit 7 of that port is the NMI mask), and afterwards puts it back in the kernel's resting state, which is masked: BIOSes leave it masked after power-on self-test, and the kernel has no NMI handler beyond the fatal dump. A constant, `NMI_ENABLED_AFTER`, is the place to change that. No test delivers an NMI, and the document lists it with the things that were not exercised.

### The BIOS's timer ticks

The kernel runs the PIT at 100 Hz ([M6](chapter:m-interrupts)). The BIOS counts its own ticks in the BIOS data area at `0x46C`, in its `INT 08h` handler, which runs only while the BIOS has interrupts on inside a call. At 100 Hz one BIOS tick is exactly one kernel tick, so after a call the difference of the count is added to the kernel's clock: capped at 10,000, and handled specially when the midnight flag goes from clear to set during the call, because the count was reset. A tick that had been raised but not yet delivered when the controllers were reprogrammed would be thrown away by the controller's reset, so the kernel adds it itself, once before and once after. The PIT is never touched: the BIOS does not reprogram channel 0 in the calls tried. The consequence the document draws is that `INT 1Ah` function 0 (ticks since midnight) is not a wall clock here, while functions 2 and 4 (the real-time clock) are.

### The 8042 byte hand-off

A key pressed during a call lands in the [8042](term:8042) controller's output buffer while IRQ 1 is masked. The controller raised its interrupt edge when the line was masked, the 8259 discarded the request when it was reprogrammed, and the controller will not raise another edge until the byte is read. So after each call [`bios_int`](sym:kernel/src/bios.rs::bios_int) polls: while the controller's status port `0x64` has bit 0 (output buffer full) set, it runs the registered handler of IRQ 1 by hand, or of IRQ 12 if bit 5 says the byte came from the mouse, up to eight times. The drivers needed no change, because they read one byte per call and keep their own rings. The test sends a key during a one-second `INT 15h` wait.

### The `Bios` trait and `NativeBios`

The HAL (`hal/hal/src/bios.rs`) defines the call as a trait with two methods: `int(vector, regs, buffer)`, which runs a real-mode software interrupt with a `BiosRegs` and an optional buffer of at most `BIOS_BUFFER_MAX` (512) bytes, and `read_low(segment, offset, out)`, which reads the first megabyte. Under DOS this is a DPMI real-mode call; natively, [`NativeBios`](sym:kernel/src/bios.rs::NativeBios) implements it over `call`. If a buffer is given, `NativeBios::int` copies it into the bounce buffer, points `ES:DI` at it, makes the call and copies it back. The shared `desk-vga` crate's VBE layer, `Vbe<NativeBios>`, therefore runs in the kernel unchanged, which is the point: the code written for DOS is the code that sets modes natively ([M12](chapter:m-graphics)). Callers that need larger buffers (a disk track) use the kernel's own functions, [`bounce_buffer`](sym:kernel/src/bios.rs::bounce_buffer) and [`read_low`](sym:kernel/src/bios.rs::read_low), directly. The kernel's raw type, [`RealModeRegs`](sym:kernel/src/bios.rs::RealModeRegs), is the 40-byte block the blob loads and stores, with `fs`, `gs` and the flags in and out; `BiosRegs` has no `fs` or `gs`, which the document lists as feedback to the HAL.

### What the scenarios show

`tools/native-test.sh` boots the `bios` test mode and checks the transcript `tests/native/expected-bios.txt` (the checker matches 101 lines of real output against it) and three screenshots against `tests/native/bios.sums`. In order, the scenarios in `kernel/src/biostest.rs` do these things.

- **Memory.** `INT 12h` reports conventional memory; E820 is read again after boot, through the bounce buffer, and equals the loader's map entry for entry; `INT 15h AH=88h` and `AX=E801` agree with it within 256 KiB (they ignore the reserved hole at the top of RAM).
- **Clock.** `INT 1Ah` functions 2 and 4 return valid BCD in range, within two seconds of the real-time clock read directly through the CMOS ports `0x70` and `0x71`.
- **Errors.** An unsupported `INT 15h` function and a read from drive `0x77` set the carry flag with the BIOS's status; a vector table entry zeroed for the test is refused as `NoHandler`; the guard returns `Busy`; a call tried from the timer's interrupt handler is refused as `InInterrupt`; the next call works.
- **Disk.** The geometry through `AH=08h` (80 cylinders, 2 heads, 18 sectors per track); sector 0 has `0x55AA` and equals stage 1 still in memory at `0x7C00` except for the five variables stage 1 writes; 16 sectors from LBA 18 equal the first 8 KiB of the running kernel; a read of sector 0 in the BIOS's CHS form (sectors count from 1) fails with the BIOS's status. A second run attaches an IDE disk and reads two sectors by LBA with the extensions `AH=41h` and `42h`.
- **A key during a call.** The inject script sends `a` while the kernel is one second into `INT 15h AH=86h`.
- **Stress.** 5,000 timed calls; 1,000 calls with the stack pointer, `GDTR`, `IDTR`, the three GDT descriptors, a hash of the IDT, six segment registers, `CR0`, the PIC masks and the interrupt flag compared after every one; and calls once per tick across four changes of the RTC second, between which the kernel must count 98 to 102 ticks.
- **VBE.** `4F00h` and `4F01h` for three modes; set 640×480×8 with the linear bit; draw a picture and compare its checksum with the one drawn in memory and with spike A's DOSBox value `0x5bc65936`; screenshot; the same at 800×600; `INT 10h AX=0003h` back to text, with a check of the BIOS data area's video mode and of `4F03h`; a screenshot of the text screen; the whole thing twice, so a card left half-way fails the second round.
- **The kernel afterwards.** A key sent at the end must arrive as an event after more than a thousand calls.

**Three cards.** The test runs on QEMU's standard (`std`), `cirrus` and `vmware` VGA cards, and the three screenshots must match the same references: the VBE picture at 640×480 and 800×600 and the restored text screen. This closes spike D's open finding, that on Cirrus the kernel could not leave VBE: the card has no Bochs registers to switch off the mode, so the tour now asks the BIOS (`INT 10h AX=0003h`), which knows how to reset any card it ships with. The document records the before and after from the same tour.

**The five negative checks** (`tools/native-test.sh`): a transcript with one changed line must fail, a wrong screenshot reference must fail, a VBE screenshot offered as the restored text screen must fail, the floppy-only transcript offered for the IDE run must fail, and a run in which the key never arrives must make the kernel panic with "the key sent during the call was lost" and exit 1.

## Why it is this way

- **A trampoline now, V86 later.** It is "the smallest thing that gives the kernel every BIOS service" and can be tested under QEMU at once; the decision about V86 waits for a need, which the document names as calls that must not turn interrupts off, or FreeDOS (`docs/N3-BIOS-PATH.md`, "The options" and "The V86 upgrade").
- **The reprogramming of the PICs, not a V86 monitor, takes the cost.** Option (a) has to put the controllers on the BIOS vectors because the BIOS's handlers are the ones that must answer; option (b) leaves them where they are and reflects the interrupt. This is the delicate part of (a), and the document says that the 8259 re-initialisation "is the thing most likely to differ on a chipset with an integrated controller" (`docs/N3-BIOS-PATH.md`, "What was and was not verified").
- **Keyboard and mouse closed, the byte rescued by polling.** The alternative, leaving them open, hands scancodes to the BIOS.
- **The mask is the set the BIOSes tried needed.** The document says so: a BIOS that waits for an interrupt a call has masked will stall.
- **NMI masked.** An NMI during the window would use a table the processor is not using; masking it removes the case.
- **One buffer, in low memory.** The BIOS cannot see above 1 MiB, so every buffer goes through the bounce buffer. The contract the document gives is the list in "The API and its contract": never from an interrupt handler, any other context fine, one call at a time, paging off or the low megabyte identity-mapped, the vector must have a handler, buffers in the bounce buffer, and no state trusted across a call except what it restores.
- **The result has two layers.** `Ok(())` means the call was made; whether the BIOS function succeeded is in the registers (the carry flag for most, `AX` for VBE).
- **Not from an interrupt handler.** A handler runs with an in-service bit set, and the controller reset would clear it.

> **From the stacks — Virtual 8086 mode.** According to the Wikipedia article on virtual 8086 mode, the mode appears in the Intel 80386 and later processors. It lets real-mode programs such as DOS applications run while a protected-mode operating system stays in control. The article says that when such a program touches hardware or certain memory regions the processor traps the event and calls a monitor, which must run at privilege level 0 in protected mode. That monitor is the thing this chapter's trampoline avoids writing.
> Source: [Virtual 8086 mode, Wikipedia](https://en.wikipedia.org/wiki/Virtual_8086_mode)

## The code

| Where | What it holds |
|---|---|
| [`kernel/bios16.asm`](repo:kernel/bios16.asm) | The blob: [`entry`](sym:kernel/bios16.asm::entry) (protected mode, saves state), [`pm16`](sym:kernel/bios16.asm::pm16) (clears `PG` and `PE`), [`rm_entry`](sym:kernel/bios16.asm::rm_entry) (real mode, loads registers, calls the handler, stores them), [`pm32_back`](sym:kernel/bios16.asm::pm32_back) (restores the kernel) and [`tramp_gdt`](sym:kernel/bios16.asm::tramp_gdt) |
| [`kernel/src/bios.rs`](repo:kernel/src/bios.rs) | [`init`](sym:kernel/src/bios.rs::init) copies the blob to `0x10000` and checks its magic; [`call`](sym:kernel/src/bios.rs::call) and [`bios_int`](sym:kernel/src/bios.rs::bios_int) are the safe and the raw call; [`BiosError`](sym:kernel/src/bios.rs::BiosError) names the refusals; [`NativeBios`](sym:kernel/src/bios.rs::NativeBios) is the HAL's `Bios`; [`STATS`](sym:kernel/src/bios.rs::STATS) counts calls, ticks and rescued bytes |
| [`kernel/src/biostest.rs`](repo:kernel/src/biostest.rs) | The scenarios above: [`memory_calls`](sym:kernel/src/biostest.rs::memory_calls), [`clock_calls`](sym:kernel/src/biostest.rs::clock_calls), [`error_paths`](sym:kernel/src/biostest.rs::error_paths), [`disk_calls`](sym:kernel/src/biostest.rs::disk_calls), [`keyboard_during_call`](sym:kernel/src/biostest.rs::keyboard_during_call), [`stress`](sym:kernel/src/biostest.rs::stress), [`vbe_after_boot`](sym:kernel/src/biostest.rs::vbe_after_boot) |
| [`hal/hal/src/bios.rs`](repo:hal/hal/src/bios.rs) | The [`Bios`](sym:hal/hal/src/bios.rs::Bios) trait, [`BiosRegs`](sym:hal/hal/src/bios.rs::BiosRegs) and `BIOS_BUFFER_MAX` |
| [`hal/drivers/pic/src/lib.rs`](repo:hal/drivers/pic/src/lib.rs) | `BIOS_BASES`, `REMAPPED_BASES` and `Pic8259::init`, which the call uses in both directions |
| [`tests/native/expected-bios.txt`](repo:tests/native/expected-bios.txt) | The expected transcript; [`bios.inject`](repo:tests/native/bios.inject) is the key script; [`bios.sums`](repo:tests/native/bios.sums) the screenshot references |
| [`tools/native-test.sh`](repo:tools/native-test.sh) | The "BIOS calls from the kernel" section: three cards, the IDE run, the negative checks |

For depth: `docs/N3-BIOS-PATH.md` is one document with all of it: [the options](doc:docs/N3-BIOS-PATH.md#the-options), [the low-memory map](doc:docs/N3-BIOS-PATH.md#the-low-memory-map), [the switching sequence](doc:docs/N3-BIOS-PATH.md#the-switching-sequence), [interrupts, the PICs, the timer and the keyboard](doc:docs/N3-BIOS-PATH.md#interrupts-the-pics-the-timer-and-the-keyboard), [the API and its contract](doc:docs/N3-BIOS-PATH.md#the-api-and-its-contract), [cost](doc:docs/N3-BIOS-PATH.md#cost), [paging](doc:docs/N3-BIOS-PATH.md#paging) and [the V86 upgrade](doc:docs/N3-BIOS-PATH.md#the-v86-upgrade).

## Worked example

**One call, `INT 1Ah` AH=02h (read the real-time clock).** The caller builds `RealModeRegs::with_eax(0x0200)`, so only `AH = 2` is set, and calls [`call(0x1A, &mut regs)`](sym:kernel/src/bios.rs::call). The checks pass. The vector table entry is at `0x1A × 4 = 0x68`; it holds the BIOS's handler as `segment:offset`. `bios_int` writes into the control block at `0x10000`: the vector (offset 4), the flags word (offset 6, `(0 & 0x0CD5) | 0x0002` = `0x0002`: the mask keeps carry, parity, auxiliary carry, zero, sign, direction and overflow, and bit 1 is always set on an x86) and the registers (from offset 8).

The controllers are reprogrammed. `BIOS_PIC_MASK` has IRQs 0, 2, 6, 8, 14 and 15 open: `1 + 4 + 64 + 256 + 16,384 + 32,768` = `0xC145` of open bits, so the mask is `!0xC145` = `0x3EBA`; the master gets `0xBA` (binary `1011 1010`: bits 0, 2 and 6 clear) and the slave `0x3E` (`0011 1110`: bits 0, 6 and 7 clear, which are IRQ 8, 14 and 15). The master's base is `0x08` and the slave's `0x70`.

The blob runs the steps of the sequence and calls the handler at `0x1A × 4`. The handler reads the clock from the CMOS and returns `CH` hours, `CL` minutes and `DH` seconds in BCD, with the carry flag clear. The blob stores the registers, returns to protected mode, and `bios_int` copies them to the caller's block. The kernel puts the controllers back on `0x20` and `0x28` with the masks it saved, restores NMI, adds the BIOS ticks (zero or one for a call this short) and polls the 8042. `biostest::clock_calls` then turns `CH`, `CL` and `DH` from BCD into seconds since midnight and compares them with the same clock read directly through the CMOS ports `0x70` and `0x71`; the transcript's line is `bios: INT 1Ah agrees with the RTC read directly through the CMOS ports to within 2 s: true`.

**E820 after boot.** The same module asks the BIOS for the memory map a second time. `e820_after_boot` writes the value 1 at offset 20 of the bounce buffer (the ACPI word that the filter reads), sets `EAX = 0xE820`, `EDX = 0x534D4150` (the ASCII "SMAP"), `ECX = 24`, `ES` to the bounce segment `0x1100` and `EBX` to the continuation (0 for the first call), and calls `INT 15h`. It reads the 24 bytes back, checks that the BIOS answered with "SMAP" in `EAX`, applies the same filter as the loader (skip empty entries and ones whose ACPI word says ignore) and continues with `EBX` until it returns 0. The result is compared entry by entry with the list the loader left, and the line printed is `bios: INT 15h AX=E820 after boot: 6 entries, the loader's map had 6, identical: true`. That is the entry list of [M7](chapter:m-memory), the same six, asked for from a running kernel.

**The cost, as the document records it.** One `INT 1Ah` AH=02h call, 5,000 in a row, timed in 10 ms kernel ticks, so each figure is a mean over a tenth of a second or more:

| Where | Per call |
|---|---|
| QEMU 11.1.2, TCG | about 47 µs (24 ticks) |
| QEMU 8.2.2, TCG | about 32 µs (16 ticks) |
| Bochs 2.7 at its default 4 MIPS | about 160 µs (81 ticks) |

The document's own words on what these are: "emulated speeds and say nothing about a 486". What a call contains, per the document: about 60 port accesses (each of the two 8259 initialisations is eight words with an I/O delay write after each, then the masks, the NMI bit, and the in-service and request register reads), about 150 instructions of the switch, the BIOS handler itself, and the post-call checks. For a real 486 the document gives "a rough guess of 100 µs for a trivial call and ten to a few hundred milliseconds for a mode set or a floppy sector" (the second part being the BIOS's own time), and says outright that this is "the right order, unmeasured". Nothing here needs to be fast: VBE calls happen on mode changes, and disk calls will be replaced by a driver.

Two notes on the numbers. The kernel prints `about N microseconds each` by multiplying ticks by 10,000 and dividing by 5,000, which gives 48 for 24 ticks, where the document says about 47. And the figure moves from run to run: runs of the chapter's commands on the host printed between 42 and 46 (21 to 23 ticks), which is the same order as the 47 the document records. The boot of the whole `bios` mode takes about five seconds.

<!-- figure bios-cost -->

## State of this layer

Verified here means that the record cites a transcript line that `tools/native-test.sh` checks on every run or a merged pull request whose CI passed. It means in QEMU, never on a real machine.

```status
id: bios.trampoline
label: The switch to real mode and back, with the kernel's CPU state identical after 1,000 calls
state: built-verified
evidence: test:tests/native/expected-bios.txt::the kernel's CPU state is the same after the calls, test:tests/native/expected-bios.txt::stress: 1000 INT 1Ah calls, sym:kernel/bios16.asm::entry
note: QEMU's own CPU model and BIOS. The order of the 8259 initialisation words and the cache state of a real 486 are not seen.

id: bios.contract
label: Refusals: no handler, busy, from an interrupt handler; the next call works
state: built-verified
evidence: test:tests/native/expected-bios.txt::a call made from the timer interrupt handler is refused as in-interrupt, test:tests/native/expected-bios.txt::Err(NoHandler(240)), sym:kernel/src/bios.rs::BiosError
note: PagingEnabled is not exercised, because the kernel never turns paging on.

id: bios.pics
label: Both 8259s put on the BIOS vectors and back, with the kernel's masks restored
state: built-verified
evidence: test:tests/native/expected-bios.txt::the tick count kept one hundred ticks to the RTC second, sym:kernel/src/bios.rs::BIOS_PIC_MASK
note: Verified on QEMU's emulated controllers. The document names the re-initialisation as the thing most likely to differ on real chipsets.

id: bios.ticks
label: The BIOS's timer ticks added to the kernel's clock
state: built-verified
evidence: test:tests/native/expected-bios.txt::the one-second wait took, test:tests/native/expected-bios.txt::kernel ticks between three RTC seconds, sym:kernel/src/bios.rs::bios_int
note: Exact only because the kernel's PIT rate equals the BIOS tick rate; a different rate needs a conversion that is not written.

id: bios.input
label: A key pressed during a call is handed to the driver by hand
state: built-verified
evidence: test:tests/native/expected-bios.txt::were waiting in the controller and were handed to the driver, test:tools/native-test.sh::the key sent during the call was lost, sym:kernel/src/bios.rs::KBC_OUTPUT_FULL
note: The negative check makes the kernel panic when the key is lost. Mouse bytes use the same path but the test sends only a key.

id: bios.nmi
label: NMI masked for the length of a call and restored to masked
state: built-unverified
evidence: sym:kernel/src/bios.rs::NMI_ENABLED_AFTER, doc:docs/N3-BIOS-PATH.md#interrupts-the-pics-the-timer-and-the-keyboard
note: The mask is written, but no NMI is delivered in any test, so the protection it gives has not been shown to work.

id: bios.trait
label: NativeBios implements the HAL's Bios trait, and the DOS VBE layer runs on it unchanged
state: built-verified
evidence: test:tests/native/expected-bios.txt::VBE mode 0x0101 set with the linear frame buffer, sym:kernel/src/bios.rs::NativeBios, sym:hal/hal/src/bios.rs::Bios
note: The trait's 512-byte buffer is too small for a disk track; the document records the feedback and the kernel uses the bounce buffer directly.

id: bios.vbe
label: VBE modes set after boot and the way back to text, on QEMU's std, cirrus and vmware cards
state: built-verified
evidence: test:tests/native/expected-bios.txt::return to text worked twice, test:tests/native/bios.sums::text, test:tools/native-test.sh::std cirrus vmware
note: Three emulated cards with the same SeaBIOS VGA BIOS family. A real card's BIOS was not seen.

id: bios.disk
label: INT 13h reads the boot floppy, and an IDE disk by LBA
state: built-verified
evidence: test:tests/native/expected-bios.txt::16 sectors from LBA 18 (the kernel) equal the first 8 KiB of the running kernel, test:tools/native-test.sh::INT 13h AH=42h read two sectors by LBA, sym:kernel/src/biostest.rs::disk_calls
note: The IDE run exists only in QEMU 11.1.2 and only on the IDE controller QEMU emulates.

id: bios.e820rtc
label: E820, the older memory calls and the RTC through the BIOS agree with the loader and the CMOS
state: built-verified
evidence: test:tests/native/expected-bios.txt::INT 15h AX=E820 after boot, test:tests/native/expected-bios.txt::agrees with the RTC read directly, sym:kernel/src/biostest.rs::e820_after_boot
note: The transcript line holds the entry count as a range 1 to 32. The identity is checked by the kernel's own assertion.

id: bios.cost
label: The cost of a call, measured in kernel ticks under three emulators
state: built-verified
evidence: test:tests/native/expected-bios.txt::bios: cost: 5000 INT 1Ah calls took, doc:docs/N3-BIOS-PATH.md#cost
note: Emulated speeds only. The Bochs figure and the QEMU 8.2.2 figure were measured by hand and are in the document. Nothing was measured on a real 486, and the document's 100 microsecond guess is unmeasured.

id: bios.others
label: The same image on QEMU 8.2.2 and on Bochs 2.7 with two BIOSes
state: built-unverified
evidence: doc:docs/N3-BIOS-PATH.md#what-was-and-was-not-verified
note: Run by hand and recorded in the document. The Bochs runs have no key injection and no screenshots.

id: bios.hang
label: A watchdog for a BIOS call that never returns
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#risks
note: There is none. A call that hangs hangs the kernel.

id: bios.paging
label: Calls with paging on (the low megabyte identity-mapped)
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#paging
note: The blob can already restore CR3 and CR0, but the call refuses with PagingEnabled until an identity map exists.

id: bios.v86
label: A V86 monitor behind the same trait
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#the-v86-upgrade
note: Designed in the document only.

id: bios.hardware
label: Any BIOS call on a real machine
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#what-was-and-was-not-verified
note: No real BIOS, VGA BIOS, floppy controller, 8042 or 8259 was involved in any run.
```

The trampoline, the controller switch, the tick accounting, the key hand-off and VBE on the three cards are {{state.bios.trampoline}} in QEMU. What the table cannot say is how a real BIOS behaves: the document says the 8259 re-initialisation, a real VGA BIOS and the real floppy controller were not seen, and that this is where a surprise is most likely.

## What V86 would add

The document says V86 is the next step when one of three things happens: a BIOS call that cannot tolerate the kernel being blind for its length (a long `INT 13h` on a slow floppy), a need to keep the kernel's own interrupts running through a call, or the move to running FreeDOS and DOS programs, which `docs/DIRECTION.md` plans. It needs a task-state segment, so the processor knows which stack to switch to on an interrupt out of V86 mode and which I/O ports a V86 task may touch; an IDT whose gates handle the general-protection fault, because with the I/O privilege level below 3 (which it has to be) `int`, `pushf`, `popf`, `iret`, `cli`, `sti` and the port instructions all fault, and a monitor reads the instruction at the faulting address and emulates it; a monitor per task with a virtual interrupt flag and a rule for each IRQ; the first megabyte plus the high memory area mapped by paging; and a way out, with a watchdog. On a 486, which has no virtual-mode extensions (`CR4.VME` arrived later), every `pushf`, `popf`, `cli` and `sti` the BIOS executes is a fault.

What it would add over the trampoline: calls without turning interrupts off and without touching the PICs, so no lost ticks and no 8042 byte to rescue; a hung call that can be pre-empted; the kernel's IDT and NMI handling in force throughout; and the machinery that runs DOS programs. What it would cost: a few hundred to a thousand lines, a harder test matrix, and a fault per sensitive instruction. `RealModeRegs`, the bounce buffer and the `Bios` trait would carry over unchanged, so the move would be an implementation swap behind `NativeBios`. None of it is built.

## Try it yourself

These commands boot the kernel in its `bios` test mode on QEMU's standard and Cirrus cards, check the transcript and the screenshots against the repository's references, and read the numbers this chapter quotes. They need NASM, a Rust toolchain and `qemu-system-i386` on the host (or the Docker fallbacks the scripts use). Output is from a run on 2026-10-23 with QEMU 11.1.2 at `44f0265`; the cost line varies by a few microseconds from run to run.

```try
needs: host: bash, nasm, cargo, qemu-system-i386, grep, tr
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh > /dev/null && tools/native-image.sh --test-mode bios --out dist/native-bios.img
dist/native-bios.img: 1474560 bytes, kernel 173968 bytes in 340 sectors, test mode bios, video text
$ tools/native-run.sh --timeout 90 --vga std --inject tests/native/bios.inject --shots dist/bios-std dist/native-bios.img > dist/bios-std.txt; echo "exit $?"
native-run: guest reported success
exit 0
$ tr -d '\r' < dist/bios-std.txt > dist/bios-std.clean; python3 -I tools/native-check.py tests/native/expected-bios.txt dist/bios-std.clean
transcript matches tests/native/expected-bios.txt (101 lines)
$ python3 -I tools/native-shot.py check tests/native/bios.sums dist/bios-std
native-shot: vbe-640 matches (640x480 84cd5f8b)
native-shot: vbe-800 matches (800x600 2c094efc)
native-shot: text matches (720x400 3eeee6dd)
$ grep -E "^bios: (INT 12h|INT 15h AX|INT 13h AH=08|keyboard: the call|the one-second|cost|stress: kernel)" dist/bios-std.clean
bios: INT 12h: 639 KiB of conventional memory
bios: INT 15h AX=E820 after boot: 6 entries, the loader's map had 6, identical: true
bios: INT 13h AH=08h drive 0x00: 80 cylinders, 2 heads, 18 sectors per track
bios: keyboard: the call returned, 1 byte(s) were waiting in the controller and were handed to the driver
bios: the one-second wait took 100 kernel ticks (the BIOS timer handler counted 100, undelivered ticks recovered 0)
bios: cost: 5000 INT 1Ah calls took 23 kernel ticks, about 46 microseconds each (emulated speed, not real hardware)
bios: stress: kernel ticks between three RTC seconds while calling INT 1Ah every tick: 100, 100 and 100
```

Now the same on the Cirrus card, whose VGA BIOS lists fewer modes and which has no Bochs registers for the old way of leaving VBE:

```try
needs: host: bash, nasm, cargo, qemu-system-i386, grep, tr
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-run.sh --timeout 90 --vga cirrus --inject tests/native/bios.inject --shots dist/bios-cirrus dist/native-bios.img > dist/bios-cirrus.txt; echo "exit $?"
native-run: guest reported success
exit 0
$ tr -d '\r' < dist/bios-cirrus.txt > dist/bios-cirrus.clean; python3 -I tools/native-check.py tests/native/expected-bios.txt dist/bios-cirrus.clean
transcript matches tests/native/expected-bios.txt (101 lines)
$ python3 -I tools/native-shot.py check tests/native/bios.sums dist/bios-cirrus
native-shot: vbe-640 matches (640x480 84cd5f8b)
native-shot: vbe-800 matches (800x600 2c094efc)
native-shot: text matches (720x400 3eeee6dd)
$ grep -E "^bios: VBE 4F00h" dist/bios-std.clean dist/bios-cirrus.clean
dist/bios-std.clean:bios: VBE 4F00h: version 3.0, 256 x 64 KiB of video memory, 93 modes listed, OEM "SeaBIOS VBE(C) 2011"
dist/bios-cirrus.clean:bios: VBE 4F00h: version 3.0, 64 x 64 KiB of video memory, 31 modes listed, OEM "SeaBIOS VBE(C) 2011"
```

The two cards list a different number of modes (93 and 31) and have a different amount of video memory, and the three screenshot checksums are the same. The `vmware` card gives the same transcript and the same screenshots as `std`.

**Exercises.** (1) Without running anything, work out `BIOS_PIC_MASK` as the two bytes written to the controllers if IRQ 1 were also left open, then say which line of the transcript you would expect to fail first, and why (the 8042 hand-off section is the hint), before trying it in a scratch branch. (2) Predict how many kernel ticks the `INT 15h AH=86h` wait takes if `CX:DX` is `0x0007A120` (500,000 microseconds), and what `the BIOS timer handler counted` would be. (3) The document guesses 100 µs for a trivial call on a 486 and says most of the cost is the 60 port accesses at about 1 µs each. Count the port accesses in `PIC.init` and `bios_int` (`hal/drivers/pic/src/lib.rs`) and say whether 60 is about right.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Trampoline
def: A small piece of code whose only job is to move the processor from one mode to another and back. The kernel's trampoline drops from 32-bit protected mode to real mode to run one BIOS interrupt and returns.

term: V86 mode
def: Virtual 8086 mode, an 80386 feature that runs real-mode code as a task under a protected-mode kernel. A monitor handles the faults of its sensitive instructions. The Desk designs it as a later upgrade to the trampoline and has not built it.

term: NMI
def: The non-maskable interrupt, which the processor delivers even when interrupts are disabled. On a PC it can also be masked at bit 7 of the CMOS index port 0x70, which the BIOS call does for its length.

term: Bounce buffer
def: A region of low memory the BIOS can reach, through which the kernel passes the structures and sectors a BIOS call reads and writes. The Desk's is 8 KiB at 0x11000.
```

```figures
id: m8-biscall
reuse: biscall
at: explainer biscall

id: m8-bios-cost
reuse: bios-cost
at: figure bios-cost
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every claim was checked against `kernel/bios16.asm`, `kernel/src/bios.rs`, `kernel/src/biostest.rs` and `hal/hal/src/bios.rs`, and the commands above were run on the host on QEMU's std and Cirrus cards (the vmware card was run too and matches). Points where the document and the code differ, none of which changes a claim: the document's sequence lists "interrupts off" among the checks of step 1, while the code does not check it but makes it so (`call` runs inside `without_interrupts`); the document says "about 47 µs" for 24 ticks where the kernel's own formula prints 48; and the comment at the top of `hal/hal/src/bios.rs` still says the native kernel "can offer the same trait from a V86 monitor or a real-mode trampoline, or not offer it", which was true before `NativeBios` existed.
