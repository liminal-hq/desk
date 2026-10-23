---
id: m-interrupts
n: M6
part: III
title: The kernel: tables, interrupts and the timer
summary: The kernel's own GDT, the 256-gate IDT and its assembly stubs, the exception dump, the remapped PICs, the IRQ registry, end-of-interrupt, the spurious lines, the PIT at 100 Hz and the bounded first-tick spin.
date: 2026-10-23
reviewed: 44f0265
watches: [kernel/src/gdt.rs, kernel/src/idt.rs, kernel/src/pic.rs, kernel/src/pit.rs, kernel/src/hal.rs, kernel/src/sync.rs, hal/hal/src/irq.rs, hal/drivers/pic/, hal/drivers/pit/, tests/native/]
log: [boot, hal]
anatomy: [desk:kernel]
---
## What it is

This chapter covers the part of the kernel that makes the machine safe to run on: the tables that tell the processor what a segment is and where to go when something happens, and the two chips that make things happen on a schedule. After the loader ([M4](chapter:m-loader)) hands over, the kernel's first acts in `kmain` are to load its own [GDT](term:gdt), build its [IDT](term:idt), remap the interrupt controllers and start the timer. Each prints one line (`gdt:`, `idt:`, `pic:`, `pit:`, in the transcript of [M5](chapter:m-boot)), and each is a precondition for everything later: without the IDT a fault resets the machine without a word, and without the timer nothing can wait.

On the map of the system ([M1](chapter:m-map)) this is the lower half of the `kernel` box of the native track, where the anatomy page lists it. The processor's rules for tables and exceptions are in [M2](chapter:m-cpu) and the chips' registers and ports in [M3](chapter:m-devices); this chapter does not repeat them, and says only what the kernel does with them. The drivers for the two chips are shared with the DOS track and are the subject of [M10](chapter:m-sysdrivers), and the traits they implement are in [M9](chapter:m-hal). What lives here is the platform's side: the stubs, the table of handlers, the order of the steps and the rule that makes shared state safe.

After the chapter you should be able to say what each of the four set-up lines does and why they come in that order, follow an [exception](term:exception) from the faulting instruction to the dump and the exit status, follow a hardware interrupt from the chip to a handler and back, say why the end-of-interrupt is sent by the kernel and not by the driver, and explain why a single `cli` is enough to protect a data structure on this machine and what would make it stop being enough.

## How it works

### The kernel's own GDT

The loader left a GDT in use, but it lives in the loader's memory, which the kernel is free to overwrite ([M4](chapter:m-loader)). [`gdt::init`](sym:kernel/src/gdt.rs::init) therefore loads a table of its own: a static array of three 64-bit [descriptors](term:descriptor), the null one and the same two the loader used, `0x00CF9A000000FFFF` for code and `0x00CF92000000FFFF` for data (base 0, limit `0xFFFFF` in 4 KiB pages, which is 4 GiB, 32-bit, [ring](term:ring) 0). The decoding is in [M2](chapter:m-cpu); the kernel's copy is the same two numbers. The [selectors](term:selector) are the constants `KERNEL_CODE = 0x08` and `KERNEL_DATA = 0x10`, the byte offsets of the two descriptors. The table is 24 bytes, so the limit given to `lgdt` is 23, which is `0x17`.

Loading the table is not enough, because the segment registers still hold copies of the old descriptors. `init` does four things in one block of assembly. It runs `lgdt`. It pushes the code selector and the address of the next instruction and executes `retf`, a far return, which is how a 32-bit program reloads `CS` (there is no `mov` to `CS`). It then moves `0x10` through `AX` into `DS`, `ES`, `FS`, `GS` and `SS`. The print that follows, `gdt: loaded, flat 4 GiB code and data segments`, is the first line the transcript owes to this chapter. Because every segment is flat and based at 0, an address in the kernel is its linear address, and the rest of the kernel never thinks about segments again.

### The 256-gate IDT and its stubs

The [IDT](term:idt) is the table the processor consults when an exception or an interrupt arrives: 256 entries of eight bytes, indexed by the [interrupt vector](term:interrupt-vector), a number from 0 to 255. Each entry is a **gate**, and the kernel's are all [interrupt gates](term:interrupt-gate): flags `0x8E`, which is present, privilege 0, 32-bit interrupt gate, and the kind that clears the processor's interrupt flag on entry. The [`Gate`](sym:kernel/src/idt.rs::Gate) struct is the layout (the low 16 bits of the handler's address, the code selector, a zero byte, the flags byte, the high 16 bits), and [`gate`](sym:kernel/src/idt.rs::gate) fills one in with selector `0x08`. The table is a `static mut` array of 256, every entry first set to `MISSING`, all zeros, whose present bit is clear. The limit given to `lidt` is 256 × 8 − 1 = 2,047, `0x7FF`.

[`idt::init`](sym:kernel/src/idt.rs::init) fills only vectors 0 to 31 (the processor's own exceptions) and vector `0x20` (the timer), then loads the table. The gates point to stubs, and the stubs are written once by an assembler macro at the top of `idt.rs`. For each exception vector `n` the macro makes a stub `isr<n>` that does at most three things: push a zero if the processor did not push an error code, push `n`, and jump to the shared tail `isr_common`. Which vectors push an error code is a mask, `0x60227D00`, whose set bits are 8, 10 to 14, 17, 21, 29 and 30; the macro tests `(1 << n) & mask` at assembly time. The effect is that every frame has the same shape, whatever the exception was. The thirty-two stub addresses are collected, by the same macro, into a read-only array `isr_table` that `init` reads.

`isr_common` runs `pushad` (all eight general registers), `cld` (so that string instructions count up, as the compiler assumes), pushes `ESP` as the argument and calls [`exception_handler`](sym:kernel/src/idt.rs::exception_handler). The handler takes a reference to a [`Frame`](sym:kernel/src/idt.rs::Frame), a struct laid out in the order the stack holds things: the eight registers as `pushad` leaves them, the vector, the error code, and then what the processor pushed (`EIP`, `CS`, `EFLAGS`). The handler never returns, and the assembly after the call is only a `cli; hlt` loop in case it ever did.

### The exception dump

`exception_handler` looks the vector up in `NAMES`, a table of 32 strings from `divide error` to `security exception` (with `reserved` where the processor defines nothing), and prints, to the serial port and the screen, a fixed set of lines: `EXCEPTION 0x0d: general protection fault`, the error code, `EIP`, `CS` and `EFLAGS`, the general registers in two lines, and `CR0`, `CR2` and `CR3`. It ends with `halting with a failure status` and calls [`exit::failure`](sym:kernel/src/exit.rs::failure), which writes `0x11` to the [debug-exit port](term:debug-exit-port) so that QEMU ends the run with status 35 and `native-run.sh` returns 1 ([M5](chapter:m-boot)).

Two details of the dump are easy to misread. The `ESP` it prints is the value `pushad` stored, which is the stack pointer at the moment the stub began pushing, not the program's: the processor had already pushed `EFLAGS`, `CS` and `EIP` (twelve bytes, because there is no change of ring and so no `SS` or `ESP`), and the stub had pushed the error code and the vector (eight more), so it is 20 bytes below the interrupted program's. And `EFLAGS` is the value from before the gate cleared the interrupt flag, so a dump whose `EFLAGS` has bit 9 (`0x200`) set tells you that interrupts were on when the fault happened. In the recorded divide-by-zero dump it is `0x00010246`: the interrupt flag, the zero and parity flags, the always-set bit 1 and the resume flag, bit 16.

### Remapping the PICs, and why

The two 8259 chips ([M3](chapter:m-devices)) turn sixteen request lines into vector numbers, a base plus the line's number. The BIOS leaves them at `0x08` for IRQ 0 to 7 and `0x70` for IRQ 8 to 15, a choice made for the 8086, which had no reserved vectors. In protected mode vectors 0 to 31 belong to the processor. With the BIOS's bases the timer, IRQ 0, would arrive as vector `0x08`, which is the double fault, and IRQ 5 as `0x0D`, which is the general protection fault; a handler could not tell a device from a failure. [`pic::init`](sym:kernel/src/pic.rs::init) therefore calls the shared driver, `PIC.init(REMAPPED_BASES, 0xFFFE)`, with bases `0x20` and `0x28` (`REMAPPED_BASES` in `hal/drivers/pic/src/lib.rs`). IRQ 0 to 7 become vectors `0x20` to `0x27` and IRQ 8 to 15 become `0x28` to `0x2F`, directly after the exceptions and with no overlap.

The driver writes eight initialisation words, each followed by an `io_wait` (the sequence and its meaning are in [M3](chapter:m-devices)), and then the masks. The mask `0xFFFE` has a one for every line but IRQ 0: `0xFE` to the master and `0xFF` to the slave, which is why the transcript says `only IRQ0 unmasked`. A masked line cannot interrupt, so nothing can arrive for which the kernel has no gate yet, and the kernel opens a line only when it registers a handler for it.

> **From the stacks — Why the defaults collide.** Wikipedia's article on the Intel 8259 says that IBM PC compatibles map the master controller's IRQ 0 to 7 to interrupts `0x08` to `0x0F` and the slave's to `0x70` to `0x77`, although the first 32 vectors are reserved by the processor for internal exceptions. It adds that most operating systems that do not use the BIOS configure the controllers to avoid the reserved range entirely. The article's claims about spurious interrupts are used below only where the code states the same.
> Source: [Intel 8259, Wikipedia](https://en.wikipedia.org/wiki/Intel_8259)

### The IRQ registry in the native platform

The shared HAL declares what an interrupt is ([M9](chapter:m-hal)): a line number, [`Irq`](sym:hal/hal/src/irq.rs::Irq), from 0 to 15 (`Irq::new(16)` is an error), and a handler, [`IrqHandler`](sym:hal/hal/src/irq.rs::IrqHandler), which is a function pointer and a context pointer, never a closure, so that an assembly stub can call it with nothing else. The kernel provides the other half in `kernel/src/hal.rs`: [`IrqTable`](sym:kernel/src/hal.rs::IrqTable), a static array of sixteen `Option<IrqHandler>`, and the implementation of the registry trait for it.

[`register`](sym:kernel/src/hal.rs::register) refuses any `Chain` but `None` (the kernel is the only owner of its vectors, so there is no previous handler to chain to) and then, inside the critical section described below, does three things in this order: it stores the handler in the table, or fails with `Busy` if the slot is taken; it calls [`idt::set_irq_gate`](sym:kernel/src/idt.rs::set_irq_gate), which points vector `0x20 + line` at the assembly stub for that line; and only then it unmasks the line at the PIC, and for a slave line the cascade input (IRQ 2) as well, without which a slave line cannot reach the processor. The order is the safety argument: the gate is complete before the line can fire. `unregister` empties the slot and masks the line; it does not clear the gate, which is harmless because a masked line sends nothing.

The sixteen stubs for hardware lines are made by a second macro in `idt.rs`. Each is `pushad`, `cld`, push the line number, `call irq_dispatch`, drop the argument, `popad`, `iretd`. [`irq_dispatch`](sym:kernel/src/hal.rs::irq_dispatch) in `hal.rs` is the one Rust function behind them: it asks the PIC whether the interrupt is spurious (below); if not it copies the handler out of the table, calls it if there is one, and then sends the [end-of-interrupt](term:eoi). The handlers in use are the timer's tick counter and, once the input code starts, the keyboard's and the mouse's (`kernel/src/input.rs` registers IRQ 1 and IRQ 12). The BIOS test mode swaps the timer's handler for a probe and back, which is the one place the kernel unregisters a line.

### End-of-interrupt and the spurious lines

A controller delivers a line once and then waits for an [EOI](term:eoi) before it will deliver that line, or any of lower priority, again. The rule of the HAL is that the platform sends it, after the handler returns, and the driver never does. The kernel follows it in one place, the end of `irq_dispatch` (`PIC.eoi(irq)`), and the driver's `eoi` writes `0x20` to the slave first for a line of 8 or more and always to the master. The reason is the DOS track, where the BIOS's handler sends the end-of-interrupt for the timer and a second one would acknowledge a different interrupt; keeping the call out of drivers keeps one driver correct on both platforms ([M10](chapter:m-sysdrivers)).

A [spurious interrupt](term:spurious-interrupt) is one the controller reports on its lowest-priority line, 7 on the master and 15 on the slave, when the request that raised it was withdrawn before the processor asked which line it was. There is no real work to do, and there is a real trap: acknowledging it would clear the in-service bit of a different, genuine interrupt. The driver's `is_spurious` reads the in-service register (it writes `0x0B` to the command port and reads it back) and says "spurious" for line 7 or 15 when bit 7 of that controller's register is clear. `irq_dispatch` then runs no handler and calls `eoi_spurious`, which does nothing for IRQ 7 and sends the master an end-of-interrupt for IRQ 15, because the master did see its cascade input and would otherwise hold back lines 3 to 7. The driver's host tests check both (the tests named in the code table below).

### The PIT at 100 Hz

[`pit::init(100)`](sym:kernel/src/pit.rs::init) programs the [PIT](term:pit) through the shared driver: `Pit::new(PORTS).program_clock(&CLOCK, hz)`. The driver divides its 1,193,182 Hz input by a divisor chosen by `(1,193,182 + hz ÷ 2) ÷ hz` in whole numbers, which for 100 Hz is 11,932, and writes the command `0x34` and the two bytes of the divisor ([M3](chapter:m-devices)). `program_clock` records the divisor in the clock in the same step, so the clock's rate cannot drift from the chip's. `init` then registers the clock's tick counter as the IRQ 0 handler with `IrqHandler::from_shared`: the counter is all atomics, so it can be a plain `static`. Its `on_irq` is one `fetch_add` (`lock xadd`, which a 486 has) and the handler runs with interrupts off. Each interrupt is a [tick](term:tick); `pit::ticks()` reads the count.

> **From the stacks — Where 1,193,182 comes from.** Wikipedia's article on the Intel 8253 says that all PC compatibles run the timer at 105/88 MHz, about 1.19318 MHz, which is one third of the NTSC colour-burst frequency, obtained by dividing the 14.31818 MHz system clock by 12. It calls this a holdover from the first CGA PCs, which derived all their frequencies from one crystal. It also says that dividing by 2^16, the largest divisor, gives the roughly 18.2 Hz interrupt that MS-DOS systems use.
> Source: [Intel 8253, Wikipedia](https://en.wikipedia.org/wiki/Intel_8253)

### The first tick, and why `hlt` comes after it

Interrupts are still off when `pit::init` returns; the kernel turns them on only in [`timer_demo`](sym:kernel/src/main.rs::timer_demo), after memory is set up and the heap demonstration has run. `timer_demo` executes `sti` and then does not halt. It spins in `while pit::ticks() == 0`, counting turns, and panics with `the timer did not tick` if it passes 200,000,000. Only when the counter has moved does it use `hlt`, in a loop that waits for ten more ticks: `hlt` stops the processor until an interrupt, and the loop condition is re-read each time it wakes. The ordering has a reason. A `hlt` with a silent timer waits for ever, because nothing will ever wake it, and the run would end only when the [watchdog](term:watchdog) killed it, with no message. The spin has a bound, so a dead timer becomes a one-line failure, and the `notick` [test mode](term:test-mode) proves that by masking IRQ 0 on purpose (the transcript is in the worked example). The bound is a count of turns and not a time, because with no timer there is no clock to count time with.

### "Interrupts off" as the lock

The kernel has one processor and no threads, so the only thing that can run while kernel code is in the middle of an update is an interrupt handler. The lock is therefore `cli`. [`sync::without_interrupts`](sym:kernel/src/sync.rs::without_interrupts) reads `EFLAGS`, clears the flag, runs the closure and sets the flag again only if it was set on entry, so the sections nest and a caller that already had interrupts off does not turn them on. The heap, the frame allocator, the registry and the PIC's mask changes all use it. The mask change is the clearest case: `mask` and `unmask` read a register that every line shares and write it back, so a handler that changed the register in between would have its change lost. Handlers themselves never need the lock, because they run with interrupts off already and may not allocate, wait or call the BIOS (`docs/HAL.md`, "Interrupts"); what they share with the core is atomics or a lock-free ring. This holds only as long as there is one processor and no preemption of the core: a second processor, or a timer interrupt that switched tasks, would each need a real lock, and `docs/HAL.md` ("Concurrency") says the core is single-threaded and cooperative until the native track decides otherwise.

## Why it is this way

- **A kernel table of its own.** The loader's GDT sits in memory the kernel may overwrite, so the kernel loads its own the moment it starts (`kernel/src/gdt.rs`, `docs/NATIVE-BOOT.md` "Interrupts and the timer"). It was written with the first kernel in pull request #6.
- **Identical frames for every exception.** The dummy error code means one stub tail, one `Frame` and one handler, and the mask is data, not logic. The cost is a table to keep right. Pull request #6 introduced it and the expected files for divide error, general protection and invalid opcode hold it to account.
- **Fail loudly and exit.** The handler prints and exits instead of trying to continue, because nothing here can recover from a fault yet; a dump with the registers, `CR0`, `CR2` and `CR3` is what a test can match, and a status of 1 is what a script can read (`docs/NATIVE-BOOT.md`). A real recovery would need a process or a task to kill, and none exists ([M2](chapter:m-cpu)).
- **Remap, do not live with the BIOS's vectors.** The BIOS bases collide with exceptions; the kernel's `0x20` and `0x28` are the first free vectors (`docs/NATIVE-BOOT.md`). The HAL's decision that the same driver serve both platforms, which is why the driver has an `init` the DOS platform never calls, was made in pull request #8 (`docs/HAL.md`, "Decisions").
- **Mask everything, open on registration.** Only IRQ 0 is open at the start, and a line opens when its handler is registered with the gate already in place. The keyboard and the mouse, added in pull request #10, register their lines this way; the registry's contract is in `docs/HAL.md`, "Interrupts".
- **The platform sends the EOI.** In the first kernel the timer's own handler sent it; the shared driver cannot, because under DOS a chained BIOS handler sends it (`docs/HAL.md`, "Decisions", number 4). The change came with the shared drivers in pull request #8, and the spurious cases were corrected in the review that followed.
- **A bounded spin before `hlt`.** The rule that every wait on hardware has a limit is the one stated in [M5](chapter:m-boot); here it is in the one place a mistake would hang the run silently.
- **`cli` as the lock.** One processor, a cooperative core and handlers that never wait make "interrupts off" sufficient and cheap, and the hardware cost of anything stronger on a 486 is the compare-and-swap instruction the project does not use (`docs/HAL.md`, "Concurrency"). The price is that the design does not carry over to more than one processor.
- **Where the document and the code differ.** `docs/NATIVE-BOOT.md` ("Interrupts and the timer") says "the IRQ0 handler increments an atomic counter and sends the end-of-interrupt". In the code the handler only increments the counter, and `irq_dispatch` sends the end-of-interrupt after it returns, which is what `docs/HAL.md` says. The comments at the top of `hal/drivers/pic/src/lib.rs` and `hal/drivers/pit/src/lib.rs` still call `kernel/src/pic.rs` and `kernel/src/pit.rs` "the reference" for the sequence, though those files now call the drivers; the reference is the driver's own tests.

## The code

| Where | What it holds |
|---|---|
| [`kernel/src/gdt.rs`](repo:kernel/src/gdt.rs) | [`GDT`](sym:kernel/src/gdt.rs::GDT) (the three descriptors), [`KERNEL_CODE`](sym:kernel/src/gdt.rs::KERNEL_CODE), [`KERNEL_DATA`](sym:kernel/src/gdt.rs::KERNEL_DATA) and [`init`](sym:kernel/src/gdt.rs::init), which loads the table and reloads every segment register |
| [`kernel/src/idt.rs`](repo:kernel/src/idt.rs) | The assembly stubs for the 32 exceptions and the 16 hardware lines, [`Gate`](sym:kernel/src/idt.rs::Gate), [`IRQ_BASE`](sym:kernel/src/idt.rs::IRQ_BASE), [`set_irq_gate`](sym:kernel/src/idt.rs::set_irq_gate), [`init`](sym:kernel/src/idt.rs::init), [`Frame`](sym:kernel/src/idt.rs::Frame), `NAMES` and [`exception_handler`](sym:kernel/src/idt.rs::exception_handler) |
| [`kernel/src/pic.rs`](repo:kernel/src/pic.rs) | [`init`](sym:kernel/src/pic.rs::init) (bases `0x20` and `0x28`, mask `0xFFFE`), [`mask_timer`](sym:kernel/src/pic.rs::mask_timer) for the `notick` mode and [`mask`](sym:kernel/src/pic.rs::mask) |
| [`kernel/src/pit.rs`](repo:kernel/src/pit.rs) | [`HZ`](sym:kernel/src/pit.rs::HZ), [`init`](sym:kernel/src/pit.rs::init) (program the chip and register the tick counter) and [`ticks`](sym:kernel/src/pit.rs::ticks) |
| [`kernel/src/hal.rs`](repo:kernel/src/hal.rs) | The native platform: [`PORTS`](sym:kernel/src/hal.rs::PORTS), [`PIC`](sym:kernel/src/hal.rs::PIC), [`CLOCK`](sym:kernel/src/hal.rs::CLOCK), [`IrqTable`](sym:kernel/src/hal.rs::IrqTable) and [`IRQS`](sym:kernel/src/hal.rs::IRQS), `register` and `unregister`, and [`irq_dispatch`](sym:kernel/src/hal.rs::irq_dispatch) |
| [`kernel/src/sync.rs`](repo:kernel/src/sync.rs) | [`without_interrupts`](sym:kernel/src/sync.rs::without_interrupts), the critical section |
| [`kernel/src/main.rs`](repo:kernel/src/main.rs) | The order of the four set-up calls in [`kmain`](sym:kernel/src/main.rs::kmain) and [`timer_demo`](sym:kernel/src/main.rs::timer_demo) with its bounded spin |
| [`hal/hal/src/irq.rs`](repo:hal/hal/src/irq.rs) | [`Irq`](sym:hal/hal/src/irq.rs::Irq), [`InterruptController`](sym:hal/hal/src/irq.rs::InterruptController), [`IrqHandler`](sym:hal/hal/src/irq.rs::IrqHandler), [`Chain`](sym:hal/hal/src/irq.rs::Chain) and [`IrqRegistry`](sym:hal/hal/src/irq.rs::IrqRegistry), with the rules for handlers in the header |
| [`hal/drivers/pic/src/lib.rs`](repo:hal/drivers/pic/src/lib.rs) | [`Pic8259`](sym:hal/drivers/pic/src/lib.rs::Pic8259), [`REMAPPED_BASES`](sym:hal/drivers/pic/src/lib.rs::REMAPPED_BASES), `is_spurious`, `eoi_spurious` and the tests `native_init_matches_the_kernel_with_its_delays` and `a_spurious_irq_15_is_acknowledged_on_the_master_only` |
| [`hal/drivers/pit/src/lib.rs`](repo:hal/drivers/pit/src/lib.rs) | [`INPUT_HZ`](sym:hal/drivers/pit/src/lib.rs::INPUT_HZ), [`divisor_for`](sym:hal/drivers/pit/src/lib.rs::divisor_for), [`Pit`](sym:hal/drivers/pit/src/lib.rs::Pit) and [`PitClock`](sym:hal/drivers/pit/src/lib.rs::PitClock) |
| [`tests/native/`](repo:tests/native/) | [`expected-div0.txt`](repo:tests/native/expected-div0.txt), [`expected-gp.txt`](repo:tests/native/expected-gp.txt), [`expected-ud.txt`](repo:tests/native/expected-ud.txt) and [`expected-notick.txt`](repo:tests/native/expected-notick.txt) |

For depth: `docs/NATIVE-BOOT.md` ([interrupts and the timer](doc:docs/NATIVE-BOOT.md#interrupts-and-the-timer)) and `docs/HAL.md` ([interrupts](doc:docs/HAL.md#interrupts-built), [concurrency](doc:docs/HAL.md#concurrency), [the unsafe policy](doc:docs/HAL.md#the-unsafe-policy)). The unsafe code in this chapter is the policy's "audited boundary": each `unsafe` block in the files above carries a one-line `SAFETY:` comment saying what makes it sound, and the drivers contain none.

## Worked example

**The divisor, and the number the transcript prints.** The kernel asks for 100 Hz. The input clock is 1,193,182 Hz, so the divisor is (1,193,182 + 50) ÷ 100 = 1,193,232 ÷ 100 = 11,932 in whole numbers, `0x2E9C`; the chip really runs at 1,193,182 ÷ 11,932 = 99.998 Hz, which the driver reports as 99,998 millihertz (the longer working is in [M3](chapter:m-devices)). `timer_demo` waits for `TICK_WAIT = 10` ticks after the first, so the chip counts 10 × 11,932 = 119,320 input clocks, which is 119,320 ÷ 1,193,182 = 0.100002 seconds: about 100 ms. The line it prints is `timer: waited for 10 ticks with hlt, elapsed 10 ticks`; both numbers are the same because `hlt` wakes on each tick and the loop stops the moment the difference reaches 10. The code only asserts that `elapsed` is at least 10; a run that lost a tick to a slow emulator would print a larger number and still pass, and the expected file pins it to `10`, so on a machine that did lose one the check would fail. The bound on the first-tick spin is 200,000,000 turns; at even one nanosecond a turn that is 0.2 s, 20 ticks' worth, and a spin that long means no interrupt is coming.

The figure replays the same test of the driver: the three writes the chip receives for 100 Hz, against the mock port bus.

<!-- explainer pit -->

**The vectors that matter.** IRQ 0 is vector `0x20`, so the gate at index 32 of the table holds the stub `irq_stub0`. The gate's eight bytes are: the low half of the stub's address, `08 00` for the selector, `00` for the zero byte, `8E` for the flags, and the high half of the address. IRQ 1, the keyboard, is `0x21`; IRQ 12, the mouse, is `0x28 + 4 = 0x2C`; and the spurious lines are IRQ 7 at `0x27` and IRQ 15 at `0x2F`.

**Each exception the negative checks raise.** Three of the deliberate failures of [M5](chapter:m-boot) are exceptions. Each is named by the file that proves it, and each file starts with `...`, then the line the mode prints, then the dump:

| Mode | Vector and name | What the kernel does | Proved by | Lines that prove it |
|---|---|---|---|---|
| `div0` | `0x00` divide error | Divides by zero | [`expected-div0.txt`](repo:tests/native/expected-div0.txt) | `EXCEPTION 0x00: divide error`, `error code 0x00000000`, `ECX 0x00000000  EDX 0x00000000` |
| `gp` | `0x0d` general protection fault | Loads selector `0x28` | [`expected-gp.txt`](repo:tests/native/expected-gp.txt) | `EXCEPTION 0x0d: general protection fault`, `error code 0x00000028`, `EAX 0x00000028` |
| `ud` | `0x06` invalid opcode | Executes `ud2` | [`expected-ud.txt`](repo:tests/native/expected-ud.txt) | `EXCEPTION 0x06: invalid opcode`, `error code 0x00000000` |

The general protection case is the one with something to decode. The error code is the selector that failed: `0x28` is `0000 0000 0010 1000` in binary, so the requested privilege is 0 (bits 0 and 1), the table indicator is 0, meaning the GDT (bit 2), and the index is `0x28 ÷ 8 = 5` (bits 3 and up). The table has indexes 0, 1 and 2, with a limit of `0x17`, so index 5 is beyond it, which is exactly what the test mode name says. The `div0` and `ud` frames carry a zero error code because the stub pushed the dummy: their vectors are not in the mask `0x60227D00` (8 and 13 are, and 13 is the one that arrives with a real code). The one more mode that involves this chapter, `notick`, is not an exception but a panic: [`expected-notick.txt`](repo:tests/native/expected-notick.txt) holds the line `test: timer masked on purpose`, then `KERNEL PANIC: panicked at src/main.rs:{N:1..9999}:{N:1..200}:` and `the timer did not tick`, which is the bounded spin giving up.

## State of this layer

```status
id: interrupts.gdt
label: The kernel's own GDT, loaded with the segment registers reloaded
state: built-verified
evidence: test:tests/native/expected.txt::gdt: loaded, test:tests/native/expected-div0.txt::CS 0x0008, sym:kernel/src/gdt.rs::init
note: Run on QEMU as a 486; every dump shows CS 0x0008, the kernel's code selector.

id: interrupts.exceptions
label: A 256-entry IDT, 32 exception stubs and the exception dump
state: built-verified
evidence: test:tests/native/expected-div0.txt::EXCEPTION 0x00: divide error, test:tests/native/expected-gp.txt::EXCEPTION 0x0d: general protection fault, test:tests/native/expected-ud.txt::EXCEPTION 0x06: invalid opcode, sym:kernel/src/idt.rs::exception_handler
note: Three of the 32 exceptions are raised by tests; the other stubs are generated by the same macro and have not been raised.

id: interrupts.remap
label: Both PICs remapped to 0x20 and 0x28, with only IRQ 0 unmasked
state: built-verified
evidence: test:tests/native/expected.txt::pic: remapped to 0x20 and 0x28, test:hal/drivers/pic/src/lib.rs::native_init_matches_the_kernel_with_its_delays, sym:hal/drivers/pic/src/lib.rs::REMAPPED_BASES
note: The initialisation sequence is checked against a mock bus on the host, and the effect in QEMU.

id: interrupts.registry
label: The IRQ registry and the dispatch stubs, with the end-of-interrupt sent after the handler
state: built-verified
evidence: test:tests/native/expected.txt::timer: waited for 10 ticks, test:tests/native/expected-bios.txt::refused as in-interrupt, sym:kernel/src/hal.rs::irq_dispatch
note: The timer, keyboard and mouse lines run through it in QEMU. The timer handler is registered, replaced and registered again in the bios test mode.

id: interrupts.pit
label: The PIT at 100 Hz, divisor 11,932, counted by the tick counter
state: built-verified
evidence: test:tests/native/expected.txt::timer: waited for 10 ticks with hlt, test:hal/drivers/pit/src/lib.rs::programming_100_hz_writes_what_the_kernel_writes, sym:kernel/src/pit.rs::init
note: In QEMU the tick count and the rate are as expected. Nothing here measures the real rate against a second clock.

id: interrupts.spin
label: The bounded first-tick spin, and a masked timer ending the run with a message
state: built-verified
evidence: test:tests/native/expected-notick.txt::the timer did not tick, sym:kernel/src/main.rs::timer_demo
note: The panic is proved by the notick mode; the spin never reaches its bound on a working timer.

id: interrupts.spurious.driver
label: Telling a spurious IRQ 7 or 15 from a real one, and acknowledging it correctly
state: built-verified
evidence: test:hal/drivers/pic/src/lib.rs::spurious_interrupts_are_told_apart_by_the_in_service_register, test:hal/drivers/pic/src/lib.rs::a_spurious_irq_15_is_acknowledged_on_the_master_only
note: Verified on the host against a mock bus only.

id: interrupts.spurious.kernel
label: A spurious interrupt arriving at the kernel's dispatch
state: built-unverified
evidence: sym:kernel/src/hal.rs::irq_dispatch, doc:docs/HAL.md#interrupts-built
note: No test raises one in the kernel; the dispatch code that handles it has run only on interrupts that were real.

id: interrupts.lock
label: Interrupts off as the critical section
state: built-unverified
evidence: sym:kernel/src/sync.rs::without_interrupts, doc:docs/HAL.md#concurrency
note: Used by the heap, frames and registry on every boot, which prove it does not break them. No test makes an interrupt arrive inside a critical section, so that the lock is what prevents a race is a design argument, not a result.

id: interrupts.other
label: The remaining exception stubs, and exceptions with an error code other than the general protection fault
state: built-unverified
evidence: sym:kernel/src/idt.rs::isr_table, doc:docs/NATIVE-BOOT.md#interrupts-and-the-timer
note: Generated by the same macro as the three that are tested; not raised by any test.

id: interrupts.hardware
label: The tables and the timer on a real machine
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The tables, the remap, the registry and the timer are {{state.interrupts.exceptions}}, {{state.interrupts.remap}}, {{state.interrupts.registry}} and {{state.interrupts.pit}} in QEMU as a 486, and on the host for the drivers. A spurious interrupt reaching the kernel and the claim that `cli` is a sufficient lock are {{state.interrupts.spurious.kernel}} and {{state.interrupts.lock}}, and a real machine is {{state.interrupts.hardware}}. QEMU's `-cpu 486` is an approximation of a processor, and nothing here ran on one.

## Try it yourself

The first three commands are the host tests of the drivers and the registry traits. The three runs after them boot the kernel in QEMU in the test modes that raise the exceptions. They need a Rust toolchain, NASM and `qemu-system-i386`.

```try
needs: host: bash, cargo, nasm, qemu-system-i386, tr, sed
ran: 2026-10-23 on the host at 4cc4ef2, QEMU 11.1.2, NASM 3.02
$ cd hal && cargo test -p desk-pic -p desk-pit 2>&1 | grep -E "^test result: ok. [1-9]"
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cd hal && cargo test -p desk-hal irq 2>&1 | grep -E "^test "
test irq::tests::a_handler_reaches_its_context ... ok
test irq::tests::a_shared_handler_needs_only_a_static ... ok
test irq::tests::lines_above_fifteen_do_not_exist ... ok
$ tools/native-build.sh > /dev/null 2>&1; tools/native-image.sh --test-mode div0 --out dist/n-div0.img > /dev/null; tools/native-run.sh dist/n-div0.img 2>&1 | tr -d '\r' | sed -n '/^test:/,$p'
test: dividing by zero

EXCEPTION 0x00: divide error
error code 0x00000000
EIP 0x0010d125  CS 0x0008  EFLAGS 0x00010246
EAX 0x00000001  EBX 0x00000500  ECX 0x00000000  EDX 0x00000000
ESI 0x00000001  EDI 0x0000000a  EBP 0x0013aff4  ESP 0x0013af04
CR0 0x00000033  CR2 0x00000000  CR3 0x00000000
halting with a failure status
native-run: guest reported failure
$ tools/native-image.sh --test-mode gp --out dist/n-gp.img > /dev/null; tools/native-run.sh dist/n-gp.img 2>&1 | tr -d '\r' | sed -n '/^test:/,/^error code/p'
test: loading a segment selector beyond the GDT

EXCEPTION 0x0d: general protection fault
error code 0x00000028
$ tools/native-image.sh --test-mode ud --out dist/n-ud.img > /dev/null; tools/native-run.sh dist/n-ud.img 2>&1 | tr -d '\r' | sed -n '/^test:/,/^error code/p'
test: executing ud2

EXCEPTION 0x06: invalid opcode
error code 0x00000000
```

Across the whole `hal/` workspace the same host tests pass (`cargo test --workspace` ran 23 suites and 179 tests, none failed, at this commit). The exit status of each failing run is 1; the `native-run:` line says so.

**Exercises.** (1) Without running it, work out the error code for a program that loads selector `0x18`, then the one for `0x2B`; say what the table indicator and privilege bits are, and change the test mode's selector in a scratch branch to check, remembering that its expected file pins `0x00000028`. (2) The kernel's dump prints `ESP 0x0013af04` and the interrupted program's stack pointer is not that. Say what you would add to the program's `ESP`, then find the line of `idt.rs` that proves you right. (3) Predict what the transcript's tick line would print if `TICK_WAIT` were 25, and what the run time would be, then change it and check against `expected.txt`. (4) In a scratch branch, remove the `pic::mask_timer()` call from the `notick` path and say which of the two checks, the transcript or the exit status, fails first.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: IDT
def: The interrupt descriptor table: 256 eight-byte gates that tell the processor which code to run for each interrupt vector. The kernel loads it with lidt; the limit is 2,047.

term: Interrupt vector
def: The number from 0 to 255 that selects a gate of the IDT. Vectors 0 to 31 are the processor's exceptions, and the kernel puts the sixteen hardware lines at 0x20 to 0x2F.

term: Exception
def: An event the processor raises itself because an instruction cannot proceed, such as a divide error, an invalid opcode or a general protection fault. There are 32 kinds, vectors 0 to 31, and the kernel's handler prints a dump of them and ends the run.

term: Interrupt gate
def: An IDT entry that sends the processor to a handler and clears the interrupt flag on the way in. All the kernel's gates are interrupt gates with flags 0x8E, so a handler is not interrupted again.

term: Spurious interrupt
def: An interrupt that the 8259 reports on its lowest-priority line (IRQ 7 or 15) after the request that caused it was withdrawn. It must be recognised from the in-service register and must not get an ordinary end-of-interrupt.

term: Tick
def: One interrupt of the timer on IRQ 0. At 100 Hz a tick is about 10 ms, and the kernel's tick counter is the number of them since the timer was started.
```

```figures
id: m6-pit
reuse: pit-test
at: explainer pit
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every claim was checked against `kernel/src/gdt.rs`, `idt.rs`, `pic.rs`, `pit.rs`, `hal.rs`, `sync.rs`, `main.rs`, `hal/hal/src/irq.rs` and the two drivers, and the commands above were run on the host. Found while writing: `docs/NATIVE-BOOT.md` says the IRQ 0 handler sends the end-of-interrupt, which the code does in `irq_dispatch`; two driver header comments still name the kernel's files as the reference for code that now calls the drivers. Ten status rows, three of them built-unverified (a spurious interrupt at the kernel, the lock, and the untested exception stubs).
