---
id: m-memory
n: M7
part: III
title: The kernel: memory and the x87
summary: The memory map as the loader leaves it, the frame bitmap, the heap, the usable-RAM arithmetic and the x87 set-up.
date: 2026-10-23
reviewed: 44f0265
watches: [kernel/src/memory.rs, kernel/src/heap.rs, kernel/src/fpu.rs, kernel/link.ld, docs/NATIVE-BOOT.md]
log: [boot, emulators]
anatomy: [desk:kernel]
---
## What it is

This chapter covers the part of the kernel that answers two questions before anything else can run: which bytes of the machine are mine to hand out, and can I do arithmetic on numbers with a fractional part. The first is `kernel/src/memory.rs`, which turns the list of regions the loader left behind into a table of 4 KiB [frames](term:frame), and `kernel/src/heap.rs`, which turns four megabytes of those frames into the heap behind Rust's `Vec`, `String` and `format!`. The second is `kernel/src/fpu.rs`, which switches on the [x87](term:x87) floating-point unit and proves it works. On the map of the system ([M1](chapter:m-map)) they are three files in the `kernel` box of the native track, on the [anatomy page](chapter:anatomydesk#box-desk-kernel), and in the boot order of [M5](chapter:m-boot) they run in the middle: the memory report comes right after the timer is programmed, the heap just after the BIOS trampoline, and the x87 after the first ten timer ticks.

The kernel has no paging and no memory protection. Every address a Rust pointer holds is a physical address, the flat segments of the [GDT](term:gdt) cover all 4 GiB, and any code can write anywhere. What the memory manager does is bookkeeping: it knows which frames are in use, so that two parts of the kernel do not take the same frame, and so that nothing is ever handed a frame that holds the loader's tables, the video memory or the kernel itself. That is a smaller promise than an operating system's, and the last section says what is missing.

After the chapter you should be able to say what the six `map` lines of the boot transcript mean, work out by hand the numbers `15871 KiB` and `3748 frames` that follow them, say why the heap starts at `0x13c000`, follow an allocation and a free through the free list, and read the one line of `fpu:` output as a check and not a decoration. The map itself is made by the loader and taught in [M4](chapter:m-loader); the processor facts used here (the control register [CR0](term:cr0), the flat model) are in [M2](chapter:m-cpu).

## How it works

### The map the loader leaves

The loader writes the [boot information block](term:boot-information-block) at physical `0x500`, and the memory map follows its 32-byte header: `map_count` entries of 24 bytes, each a 64-bit base, a 64-bit length, a 32-bit type and a 32-bit ACPI word. That is the shape of an [E820](term:e820) entry, and the loader rewrites the answers of the older calls into the same shape ([M4](chapter:m-loader)), so the kernel parses one format. [`memory::regions`](sym:kernel/src/memory.rs::regions) is the whole parser: it takes the address of the block, adds 32, and calls the result a slice of `Region`. The type `1` is [`KIND_USABLE`](sym:kernel/src/memory.rs::KIND_USABLE); `2` is reserved, `3` ACPI reclaimable, `4` ACPI NVS and `5` bad memory, and `report` prints those names.

In the QEMU run recorded for the progress report (`tools/progress-report/media/native-serial.txt`; the boot is walked through in [M5](chapter:m-boot)) the six entries are:

| Base | End | Type | Length |
|---|---|---|---|
| `0x00000000` | `0x0009fc00` | usable | 654,336 bytes (639 KiB) |
| `0x0009fc00` | `0x000a0000` | reserved | 1 KiB, where the BIOS keeps its extended data area |
| `0x000f0000` | `0x00100000` | reserved | 64 KiB, the system BIOS ROM |
| `0x00100000` | `0x00fe0000` | usable | 15,597,568 bytes (15,232 KiB) |
| `0x00fe0000` | `0x01000000` | reserved | 128 KiB at the top of the 16 MiB the VM was given |
| `0xfffc0000` | `0x100000000` | reserved | 256 KiB at the top of the 32-bit address space, where the firmware ROM is mapped |

The two usable entries are the 639 KiB of conventional memory and the 15 MiB above the first megabyte, less the hole at the top. Nothing is listed between `0xa0000` and `0xf0000`: that is the video memory and the option ROMs, and the BIOS does not say what it is, only that it is not RAM (the kernel treats the whole first megabyte as spoken for in any case, below). The explanations of what each reserved entry is are the usual ones for a PC and are not printed by the machine; the transcript has only the numbers and the word "reserved".

### The frame bitmap

A *frame* is a 4,096-byte, 4 KiB-aligned piece of physical memory; the constant is [`FRAME_SIZE`](sym:kernel/src/memory.rs::FRAME_SIZE). The kernel tracks them in a [frame bitmap](term:frame-bitmap): one bit per frame, kept in `static mut BITMAP`, an array of 32-bit words. A set bit means the frame is in use *or does not exist*, and a clear bit means it is free. The bitmap covers 256 MiB: `MAX_BYTES` is `256 * 1024 * 1024`, so `MAX_FRAMES` is 65,536 and the array has 2,048 words, 8,192 bytes. RAM above 256 MiB is ignored for now, which the source and `docs/NATIVE-BOOT.md` ("Memory") both say plainly. Sixty-five thousand frames is a bit per 4 KiB, so the whole table costs one bit in 32,768 of the memory it describes.

One detail of this build is worth seeing. The array starts as all ones (`[!0; MAX_FRAMES / 32]`: everything unavailable until proved otherwise), and a table of ones is not zero, so the linker places it in `.data`, not `.bss`: in the kernel built for this chapter the symbol is 8,192 bytes at `0x128790`, inside the flat image the loader copies, while the 64 KiB stack is in `.bss` and costs nothing on the floppy. Whether the bitmap is in `.data` or `.bss` is a property of how the compiler lays out a static, not of the design, and the figures change with the build.

[`memory::init`](sym:kernel/src/memory.rs::init) builds the table in four passes, and the order is the design:

1. **Free what is usable, rounded inward.** For each entry of type 1 it frees the frames from `ceil(base / 4096)` up to `floor((base + len) / 4096)`, so a frame that is only partly inside the entry stays in use. The first entry ends at `0x9fc00`, which is frame 159.75, so frame 159 is not freed: three-quarters of it is RAM, and a frame is all or nothing.
2. **Reserve everything else, rounded outward.** For each entry of any other type and non-zero length it marks the frames from `floor(base / 4096)` to `ceil((base + len) / 4096)` as used, so a frame that is only partly reserved is reserved. This pass runs *after* the first, which makes an overlap between a usable and a reserved entry resolve to reserved: the safe side. In the recorded map no entry overlaps another, so this pass changes nothing (every reserved frame is already in use), but a BIOS that listed the same bytes twice would still be handled.
3. **Reserve the first megabyte.** Frames 0 to 255 are marked used whatever the map says. The first megabyte holds the real-mode vector table, the BIOS data area, the loader's boot information and the BIOS font, the staging area the kernel was loaded through (reused for the BIOS trampoline, [M8](chapter:m-bioscalls)), the video memory and the option ROMs. Nothing in it is the kernel's to hand out.
4. **Reserve the kernel image.** [`kernel_range`](sym:kernel/src/memory.rs::kernel_range) returns the addresses of the linker symbols `__kernel_start` and `__kernel_end` (`kernel/link.ld` defines them), and the frames between them are marked used. The end is `ALIGN(4096)` after `.bss`, and the range includes the `.bss` and the stack, because both live in memory the loader did not copy.

A set-used or set-free call changes the bit only if it would change it, and keeps a count, `FREE`, of the clear bits as it goes, so [`free_frames`](sym:kernel/src/memory.rs::free_frames) is a read, not a scan.

Two more functions touch the same table for the sake of devices. [`overlaps_ram`](sym:kernel/src/memory.rs::overlaps_ram) says whether a range overlaps the first megabyte, the kernel image or any usable entry of the map, and [`reserve`](sym:kernel/src/memory.rs::reserve) marks a range used. The video code calls the first on the VBE frame buffer's address and, if it is clear, the second, so a frame buffer is never mistaken for RAM; it prints `video: mode ... frame buffer ... overlaps no RAM, heap or kernel; reserved` ([M12](chapter:m-graphics)).

### Taking and giving frames

[`alloc_contiguous`](sym:kernel/src/memory.rs::alloc_contiguous) is [first fit](term:first-fit): it walks the bitmap from frame 0, counts the length of the current run of free frames, and as soon as the run reaches the count asked for it marks them used and returns the address of the first. It always returns the lowest run that fits; it never looks for a better one. [`alloc_frame`](sym:kernel/src/memory.rs::alloc_frame) is the case of one frame, and [`free_frame`](sym:kernel/src/memory.rs::free_frame) clears one bit. All three run inside `without_interrupts`, the same lock the rest of the kernel uses on a single processor ([M6](chapter:m-interrupts)). A scan from the bottom is slow, linear in the table (up to 65,536 bits) for every call, and that is a choice: the kernel allocates few frames, and a table that is easy to read is easier to trust. The `frames: alloc and free of two frames ok` line is the test: the kernel takes two frames, checks they differ and are 4 KiB-aligned, frees both and checks that the free count is what it was.

### The heap

Rust's `alloc` crate needs a global allocator before a `Vec` can exist. [`heap::init`](sym:kernel/src/heap.rs::init) asks the frame allocator for 1,024 contiguous frames (`HEAP_FRAMES`, 4 MiB), which with first fit come from the lowest free run that is long enough: right after the kernel image, at `0x13c000`. It hands the region to the [heap](term:heap), a [free list](term:free-list): a linked list of the blocks that are not in use.

Each free block begins with a `Node` of two words, its `size` and a `next` pointer, which lives in the free memory itself, so tracking a block costs nothing extra. The list is *sorted by address*. Every block and every allocation is a multiple of 8 bytes (`GRAIN`) and 8-byte aligned, so any remainder is big enough to hold a node.

- **Allocate.** [`FreeList::alloc`](sym:kernel/src/heap.rs::FreeList) rounds the size up to a multiple of 8 (and to at least a node), takes the larger of the requested alignment and 8, and walks the list for the first block that can hold the allocation at its alignment and leaves a remainder at each end that is either zero or big enough for a node. It unlinks that block, gives back what is left in front of the allocation and what is left behind it, and returns the aligned address. If nothing fits it returns null, which Rust turns into an allocation failure.
- **Free.** `dealloc` recomputes the rounded size from the `Layout` (so it needs the same layout that was allocated, as Rust guarantees) and inserts the block in the sorted list. `insert` finds the place by address and merges the new block with the neighbour behind it if they touch, and with the neighbour in front of it if they touch. Because the list is in address order, the two neighbours are exactly the ones adjacent to the insertion point, and no search for them is needed. This merging is what stops the heap from breaking into pieces too small to use.
- **The lock.** The `Heap` type wraps the list in an `UnsafeCell`, declares itself `Sync` on the grounds that there is one processor, and runs each call inside `without_interrupts`. An interrupt handler that allocated while the main line was in the middle of the list would corrupt it; with interrupts off during the call, it cannot happen.
- **The hook.** `#[global_allocator] static HEAP` is the line that makes `Vec`, `String` and `format!` use it.

The state of the heap is two numbers: `free_bytes` and the list. After the demo in `kmain` builds a `Vec<u32>` of ten squares (sum 385), a `String` and a formatted string, and drops all three, the transcript prints `heap: 4096 KiB free after the demo`: the free total is back to 4 MiB. That shows the totals balance. It does not show by itself that the list is back to one block, which is what the merging is for; no test inspects the list.

### The usable-RAM line and the frame count

The transcript prints three totals after the map: `memory: usable RAM 15871 KiB`, `memory: kernel image 0x100000-0x13c000` and `memory: 3748 frames of 4 KiB free after reservations`. They are measuring different things. The first is a sum over the *entries as the BIOS gave them*: `report` adds `len / 1024` for each usable entry, with no rounding to frames and no subtraction for the first megabyte or the kernel. The third is a count of frames after all four passes. The worked example derives both from the six entries.

### The x87

The compiler's target (`kernel/i486-desk-none.json`) has no SSE and no soft-float, so a `f64` multiplication in Rust becomes an x87 instruction. The processor will execute x87 instructions only if `CR0` allows it, and [`fpu::init`](sym:kernel/src/fpu.rs::init) sets the four bits that matter (they are tabled in [M2](chapter:m-cpu)):

| Bit | Name | Set by `init` to | Meaning |
|---|---|---|---|
| 2 | EM | 0 | Do not emulate: run x87 instructions instead of trapping them |
| 1 | MP | 1 | A `WAIT` instruction respects TS |
| 5 | NE | 1 | Report an x87 error as exception 16, not through the old external wiring (IRQ 13) |
| 3 | TS | 0 | No "task switched" trap: nothing is switching tasks, so there is no lazy state switching to arrange |

then runs `fninit`, which puts the unit in a known state: all exceptions masked, extended precision, round to nearest. The first thing [`fpu::demo`](sym:kernel/src/fpu.rs::demo) does is store the control word with `fnstcw` and `assert_eq!` it is `0x037F`. That is `0000 0011 0111 1111`: bits 0 to 5 are the six exception masks (all masked), bit 6 is a reserved bit that `fninit` sets, bits 8 and 9 are the precision control (`11`, 64-bit mantissa) and bits 10 and 11 the rounding control (`00`, nearest). A machine without a working FPU would not give that answer, so the assertion doubles as the presence test. It then computes `sqrt(2)` with an explicit `fsqrt`, and the harmonic sum 1 + 1/2 + … + 1/10 with an ordinary Rust loop, prints them as `fpu: control word 0x037f, sqrt(2) = 1.414214, H(10) = 2.928968`, and asserts that the square of the root is 2 and the sum is 2.928968253968254, each to within 10⁻¹². The loop is the interesting case: it shows that the compiler's own `f64` code is x87 code and that it runs.

The figure that follows is the loader's chain of three memory-map calls, shared with [M4](chapter:m-loader); it shows where the six entries above come from.

<!-- explainer memchain -->

## Why it is this way

- **A bitmap, not a list of free ranges.** One bit per frame is the simplest thing that supports freeing a single frame and finding a run, with a fixed size that needs no allocator of its own. It costs a linear scan, which does not matter at this scale (`docs/NATIVE-BOOT.md`, "Memory").
- **Inward for usable, outward for the rest.** A frame is usable only if all of it is RAM, and reserved if any of it is not. The two roundings turn every uncertainty into the safe direction, and doing the reserved pass second makes the BIOS's overlaps safe too (`kernel/src/memory.rs`, the comment on `init`).
- **256 MiB.** The bitmap is a static array, so its size is fixed at build time. The 486 targets this project aims at had far less memory than that; larger RAM is simply not used, and the limit is one constant.
- **The first megabyte is never handed out.** Not all of it is in the map as reserved (the gap between `0xa0000` and `0xf0000` is not listed at all), and the kernel borrows parts of it itself: the BIOS trampoline, its bounce buffer and its real-mode stack are in frames 16 to 19 ([M8](chapter:m-bioscalls); `docs/N3-BIOS-PATH.md`, "The low-memory map"). Marking it all used once is simpler and safer than describing each piece.
- **A first-fit free list, sorted by address.** The heap is "a plain version of a well-known design, simple rather than fast" in the document's words. Address order is what makes merging a local operation. Rounding to 8 bytes keeps every remainder able to hold a node. First fit is not the best at avoiding fragmentation, and nothing here measures it.
- **"Interrupts off" is the lock.** The kernel has one processor and handlers that may allocate, so the cheapest correct lock is to disable interrupts around the critical section (`docs/NATIVE-BOOT.md`, "Memory"; [M6](chapter:m-interrupts)).
- **The x87 is set up explicitly.** The loader leaves `CR0` with only `PE` of its own choosing, and a BIOS may leave `EM` or `TS` set; `fpu::init` makes the state a fact of the kernel and not an assumption about the firmware. `docs/NATIVE-BOOT.md` ("The FPU") adds that some 486SX machines have no FPU and that this kernel assumes one.
- **No paging yet, on purpose.** `docs/N3-BIOS-PATH.md` ("Paging") records what the BIOS trampoline will need when paging arrives, which is the best statement of what the design has deferred.

> **From the stacks — The FPU on the chip.** According to the Wikipedia article on the Intel 80486, the 486 integrated the floating-point unit on the chip, and the enhanced unit was significantly faster than the i387 per cycle. The i387 had been a separate, optional math coprocessor installed in a motherboard socket beside the i386. The article describes the i486SX as an i486DX with the FPU disabled, and says later versions removed it from the die. That is why this kernel's document notes that a 486SX machine may have no FPU for `fpu::demo` to find.
> Source: [Intel 80486, Wikipedia](https://en.wikipedia.org/wiki/Intel_80486)

## The code

| Where | What it holds |
|---|---|
| [`kernel/src/memory.rs`](repo:kernel/src/memory.rs) | [`Region`](sym:kernel/src/memory.rs::Region) is an E820 entry; [`regions`](sym:kernel/src/memory.rs::regions) reads them; [`init`](sym:kernel/src/memory.rs::init) builds the bitmap; [`report`](sym:kernel/src/memory.rs::report) prints the totals; [`alloc_contiguous`](sym:kernel/src/memory.rs::alloc_contiguous), [`alloc_frame`](sym:kernel/src/memory.rs::alloc_frame) and [`free_frame`](sym:kernel/src/memory.rs::free_frame) are the first-fit allocator; [`overlaps_ram`](sym:kernel/src/memory.rs::overlaps_ram) and [`reserve`](sym:kernel/src/memory.rs::reserve) keep device memory out of it |
| [`kernel/src/heap.rs`](repo:kernel/src/heap.rs) | [`FreeList`](sym:kernel/src/heap.rs::FreeList) is the sorted list with `insert` (merging), `alloc` and `dealloc`; [`Heap`](sym:kernel/src/heap.rs::Heap) is the `GlobalAlloc` that locks it; [`init`](sym:kernel/src/heap.rs::init) takes the 4 MiB; [`free_bytes`](sym:kernel/src/heap.rs::free_bytes) reads the total |
| [`kernel/src/fpu.rs`](repo:kernel/src/fpu.rs) | [`init`](sym:kernel/src/fpu.rs::init) edits `CR0` and runs `fninit`; [`demo`](sym:kernel/src/fpu.rs::demo) checks the control word and two results |
| [`kernel/link.ld`](repo:kernel/link.ld) | Defines `__kernel_start` and `__kernel_end` (`ALIGN(4096)` after `.bss`), which `kernel_range` turns into the reserved range |
| [`kernel/src/main.rs`](repo:kernel/src/main.rs) | [`kmain`](sym:kernel/src/main.rs::kmain) calls `memory::init`, `memory::report`, the two-frame test, `heap::init`, the allocation demo and `fpu::init` in that order |
| [`tests/native/expected.txt`](repo:tests/native/expected.txt) | The transcript lines these files print, with the BIOS-dependent numbers as ranges |

For depth: `docs/NATIVE-BOOT.md` ([memory](doc:docs/NATIVE-BOOT.md#memory) and [the FPU](doc:docs/NATIVE-BOOT.md#the-fpu)) and `docs/N3-BIOS-PATH.md` ([paging](doc:docs/N3-BIOS-PATH.md#paging)).

## Worked example

**The usable-RAM line.** The two usable entries of the recorded map are `0x00000000`–`0x0009fc00` and `0x00100000`–`0x00fe0000`. Their lengths are `0x9fc00` = 654,336 bytes and `0xee0000` = 15,597,568 bytes. Dividing each by 1,024, as `report` does, gives 639 KiB and 15,232 KiB, and 639 + 15,232 = 15,871 KiB, the number in `memory: usable RAM 15871 KiB`. Each entry's length is a whole number of KiB here, so integer division loses nothing.

**The frame count.** The same two entries, rounded inward to 4,096-byte frames:

| Usable entry | First frame | Last frame (exclusive) | Frames freed |
|---|---|---|---|
| `0x00000000`–`0x0009fc00` | 0 | `0x9fc00 / 4096` = 159 (159.75 rounded down) | 159 |
| `0x00100000`–`0x00fe0000` | `0x100000 / 4096` = 256 | `0xfe0000 / 4096` = 4,064 | 3,808 |

That is 159 + 3,808 = 3,967 frames free after pass 1, or 15,868 KiB: 3 KiB less than the 15,871 KiB the BIOS reported, which is the three-quarters of frame 159 that is RAM (`0x9f000` to `0x9fc00`, `0xc00` = 3,072 bytes) but is not freed because the frame is not whole. Pass 2 changes nothing, because the reserved entries are in frames that are already used (the entry at `0x9fc00` rounds outward to frame 159; `0xf0000`–`0x100000` is frames 240 to 255; `0xfe0000`–`0x1000000` is frames 4,064 to 4,095; and the entry at `0xfffc0000` starts beyond the 65,536 frames the bitmap covers, so its range is empty). Pass 3 marks frames 0 to 255 used; of them only the 159 frames of the first entry were free, so the count falls by 159, to 3,808, which are all of the second entry. Pass 4 marks the kernel: `__kernel_start` is `0x100000` and `__kernel_end` `0x13c000`, 0x3c000 = 245,760 bytes, which is 60 frames (numbers 256 to 315), all of them free before, so the count falls by 60, to **3,748**. The transcript line is `memory: 3748 frames of 4 KiB free after reservations`.

There is an independent check from the other side. The machine was given 16 MiB, 4,096 frames. The frames that are not free are the first megabyte (256), the kernel (60) and the 128 KiB reserved at the top (32), and 4,096 − 256 − 60 − 32 = 3,748.

Where the kernel's 245,760 bytes come from, in the build made for this chapter: the flat image is 173,968 bytes, ending at `0x12a790`; `.bss` starts at the next 4 KiB boundary, `0x12b000`, and runs to `0x13b9f0`, of which 65,536 bytes (`0x12b000`–`0x13b000`) are the stack the entry stub gives the kernel; and `__kernel_end` is the next boundary after that, `0x13c000`. The size changes whenever the kernel does, which is why the expected transcript accepts `{HEX}` for both ends and `3000..4096` for the count (`tests/native/expected.txt`).

**The heap's place.** The heap asks for 1,024 contiguous frames. The first free frame in the table is 316 (`0x13c000`, the frame after the kernel), the run of free frames from there is 3,748 long, and so the heap is frames 316 to 1,339: `0x13c000` to `0x53c000`, and the transcript prints `heap: 4096 KiB at 0x13c000`. The two frames of the allocator test, taken and given back before the heap exists, were frames 316 and 317. After the heap there are 3,748 − 1,024 = 2,724 free frames; the transcript does not print that number, it is derived.

<!-- explainer m-frames -->

**One allocation.** The demo's first allocation is a `Vec<u32>` of ten elements, 40 bytes with 4-byte alignment. `round` makes the size 40 (already a multiple of 8) and the alignment 8. The list holds one block, 4 MiB at `0x13c000`; the first fit is that block, the aligned start is its start, and the remainder is 4,194,264 bytes, far above the 8-byte minimum, so `alloc` unlinks the block, inserts a remainder block at `0x13c028` of that size and returns `0x13c000`. (The transcript does not print addresses of allocations; this follows the code.) Freeing it inserts a 40-byte block at `0x13c000`: the list is searched for the first node at a higher address, which is `0x13c028`, `0x13c000 + 40` equals it, so the new block absorbs it and the list is one block again. The `alloc: Vec sum 385` line is the test that the arithmetic over this memory is right: 1 + 4 + … + 100 = 385, which is `0x181`, the number in the `format` string.

## State of this layer

Verified here means that the record cites a transcript line that `tools/native-test.sh` checks on every run or a merged pull request whose CI passed. It means in QEMU, never on a real machine.

```status
id: memory.map
label: The kernel reads the loader's memory map and prints it with the usable total
state: built-verified
evidence: test:tests/native/expected.txt::memory: usable RAM, test:tests/native/expected.txt::memory: kernel image, sym:kernel/src/memory.rs::regions
note: The six entries depend on the BIOS and the expected transcript replaces them with a line of dots. The E801 and AH=88 builds print the same total (M4).

id: memory.bitmap
label: The frame bitmap over 256 MiB, usable regions rounded inward, the rest outward, first megabyte and kernel reserved
state: built-verified
evidence: test:tests/native/expected.txt::frames of 4 KiB free after reservations, sym:kernel/src/memory.rs::init
note: Checked as a range (3000 to 4096 frames). The exact count follows from the map by arithmetic, which the chapter's own test repeats.

id: memory.frames
label: First-fit allocation of one frame or a run, and freeing
state: built-verified
evidence: test:tests/native/expected.txt::frames: alloc and free of two frames ok, sym:kernel/src/memory.rs::alloc_contiguous
note: Only one frame at a time is allocated and freed in the transcript; the run search is exercised by the heap's request for 1,024.

id: memory.devices
label: A frame buffer is checked against RAM and then reserved
state: built-verified
evidence: test:tests/native/expected-gfx-vbe.txt::overlaps no RAM, sym:kernel/src/memory.rs::reserve
note: Only the linear frame buffer of QEMU's VBE modes is checked. The branch where it overlaps RAM has not run.

id: memory.heap
label: A 4 MiB first-fit free list with merging behind the global allocator
state: built-verified
evidence: test:tests/native/expected.txt::heap: 4096 KiB free after the demo, test:tests/native/expected.txt::alloc: Vec sum 385, sym:kernel/src/heap.rs::FreeList
note: The demo allocates and frees a few small objects. No test fragments the heap, runs it out of memory or allocates from an interrupt handler.

id: memory.ceiling
label: RAM above 256 MiB is ignored
state: built-unverified
evidence: sym:kernel/src/memory.rs::MAX_BYTES, doc:docs/NATIVE-BOOT.md#memory
note: Stated in the source and the document. No run had that much memory, and the QEMU runs are given 16 MiB.

id: fpu.init
label: The x87 is enabled (EM off, MP and NE on, TS off) and initialised with fninit
state: built-verified
evidence: test:tests/native/expected.txt::fpu: control word 0x037f, test:tests/native/expected.txt::fpu: results check out, sym:kernel/src/fpu.rs::init
note: QEMU as a 486 has an FPU. What a 486SX without one would do is not tested; the document says only that the kernel assumes one.

id: fpu.errors
label: An x87 error is reported as exception 16
state: built-unverified
evidence: sym:kernel/src/fpu.rs::init, doc:docs/NATIVE-BOOT.md#the-fpu
note: The NE bit is set, but no test provokes an x87 error, and vector 16 is one of the 32 exception gates that print the general exception dump.

id: memory.paging
label: Paging, page tables and memory protection
state: planned
evidence: doc:docs/N3-BIOS-PATH.md#paging
note: The kernel runs with physical addresses. The document says what the BIOS path will need when paging arrives.

id: memory.hardware
label: Any of this on a real machine
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The frame bitmap and the heap are {{state.memory.heap}} in QEMU, and the x87 set-up is {{state.fpu.init}}. What the transcript cannot show is the machine behaving differently from the emulator: a real BIOS lists its own map, a real 486 may or may not have an FPU, and a heap that runs for hours has not been tried.

## What is missing

There are no page tables: every pointer is a physical address, and the `identity-mapped` remarks in the source mean only that there is nothing else to be mapped to. There is no protection: the segments are flat, every part of the kernel runs in ring 0, and a stray write can destroy the bitmap or the free list. Nothing frees the heap to the frame allocator or grows it; 4 MiB is the size for ever. Frames above 256 MiB are not tracked. A task switch would need the x87 state to be saved and `CR0.TS` used to find out who owns the unit, and none of that exists, because there is one thread and the interrupt handlers do not use floating point (nothing in `kernel/src` saves x87 state). The document for the BIOS path ([M8](chapter:m-bioscalls)) says what its trampoline needs from paging when the time comes.

## Try it yourself

The first block builds the native image and boots it, then reads the memory lines from the serial capture, and the second does the chapter's arithmetic from those lines. They need NASM, a Rust toolchain and `qemu-system-i386` on the host (or the Docker fallbacks the scripts use). Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: bash, nasm, cargo, qemu-system-i386, grep, tr
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh > /dev/null && tools/native-image.sh > /dev/null && tools/native-run.sh --serial dist/serial.txt > /dev/null; echo "exit $?"
exit 0
$ tr -d '\r' < dist/serial.txt | grep -E "^  map|^memory:|^frames:|^heap:|^fpu:"
  map 0x00000000-0x0009fc00 usable
  map 0x0009fc00-0x000a0000 reserved
  map 0x000f0000-0x00100000 reserved
  map 0x00100000-0x00fe0000 usable
  map 0x00fe0000-0x01000000 reserved
  map 0xfffc0000-0x100000000 reserved
memory: usable RAM 15871 KiB
memory: kernel image 0x100000-0x13c000
memory: 3748 frames of 4 KiB free after reservations
frames: alloc and free of two frames ok
heap: 4096 KiB at 0x13c000
heap: 4096 KiB free after the demo
fpu: control word 0x037f, sqrt(2) = 1.414214, H(10) = 2.928968
fpu: results check out
$ python3 -I -c "print((0x9fc00 >> 10) + (0xfe0000 - 0x100000 >> 10), 0x9fc00 // 4096 + (0xfe0000 - 0x100000) // 4096, 4096 - 256 - (0x13c000 - 0x100000) // 4096 - 32)"
15871 3967 3748
```

The three numbers are the usable KiB, the frames freed by pass 1, and the free frames left after the first megabyte, the kernel and the top hole are taken out. The same boot with a different map: the loader's fallback builds print the same `15871 KiB`, as [M4](chapter:m-loader) shows, and the frame count with them is the same too, because the maps differ only in the entries that are not RAM.

**Exercises.** (1) Predict the number on the `memory: ... frames` line if the VM were given 32 MiB instead of 16 (`-m 32` in `tools/native-run.sh`'s command line, `docs/NATIVE-BOOT.md` "Running it and the CPU model"): work out which entries move, then run it and compare; the expected transcript accepts only 3,000 to 4,096 frames, so the test would reject the new number, which is worth seeing. (2) Change `HEAP_FRAMES` in a scratch branch to 2,048 and predict the new `heap:` line and the frame count after it; which transcript line, if any, would make the test fail? (3) Change the `set` mask in `fpu::init` so that `NE` is not set, and say which line of the transcript would notice (the answer is none, which is what the status row `fpu.errors` records).

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Frame
def: A 4,096-byte, 4 KiB-aligned piece of physical memory, the unit the kernel's allocator hands out. A frame is free only if all of it is RAM.

term: Frame bitmap
def: The kernel's table of frames, one bit per frame over the first 256 MiB. A set bit means the frame is in use or does not exist; a clear bit means it is free.

term: Heap
def: The region of memory the kernel allocates small objects from, behind Rust's global allocator. The Desk's is 4 MiB of contiguous frames taken from the frame allocator at boot.

term: Free list
def: The heap's record of the blocks that are not in use: a linked list, sorted by address, whose nodes live inside the free blocks themselves. Neighbouring blocks are merged when one is freed.

term: First fit
def: An allocation rule that takes the first block or run that is big enough, searching from the lowest address, and never looks for a better one. The frame allocator and the heap both use it.

term: x87
def: The 80x87 floating-point unit: the instructions and registers for real-number arithmetic that a 486 has on the chip. The kernel's compiler emits them for f32 and f64 because SSE is off.
```

```figures
id: m7-memchain
reuse: memchain
at: explainer memchain

id: m7-frames
kind: explainer
track: native
title: From the memory map to the frame bitmap and the heap
alt: A step-through of the kernel's frame bitmap built from the six entries of the recorded memory map: usable regions freed, other regions reserved, the first megabyte and the kernel reserved, then the 4 MiB heap taken by first fit.
caption: The six map entries replayed through memory::init; 3,748 frames free, the heap at 0x13c000.
at: explainer m-frames
thumb: svg
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The numbers in the worked example were derived from the code and checked against the transcript (15,871 KiB and 3,748 frames), and the commands above were run on the host. One document-against-code point: `docs/NATIVE-BOOT.md` says the reported usable RAM is "the sum of the usable entries as the BIOS gave them", which is right, and the same document's "Memory" section says nothing of the fact that the bitmap sits in `.data`, which is a property of the build and not a disagreement.
