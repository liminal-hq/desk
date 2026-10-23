# Spike n3: a real BIOS path for the native kernel

Spike n3 gives the protected-mode Rust kernel the BIOS back, one call at a time. After the loader switches to protected mode the kernel has no BIOS, and [spike D](D-NATIVE-VGA.md) found what that costs: the kernel cannot leave a VBE mode on any card but QEMU's Bochs one, cannot change resolution, and cannot read a disk before it has its own drivers. This spike adds a **real-mode trampoline**: a one-page blob that drops the CPU back to real mode, runs one `INT n` with a register block and a buffer below 1 MiB, and returns, plus the Rust around it that keeps the kernel's interrupts, timer and keyboard whole. It implements the HAL's `Bios` trait for the native platform, so the `desk-vga` VBE layer runs in the kernel unchanged. The design alternatives, the mechanism, the contract and the measurements are below.

## Contents

- [Result](#result)
- [Method](#method)
- [The options](#the-options)
- [The low-memory map](#the-low-memory-map)
- [The switching sequence](#the-switching-sequence)
- [Interrupts, the PICs, the timer and the keyboard](#interrupts-the-pics-the-timer-and-the-keyboard)
- [The API and its contract](#the-api-and-its-contract)
- [How it is tested](#how-it-is-tested)
- [Results](#results)
- [Cost](#cost)
- [Paging](#paging)
- [The V86 upgrade](#the-v86-upgrade)
- [HAL feedback](#hal-feedback)
- [Risks](#risks)
- [What was and was not verified](#what-was-and-was-not-verified)
- [What it changes](#what-it-changes)

## Result

**The trampoline works, and it closes spike D's open finding.** From the running kernel, with no loader involvement:

- **VBE after boot.** `INT 10h` AX=4F00h, 4F01h, 4F02h and 4F03h through `desk_vga::vbe::Vbe<NativeBios>`, unchanged. 640×480×8 and 800×600×8 linear modes are set after boot, draw spike A's picture (the 640×480 one reads back with spike A's DOSBox checksum `0x5bc65936`), and `INT 10h` AX=0003h returns to a text screen, **on QEMU's std, cirrus and vmware cards alike**, twice in a row on each. Spike D's failing case, leaving VBE on Cirrus, now works: the tour's `leave_vbe` falls back to the BIOS when the card has no Bochs registers, and all seven screenshots of the Cirrus run match the std references.
- **INT 13h.** The floppy the kernel booted from reads through the BIOS: sector 0 has `0x55AA` and equals the copy of stage 1 still at `0x7C00`; 16 sectors from LBA 18 equal the first 8 KiB of the running kernel. With an IDE disk attached, the LBA extensions (AH=41h and 42h) work too.
- **INT 15h and INT 1Ah.** E820 after boot returns exactly the loader's map; AH=88h and AX=E801 agree with it; the RTC through the BIOS agrees with the CMOS read directly.
- **The kernel survives.** An unsupported function, a drive that does not exist and a vector with no handler are refused or answered with the BIOS's error flag; a call from an interrupt handler is refused; the next call works. 1,000 calls leave the stack, GDT, IDT, segments, CR0 and PIC masks identical after every one, and the kernel's tick count keeps 100 ticks to each RTC second while it makes a call every tick. A key sent while the BIOS is running is not lost.
- **Cost.** About 30 to 50 microseconds a call under QEMU's TCG, hedged below.
- **Other BIOSes.** The same kernel image ran (without key injection) on Bochs 2.7 built as a 486 with BIOS-bochs-legacy and BIOS-bochs-latest, and on Ubuntu 24.04's QEMU 8.2.2. Nothing ran on real hardware.

The recommendation: **keep the trampoline as the native BIOS path** until there is a reason for V86, which is calls that must not turn interrupts off, or FreeDOS. [The V86 upgrade](#the-v86-upgrade) says what that would add.

## Method

- **Design first**, against the Intel manual's rules for switching back to real mode (disable interrupts, leave paging from identity-mapped code, load 64 KiB byte-granular 16-bit descriptors into every segment register, clear `CR0.PE`, far jump, reload the real-mode vector table, then the way back in reverse) and against what the loader already does in the other direction.
- **Build** `kernel/bios16.asm` (NASM, assembled flat by `tools/native-build.sh`, embedded in the kernel with `include_bytes!`), `kernel/src/bios.rs` (the Rust side) and `kernel/src/biostest.rs` (the scenarios).
- **Prove it** with scenarios whose transcript lines `tools/native-test.sh` checks, on three QEMU cards, with screenshots, with negative checks. Then try the same image on other BIOSes in Docker where the emulator allows it (Ubuntu's QEMU 8.2.2, Bochs 2.7 with its two BIOSes and with SeaBIOS 1.13, and 86Box).
- **Measure** calls in kernel ticks.

## The options

The kernel needs the BIOS for three kinds of thing, in order of how soon: video (leave a VBE mode, set another resolution), disk (before native floppy and IDE drivers exist) and system information (memory map, time). Four ways to get it:

| Option | What it is | Interrupts | Paging | Cost and risk |
|---|---|---|---|---|
| **(a) Real-mode trampoline** | Switch the CPU back to real mode for one call, switch forward again | Off for the call; the BIOS's own handlers serve its IRQs; the PICs are put on the BIOS vectors and back | The code at `0x10000` must be identity-mapped while `CR0.PG` changes, or paging is off for the call | About 200 lines of assembly (most of them comments) and 450 of Rust; a fixed cost per call; the kernel is blind for the duration; a BIOS that never returns takes the kernel with it |
| **(b) V86 mode** | A virtual-8086 task under the kernel, with a monitor that handles the faults of sensitive instructions | On: the kernel keeps its IDT and PIC remap; the monitor decides which IRQs to reflect into the BIOS | Needs paging only for the 1 MiB window (a V86 task is linear 0 to `0x10FFEF`); can run with paging on | A TSS, an IDT that routes the V86 faults, and a monitor that emulates `int`, `pushf`, `popf`, `iret`, `cli`, `sti` and port I/O: some 600 to 1,000 lines and a real test matrix; every sensitive instruction costs a fault on a 486 |
| **(c) An x86 emulator** (`x86emu`, as `vbetool` and `v86d` use) | Run the BIOS's code in an interpreter | None involved | None involved | Some 10,000 lines of C or a port; slow on a 486; port I/O and IRQ waits have to be forwarded, so disk calls are awkward; good for video only |
| **(d) Everything in the loader** (what spike D did) | Ask the BIOS before protected mode | Not applicable | Not applicable | Free and robust; no change of resolution, no way back to text on cards without a generic leave, no disk after boot |

**Recommendation: (a) first, (b) later.** The trampoline is the smallest thing that gives the kernel every BIOS service, it can be tested in QEMU on day one, and it keeps the decision about V86 for when the need is real. (c) is the only choice for a CPU that is not x86 and is far too heavy here; (d) is what the kernel already has and is the reason for this spike. What each option means for the things that cause trouble:

- **Interrupts.** (a) runs the BIOS with the kernel's interrupt handling switched off. The BIOS may enable interrupts inside the call (INT 13h does, to wait for the floppy), and then its own handlers must be what answers, which means the vector table must be the BIOS's and the 8259s must send the BIOS's vectors (`0x08` to `0x0F`, `0x70` to `0x77`), not the kernel's (`0x20` to `0x2F`). So a call reprograms both controllers and puts them back. (b) does not: the 8259s stay at the kernel's vectors, an IRQ arrives at the kernel's handler even while a V86 task runs, and the monitor either handles it or reflects it to `INT 08h` and the rest.
- **The PIC.** The controllers' masks and vector bases are the state that makes (a) delicate; see [the section on interrupts](#interrupts-the-pics-the-timer-and-the-keyboard). The in-service registers must be clear: a call from inside an interrupt handler would reset a controller with an interrupt half served.
- **NMI.** It is delivered through the IDT in protected mode and the real-mode vector table in real mode. In (a) there is a window where the CPU is in real mode with the kernel's IDT still loaded in memory but not in the register, in which an NMI would fetch from the wrong table, so NMI is masked at port `0x70` for the call and the kernel's resting state is restored. In (b) NMI always goes through the kernel's IDT to ring 0, which is better. An SMI cannot be masked by the kernel in either.
- **SMP.** A 486-class target has one processor, so the trampoline assumes it. On a multiprocessor machine the other processors would have to be parked and the interrupt controller would be an APIC; neither exists here.
- **Paging.** The kernel has none yet. [The paging section](#paging) states exactly what (a) needs when it arrives.

## The low-memory map

The kernel is linked at 1 MiB, which real mode cannot execute from, so the trampoline lives below 1 MiB. The first megabyte is reserved by the frame allocator already; the part this spike claims is the staging area where stage 2 first loads the kernel (`0x10000`), free once stage 2 has copied it up.

| Physical | Size | What | Notes |
|---|---|---|---|
| `0x00000` | 1 KiB | Real-mode vector table | Never written by the kernel; the trampoline loads `lidt` with base 0 and limit `0x3FF` and calls handlers through it |
| `0x00400` | 256 B | BIOS data area | Read for the tick count (`0x46C`), the midnight flag (`0x470`) and the video mode (`0x449`) |
| `0x00500` | to `0x820` | Boot information and memory map | The loader's; untouched |
| `0x00820` | 32 B | Video block | The loader's |
| `0x02000` | 4 KiB | The BIOS 8×16 font | The loader's; the text restore reads it |
| `0x03000` | 768 B | VBE scratch blocks | The loader's, free after boot; not used |
| `0x07C00` | 512 B | Stage 1 | Still in memory; a scenario compares it with the disk |
| `0x07E00` | 8.5 KiB | Stage 2 | |
| `0x10000` | 4 KiB | **The trampoline blob** | The control block at offsets `0x000` to `0x02F`, the entry at `0x100`, the code, its tables and the 16-bit GDT |
| `0x11000` | 8 KiB | **The bounce buffer** | For ES:DI buffers (the 512-byte VBE information block, the 256-byte mode block), disk address packets and sector reads (up to 16 sectors) |
| `0x13000` | 4 KiB | **The real-mode stack** | SS = `0x1000`, SP starts at `0x4000`, so the stack is `0x13000` to `0x13FFF` |
| `0x14000` | | free | |
| `0x9FC00` | | EBDA | Never touched |

All three regions lie inside the one 64 KiB physical block `0x10000` to `0x1FFFF` and below 16 MiB, so a floppy DMA into the bounce buffer can neither cross a 64 KiB boundary nor go out of the 8237's reach. The blob is "position independent" in the sense that matters: its 16-bit code only uses offsets from its own start, and the 32-bit code reaches its data through the single constant `BASE` (`0x10000`) that `kernel/src/bios.rs` repeats as `LOW_BASE`. A different address is a change to two constants.

## The switching sequence

Everything between "the kernel calls" and "the kernel is back" runs with interrupts off and NMI masked. Steps 1 to 3 and 12 to 14 are Rust (`bios.rs`); the rest is `bios16.asm`.

| # | Mode | What happens |
|---|---|---|
| 1 | 32-bit | Checks: the guard (an `xchg`) is free; interrupts off; no 8259 in-service bit set; `CR0.PG` clear; the vector table entry for the interrupt is not `0000:0000` |
| 2 | 32-bit | Saves the PIC masks; reads the BIOS tick count and the timer's request bit; masks NMI (`out 0x70, 0x80`) |
| 3 | 32-bit | Puts both 8259s on the BIOS vectors `0x08` and `0x70` with the mask [below](#interrupts-the-pics-the-timer-and-the-keyboard); writes the vector, the flags image and the registers into the control block at `0x10000` |
| 4 | 32-bit | **Entry** (`0x10100`): `pushfd`, `pushad`, `cli`; saves ESP, GDTR, IDTR, CR0, CR3 and the five segment selectors in the blob |
| 5 | 32-bit | `lgdt` the blob's table; loads selector `0x20` into DS, ES, FS, GS and SS; sets ESP to `0x4000`; far jump to selector `0x18` |
| 6 | 16-bit protected | Clears `CR0.PG` and `CR0.PE`; far jump to `0x1000:rm_entry` |
| 7 | real | DS and SS = `0x1000`, SP = `0x4000`, ES, FS and GS = 0; `lidt` with limit `0x3FF`, base 0; reads the handler's far address from the vector table |
| 8 | real | Loads the caller's flags and pushes a copy; loads ES, FS, GS, EBP, ESI, EDI, EDX, ECX, EBX, EAX and last DS from the control block; `call far` to the handler, which returns with `iret` (or `retf 2`) as it would from an `INT` |
| 9 | real | Stores the registers and the flags the handler returned; `cli`, `cld`; `lgdt`; sets `CR0.PE`; far jump to selector `0x08` at `BASE + pm32_back` |
| 10 | 32-bit | Loads the flat data selector everywhere; ESP from the saved value; restores GDTR and IDTR; reloads the saved segment selectors (SS last); restores CR3 and then CR0 (so `CR0.PG` returns last, if it was set) |
| 11 | 32-bit | `popad`, `popfd`, `ret` to Rust |
| 12 | 32-bit | Puts the PICs back on `0x20` and `0x28` with the saved masks; restores NMI to the kernel's resting state (masked) |
| 13 | 32-bit | Adds the ticks the BIOS counted and any undelivered timer tick to the kernel's clock |
| 14 | 32-bit | Hands a keyboard or mouse byte left in the 8042 to the driver's handler by hand; counts the call |

The blob's GDT (`tramp_gdt`) has the kernel's two flat selectors at the same numbers, so the kernel's own selectors still mean what they did, and two 16-bit descriptors with a base at the blob:

| Selector | Descriptor | Base | Limit | What |
|---|---|---|---|---|
| `0x00` | `0x0000000000000000` | | | Null |
| `0x08` | `0x00CF9A000000FFFF` | 0 | 4 GiB (4 KiB granularity) | 32-bit code, ring 0, execute and read |
| `0x10` | `0x00CF92000000FFFF` | 0 | 4 GiB | 32-bit data, read and write |
| `0x18` | `0x00009A010000FFFF` | `0x10000` | `0xFFFF`, byte granularity | **16-bit code**, ring 0, execute and read |
| `0x20` | `0x000092010000FFFF` | `0x10000` | `0xFFFF`, byte granularity | **16-bit data**, ring 0, read and write |

Loading `0x20` into every data segment and jumping through `0x18` before clearing `CR0.PE` is what leaves the hidden parts of the segment registers with a 64 KiB byte-granular limit, which real mode keeps using; a CS or DS left with a 4 GiB limit would be "big real mode", which works on most CPUs and is not what a BIOS or a 486 manual promises. The base `0x10000` equals what real mode computes for segment `0x1000`, so the first real-mode instruction after the far jump agrees with the cached state.

**Calling the handler through the vector table rather than self-modifying an `int n`** removes a hazard: a 486's prefetch queue could hold the old opcode. The handler sees what an `INT` gives it (CS:IP and a flags word on the stack) except that `INT` also clears `IF` and `TF`, and interrupts are already off.

## Interrupts, the PICs, the timer and the keyboard

**Two interrupt worlds.** The BIOS's handlers expect IRQ 0 to arrive as `INT 08h` and IRQ 1 as `INT 09h`; the kernel's arrive at `0x20` and `0x21`. So the call reprograms both 8259s to the BIOS bases before it switches and back afterwards. It uses the shared `desk_pic::Pic8259::init` for both directions, so the initialisation words and the I/O delays exist once.

**The mask during a call** (`BIOS_PIC_MASK` in `bios.rs`; bit n masks IRQ n) leaves open the lines whose BIOS handlers a call can depend on and closes the rest:

| Open | Why |
|---|---|
| IRQ 0, timer | The BIOS tick (`0x46C`) and the floppy motor timeout; INT 15h AH=86h and others wait on time |
| IRQ 2, cascade | Reaches the slave |
| IRQ 6, floppy | INT 13h waits for the controller's interrupt |
| IRQ 8, RTC | INT 15h AH=83h and AH=86h on some BIOSes |
| IRQ 14, 15, IDE | INT 13h on a hard disk on BIOSes that wait for the interrupt rather than poll |

Everything else is masked, **above all IRQ 1 (keyboard) and IRQ 12 (mouse)**: if they were open, the BIOS's INT 09h handler would read the scancode from port `0x60` into its own buffer and the kernel's driver would never see it. A byte that arrives during a call therefore stays in the 8042's output buffer, and that is its own problem: the controller raised its interrupt edge while the line was masked, the 8259 discarded the request when it was reprogrammed (`ICW1` clears the request register), and the 8042 will not raise another edge until the byte is read. **The fix is to poll after the call**: while the 8042's status shows an output byte, run the registered handler of IRQ 1 (or IRQ 12 if the status says the byte is the mouse's) by hand, as the interrupt would have, up to eight times. The test sends a key during a one-second wait inside `INT 15h AH=86h` and checks that the driver delivers it.

**The timer.** The kernel keeps the PIT at 100 Hz, so a BIOS tick, counted in the BIOS data area at `0x46C` by the BIOS's `INT 08h` handler, is exactly one kernel tick. The BIOS handler runs only while the BIOS has interrupts on inside a call; after the call the difference of the count is added to the kernel's clock (a delta is capped at 10,000, and nothing is added when the midnight flag is set, because the count was reset). A tick that was raised but not yet delivered when the controllers were reprogrammed is also added, once at the start (it would be discarded by the reset, and the kernel owes it) and once at the end (the BIOS never enabled interrupts, or enabled them too late). The PIT itself is not touched: the BIOS does not reprogram channel 0 in the calls tried, and the test proves it: while the kernel made a call every tick for four RTC seconds it counted 100 ticks to each. A BIOS that did would show up as drift there. Because the BIOS count advances at the kernel's rate, **`INT 1Ah` AH=00h (ticks since midnight) is not a wall clock** here; AH=02h and AH=04h (the RTC) are.

**NMI** is masked for the whole call by setting bit 7 of port `0x70`, and restored to the kernel's resting state afterwards, which is masked: BIOSes leave it masked after POST, and the kernel has no NMI handler beyond the fatal dump. A kernel that wants NMI later changes `NMI_ENABLED_AFTER`.

**Spurious and nested interrupts.** A call refuses to run if any in-service bit is set, because the controller reset would clear it under a handler that has not sent its end-of-interrupt. The kernel's handlers send the end-of-interrupt before they return, so a call from the main line or from the idle loop always passes, and a call from a handler never does. The in-service check, the guard and the interrupts-off section together make a call non-reentrant.

## The API and its contract

```rust
// kernel/src/bios.rs
pub struct RealModeRegs { eax, ebx, ecx, edx, esi, edi, ebp: u32, es, ds, fs, gs, flags: u16, reserved: u16 }

pub fn init() -> Result<(), BiosError>;                                   // copy the blob below 1 MiB; once, at boot
pub fn call(vector: u8, regs: &mut RealModeRegs) -> Result<(), BiosError>;  // the safe call
pub unsafe fn bios_int(vector: u8, regs: &mut RealModeRegs) -> Result<(), BiosError>;  // the raw call, preconditions below
pub fn bounce_buffer() -> Bounce;       // len() = 8192, segment() = 0x1100, write_low(offset, &[u8]), read_low(offset, &mut [u8])
pub fn read_low(segment: u16, offset: u16, out: &mut [u8]) -> Result<(), BiosError>;   // any real-mode address in the first MiB
pub struct NativeBios;                  // impl desk_hal::Bios: `int` with an optional buffer (up to 512 bytes), and `read_low`

pub enum BiosError { NotReady, Busy, InInterrupt, PagingEnabled, NoHandler(u8), BadAddress }
```

**The contract**, in the order that matters:

1. **Never from an interrupt handler.** A handler runs with an 8259 in-service bit set; `call` returns `InInterrupt`. Never from the panic path or the exception dump either (they would not return).
2. **Any other context is fine**: the main line, the idle loop between `hlt`s, with the interrupt flag on or off. `call` runs the sequence with interrupts off and puts the flag back as it was; the scenarios check that it does.
3. **One call at a time.** The guard is a flag set by `swap` (an `xchg`; the HAL bans compare-and-swap), and a second `call` while one is in progress returns `Busy`. On one processor this can only be reached by an interrupt that arrives before the first call has turned interrupts off, and the in-service check would refuse it anyway.
4. **Paging off, or the low megabyte identity-mapped** ([below](#paging)). `call` returns `PagingEnabled` if `CR0.PG` is set; the blob can already restore it, but nothing maps it yet.
5. **The vector must have a handler.** A vector table entry of `0000:0000` returns `NoHandler(n)` without switching. This catches an unset user vector; it does not protect against an entry that points at nonsense, which a BIOS bug or an overwritten table would give.
6. **Buffers go in the bounce buffer.** The BIOS cannot see the kernel's memory above 1 MiB. Write the input with `write_low` before the call, set `es:di` (or `ds:si` for a disk address packet) to `bounce.segment():offset`, and read the output with `read_low` after. The contents are the BIOS's during the call and stay as it left them, so VBE's far pointers into the block remain readable until the next call. `NativeBios::int` does this for the `Bios` trait's one optional buffer.
7. **The result is two layers.** `Ok(())` says the call was made; whether the BIOS function succeeded is in the registers (the carry flag for most, `AX` for VBE). The vector table is never written by the kernel.
8. **No state is trusted across a call except what it restores**: the CPU registers `pushad` covers, ESP, the segment selectors, GDTR, IDTR, CR0, CR3, the PIC masks and vector bases, the NMI state, the tick count and the 8042's output buffer. The VGA, the DMA controller's mask and the floppy controller are the BIOS's to change, and a driver that holds state in a device the BIOS touches must save and restore it itself. In particular **a native driver must be quiet before a call**: a registered IRQ 14 handler for a native IDE driver, for instance, would see no interrupt during the call, because the line is the BIOS's then.

`unsafe fn bios_int` is `call` without the checks and the guard, for a caller that has made them; its safety comment lists the preconditions.

## How it is tested

`tools/native-test.sh`, "BIOS calls from the kernel", boots a `bios` test mode (`kernel/src/biostest.rs`) and checks `tests/native/expected-bios.txt` against the transcript and `tests/native/bios.sums` against three screenshots taken through the QEMU monitor.

| Scenario | What it proves |
|---|---|
| Memory | `INT 12h`; E820 re-read after boot equals the loader's map entry for entry; AH=88h and AX=E801 within 256 KiB of the map (they ignore the reserved hole at the top of RAM) |
| Clock | `INT 1Ah` AH=02h and 04h are valid BCD in range and within 2 s of the RTC read through the CMOS ports |
| Errors | An unsupported `INT 15h` function and a read from drive `0x77` set the carry flag with the BIOS's status; a vector table entry zeroed for the test is refused as `NoHandler`; the guard returns `Busy`; a call tried from the timer's interrupt handler (swapped in for the test) is refused as `InInterrupt`; the next call works |
| Repeats and idle | Two calls in a row; five calls from the idle loop (`sti; hlt; call`) with the interrupt flag surviving; the CPU snapshot equal afterwards |
| Disk | Geometry through AH=08h; AH=41h; sector 0 has `0x55AA` and equals stage 1 at `0x7C00` except the five variables stage 1 writes; 16 sectors from the second track equal the first 8 KiB of the kernel in memory; a read of sector 0 fails with the BIOS's status. A second run adds an IDE disk: AH=41h reports extensions and AH=42h reads two sectors by LBA that hold the disk's known text and fails beyond the end |
| A key during a call | The inject script sends `a` while the kernel is one second into `INT 15h AH=86h`; the call returns, the driver has been handed the byte and the key event comes out of the ring |
| Cost and stress | 5,000 plain calls timed; then 1,000 calls with the stack pointer, GDTR and IDTR, the three GDT descriptors, a hash of the IDT, six segment registers, CR0, the PIC masks and the interrupt flag compared after every one; then calls once a tick until four RTC second changes are seen, and the kernel ticks between them must be 98 to 102 |
| VBE | 4F00h, 4F01h for modes `0x101`, `0x103` and `0x105`; set 640×480×8 with the linear bit, check 4F03h, draw the picture and compare its checksum with the one drawn in memory (and with spike A's DOSBox value for 640×480), screenshot; the same for 800×600×8; `INT 10h` AX=0003h and check the BIOS data area's video mode and 4F03h; screenshot of the text screen; the whole thing twice, so a card left half-way would fail the second round |
| The kernel afterwards | The scenario ends with a key from the inject script, which must arrive as an event, after the kernel has made more than a thousand calls |

**Negative checks:** a transcript with one changed line, a wrong screenshot reference, a VBE screenshot passed off as the restored text screen, the floppy-only transcript against the IDE run, and a run in which the key never arrives (the kernel must panic with "the key sent during the call was lost" and exit 1). The Cirrus run of spike D's tour (`gfx-vbe-cirrus`) has its own, and rejects the std transcript.

A `bios-solo` mode runs the same scenarios without the steps that need typed keys, for emulators that cannot inject them (Bochs here).

## Results

QEMU 11.1.2 on the development machine (its bundled SeaBIOS, `-cpu 486 -m 16`) is the reference; the checks run from `tools/native-test.sh`. The same image was also run by hand on the others.

| Check | std | cirrus | vmware | QEMU 8.2.2 (std, cirrus, vmware) | Bochs 2.7, BIOS-bochs-legacy | BIOS-bochs-latest | SeaBIOS 1.13 under Bochs |
|---|---|---|---|---|---|---|---|
| VBE version, modes listed | 3.0, 93 | 3.0, 31 | 3.0, 93 | the same | 2.0, 24 | 2.0, 24 | none: 4F00h answers "not supported" and the call returns cleanly |
| VBE 4F02h set 640×480×8 LFB, picture checksum | `0x5bc65936`, equals spike A | same | same | same | same | same | not reached |
| 800×600×8 | set, matches memory | same | same | same | same | same | not reached |
| `INT 10h` AX=0003h, text screen restored | yes, screenshot is text | **yes** | yes | yes | yes (BIOS data area mode 3; no screenshot) | yes (same) | not reached |
| Set, draw, text, twice | yes | yes | yes | yes | yes | yes | not reached |
| `INT 13h` floppy read, equal to the bytes in memory | yes | yes | yes | yes | yes | yes | yes |
| `INT 13h` LBA (AH=42h) on IDE | yes | | | | not run | not run | not run |
| E820 after boot equals the loader's map | yes | yes | yes | yes | yes | yes | yes |
| `INT 1Ah` equals the CMOS | yes | yes | yes | yes | yes | yes | yes |
| 1,000-call stress, state identical | yes | yes | yes | yes | yes | yes | yes |
| 100 kernel ticks per RTC second | yes | yes | yes | yes | yes | yes | yes |
| Error paths, `InInterrupt`, `NoHandler`, `Busy` | yes | yes | yes | yes | yes | yes | yes |

Things the runs showed:

- **Spike D's Cirrus failure is resolved**, with the before and after from the same tour: before, `leave_vbe` found no Bochs registers (`read 0x0000`) and the run ended with a 640×400 mode-13h screenshot whose checksum was `af412ec3` instead of the reference `22f676d7`, and a text screenshot with 15 colours instead of 2; after, the tour calls `INT 10h` AX=0003h, and all seven screenshots equal the std ones. The register path that D described as the fallback still stands for text restores when VBE was never used.
- **A cylinder past the end of the floppy kills Bochs's floppy model** before the BIOS can answer ("`io: norm r/w parms out of range`", a fatal panic by Bochs's configuration), so the disk error test uses sector 0, which every BIOS rejects.
- **Bochs's VGA BIOS returns the mode from 4F03h without the linear-frame-buffer bit** (`0x0101` where SeaBIOS gives `0x4101`), so the scenario compares the low 14 bits.
- **SeaBIOS 1.13 as the system BIOS under Bochs, with Bochs's LGPL VGA BIOS, offers no VBE at all**: 4F00h returns without `AL=4Fh`, which `Vbe::info` reports as `Unsupported` and the scenario, which requires VBE, turns into a panic after everything else has passed. That is the "no VBE BIOS" case a real machine can be in.
- **QEMU 8.2.2 gives the same transcript and the same screenshots** as 11.1.2, which is what the CI image's QEMU is (Ubuntu 24.04).

## Cost

One `INT 1Ah` AH=02h call, 5,000 in a row, timed in 10 ms kernel ticks (so each figure is a mean over a tenth of a second or more):

| Where | Per call |
|---|---|
| QEMU 11.1.2, TCG | about 47 µs (24 ticks) |
| QEMU 8.2.2, TCG | about 32 µs (16 ticks) |
| Bochs 2.7 at its default 4 MIPS | about 160 µs (81 ticks) |

These are emulated speeds and say nothing about a 486. What the figure contains: about 60 port accesses (each of the two 8259 initialisations is eight words with an I/O delay write after each, then the masks; the NMI bit; the in-service and request register reads), about 150 instructions of the switch, the BIOS handler itself (a few hundred instructions for the RTC read), and the post-call checks (two request-register reads, the 8042 status). A 486 spends most of it on the port accesses at about a microsecond each on the ISA bus, so **a rough guess of 100 µs for a trivial call and ten to a few hundred milliseconds for a mode set or a floppy sector** (those are the BIOS's own time, not the trampoline's) is the right order, unmeasured. Nothing here needs to be fast: VBE calls happen on mode changes and disk calls are replaced by a driver once there is one.

## Paging

The kernel has no paging, so a physical address is a pointer. When paging arrives, the trampoline needs exactly this:

1. **The blob's page (`0x10000` to `0x10FFF`) identity-mapped** (linear equals physical) in the page tables that are live when `call` runs. The blob executes at `EIP = 0x10100` through `0x10FFF` before `CR0.PG` is cleared and again after it is restored (`bios16.asm` does both: it clears `PG` and `PE` together in 16-bit protected mode, and restores CR3 and then CR0 in the flat 32-bit code). An instruction fetch from an address that stops being mapped at that moment faults with nowhere to go. The simplest way to keep this true is an **identity-mapped low megabyte**, read and write, supervisor, in every address space the kernel might call from.
2. **The bounce buffer, the control block and the vector table and BDA reachable by the kernel at the linear addresses `bios.rs` uses**, which today are the physical addresses (`LOW_BASE`, `BOUNCE_BASE`, `0x46C`, address 0 for the table). With the low megabyte identity-mapped they stay valid; if the kernel instead maps low memory at another linear address, `bios.rs` has to translate (four constants and `copy_from_low`).
3. **The kernel stack saved across the call** is a linear address; it stays valid because CR3 is restored unchanged.
4. **No changes to `CR4`** on a CPU that has one. A 486 does not. On later CPUs with `PSE`, `PAE` or `PGE`, the blob would clear `PG` with those still set, which is legal for `PSE` and `PGE` but needs `PAE` cleared after `PG` and restored before it; out of scope for a 486 target, and the blob would need it then.
5. **A TLB flush on the way back**: loading CR3 flushes the non-global entries, which the restore does.

Or, instead: **disable paging for the call**: the trampoline already does when the CR0 it saved had `PG` set, so what `call` refuses today (`PagingEnabled`) can instead be allowed once the identity map exists. The risk to watch is an interrupt handler or a second processor using the page tables meanwhile; with interrupts off and one processor neither exists.

## The V86 upgrade

V86 is the next step when one of three things happens: a BIOS call that cannot tolerate the kernel being blind for its length (a long INT 13h on a slow floppy, which loses nothing here but is felt as a stall); a need to keep the kernel's own interrupts running through a call (a native sound or network driver with a deadline); or the move to **running FreeDOS and DOS programs**, which `DIRECTION.md` plans and which V86 is the first step towards.

What it needs:

- **A TSS.** Interrupts and exceptions out of V86 mode switch to ring 0 on the stack named in the TSS (`SS0:ESP0`), and the frame the CPU pushes includes the V86 segment registers (GS, FS, DS, ES). The TSS also holds the I/O permission bitmap, which decides which ports a V86 task may touch directly and which fault.
- **An IDT whose gates handle `#GP` from V86 mode.** With `IOPL` below 3, which it has to be (at 3 the task could clear the interrupt flag and take the machine), `int n`, `pushf`, `popf`, `iret`, `cli`, `sti`, `in` and `out` (for ports the bitmap denies) all fault. The monitor reads the instruction at the faulting `CS:IP` and emulates it on the V86 stack and register image. A 486 has no `CR4.VME` to do the flag work in hardware, so **every `pushf`, `popf`, `cli` and `sti` the BIOS executes is a fault**, which for a BIOS that uses them freely costs a few hundred cycles each.
- **A monitor.** Per V86 task: the register image, the virtual interrupt flag, the virtual PIC and what it does with each IRQ (**reflect** it into the BIOS's `INT 08h` and the rest through the V86 vector table, or handle it in the kernel and keep it from the BIOS), and what the BIOS and later DOS programs are allowed to touch. Video, keyboard and timer are the three hard ones, and for the BIOS-call use only a small piece of that is needed.
- **Linear 0 to `0x10FFEF`** mapped for the task (the first megabyte and the high memory area with A20 on), by paging, so it needs the page tables that are also what (a) needs when paging arrives.
- **A way out:** a return-to-kernel convention such as a special `int` the monitor recognises, plus a watchdog (a V86 BIOS that hangs can be preempted by the timer, which the trampoline cannot do).

What it adds over (a): calls **without turning interrupts off and without touching the PICs**, so no lost ticks and no 8042 byte to rescue; a hung BIOS call can be interrupted; the kernel's `IDT` and NMI handling stay in force throughout; and the same machinery runs **DOS programs** with the BIOS and DOS services behind a virtual machine, which is how FreeDOS would run in a window. What it costs: a few hundred to a thousand lines and a harder test matrix, a fault per sensitive instruction (on a 486 without VME), and the open question of what to virtualise. The trampoline's `RealModeRegs`, the bounce buffer and the `Bios` trait carry over unchanged, so the move is an implementation swap behind `NativeBios`.

## HAL feedback

- **The `Bios` trait is implementable natively as it stands.** `NativeBios` implements `int` and `read_low` exactly as `hal/hal/src/bios.rs` describes them, and `desk_vga::vbe::Vbe<NativeBios>` runs VBE 4F00h through 4F03h in the kernel without a change. That settles the HAL's open question "VBE natively: real-mode BIOS calls before entering protected mode, a V86 monitor, or register-level drivers per card" for now: **through a real-mode call from the kernel, and a V86 monitor later behind the same trait.**
- **`BIOS_BUFFER_MAX` (512) is too small for disk calls.** A VBE block fits; an INT 13h read of one track does not, and the trait takes one buffer, copied in and out. The native implementation has an 8 KiB bounce buffer and the kernel's own code uses it directly (`bounce_buffer`); a trait method for a disk call would need the constant raised to 4,096 or the buffer split into input and output. Not changed here: the DOS platform's buffer (a DPMI transfer buffer) would need the same size.
- **`BiosRegs` has no `fs`, `gs` or flags input.** Native `RealModeRegs` carries `fs` and `gs` (set to zero from a `BiosRegs`), and the flags are an input as well as an output; the trait's `flags` is documented as output only. The native implementation passes the caller's `flags` in, which is harmless. If a driver needs a carry set on entry (`INT 15h` AH=24h-style calls do not), the doc comment should say so.
- **The trait says nothing about interrupt context or re-entrancy.** The native `call` refuses from an interrupt handler and takes a guard; the DOS implementation under DPMI has its own rules. A sentence in the trait documentation would make the contract the same on both platforms: "not from an interrupt handler, not re-entrant".
- **`Display::set_mode` can now return to text on the native side**: the native `restore_text_mode` is `INT 10h` AX=0003h through `NativeBios`, as under DOS, and spike D's "VBE is a one-way trip" recommendation no longer applies where the trampoline is present. The register path in `desk-vga` stays for modes that were never VBE.
- **A `desk_pic` addition.** `Pic8259::in_service_mask` and `pending_mask` (OCW3 reads of the in-service and request registers), with a scripted test. They are needed to refuse a call from an interrupt handler and to account for a tick the controller reset would discard.
- **Handing a byte to a driver by hand.** A BIOS call leaves a byte in the 8042 that the driver's handler must be run on by hand (`IrqTable::run_handler`); the drivers themselves needed no change, because `KbdIrq` reads one byte per call and the rings do the rest.

## Risks

- **A BIOS that does not return, or that uses the machine beyond what the trampoline gives it.** There is no watchdog: a call that hangs hangs the kernel. Real BIOSes that expect an `INT 13h` to wait on a motor that is not running will time out themselves; one that needs an interrupt the call has masked (IRQ 1 and 12 on purpose) will wait for it.
- **BIOSes that touch more than the registers.** VBE in particular can change the DAC, the video memory and the card's extended registers; that is the point of using it, and the kernel must not assume a VGA register state across a call. Some BIOSes move data around in the EBDA or use memory just above the bounce buffer; the map reserves nothing past `0x14000`, and a BIOS that writes there would corrupt whatever the kernel puts there later (nothing yet).
- **SMM and APM or laptop BIOSes.** System management interrupts, an APM BIOS that wants its own segments and a stack in the 16-bit protected mode (the APM 32-bit interface is not used), and BIOSes that keep a trap flag on, are not exercised at all.
- **16-bit BIOSes written for a DOS-like state.** A BIOS call may assume `SS:SP` has a few hundred bytes (we give 4 KiB), DS is the caller's (it is, from the register block), the direction flag is as the caller's flags give it (clear unless asked), and A20 is in the state the loader left (on).
- **Interrupt flag and pending interrupts.** The call runs the BIOS with interrupts off at entry (as `INT` does) and lets the BIOS decide; a BIOS that expects interrupts from a device the call has masked will stall. The mask above is the set that the BIOSes tried needed.
- **The timer accounting is exact only because the kernel's rate is the BIOS tick's.** If the kernel changes the PIT rate (a lower tick frequency), a BIOS tick stops being a kernel tick and the catch-up in `bios_int` would add the wrong amount; the conversion is one multiply, to be written when the rate changes.
- **Stage 2 and the blob share a staging area.** The loader reads the kernel to `0x10000` and copies it up; the blob is copied there after that, in `bios::init`. A loader that kept using that area after the kernel started would conflict; none does.

## What was and was not verified

**Run:**

- `tools/native-test.sh` end to end on the development machine (QEMU 11.1.2, NASM 3.02, SeaBIOS under `-cpu 486 -m 16`), including the new BIOS section on std, cirrus and vmware, the IDE run, the Cirrus run of spike D's tour and the negative checks; `tools/check-headers.sh`; the HAL's host tests, clippy and the kernel-target clippy.
- The image on Ubuntu 24.04's QEMU 8.2.2 (the `desk-native-qemu:1` Docker image): transcript and screenshots equal on all three cards.
- The `bios-solo` image on Bochs 2.7 built as a 486 (the Docker image of the Bochs spike) with BIOS-bochs-legacy, BIOS-bochs-latest and SeaBIOS 1.13: the results above.
- Screenshots looked at by eye (the VBE picture at 800×600 and the restored text screen on Cirrus).

**Not run or not verified:**

- **Any real BIOS or real hardware.** Every BIOS above is an emulator's. A real 486 BIOS, a VGA BIOS for a real S3 or Cirrus, a real floppy controller and a real 8042 and PIC (the order of ICW writes, the delays, `ICW1` clearing the request register) were not seen. The 8259 re-initialisation in particular is the thing most likely to differ on a chipset with an integrated controller.
- **86Box.** Its AMIBIOS 486 board has neither E820 nor E801, and this branch's loader needs one, so the kernel never starts there; the loader of the 86Box spike has an `INT 15h` AH=88h fallback that is not part of this branch.
- **The Bochs runs have no key injection and no screenshots**: they prove the calls and the VBE sequence, not the text screen's pixels or the lost-key path.
- **The hard-disk LBA path on Bochs and on the older QEMU.** Only QEMU 11.1.2 ran it.
- **Paging, V86, NMI delivery and a BIOS that enables NMI**, **APM and SMM**, **a BIOS that never returns**: none exist or were exercised.
- **Cost on a real 486**, and the guess in [Cost](#cost).
- **The reference checksums on other QEMU versions** beyond 8.2.2 and 11.1.2, and on a card other than the three.

## What it changes

- **`docs/D-NATIVE-VGA.md`**: the Cirrus finding is closed (a kernel with the trampoline leaves VBE through the BIOS on any card), the recommendation of a native `Bios` is done, and the "one-way trip" of VBE is no longer true when the trampoline is present.
- **`docs/NATIVE-BOOT.md`**: the low-memory map gains the trampoline, the bounce buffer and the real-mode stack, and the kernel's section gains the BIOS path.
- **`docs/HAL.md`**: the open question on VBE natively is answered; the feedback is above.
- **The next steps this enables**: a mode-setting `Display` for the native platform that changes resolution after boot; a floppy reader for a file system before native floppy and IDE drivers; a mouse and keyboard that survive calls (done); and the choice between a V86 monitor and FreeDOS-by-reboot, which can now be made with a working BIOS path to compare against.
