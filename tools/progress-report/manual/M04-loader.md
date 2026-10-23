---
id: m-loader
n: M4
part: II
title: The boot sector and the loader
summary: The floppy image, the 512-byte boot sector, stage 2, the A20 test, the memory map, the boot information block, the switch to protected mode and the copy to 1 MiB.
date: 2026-10-23
reviewed: 44f0265
watches: [boot/, tools/native-image.sh, tools/native-build.sh, kernel/link.ld]
log: [boot, pixels, emulators]
anatomy: [desk:boot]
---
## What it is

The loader is everything that runs between the BIOS handing over control and the first instruction of the Rust kernel. It is two programs written in NASM: **stage 1**, the 512-byte [boot sector](term:boot-sector), whose only job is to read the rest of the floppy into memory, and **stage 2**, which runs in 16-bit real mode while the BIOS can still be called, collects everything the kernel will need to know about the machine, switches the processor to 32-bit protected mode and starts the kernel at 1 MiB. In the map of the system ([M1](chapter:m-map)) it is the box `boot` of the native track, between the BIOS below it and the kernel above it, and on the [anatomy page](chapter:anatomydesk#box-desk-boot) it is the one box that only exists on the native track: the hosted track starts from DOS and has no loader of its own.

The loader is small and old-fashioned on purpose. It cannot assume a file system, a heap, or even that the memory above 1 MiB is reachable when it starts. What it can rely on is the BIOS: a ROM full of services reached with software interrupts, which exist only in real mode. That single fact shapes the whole design. Everything that needs the BIOS has to happen before the switch to protected mode, so the loader asks every question the kernel will want answered (how much memory is there, what font is on the screen, which video mode can we have) and leaves the answers in a small block of memory.

After this chapter you should be able to say what each of the 2,880 sectors of the image is for, follow a read of the kernel from the floppy controller to the right place in memory, explain why the loader opens the A20 gate and how it knows it worked, read the boot information block, decode the three descriptors of the loader's GDT by hand, and change the loader without breaking a test you did not know existed. The processor facts it leans on (segments, the A20 gate, descriptors, the switch) are taught in [M2](chapter:m-cpu), and the devices it talks to (the keyboard controller, the serial port, the BIOS services) in [M3](chapter:m-devices); this chapter says what the loader does with them. The next chapter, [M5](chapter:m-boot), follows the whole boot from power-on to `boot complete`, line by line.

## How it works

### The image and what is on it

The native track boots from a floppy disk image: 2,880 sectors of 512 bytes, 1,474,560 bytes, the geometry of a 3.5-inch high-density disk (80 cylinders, 2 heads, 18 sectors per track). It is assembled by `tools/native-image.sh` from three pieces that `tools/native-build.sh` makes, and laid out so that every read the loader makes is a whole track or part of one.

| Sectors (LBA) | What | Size |
|---|---|---|
| 0 | Stage 1, the boot sector | 512 bytes |
| 1 to 17 | Stage 2, padded to 17 sectors | 8,704 bytes |
| 18 onward | The flat kernel image: 340 sectors in the builds this chapter was checked with (173,968 bytes in the one rebuilt for its commands; `snapshots/measured.json`, which the figure reads, records 174,032) | about 174,000 bytes |
| the rest | Zeros, to 2,880 sectors | |

Three bytes are patched into the image after the build, so the loader never has to be rebuilt when the kernel grows or a test wants a different behaviour: the kernel's length in sectors, as a 16-bit number at offset 3 of stage 1; the *test mode* at offset 3 of stage 2 (a byte the kernel reads to decide whether to misbehave on purpose, taught in [M5](chapter:m-boot)); and the *video request* at offset 4 of stage 2 (0 keeps the BIOS text screen, 1 asks for a graphics mode). Each of these is a field at a fixed offset behind a three-byte opening (a short jump and a `nop`) at the start of its stage, which is why both stages begin with `jmp short entry` and a `nop`.

### Stage 1: the boot sector

When the machine is switched on the BIOS runs its self-test, finds a drive to boot from, reads the first sector of that drive to physical address `0x7C00` and jumps to it, in real mode, with the BIOS's number for the drive in the `DL` register. That sector is only accepted as a boot sector if its last two bytes are the [boot signature](term:boot-signature), `0x55` then `0xAA`. Stage 1 ends with `dw 0xAA55` (a 16-bit word, stored little-endian, so the bytes on the disk are `55 AA`) after padding the file to exactly 510 bytes with zeros.

> **From the stacks — The two bytes at the end.** The last two bytes of a boot sector are `0x55 0xAA`, at offsets `0x1FE` and `0x1FF`. According to the Wikipedia article on the boot sector, most BIOSes have checked for the signature since at least the IBM PC/AT, while the original IBM PC and some other machines did not. The article does not say who chose the value or why. For the Desk it means only that the sector carries the signature in case the BIOS we boot on is one that looks.
> Source: [Boot sector, Wikipedia](https://en.wikipedia.org/wiki/Boot_sector)

Stage 1 then does what every real-mode program must do first, in this order:

1. Make `CS`, `DS`, `ES` and `SS` all mean segment 0 (some BIOSes enter at `07C0:0000`, which is the same address with a different segment) and put the stack just below the sector, so that it grows down from `0x7C00` away from the code. The far jump to `0000:.flushed` fixes `CS`.
2. Save `DL`, because INT 13h needs the drive number back on every call.
3. Print `Desk boot` with the BIOS teletype service (INT 10h, AH=0Eh). This goes to the screen only; the serial port is not yet set up.
4. Read stage 2 to `0000:7E00`, directly after the boot sector, then the kernel into the low buffer at `0x10000`.
5. Jump to `0000:7E00` with `DL` still holding the drive.

The kernel is read to `0x10000` and not to its final home because real-mode addresses do not reach 1 MiB, and the A20 gate that would allow it is not open yet. The kernel waits there until stage 2 has switched to protected mode and can move it.

**Reading from the floppy.** `read_sectors` turns a logical sector number into a place on the disk and calls the BIOS. The BIOS interface is [CHS](term:chs) (cylinder, head, sector), but the image's layout is described in [LBA](term:lba), a single sector number counting from zero. The conversion is arithmetic on the geometry: sector = LBA mod 18 + 1 (sectors count from 1), head = (LBA ÷ 18) mod 2, cylinder = LBA ÷ 36. Two rules decide how many sectors one call reads:

- **Up to the end of the track.** A floppy controller can stream a whole track in one rotation, so the loader reads from the starting sector to the end of the track, not one sector at a time.
- **Never across a 64 KiB boundary.** The floppy controller moves its data into memory by [DMA](term:dma), and the PC's DMA controller works on 64 KiB pages: a transfer cannot cross a physical address that is a multiple of `0x10000`. The loader computes how many sectors fit before the next boundary, `(0x1000 - (ES & 0xFFF)) / 32` sectors, and reads no more than that, then continues with the next call.

Each call is INT 13h AH=02h with `AL` the sector count, `CH` the cylinder, `CL` the sector, `DH` the head and `ES:BX` the destination. On an error the loader resets the disk system (AH=00h), tries again, and gives up after five tries by printing `Disk read error` and stopping. Stopping means writing the value `0x11` to port `0xF4`, which ends a QEMU run with a failure status and does nothing anywhere else, and then halting. Stage 1 prints a dot after every call, which is the only progress indicator a floppy boot has.

<!-- explainer m-image -->

The figure replays this algorithm on the real constants and the real kernel size, one INT 13h call per step. It is a replay of the code, not a capture of the controller: the evidence that the reads worked is that the kernel arrives intact and boots.

### Stage 2: collecting what the kernel will need

Stage 2 is the loader proper. Its header comment lists its seven steps, and each one exists because the kernel cannot do it for itself.

**1. Output.** Stage 2 starts the first serial port, COM1, at 115,200 baud, 8 data bits, no parity, one stop bit, with the FIFOs on, and every character it prints goes both to the BIOS screen and to the serial port. A headless run of the boot (the only kind the tests do) therefore records the loader's lines too. Writing a byte waits, bounded, for the transmitter to be free.

**2. The A20 gate.** The 8086 could address a megabyte, and an address past the top wrapped round to the bottom; the PC kept that behaviour behind a gate until the program asks for it to be switched off ([M2](chapter:m-cpu) tells the history). Stage 2 does not assume the gate's state: it tests it. It writes `0x00` at `0000:0500` and `0xFF` at `FFFF:0510` and reads back the first. The first address is physical `0x00500`; the second is `0xFFFF0 + 0x510 = 0x100500`, one megabyte and 0x500 bytes up. With the gate closed, bit 20 of every address is forced to zero and the two are the same byte, so the read-back is `0xFF`. With the gate open they are different bytes, and the read-back is `0x00`. The original contents are put back afterwards.

If the gate is closed, stage 2 tries two ways to open it, testing after each (the test is repeated for a while, because hardware may take a moment):

- **The fast gate:** bit 1 of the system control port at `0x92`, written with bit 0 clear, because setting bit 0 of that port resets the CPU.
- **The keyboard controller:** the PC/AT's original way, where the gate is bit 1 of the controller's output port. The loader disables the keyboard (command `0xAD` on port `0x64`), reads the output port (`0xD0`), writes it back with bit 1 set (`0xD1`), and re-enables the keyboard (`0xAE`), waiting for the controller between each step. The waits are bounded, so a dead controller cannot hang the boot.

If neither works the loader prints `cannot enable a20` and stops. It reports which path it took: `a20 already enabled`, `a20 enabled by the fast gate` or `a20 enabled by the keyboard controller`.

**3. The memory map.** Only the BIOS knows what RAM the machine has, and it can only be asked in real mode. Stage 2 tries three BIOS calls in order, from the newest to the oldest, and the first that works wins.

| Call | What it gives | What the loader does with it |
|---|---|---|
| INT 15h AX=E820h, the [E820](term:e820) map | The whole map, one 24-byte entry per call: a 64-bit base, a 64-bit length and a type (1 is usable RAM) | Walks it with `EBX` as the continuation, keeps up to 32 entries, skips empty ones and ones marked "ignore" |
| INT 15h AX=E801h, the [E801](term:e801) call | Two numbers: KiB between 1 MiB and 16 MiB, and 64 KiB blocks above 16 MiB | Rewrites them as entries in the E820 shape, preceded by conventional memory from INT 12h |
| INT 15h AH=88h | KiB above 1 MiB, nothing more (15 MiB at most on most BIOSes) | The same, with two entries |

Rewriting the older answers in the newer format is a design choice with a payoff: the kernel parses one format and never learns which call produced it. It is told, though, because the loader records which source it used (1 for E820, 2 for E801, 3 for AH=88) and prints it. The figure steps through the three calls in the order the loader tries them, with the line each prints.

<!-- explainer memchain -->

**4. The boot information block.** The loader writes everything it has learned at the fixed physical address `0x500`, in the [boot information block](term:boot-information-block), and passes that address to the kernel. All fields are 32 bits:

| Offset | Field | Value in a normal boot |
|---|---|---|
| 0 | magic | `DESK`, `0x4B534544` |
| 4 | boot drive | `0` (the first floppy) |
| 8 | test mode | the patched byte; 0 for a normal boot |
| 12 | map source | 1 E820, 2 E801, 3 AH=88 |
| 16 | map entries | 6 in the QEMU run quoted in M5 |
| 20 | version | 1 (a kernel reads the next two fields only when this is at least 1) |
| 24 | address of the video block | `0x820` |
| 28 | address of the BIOS font | `0x2000`, or 0 for none |
| 32 on | the memory map, 24 bytes per entry | up to 32 entries, which fills the space up to `0x820` |

The version field exists so the block can grow without breaking a kernel built for an older loader. The design of the block and of the video block that follows it is in `docs/D-NATIVE-VGA.md`. The figure shows what the loader asks the BIOS for and where the answers end up.

<!-- figure bootinfo -->

**5. The font and the video request.** The kernel cannot call the BIOS, and the one thing it cannot get back once it has left text mode is the BIOS's 8×16 character font. Stage 2 asks for it (INT 10h AX=1130h, BH=06h) and copies the 4,096 bytes to `0x2000`. If the image asks for it (the patched byte at offset 4), stage 2 then also asks VBE for an 8-bit mode with a linear frame buffer: it reads the controller information, walks the mode list, picks a mode (640×480 if the BIOS lists one, otherwise the largest up to 1,280 pixels wide), sets it, and records the whole outcome, success or the reason for failure, in a 32-byte video block at `0x820`. This step never fails the boot. [M12](chapter:m-graphics) teaches what VBE is and how the kernel uses the result.

**6. Protected mode.** Stage 2 disables interrupts (there are no interrupt handlers for protected mode yet), loads the address of a three-entry descriptor table with `lgdt`, sets bit 0 of control register CR0 (`PE`, protection enable), and does a far jump to selector `0x08`. The far jump has two jobs: it loads `CS` with the 32-bit code segment, and it discards instructions the processor had already fetched in 16-bit mode. Both the table and the switch are taught in [M2](chapter:m-cpu); the three descriptors of this table are decoded below.

**7. The copy and the jump.** In 32-bit code, with all data segments set to selector `0x10`, stage 2 sets a stack at `0x7000`, reads the kernel's length in sectors from stage 1's still-resident sector (`0x7C03`), copies that many sectors, as 32-bit words, from `0x10000` to `0x100000`, puts `0x500` in `EAX` and jumps to `0x100000`. The kernel's first instructions, and what they do with `EAX`, are in [M5](chapter:m-boot).

Stage 2 can be built with flags that exercise the paths a normal QEMU boot never takes: `-DA20_TEST=1` closes the gate first so the fast-gate code runs, `-DA20_TEST=2` also skips the fast gate so the keyboard-controller code runs, `-DNO_E820` pretends the BIOS lacks E820, `-DNO_E820 -DNO_E801` pretends it lacks both, and `-DVBE_FAIL=1` makes the final VBE call report failure. `tools/native-test.sh` builds and boots each.

## Why it is this way

- **Two stages.** A boot sector has 512 bytes, and one of those is for the signature. Reading a floppy by tracks with retries, testing the A20 gate three ways, walking three memory-map calls and asking VBE for a mode is many times that. Stage 1 can only read; stage 2, which has 17 sectors (8,704 bytes; the build fails if it outgrows them), does the thinking. This is the usual split, recorded as the boot flow in `docs/NATIVE-BOOT.md`.
- **Real mode for everything that needs the BIOS.** The kernel runs in protected mode with flat segments, and the BIOS does not run there. Letting the kernel call the BIOS again was explored later, with a trampoline that drops back to real mode for one call ([M8](chapter:m-bioscalls), `docs/N3-BIOS-PATH.md`), but the loader still asks its questions up front: the font and the memory map must be known before anything else runs.
- **One memory-map format.** The fallbacks exist because a machine's BIOS may be older than the calls. The AH=88 fallback was added after 86Box, emulating a 1993 AMIBIOS, showed that BIOS had neither E820 nor E801 and the loader stopped with `no memory map from the BIOS` ([PR #16](pr:16); `docs/NATIVE-EMULATORS.md`, "What the emulators exposed"). It reports at most 15 MiB on most BIOSes and 63 MiB on some, so RAM above that is invisible if it is the only call available. That is a cost the loader accepts.
- **Patched bytes instead of build flags.** Changing the kernel's length or the test mode means rewriting one byte of the image, not reassembling the loader, so one loader binary serves every test mode (`docs/NATIVE-BOOT.md`, "Image layout"). The loader variants that do need different code (the A20 and memory-map paths) are build flags, because they change the code that runs.
- **A fixed address and a version.** A block at a known address with a version number is the simplest extension scheme that is safe: a version 2 may add fields at the end, and a kernel that only knows version 1 ignores them (`docs/D-NATIVE-VGA.md`, "The boot information block").
- **The geometry is assumed.** Stage 1 hard-codes 18 sectors per track and 2 heads. Booting from a hard disk or from a drive with another geometry is not supported, and `docs/NATIVE-BOOT.md` says so.
- **The kernel is staged below 1 MiB, then copied.** Real mode cannot address the megabyte the kernel is linked at (`kernel/link.ld` places it at `0x100000`). The copy waits until protected mode, where flat 32-bit addresses reach it.

> **From the stacks — A megabyte that is not a megabyte.** The image is called a 1.44 MB floppy, and it is 1,474,560 bytes, which is 2,880 sectors of 512 bytes. The Wikipedia article on floppy disks says the double-sided high-density drive's capacity was "actually 1440 KiB", and that the 1.44 MB (1,474,560 bytes) high-density version shipped with the IBM Personal System/2 line in 1987. So the label is a rounding of 1,440 KiB, and the Desk's image is the whole disk, byte for byte.
> Source: [Floppy disk, Wikipedia](https://en.wikipedia.org/wiki/Floppy_disk)

## The code

| Where | What it holds |
|---|---|
| [`boot/stage1.asm`](repo:boot/stage1.asm) | The boot sector. [`read_sectors`](sym:boot/stage1.asm::read_sectors) turns LBA into CHS and splits at tracks and 64 KiB boundaries; [`read_chunk`](sym:boot/stage1.asm::read_chunk) makes one INT 13h call with retries |
| [`boot/stage2.asm`](repo:boot/stage2.asm) | The loader. [`a20_enabled`](sym:boot/stage2.asm::a20_enabled) is the test, [`a20_keyboard`](sym:boot/stage2.asm::a20_keyboard) the fallback, [`map_e820`](sym:boot/stage2.asm::map_e820), [`map_e801`](sym:boot/stage2.asm::map_e801) and [`map_88`](sym:boot/stage2.asm::map_88) the three maps, [`video_setup`](sym:boot/stage2.asm::video_setup) and [`load_font`](sym:boot/stage2.asm::load_font) the video, [`gdt`](sym:boot/stage2.asm::gdt) and [`pm_entry`](sym:boot/stage2.asm::pm_entry) the switch and the copy |
| [`tools/native-build.sh`](repo:tools/native-build.sh) | Assembles both stages (with NASM, from the host or from Docker) and builds the kernel; `--nasm-flags` passes the variant flags |
| [`tools/native-image.sh`](repo:tools/native-image.sh) | Lays out the image and patches in the kernel length, the test mode and the video request |
| [`kernel/src/main.rs`](repo:kernel/src/main.rs) | [`BootInfo`](sym:kernel/src/main.rs::BootInfo) is the kernel's view of the block the loader writes |
| [`kernel/link.ld`](repo:kernel/link.ld) | Places the kernel at `0x100000`, which is the address stage 2 jumps to |
| [`boot/README.md`](repo:boot/README.md) | A short guide to the directory |

For depth: `docs/NATIVE-BOOT.md` ([boot flow](doc:docs/NATIVE-BOOT.md#boot-flow) and [image layout](doc:docs/NATIVE-BOOT.md#image-layout)), `docs/D-NATIVE-VGA.md` ([the loader's VBE step](doc:docs/D-NATIVE-VGA.md#the-loader-vbe-before-protected-mode) and [the boot information block](doc:docs/D-NATIVE-VGA.md#the-boot-information-block)) and `docs/NATIVE-EMULATORS.md` ([what the emulators exposed](doc:docs/NATIVE-EMULATORS.md#what-the-emulators-exposed)).

## Worked example

**The kernel's 340 sectors.** The kernel is about 174,000 bytes (173,968 in the build rebuilt for the commands below), which is a little under 340 sectors of 512 bytes, so the image tool rounds up and patches 340 into stage 1 (the bytes `54 01`, little-endian, at offset 3). It starts at LBA 18. LBA 18 divided by 18 is track 1 with no remainder, so sector 1; track 1 is head 1 of cylinder 0. Stage 1 reads to `0x10000`, and the first read takes the whole track: 18 sectors, which end at `0x12400`.

Seven full tracks later the reads are at `0x1FC00`, and the track in hand (LBA 144, cylinder 4, head 0) has 18 sectors but only 2 of them fit before `0x20000`, the next 64 KiB boundary. So the read is split. It reads 2 sectors, and the next call picks up at LBA 146. That is 146 ÷ 18 = 8 remainder 2, so sector 3 of track 8; head = 8 mod 2 = 0; cylinder = 8 ÷ 2 = 4. It reads the remaining 16 sectors of the track to `0x20000`–`0x21FFF`. The same split happens at `0x30000`, where the track at LBA 270 is read as 4 sectors and then 14. The whole kernel takes 21 calls of INT 13h. The explainer above shows each.

**The loader's three descriptors.** The table in `boot/stage2.asm` is `0` for the null descriptor, then `0x00CF9A000000FFFF` for code and `0x00CF92000000FFFF` for data. Written as the eight bytes the processor reads (least significant first), the code descriptor is `FF FF 00 00 00 9A CF 00`:

| Bytes | Field | Value | Meaning |
|---|---|---|---|
| `FF FF` | limit, bits 0 to 15 | `0xFFFF` | together with the nibble below: a limit of `0xFFFFF` |
| `00 00 00` and `00` | base, bits 0 to 23 and 24 to 31 | 0 | the segment starts at address 0 |
| `9A` | access | `1001 1010` | present, ring 0, a code or data segment, executable, readable |
| `CF` | flags and limit bits 16 to 19 | `1100` and `1111` | 4 KiB granularity, 32-bit, and limit bits `0xF` |

With the granularity bit set, the limit `0xFFFFF` counts 4 KiB units: (`0xFFFFF` + 1) × 4,096 = 4 GiB. The data descriptor differs only in its access byte, `0x92` (`1001 0010`): not executable, writable. Both start at 0 and span the whole 32-bit address space, so a segment-relative address and a physical address are the same number, which is what "flat" means. Selector `0x08` is the second entry (index 1, times 8) and `0x10` the third.

**The memory the BIOS reported.** In the recorded QEMU run the six E820 entries that stage 2 stored include two of usable RAM: `0x00000000`–`0x0009fc00` and `0x00100000`–`0x00fe0000`. The first is 0x9fc00 = 654,336 bytes = 639 KiB. The second is 0xee0000 = 15,597,568 bytes = 15,232 KiB. Together, 639 + 15,232 = 15,871 KiB, which is what the kernel prints as `memory: usable RAM 15871 KiB`. The same total comes out of the E801 and AH=88 builds (below), each with only two entries.

## State of this layer

Verified here means that the record cites a test that runs in `tools/check.sh` or a merged pull request whose CI passed. It means in QEMU, never on a real machine.

```status
id: loader.stage1
label: Stage 1 loads stage 2 and the kernel by track, with 64 KiB splits
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::boot complete, sym:boot/stage1.asm::read_sectors
note: Every boot reads the kernel across two 64 KiB boundaries and arrives intact. QEMU may not enforce the boundary itself; the rule is the hardware's, recorded in the docs.

id: loader.retry
label: Stage 1 retries a failed read five times and then stops with a message
state: built-unverified
evidence: sym:boot/stage1.asm::read_chunk, doc:docs/NATIVE-BOOT.md#boot-flow
note: No test makes a read fail, so the retry and the Disk read error path have never run.

id: loader.a20
label: A20 is tested, and opened by the fast gate or the keyboard controller
state: built-verified
evidence: test:tools/native-test.sh::a20-fast, test:tools/native-test.sh::a20-keyboard, test:tests/native/expected.txt::a20 already enabled, sym:boot/stage2.asm::a20_enabled
note: The two opening paths run only in the test builds with the gate closed on purpose, because QEMU opens it before the boot sector runs.

id: loader.e820
label: The E820 memory map, in the shared entry format
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::memory map from E820, sym:boot/stage2.asm::map_e820

id: loader.fallbacks
label: The E801 and AH=88 fallbacks, rewritten as E820-style entries
state: built-verified
evidence: test:tools/native-test.sh::e801, test:tools/native-test.sh::e88, sym:boot/stage2.asm::map_88

id: loader.bootinfo
label: The boot information block at 0x500, with a version
state: built-verified
evidence: test:tests/native/expected.txt::boot: drive 0x00, test:tests/native/expected.txt::video: boot info version 1, sym:kernel/src/main.rs::BootInfo

id: loader.font
label: The BIOS font is copied for the kernel's return to text mode
state: built-verified
evidence: test:tests/native/expected.txt::stage2: font: 8x16 copied from the BIOS, test:tests/native/expected.txt::video: 8x16 font, sym:boot/stage2.asm::load_font

id: loader.vbe
label: A VBE 2.0 linear frame buffer mode, or a recorded reason why not
state: built-verified
evidence: test:tests/native/expected-gfx-vbe.txt::stage2: vbe: mode set with the linear frame buffer, test:tests/native/expected-gfx-vbefail.txt::stage2: vbe: the BIOS refused to set the mode, sym:boot/stage2.asm::video_setup
note: Verified on QEMU's standard VGA card. Real cards may list other modes; see M12.

id: loader.switch
label: Flat GDT, switch to protected mode and the copy to 1 MiB
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::gdt: loaded, sym:boot/stage2.asm::pm_entry

id: loader.image
label: The image tool: layout, patched bytes and the size limits
state: built-verified
evidence: sym:tools/native-image.sh::ksectors, test:tests/native/expected.txt::boot complete

id: loader.bochs
label: The same image boots in Bochs and, after a loader fix, in 86Box on a 1993 AMIBIOS
state: built-unverified
evidence: pr:16, doc:docs/NATIVE-EMULATORS.md#results
note: Run by hand and recorded in the document; CI does not run Bochs or 86Box, and the pull request is not shown merged with CI passing.

id: loader.harddisk
label: Booting from a hard disk, a CD-ROM or another floppy geometry
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified

id: loader.hardware
label: Booting on a real machine
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The loader's reads, its three fallbacks and its switch are all {{state.loader.switch}} in QEMU. What the table cannot say is how a real BIOS and a real floppy controller behave: the 64 KiB rule, the retry path and the A20 methods are written from the hardware's documented behaviour, and only the emulators have been asked.

## Try it yourself

These commands build the loader and the image, look at the bytes this chapter talks about, and boot the loader variants. They need `nasm` and a Rust toolchain on the host (or the Docker fallbacks the scripts use) and `qemu-system-i386`. Output is from a run on 2026-10-23 with NASM 3.02 and QEMU 11.1.2 at `44f0265`; the sizes change when the kernel does.

```try
needs: host: bash, nasm, cargo, qemu-system-i386, xxd (or Docker for nasm and qemu)
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh
dist/native/kernel.bin: 173968 bytes, load address 0x100000
$ tools/native-image.sh
dist/desk-native.img: 1474560 bytes, kernel 173968 bytes in 340 sectors, test mode none, video text
$ xxd -s 510 -l 2 dist/desk-native.img
000001fe: 55aa                                     U.
$ xxd -s 3 -l 2 dist/desk-native.img
00000003: 5401                                     T.
$ xxd -s 515 -l 2 dist/desk-native.img
00000203: 0000                                     ..
```

The first `xxd` shows the signature, the second the kernel length 340 (`0x0154`) stored low byte first, and the third the stage 2 test mode and video request bytes, both zero for a normal boot. Now boot it and look at the loader's lines, then rebuild the loader as though the BIOS had no E820, and again as though it had neither E820 nor E801:

```try
needs: host: bash, nasm, cargo, qemu-system-i386
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-run.sh --serial dist/serial.txt > /dev/null; tr -d '\r' < dist/serial.txt | sed -n 2,6p
stage2: loader running
stage2: a20 already enabled
stage2: memory map from E820, entries: 6
stage2: font: 8x16 copied from the BIOS
stage2: vbe: not requested, the screen stays in text mode
$ tools/native-build.sh --nasm-flags "-DNO_E820" > /dev/null && tools/native-image.sh --out dist/e801.img > /dev/null && tools/native-run.sh --serial dist/e801.txt dist/e801.img > /dev/null; tr -d '\r' < dist/e801.txt | grep -E "memory map from|usable RAM"
stage2: memory map from E801, entries: 2
boot: drive 0x00, memory map from E801 (2 entries)
memory: usable RAM 15871 KiB
$ tools/native-build.sh --nasm-flags "-DNO_E820 -DNO_E801" > /dev/null && tools/native-image.sh --out dist/e88.img > /dev/null && tools/native-run.sh --serial dist/e88.txt dist/e88.img > /dev/null; tr -d '\r' < dist/e88.txt | grep -E "memory map from|usable RAM"
stage2: memory map from INT 15h AH=88, entries: 2
boot: drive 0x00, memory map from INT 15h AH=88 (2 entries)
memory: usable RAM 15871 KiB
$ tools/native-build.sh
dist/native/kernel.bin: 173968 bytes, load address 0x100000
```

The last command puts the normal loader back. To run all of the loader variants and the checks that go with them, `tools/native-test.sh` does everything the CI does for the native boot; it takes some minutes.

**Exercises.** (1) Without running anything, work out the cylinder, head and sector of LBA 18, LBA 36 and LBA 146, then find them in the figure. (2) Before looking at the output above, predict why the E801 and AH=88 builds have only two entries and still print the same usable RAM as the E820 build, which has six; the answer is in the E820 entries of the worked example. (3) In a scratch branch, change `RETRIES` in `boot/stage1.asm` to 1 and say which test, if any, notices; then say what you would have to do to a test to make a read fail on purpose.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Boot sector
def: The first 512-byte sector of a bootable disk. The BIOS reads it to physical address 0x7C00 and jumps to it. On the Desk's floppy it is stage 1 of the loader.

term: Boot signature
def: The two bytes 0x55 0xAA at the end of a boot sector (offsets 510 and 511), which a BIOS may check before it will run the sector.

term: CHS
def: Cylinder, head, sector: the way the BIOS's INT 13h disk calls name a place on a floppy. Cylinders and heads count from 0, sectors from 1.

term: LBA
def: Logical block address: a sector's number counting from 0 across the whole disk. The loader converts it to CHS for the BIOS.

term: DMA
def: Direct memory access. A device moves data into memory without the CPU copying it. The PC's DMA controller used for the floppy cannot carry a transfer across a 64 KiB physical boundary.

term: E820
def: INT 15h function E820h, the BIOS call that returns the physical memory map one entry at a time, each with a base, a length and a type (1 is usable RAM).

term: E801
def: INT 15h function E801h, an older BIOS call that reports the memory between 1 and 16 MiB in KiB and the memory above 16 MiB in 64 KiB blocks.

term: Boot information block
def: The block of facts stage 2 leaves at physical 0x500 for the kernel: a magic number, the boot drive, the test mode, where the memory map came from, the map itself, and the addresses of the video block and the BIOS font.
```

```figures
id: m4-image
kind: explainer
track: native
title: The floppy image and the reads of the boot sector
alt: A step-through of stage 1 reading stage 2 and the kernel from the floppy, one INT 13h call at a time, showing the track read and the memory it fills, with the splits at 64 KiB boundaries.
caption: Stage 1's 22 reads, replayed from the real constants, with the splits at 0x20000 and 0x30000.
at: explainer m-image
thumb: steps

id: m4-memchain
reuse: memchain
at: explainer memchain

id: m4-bootinfo
reuse: boot-info
at: figure bootinfo
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every status row was checked against the repository and the commands above were run on the host. Two disagreements between the documents and the code were found: `docs/D-NATIVE-VGA.md` lists the map source as 1 (E820) or 2 (E801) where the code also uses 3 (AH=88), and `docs/NATIVE-BOOT.md` says `tests/native/wrong-expected.txt` "differs in one number", while the file is a stale copy that is rejected at its first missing line (see M5).
