---
id: hal
n: 10
title: A shared language for hardware
date: 2026-10-23
summary: One small crate that both tracks can build drivers against, with one place where unsafe code is allowed, no compare-and-swap anywhere, and drivers that can be tested on an ordinary computer with no emulator at all.
prs: [8]
---
By the end of merge day the repository had two programs that touched the same chips and shared none of the code. The native kernel had its own serial port, timer and interrupt controller code in `kernel/src`, and the DOS runtime had its own timer hook in `rt`. Written twice, each would be tested only inside an emulator, and the two copies would drift. Pull request #8 (branch `feat/hal-design`, {{pr8.state}}) is the answer the archive has been waiting for: a design for a hardware abstraction layer, written as a document, `docs/HAL.md`, and as a small Cargo workspace, `hal/`, that proves the design is not wishful.

The shape is one crate, `desk-hal`, with **two edges**. The *lower edge* is what drivers use: port I/O, interrupts, memory-mapped regions, a tick source. The *upper edge* is what the Desk core will use: a display, input queues, a clock, sound, storage. They live in one crate because they share types such as `Irq`, `HalError` and the lock-free ring. Around that crate the design sorts all code into four kinds, and the rule that keeps the tracks from drifting is that **a line of device logic exists once**, in a driver.

| Kind | What it is | Must not call |
|---|---|---|
| HAL | The traits and small shared types | Anything platform-specific |
| Driver | Device logic written once: register sequences, timing rules, decoding | DOS, BIOS, DPMI or kernel internals |
| Platform | The HAL implemented for one environment: the DOS runtime, the native kernel, a host mock | The Desk core |
| Desk core | The OS-independent shell | Ports, DOS, DJGPP, kernel internals, a driver's registers |

<!-- figure hal-layers -->

Three decisions in the document are worth stopping on, because each is the kind of thing that is cheap to settle now and expensive later.

**Static dispatch.** A driver is `Serial16550<P: PortIo>`: generic over the bus, not handed a `&dyn PortIo`. A binary has exactly one platform, so monomorphisation costs no code size and leaves no indirect call on a 486's interrupt path. The mock bus used in the tests is just another `P`. Trait objects were the fallback and are rejected in the document with that reason.

**One `unsafe` boundary per platform.** The port methods are safe. What is dangerous is *holding the bus*, so that is where the `unsafe` goes: making a real port bus, `RawPorts::new`, is the one audited step on each platform, with a one-line justification of what makes it sound. Drivers contain no `unsafe` at all; the three built drivers have none. The same idea guards memory-mapped regions and interrupt handlers, each with a single audited constructor. This is how the design lets the capability be the value: a driver is given a bus, a base port and nothing else.

**No compare-and-swap, anywhere.** The interrupt handlers talk to the rest of the system through atomics and a lock-free single-producer, single-consumer ring, and the document allows only loads, stores, `swap` (`xchg`) and `fetch_add` (`lock xadd`). A 486 does have `CMPXCHG`, but DOSBox's `cputype = 486` rejects it (the d0 spike found this, chapter 8), and CI runs in DOSBox, so code that used it would pass on the machine it is meant for and fail on the one that checks it. Anything 64-bit would need `CMPXCHG8B`, which the 486 lacks (it arrived with the Pentium), so the target's maximum atomic width is 32 bits. `hal/clippy.toml` bans the methods, and a temporary function was used to check that the ban fires.

Everything in `hal/` builds for the host, so `cargo test` runs the drivers on the development machine against a mock port bus, with models of the 16550, the 8254 and the 8259 behind it. The pull request's document counts {{pr8.hostTests}} tests, which agrees with a count of `#[test]` in the tree. The one below is real: it is `programming_100_hz_writes_what_the_kernel_writes` from `hal/drivers/pit`, which hands the bus a script of the three port writes that the native kernel makes to start the timer at 100 Hz, written out by hand, and fails unless the driver makes exactly those, in that order.

<!-- explainer pit -->

What this does not prove is the other half of the sentence. Nothing in this pull request, as it was written, ran in the kernel, under DOS, in an emulator or on hardware: no binary used the crates yet, and `RawPorts` had never executed an instruction. The equivalence with the kernel is shown by tests against values written out by hand, not by running the kernel against the mock. The next two chapters are the first consumers, and they came in pull requests of their own.

The review found real mistakes, which is worth recording as the pattern the rest of the archive will show: the BIOS programs the timer in mode 3 and the first draft restored mode 2; a spurious IRQ 15 needed an end-of-interrupt on the master only, which the trait could not express; and a speculative wall-clock trait had no implementation and was moved out of the code. The pull request's own comment lists each with its fixing commit.

> **From the stacks — Why 1,193,182.** Every PC-compatible timer ticks at 1.193182 MHz, an odd number that is one third of the NTSC colour-burst frequency and one twelfth of the system crystal. The Intel 8253 was the original timing chip in IBM PC compatibles. As the story is usually told, the PC borrowed a television crystal because such parts were cheap, which is why the Desk's divisor for 100 Hz is 11,932 and not a round number. Wikipedia states the ratios but does not say why the frequency was chosen, so the cheap-crystal reason is the common story and not something I can cite.
> Source: [Programmable interval timer, Wikipedia](https://en.wikipedia.org/wiki/Programmable_interval_timer)

## The shelves

### Catalogued
- The `hal/` workspace passes {{pr8.hostTests}} host tests, `cargo clippy -D warnings` and `cargo fmt --check`, and builds for the kernel's target and the pure-Rust DOS target; CI passed on the merge commit `{{pr8.head}}` of pull request #8, which is {{pr8.state}}.
- The 16550, 8254 and 8259 drivers write the same register sequences as the native kernel's code, checked against values written out by hand (for the PIT, the three writes in the step-through above).
- A throwaway crate that used the ring, the clock and all three drivers for the kernel target emitted no `cmpxchg`, which is the document's own check on the no-compare-and-swap rule.

### Uncatalogued
- Nothing in this pull request, as it was written, ran in an emulator or on real hardware; the later pull requests (chapters 11 and 12) are the first to use the crates in QEMU and DOSBox.
- The mock chip models are the author's reading of the data sheets; none has been compared with a real 16550, 8254 or 8259.
- Equivalence with the kernel is by hand-written values. The code-size cost of moving the kernel and the runtime onto the HAL is not measured.
- Everything the document marks as sketch: the sound, storage and physical-memory surfaces, DMA under a memory manager such as EMM386 (which needs Virtual DMA Services), and port I/O under DPMI hosts other than CWSDPMI.

### Wish list
- Drivers for the OPL chip, the Sound Blaster, the DMA controller, IDE and the floppy.
- A wall clock over the CMOS real-time clock natively and DOS calls on the hosted side.
- A decision on preemption, which would reopen the single-consumer side of the rings.
- A home for `rt` and the HAL crates if Fizz Episode 2 wants them without the Desk.

```figures
id: hal-layers
kind: diagram
track: shared
title: The layers of the HAL
alt: Layered diagram: the Desk core above one crate with an upper and a lower edge, drivers generic over the lower edge, and three platforms below, each holding the one audited unsafe step.
caption: Core, one crate with two edges, drivers and platforms.
at: figure hal-layers
thumb: svg

id: pit-test
kind: explainer
track: shared
title: A PIT divisor test, step by step
alt: A step-through of a real host test that checks the three port writes the PIT driver makes for 100 Hz against a script on a mock port bus.
caption: The real test code, the divisor arithmetic and the mock bus.
at: explainer pit
thumb: steps
```
