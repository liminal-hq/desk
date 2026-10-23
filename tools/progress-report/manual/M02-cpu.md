---
id: m-cpu
n: M2
part: I
title: The processor: real mode, protected mode and the A20 gate
summary: Segments and offsets, the 1 MiB limit and the A20 gate, descriptors and the GDT, the switch to protected mode, interrupts, and what a 486 lacks.
date: 2026-10-23
reviewed: 44f0265
watches: [boot/, kernel/src/gdt.rs, kernel/src/idt.rs, kernel/src/fpu.rs, kernel/i486-desk-none.json, kernel/bios16.asm, docs/NATIVE-BOOT.md, docs/N3-BIOS-PATH.md]
log: [boot, bios]
anatomy: [desk:boot, desk:kernel]
---
## What it is

A PC processor of the 486 era starts every boot as a very fast 8086. That is the first fact this chapter teaches and the reason the rest of the boot code looks the way it does. In that mode, [real mode](term:real-mode), the processor can reach one megabyte of memory, in windows of 64 KiB, with no protection of any kind, and the firmware's services (the BIOS) can be called. The Rust kernel needs the opposite: a flat 32-bit address space, access to all the memory the machine has, and a way to say which code may do what. The processor can do that too, in [protected mode](term:protected-mode), but only after a program tells it how and flips one bit. The loader ([M4](chapter:m-loader)) is the program that does so for the native track, and this chapter explains what it does and why in terms of the processor.

In the map of the system ([M1](chapter:m-map)) this is not a layer of code but the machine the boot box ([`desk:boot`](chapter:anatomydesk#box-desk-boot)) and the kernel box ([`desk:kernel`](chapter:anatomydesk#box-desk-kernel)) stand on. The chapter has three jobs: to teach how a real-mode address is made and why a megabyte was a ceiling; to teach the A20 gate, a piece of motherboard wiring that the loader has to test and sometimes open; and to teach what a descriptor table is, how the switch to protected mode works, and what the kernel does and does not use of the processor's protection (it uses almost none).

After the chapter you should be able to turn a `segment:offset` pair into a physical address by hand, say what the loader's A20 test does with two particular addresses and what each outcome means, decode an eight-byte descriptor, and read the `CR0` line of an exception dump. The devices the loader also talks to (the keyboard controller, the interrupt controllers) are in [M3](chapter:m-devices); the kernel's own tables and interrupts are in [M6](chapter:m-interrupts), and the x87 in [M7](chapter:m-memory). Nothing in this chapter was run on a real 486: the tests are in QEMU as a 486, and where the hardware's own behaviour matters the chapter says it comes from the documents and not from a measurement.

## How it works

### Real mode: a segment and an offset

The registers of an 8086 are 16 bits wide, which can name 64 KiB. Its address bus has 20 lines, which can reach 1 MiB. The two are reconciled with two registers per address: a [segment](term:segment) and an [offset](term:offset). The processor shifts the segment left by four bits (multiplies it by 16) and adds the offset, and the sum is the [linear address](term:linear-address), which in real mode is also the physical address that goes to the memory. So `07C0:0000` and `0000:7C00` are the same byte, `0x07C00`, which is why stage 1 of the loader has to make its segment registers agree before it trusts any address: the BIOS may enter it either way (`boot/stage1.asm`, "Make CS, DS, ES and SS agree on segment 0").

Many pairs name each byte, and the largest address a pair can make is not the top of the megabyte. The pair `FFFF:FFFF` is `0xFFFF0 + 0xFFFF = 0x10FFEF`, a little over `0xFFFFF`, which is the last byte of the first megabyte. What happens to the sum when it needs a twenty-first bit is the subject of the next section.

Everything the BIOS offers is reached in this mode, with a software interrupt. `INT 13h` reads the floppy, `INT 15h` describes memory, `INT 10h` talks to the display. The processor finds a handler through the **interrupt vector table**: 256 four-byte entries at physical address 0, each a `segment:offset` pair, so `INT n` loads the pair at `4 × n`. The table is 1,024 bytes, and that number appears in the code that returns from protected mode to real mode for a BIOS call, which loads an interrupt table with limit `0x3FF` and base 0 (`docs/N3-BIOS-PATH.md`, "The switching sequence", step 7).

The loader's own memory map shows how little room real mode leaves. The boot sector is at `0x7C00`, stage 2 follows it, the kernel is staged at `0x10000`, and the boot information the kernel will read is at `0x500` (`docs/NATIVE-BOOT.md`, "Image layout"). It is all below `0x100000`.

### The wrap-around and the A20 gate

An 8086 has no twenty-first address line, so an address that needs one loses the bit and lands at the bottom of memory: `FFFF:0010` is `0x100000` on paper and `0x00000` in the machine. Some programs of the period were written to depend on that. Later processors have more address lines, and with the extra line free `FFFF:0010` would reach the second megabyte instead. To keep the old programs working the PC/AT put a gate on the motherboard that holds the twenty-first line, called [A20](term:a20) (address line 20, counting from line 0), at zero until software asks for it to be released. A machine in that state is in "wrapped" mode, and every other megabyte of memory would alias the one below it if the gate stayed shut, which is the reason the loader, which wants memory above 1 MiB, has to open it (`boot/stage2.asm`, header comment).

> **From the stacks — Sixty-four kilobytes too many.** Real mode forms a 20-bit address by shifting a 16-bit segment left four bits and adding a 16-bit offset, so the largest address is 0xFFFF0 + 0xFFFF = 0x10FFEF, a little past 1 MB. According to the Wikipedia article on real mode, the 8086, 8088 and 80186 wrapped such an address around to the bottom of the address range, while the 80286 reached into the second megabyte instead. The article adds that some programs written before the 80286 were designed to take advantage of the wrap-around. It names none of those programs.
> Source: [Real mode, Wikipedia](https://en.wikipedia.org/wiki/Real_mode)

Stage 2 does not assume what state the gate is in. It tests it, with two addresses that are the same byte if and only if the gate is shut. The worked example below follows the test with the real numbers. If the gate is shut, the loader tries two ways to open it, and tests again after each (the test is repeated for a short while, because hardware may take a moment):

1. **The fast gate.** Bit 1 of the system control port at `0x92` opens A20. The loader sets bit 1 and clears bit 0, because bit 0 of the same port resets the processor. Not every chipset has the port, which is why the loader's comment calls the next method "the fallback for chipsets without port 0x92".
2. **The keyboard controller.** The AT's original gate was an output line of the keyboard controller (an Intel 8042). The loader disables the keyboard (command `0xAD` to port `0x64`), reads the controller's output port (`0xD0`, then a read of port `0x60`), writes it back with bit 1 set (`0xD1`, then a write to `0x60`), and enables the keyboard again (`0xAE`). Between the steps it waits for the controller to be ready, with a bound, so a dead controller cannot hang the boot.

> **From the stacks — A gate on the keyboard controller.** Wikipedia's article on the A20 line says that when IBM designed the PC AT (1984) it chose the Intel 80286, which did not hold A20 at zero in real mode, so programs relying on the wrap-around would no longer have worked. The article says IBM's remedy was a gate on the motherboard, originally connected to the Intel 8042 keyboard controller. It does not say why that controller was chosen. The Desk's loader still keeps that method as its fallback.
> Source: [A20 line, Wikipedia](https://en.wikipedia.org/wiki/A20_line)

A third way exists, a BIOS call (`INT 15h` function `2401h`), and the loader does not use it. `docs/NATIVE-BOOT.md` lists chipset-specific methods as unverified: the two the loader tries are the two that the emulators exercise, and a real machine whose chipset wants something else would make the loader print `stage2: cannot enable a20` and stop. QEMU opens the gate before the boot sector runs, so a normal boot prints `a20 already enabled`; the two opening paths run only in test builds that close the gate first on purpose (`-DA20_TEST=1` and `-DA20_TEST=2`, run by `tools/native-test.sh`).

### Descriptors, selectors and the GDT

Protected mode keeps the idea of a segment and changes what a segment register holds. In real mode the register holds the base shifted right by four. In protected mode it holds a [selector](term:selector), a 16-bit number that picks a [descriptor](term:descriptor) from a table. A descriptor is eight bytes that say where a segment starts (its base), how big it is (its limit), what it may be used for and who may use it. The processor reads the descriptor once, when the register is loaded, and keeps a copy that the program cannot see; every later access through the register uses the copy.

The table is the [GDT](term:gdt), the global descriptor table, and the processor learns where it is from the `lgdt` instruction, which loads a six-byte pointer: a 16-bit limit (the table's size in bytes, minus one) and a 32-bit base. The loader's pointer is `gdt_ptr`, with limit `3 × 8 − 1 = 23` (`0x17`).

A selector has three fields. Bits 3 to 15 are the index of the descriptor, bit 2 chooses between the global table and a local one (the Desk uses only the global), and bits 0 and 1 are the requested privilege level. So selector `0x08` is index 1, global table, privilege 0, and `0x10` is index 2. The first descriptor of a GDT, index 0, must be a null descriptor, and a null selector may be loaded into a data segment register but any access through it faults.

The eight bytes of a descriptor are split across the bytes in an awkward order (the base and the limit are each in pieces), and the table below gives the pieces:

| Bytes | Holds |
|---|---|
| 0 and 1 | Limit, bits 0 to 15 |
| 2, 3 and 4 | Base, bits 0 to 23 |
| 5 | Access byte: present (bit 7), privilege level (bits 5 and 6), descriptor type (bit 4, 1 for code or data), and four type bits |
| 6 | Limit bits 16 to 19 (low nibble) and the flags (high nibble): granularity, default operand size, long mode, available |
| 7 | Base, bits 24 to 31 |

For a code segment, the four type bits are executable (bit 3) set, conforming, readable and accessed; for data, executable clear, direction, writable and accessed. The granularity flag decides whether the limit counts bytes or 4 KiB units, and the D/B flag says the segment's default operand size is 32 bits. The worked example below decodes the loader's two segments with this table.

The loader's three descriptors make the simplest useful model, called **flat**: the null descriptor, a code segment and a data segment, both starting at 0 and spanning all 4 GiB. With a base of 0, a segment-relative address is the physical address, and Rust code can treat pointers as plain 32-bit numbers. The kernel builds an identical table of its own as soon as it starts, because the loader's lives in memory the kernel may overwrite (`kernel/src/gdt.rs`).

### The switch

Entering protected mode is four instructions, in this order (`boot/stage2.asm`, "Protected mode"):

1. `cli`. There is no interrupt table for protected mode yet, and the real-mode one would be read as a protected-mode table if an interrupt arrived. Interrupts were on until here: stage 1 turns them on after setting its stack (`sti`), and nothing turns them off before this point.
2. `lgdt [gdt_ptr]`.
3. Set bit 0 of [CR0](term:cr0) (`mov eax, cr0`, `or eax, 1`, `mov cr0, eax`). CR0 is a control register, a 32-bit register of machine-wide switches, and bit 0, `PE` (protection enable), is the one that changes the mode. The processor is now in protected mode, but `CS` still holds the old real-mode value and the processor has already fetched some instructions in 16-bit mode.
4. `jmp 0x08:pm_entry`. A far jump loads `CS` through selector `0x08`, which names the 32-bit code descriptor, and discards the instructions already fetched. From the next instruction the processor is executing 32-bit code.

`pm_entry` is assembled for 32-bit mode (`bits 32`). It loads selector `0x10` into `DS`, `ES`, `FS`, `GS` and `SS`, sets a stack at `0x7000`, copies the kernel from `0x10000` to `0x100000`, and jumps to it ([M4](chapter:m-loader)).

Going back is possible and the kernel does it, for one BIOS call at a time (`docs/N3-BIOS-PATH.md`, "The switching sequence", and [M8](chapter:m-bioscalls)). The route out of protected mode passes through 16-bit protected-mode segments with 64 KiB limits first, because the descriptor copy that the processor keeps in each segment register survives the clearing of `PE`: a data segment left with a 4 GiB limit would carry into real mode, which works on most processors and is not what the manuals promise. The kernel's trampoline table therefore has two more descriptors, 16-bit code at `0x18` and data at `0x20`, with the kernel's two flat selectors at the same numbers as before.

### Privilege levels: rings

The two bits of privilege in a descriptor's access byte, and in a selector, name four [rings](term:ring) numbered 0 to 3. Ring 0 is the most privileged. The rule the processor enforces is that code running at one ring may not load a segment, or call through a gate, that belongs to a more privileged ring without the gate allowing it; and ring 0 is the only one that may execute the instructions that load descriptor tables and control registers.

The Desk uses one ring. Both loader descriptors have access bytes with the two privilege bits clear (`0x9A` and `0x92`), the kernel's table is the same two numbers (`kernel/src/gdt.rs`), and the kernel's interrupt gates are all `0x8E`: present, privilege 0, a 32-bit interrupt gate. Nothing in the kernel creates a task-state segment, a call gate or a code descriptor with a lower privilege; the "Try it yourself" section shows the search. That is why the anatomy page can say that system calls and tasks are planned ([M22](chapter:m-frontier)): there is no ring 3 to call from. It is a fact about the code and not a gap in the documents.

The hosted track runs in a different place. A DOS program under a DPMI host such as CWSDPMI is an ordinary client of the host and runs at a lower privilege than the host, and the project's own spike recorded that port writes to the VGA's palette from that ring worked without a fault under CWSDPMI (`docs/A-VGA-VBE.md`). The native track has nothing like that yet.

### Interrupts and exceptions in both modes

The processor raises an **exception** when an instruction cannot proceed (divide by zero, a bad opcode, a general protection fault, 32 kinds in all, vectors 0 to 31) and takes an **interrupt** when a device asks, through the interrupt controllers ([M3](chapter:m-devices)). Both end in a handler found by number. In real mode the number indexes the vector table described above. In protected mode it indexes the **IDT**, the interrupt descriptor table, whose entries are 8-byte gates: a code selector, an offset, and flags that include the privilege level and the kind of gate. The kernel's `Gate` struct and its `gate()` function build them, all with selector `0x08` and flags `0x8E`, which also makes the processor clear the interrupt flag on entry. There are 256 gates in the table and 32 exception stubs plus the timer's are filled in; the rest are marked missing.

Some exceptions push an error code after the usual return information. The kernel's stubs push a dummy zero for those that do not, so every frame looks the same; the mask `0x60227D00` in `kernel/src/idt.rs` lists the vectors that do push one: 8, 10 to 14, 17, 21, 29 and 30. When the kernel loads a selector beyond its table, the processor raises a general protection fault (vector 13) and the error code is the selector that caused it, which the worked example uses. Everything after the vector number is the subject of [M6](chapter:m-interrupts).

### The x87 and the CR0 bits

Control register CR0 carries more than `PE`. These are the bits that appear in this repository:

| Bit | Name | The kernel's setting | Meaning |
|---|---|---|---|
| 0 | PE | set (by the loader) | Protected mode on |
| 1 | MP | set | `wait` respects the task-switched bit |
| 2 | EM | cleared | Emulate the FPU: trap x87 instructions if set |
| 3 | TS | cleared | Task switched: trap x87 instructions after a task switch if set |
| 4 | ET | set | The type of coprocessor; reads as set on a 486 |
| 5 | NE | set | x87 errors raise exception 16 rather than the old external wiring |
| 29, 30 | NW, CD | clear | Cache write-through and cache disable |
| 31 | PG | clear | Paging; the kernel does not use it |

`kernel/src/fpu.rs` edits only the x87 bits: it clears `EM` and `TS` and sets `MP` and `NE`, then runs `fninit`. `docs/NATIVE-BOOT.md` ("The FPU") notes that some 486SX machines have no FPU and that this kernel assumes one. The value the exception dump prints, `CR0 0x00000033`, is `PE | MP | ET | NE`: `1 + 2 + 16 + 32 = 51 = 0x33`. The expected transcripts accept the cache bits either way, `{BITS:0x9fffffff=0x00000033}`, because a BIOS leaves them as it likes.

### What a 486 lacks

The kernel target says it plainly (`kernel/i486-desk-none.json`): `cpu` is `i486`, `features` is `-mmx,-sse,-sse2`, and `max-atomic-width` is 32. A 486 has no MMX or SSE, so the compiler must not use them; the floating-point maths that `fpu::demo` does is compiled to x87 instructions. It has no `CMOV`, which arrived with the Pentium Pro, and no 8-byte compare-and-exchange (`CMPXCHG8B`), so nothing wider than 32 bits is atomic. A 486 does have a 32-bit `CMPXCHG`, but the shared code avoids it anyway, because DOSBox's 486 model rejects it ([M1](chapter:m-map); `docs/HAL.md`, "Decisions"). The `CPUID` instruction appeared on some late 486s and not on others, and nothing in `kernel/src` uses it.

How well QEMU enforces this is described unevenly. `docs/NATIVE-BOOT.md` says `-cpu 486` rejects `CMOV` and `CMPXCHG8B` but not `RDTSC`, `RDMSR`, MMX or a CR4 access, which the Bochs 486 does; a comment in `tools/native-run.sh` says the option keeps the guest to "no CPUID, no TSC, no MMX or SSE". The two disagree about TSC and MMX and this chapter did not test either; the safe reading is that the target file is the real limit and the emulator is a second line of defence.

## Why it is this way

- **Test the gate, do not assume it.** QEMU leaves A20 open; a real BIOS may or may not, and the 86Box run on a 1993 AMIBIOS is the reminder that BIOSes differ ([`docs/NATIVE-EMULATORS.md`](doc:docs/NATIVE-EMULATORS.md#what-the-emulators-exposed)). The test costs a few instructions and needs no knowledge of the chipset. The loader tests first, then the fast gate, then the keyboard controller, with each step bounded.
- **The fast gate before the keyboard controller.** Port `0x92` is a single write; the controller sequence is eight steps with waits. The loader tries the cheap one first and keeps the AT's original method as the fallback for chipsets that lack the port. The bit that must stay clear, bit 0 of port `0x92`, resets the machine, so the code explicitly clears it.
- **Flat segments.** A flat model removes segments from the programmer's world: a pointer is an address, a `Vec` is a base and a length, and the compiler needs no memory model. The cost is that nothing is protected, which `docs/DIRECTION.md` lists as an open question ("Memory protection").
- **The kernel reloads its own GDT.** The loader's table is in loader memory that the kernel is free to overwrite, so `gdt::init` loads an identical one that lives in the kernel and reloads every segment register with a far return (`kernel/src/gdt.rs`).
- **One ring.** The kernel runs as one program in one address space; adding a ring 3 would need a task-state segment, a call gate or a software-interrupt gate with a lower privilege, and a program to run. None exists, and the open questions in `docs/DIRECTION.md` (scheduling, memory protection, app isolation) decide whether any will.
- **The FPU is set up by the kernel, not assumed.** The loader leaves `CR0` with only `PE` of its own choosing, and the BIOS may leave `EM` or `TS` set. `fpu::init` makes the state explicit and `fpu::demo` asserts the control word `0x037F` that `fninit` leaves, which also shows there is a working FPU.
- **486 instructions only.** The target is the oldest machine the project aims at (`docs/DIRECTION.md`, "The vision"), so a newer instruction is a bug that would pass on a modern machine and fail on the target.

## The code

| Where | What it holds |
|---|---|
| [`boot/stage2.asm`](repo:boot/stage2.asm) | [`a20_enabled`](sym:boot/stage2.asm::a20_enabled) is the test, [`a20_wait`](sym:boot/stage2.asm::a20_wait) repeats it, [`a20_keyboard`](sym:boot/stage2.asm::a20_keyboard) is the AT method; the fast gate is the `in al, 0x92` block of `entry`; [`gdt`](sym:boot/stage2.asm::gdt), [`gdt_ptr`](sym:boot/stage2.asm::gdt_ptr) and [`pm_entry`](sym:boot/stage2.asm::pm_entry) are the table, its pointer and the 32-bit entry |
| [`boot/stage1.asm`](repo:boot/stage1.asm) | [`entry`](sym:boot/stage1.asm::entry) makes the segment registers agree on segment 0 and sets the stack: the real-mode set-up that precedes everything |
| [`kernel/src/gdt.rs`](repo:kernel/src/gdt.rs) | [`GDT`](sym:kernel/src/gdt.rs::GDT) (three `u64`s), the selector constants [`KERNEL_CODE`](sym:kernel/src/gdt.rs::KERNEL_CODE) and [`KERNEL_DATA`](sym:kernel/src/gdt.rs::KERNEL_DATA), and [`init`](sym:kernel/src/gdt.rs::init), which loads the table and reloads the segment registers |
| [`kernel/src/idt.rs`](repo:kernel/src/idt.rs) | [`Gate`](sym:kernel/src/idt.rs::Gate), [`gate`](sym:kernel/src/idt.rs::gate) (flags `0x8E`), the 256-entry table, and [`exception_handler`](sym:kernel/src/idt.rs::exception_handler), which prints the vector name and `CR0` |
| [`kernel/src/fpu.rs`](repo:kernel/src/fpu.rs) | [`init`](sym:kernel/src/fpu.rs::init) edits the x87 bits of CR0; [`demo`](sym:kernel/src/fpu.rs::demo) checks the control word and two results |
| [`kernel/i486-desk-none.json`](repo:kernel/i486-desk-none.json) | The target: `i486`, no MMX or SSE, atomics no wider than 32 bits |
| [`kernel/bios16.asm`](repo:kernel/bios16.asm) | [`tramp_gdt`](sym:kernel/bios16.asm::tramp_gdt), the five-entry table of the BIOS trampoline, with its two 16-bit descriptors |

For depth: `docs/NATIVE-BOOT.md` ([boot flow](doc:docs/NATIVE-BOOT.md#boot-flow), [the kernel target](doc:docs/NATIVE-BOOT.md#the-kernel-target) and [the FPU](doc:docs/NATIVE-BOOT.md#the-fpu)) and `docs/N3-BIOS-PATH.md` ([the switching sequence](doc:docs/N3-BIOS-PATH.md#the-switching-sequence)).

## Worked example

**The A20 test, byte by byte.** `a20_enabled` points `DS` at segment 0 and `ES` at segment `0xFFFF` (it loads 0, then `not ax`). It reads the byte at `0000:0500` and the byte at `FFFF:0510` so it can put them back, writes `0x00` to the first and `0xFF` to the second, and then reads the first one. The two addresses:

| Pair | Arithmetic | Physical address |
|---|---|---|
| `0000:0500` | `0x0000 × 16 + 0x0500` | `0x00500` |
| `FFFF:0510` | `0xFFFF × 16 + 0x0510 = 0xFFFF0 + 0x510` | `0x100500` |

The second address is one megabyte plus `0x500`: it needs bit 20, and it is the same as the first with bit 20 added. With A20 held at zero the bit is lost, `0x100500` becomes `0x00500`, and the two pairs are one byte. The write of `0xFF` came second, so the read of the first location returns `0xFF` and the gate is **closed**. With the gate open the two are different bytes, the first still holds `0x00`, and the gate is **open**. The comparison `cmp byte [ds:si], 0xFF` sets the zero flag exactly in the closed case, and the routine returns zero-set for "closed" and zero-clear for "open". The original bytes are put back whichever way it goes, because the test borrows memory it does not own: `0x500` is the address stage 2 uses for the boot information block, which it writes just after. The "Try it yourself" section prints the two physical addresses.

**The loader's three descriptors.** The table in `boot/stage2.asm` is `dq 0`, then `0x00CF9A000000FFFF` for code and `0x00CF92000000FFFF` for data. In memory a descriptor is little-endian, so the code descriptor is the bytes `FF FF 00 00 00 9A CF 00`:

| Byte | Value | Field |
|---|---|---|
| 0 and 1 | `FF FF` | Limit, bits 0 to 15: `0xFFFF` |
| 2, 3 and 4 | `00 00 00` | Base, bits 0 to 23: 0 |
| 5 | `9A` = `1001 1010` | Present (1), privilege 00 (ring 0), type bit 1 (code or data), then executable (1), conforming (0), readable (1), accessed (0) |
| 6 | `CF` | Flags `1100`: granularity 1 (4 KiB units), D/B 1 (32-bit), long mode 0, available 0; and limit bits 16 to 19: `0xF` |
| 7 | `00` | Base, bits 24 to 31: 0 |

The full limit is `0xFFFFF` units of 4 KiB: `(0xFFFFF + 1) × 4096 = 4 GiB`, so the last valid offset is `0xFFFFFFFF`. The data descriptor differs in byte 5 only: `0x92` = `1001 0010`, which is present, ring 0, data, executable clear, direction clear (grows up), writable (1), accessed clear. The null descriptor is eight zero bytes. The three are numbered by their offsets in the table: index 0 at offset 0 (selector `0x00`), index 1 at offset 8 (selector `0x08`, the code segment `jmp 0x08:pm_entry` uses) and index 2 at offset 16 (selector `0x10`, the data segment). The table is 24 bytes, so the pointer's limit is 23.

**A selector the table does not have.** The `gp` test mode loads the selector `0x28` into a segment register. `0x28` is index 5 (`0x28 ÷ 8`), global table, ring 0. The table has indices 0 to 2, so index 5 is past its limit and the processor raises a general protection fault with the selector as the error code. The transcript the expected file `tests/native/expected-gp.txt` checks says `EXCEPTION 0x0d: general protection fault` and `error code 0x00000028`, and the dump's `CS` is `0x0008`, the kernel's own code selector with privilege 0. [M5](chapter:m-boot) has the whole transcript.

## State of this layer

"Verified" means that a test runs in `tools/check.sh` or a merged pull request passed its CI, in QEMU as a 486. It never means a real machine.

```status
id: cpu.a20test
label: The loader tests the A20 gate with 0000:0500 and FFFF:0510
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::a20 already enabled, sym:boot/stage2.asm::a20_enabled
note: QEMU opens the gate first, so the normal boot only ever sees the open case; the closed case runs in the test builds below.

id: cpu.a20open
label: The gate is opened by the fast gate (port 0x92) or the keyboard controller
state: built-verified
evidence: test:tools/native-test.sh::a20-fast, test:tools/native-test.sh::a20-keyboard, sym:boot/stage2.asm::a20_keyboard
note: Run only in the test builds that close the gate on purpose. QEMU's models of the two controls accept the writes; how a real chipset reacts is from the documents.

id: cpu.a20bios
label: Opening the gate with INT 15h function 2401h, or a chipset-specific method
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
note: The loader prints cannot enable a20 and stops if neither of its two methods works.

id: cpu.switch
label: Flat GDT, CR0.PE and the far jump to 32-bit code, then the copy to 1 MiB
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::gdt: loaded, sym:boot/stage2.asm::pm_entry
note: Every boot test passes through it. The loader's table itself is not printed; the kernel's line proves the switch worked.

id: cpu.gdt
label: The kernel's own flat GDT and the reload of the segment registers
state: built-verified
evidence: test:tests/native/expected.txt::gdt: loaded, sym:kernel/src/gdt.rs::init

id: cpu.idt
label: A 256-gate IDT, 32 exception stubs and the exception dump
state: built-verified
evidence: test:tests/native/expected-div0.txt::EXCEPTION 0x00: divide error, test:tests/native/expected-gp.txt::error code 0x00000028, test:tests/native/expected-ud.txt::EXCEPTION 0x06: invalid opcode, sym:kernel/src/idt.rs::exception_handler

id: cpu.cr0
label: CR0 is 0x33 in the kernel: PE, MP, ET and NE set, paging off
state: built-verified
evidence: test:tests/native/expected-div0.txt::0x00000033, test:tests/native/expected-gp.txt::0x00000033, sym:kernel/src/fpu.rs::init

id: cpu.fpu
label: The x87 is enabled and works: control word 0x037F, sqrt and a harmonic sum
state: built-verified
evidence: test:tests/native/expected.txt::fpu: results check out, sym:kernel/src/fpu.rs::demo
note: A kernel that assumes an FPU is present; a 486SX has none.

id: cpu.ring0
label: Everything runs at ring 0 with flat segments and 0x8E gates
state: built-verified
evidence: test:tests/native/expected-div0.txt::CS 0x0008, sym:kernel/src/gdt.rs::KERNEL_CODE, sym:kernel/src/idt.rs::gate
note: The dump shows the kernel's code selector with privilege 0; that nothing creates a lower ring is a search of kernel/src, not a test.

id: cpu.i486
label: The kernel is compiled for an i486 with no MMX, SSE or atomics wider than 32 bits
state: built-verified
evidence: sym:kernel/i486-desk-none.json::i486, test:tests/native/expected.txt::boot complete, sym:tools/native-run.sh::-cpu 486
note: It boots under QEMU's 486 model. What that model rejects is described unevenly in the documents, as the chapter says; no real 486 has run it.

id: cpu.usermode
label: Ring 3, a task-state segment, call gates or system calls
state: planned
evidence: doc:docs/DIRECTION.md#open-questions

id: cpu.paging
label: Paging and memory protection
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#paging, doc:docs/DIRECTION.md#open-questions

id: cpu.hardware
label: Any of this on a real 486 or a real chipset
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The A20 test, the switch and the kernel's tables are {{state.cpu.switch}} in QEMU. The two ways of opening the gate are {{state.cpu.a20open}} only in builds that close it first. Everything about real chipsets is {{state.cpu.hardware}}.

## Try it yourself

These commands print the arithmetic of the chapter, then build the loader with the gate closed on purpose so each way of opening it runs, and show the exception dump the general protection fault makes. They need NASM, a Rust toolchain and `qemu-system-i386` (or the Docker fallbacks the scripts use). The output is from a run on 2026-10-23 with NASM 3.02 and QEMU 11.1.2 at `44f0265`.

```try
needs: host: python3 (the arithmetic only)
ran: 2026-10-23 on the host
$ python3 -c "print(hex(0x0000*16+0x500), hex(0xFFFF*16+0x510), hex((0xFFFF*16+0x510) & ~0x100000), hex(0xFFFF*16+0xFFFF))"
0x500 0x100500 0x500 0x10ffef
$ python3 -c "import re; [print(hex(d), 'base', hex(((d>>16)&0xFFFFFF)|((d>>56)<<24)), 'limit', hex((d&0xFFFF)|(((d>>48)&0xF)<<16)), 'access', hex((d>>40)&0xFF), 'flags', hex((d>>52)&0xF)) for d in (int(x,16) for x in re.findall(r'dq (0x[0-9A-F]+)', open('boot/stage2.asm').read()))]"
0xcf9a000000ffff base 0x0 limit 0xfffff access 0x9a flags 0xc
0xcf92000000ffff base 0x0 limit 0xfffff access 0x92 flags 0xc
```

The first line is the two A20 test addresses, the same address with bit 20 cleared (what a closed gate does), and the largest real-mode address. The second reads the loader's own source and decodes the two descriptors it finds, so if the table were changed it would show it.

```try
needs: host: bash, nasm, cargo, qemu-system-i386
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh --nasm-flags "-DA20_TEST=1" > /dev/null && tools/native-image.sh --out dist/a20f.img > /dev/null && tools/native-run.sh --serial dist/a20f.txt dist/a20f.img > /dev/null; tr -d '\r' < dist/a20f.txt | sed -n 2,3p
native-run: guest reported success
stage2: loader running
stage2: a20 enabled by the fast gate
$ tools/native-build.sh --nasm-flags "-DA20_TEST=2" > /dev/null && tools/native-image.sh --out dist/a20k.img > /dev/null && tools/native-run.sh --serial dist/a20k.txt dist/a20k.img > /dev/null; tr -d '\r' < dist/a20k.txt | sed -n 2,3p
native-run: guest reported success
stage2: loader running
stage2: a20 enabled by the keyboard controller
$ tools/native-build.sh
dist/native/kernel.bin: 173968 bytes, load address 0x100000
$ tools/native-image.sh --test-mode gp --out dist/n-gp.img > /dev/null; tools/native-run.sh dist/n-gp.img 2>&1 | tr -d '\r' | grep -E "EXCEPTION|error code|CS |CR0"
EXCEPTION 0x0d: general protection fault
error code 0x00000028
EIP 0x0010d0dc  CS 0x0008  EFLAGS 0x00010212
CR0 0x00000033  CR2 0x00000000  CR3 0x00000000
$ tools/native-image.sh --out dist/n.img > /dev/null; tools/native-run.sh --serial dist/n.txt dist/n.img > /dev/null 2>&1; tr -d '\r' < dist/n.txt | grep -E "^gdt|^fpu"
gdt: loaded, flat 4 GiB code and data segments
fpu: control word 0x037f, sqrt(2) = 1.414214, H(10) = 2.928968
fpu: results check out
$ grep -rniE "ltr|tss|sysenter|cpuid|cmov" kernel/src | grep -v "invalid TSS" | wc -l
0
```

The first two builds close the gate before the test, once so the fast gate runs and once so it is skipped and the keyboard controller runs; the third command puts the normal loader back. The last command is the search behind the claim that the kernel uses one ring: it looks for the task register, a task-state segment, the system-call instructions, `CPUID` and `CMOV` in the kernel's sources and finds nothing, apart from the name of exception 10 in the table of exception names (filtered out). It is a text search, not a proof, and it would miss code built by a macro.

**Exercises.** (1) Without running anything, say what `FFFF:0010` and `FFFF:0020` are in a machine with the gate closed, then with it open. (2) Decode, by hand, a data descriptor with base `0x00100000`, limit `0xFFFF` in bytes, ring 0, read and write, 16-bit, and write down its eight bytes; then say what `mov ds, ax` with that selector would let a program reach. (3) In a scratch branch, change the byte `0x92` in the data descriptor to `0x9A` and predict which line of the transcript the kernel prints last, and why, before running `tools/native-test.sh`.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Real mode
def: The mode a PC processor starts in: 16-bit registers, addresses formed from a segment and an offset, one megabyte reachable, no protection, and the BIOS services available.

term: Protected mode
def: The processor mode in which segment registers hold selectors that name descriptors, memory can be 32-bit and flat, and privilege levels are enforced. The loader enters it by setting CR0.PE and jumping through a code selector.

term: Segment
def: In real mode, a 16-bit number that, shifted left four bits, gives the base of a 64 KiB window. In protected mode, a region described by a descriptor.

term: Offset
def: The 16-bit or 32-bit number added to a segment's base to reach a byte inside it.

term: Linear address
def: The address a segment and an offset produce: base plus offset. In real mode it is the physical address; with paging off in protected mode it is too.

term: A20
def: Address line 20 of the bus. On the PC/AT a gate holds it at zero so that addresses wrap at 1 MiB as they did on the 8086, until software opens the gate by the keyboard controller or the fast gate on port 0x92.

term: Descriptor
def: Eight bytes in a descriptor table that describe one segment: its base, limit, type, privilege level and size flags.

term: Selector
def: The 16-bit value in a segment register in protected mode: an index into a descriptor table, a table bit and a requested privilege level.

term: GDT
def: The global descriptor table, the array of descriptors that the lgdt instruction points the processor at. The Desk's has three entries: null, flat code at 0x08 and flat data at 0x10.

term: CR0
def: Control register 0, a 32-bit register of processor-wide switches. Bit 0 (PE) turns on protected mode; bits 1 to 5 govern the x87; bit 31 turns on paging.

term: Ring
def: One of four privilege levels, 0 to 3, held in descriptors and selectors. Ring 0 is the most privileged. The Desk's kernel runs everything at ring 0.
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The A20 addresses and the descriptor decoding were checked by running them (see the commands above), and the loader's A20 paths were run in QEMU. Disagreements found: `docs/NATIVE-BOOT.md` says of the switch that "interrupts stay off throughout", but stage 1 turns them on (`sti`) and stage 2 turns them off only just before `lgdt`; a comment in `tools/native-run.sh` says `-cpu 486` leaves the guest no TSC or MMX, where `docs/NATIVE-BOOT.md` says QEMU's model does not reject `RDTSC`, `RDMSR` or MMX (not tested here); and `docs/HAL.md` says the port I/O from ring 3 under CWSDPMI is not yet verified, where `docs/A-VGA-VBE.md` records it working.
