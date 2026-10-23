---
id: m-graphics
n: M12
part: IV
title: Drivers III: VGA and VBE
summary: The desk-vga driver: mode 13h and 12h by register programming, planes, the DAC and fades, text mode and the way back, the VBE layer over the Bios trait, the linear frame buffer on each track, the software cursor and what QEMU hides.
date: 2026-10-23
reviewed: 44f0265
watches: [hal/drivers/vga/, hal/hal/src/video.rs, kernel/src/video.rs, kernel/src/gfx.rs, kernel/src/vga.rs, spikes/d0-rust-dos/rt/src/vga.rs, tests/native/]
log: [pixels, browser, bios]
anatomy: [desk:drivers, dos:drivers, dos:vga, dos:lfb]
---
## What it is

This chapter is about putting pixels on the screen. The driver is `desk-vga`, one crate in `hal/drivers/vga` that two very different programs use: the native kernel, which has no BIOS once it has left the loader, and the hosted DOS program, which has DOS, DPMI and a BIOS underneath it. It does three jobs. It programs the VGA chip's registers directly, to set mode 13h (320 by 200, 256 colours) or mode 12h (640 by 480, 16 colours) or to get back to text, with no BIOS call. It loads and fades the colour table. And it speaks VBE 2.0, the BIOS extension that offers higher-resolution modes with a [linear frame buffer](term:lfb), through a trait that lets each platform decide how a real-mode call is made. Everything that draws (lines, rectangles, a cursor, a read-back checksum) is written once, against a `Canvas` trait, over any of the three kinds of surface.

On the map ([M1](chapter:m-map)) it is the video half of the HAL's drivers: the box [`drivers`](chapter:anatomydesk#box-desk-drivers) on the native page and [`drivers`](chapter:anatomydos#box-dos-drivers), [`vga`](chapter:anatomydos#box-dos-vga) and [`lfb`](chapter:anatomydos#box-dos-lfb) on the DOS page. The hardware facts (what the [VGA](term:vga) is, the [DAC](term:dac), [planar mode](term:planar-mode), what [VBE](term:vbe) added) are taught in [M3](chapter:m-devices) and are not repeated; the loader's side of VBE, which asks the BIOS for a mode before the switch to protected mode, is in [M4](chapter:m-loader); the trampoline that lets the kernel call the BIOS afterwards is [M8](chapter:m-bioscalls). This chapter is what the driver does with the registers and how each track gets its frame buffer.

After it you should be able to follow the 128 port writes that set mode 13h and say what each group of them does, compute where a pixel lives in mode 13h, mode 12h and a VBE mode, say why the Desk loads its own palette and fades through the DAC, explain why leaving a VBE mode is harder than entering it, and tell which of the project's video claims have run and which of them depend on an emulator being kinder than a real card. Nothing here has run on a real VGA chip or a real monitor. Wherever the emulators hide something a real card would not, the text names the document that records it.

## How it works

### The register file and the mode tables

A VGA is five groups of registers behind a few port pairs, and `hal/drivers/vga/src/regs.rs` names the ports: the attribute controller at `0x3C0` (reads at `0x3C1`), the miscellaneous output register at `0x3C2` (read at `0x3CC`), the sequencer at `0x3C4` and `0x3C5`, the graphics controller at `0x3CE` and `0x3CF`, the CRT controller at `0x3D4` and `0x3D5`, and the input status register at `0x3DA`. Most of them are indexed: write the register's number to the first port and its value to the second. The attribute controller is the odd one, with a single port for both index and data and a flip-flop that says which the next write is, reset by reading `0x3DA`.

A [mode set](term:mode-set) by registers writes a *complete register image*, and the driver holds three as constants of type `ModeRegs`: the miscellaneous byte, five sequencer registers, 25 CRTC registers, nine graphics controller registers and 21 attribute registers. `MODE_13H` has miscellaneous `0x63`, sequencer `03 01 0F 00 0E`, graphics controller `00 00 00 00 00 40 05 0F FF` and CRTC `5F 4F 50 82 54 80 BF 1F 00 41 ...`. Three entries explain the mode. Sequencer register 4 is `0x0E`: chain-4, which makes four planes look like one run of bytes, one byte per pixel. Graphics controller register 5 is `0x40`: the 256-colour shift register. CRTC register 9 is `0x41`: its low five bits are 1, so each row of 320 pixels is scanned twice, which is how 200 rows fill 400 lines (the CRTC's vertical display end, register `0x12` of `8F` with its high bit in register 7, is line 399). `MODE_12H` instead has sequencer 4 as `0x06` (planar, no chain-4), graphics controller 5 as `0x00` and miscellaneous `0xE3`, the 480-line polarity. `MODE_03H`, text mode, has miscellaneous `0x67` and the odd/even layout.

The tables are "the standard ones". `regs.rs` says they were written from the FreeVGA register references and the tables the Linux console and SVGALIB use, not copied from one source, and `docs/A-VGA-VBE.md` ("Register tables") says what checked them: DOSBox Staging's BIOS programs the same registers for the same modes apart from two differences, which the document lists. A real VGA with real monitor timing has not seen them, and that is stated in the header of `regs.rs`, in the document and again below.

### Programming a mode

<!-- explainer vga13 -->

The figure steps through the same 128 writes with their real values, then loads one colour into the DAC and stores one pixel.


`set_mode_registers` writes a table to the ports in the order the IBM VGA technical reference gives (as FreeVGA documents it). The order is the substance, and the host test `mode_13h_writes_the_documented_sequence` asserts it, all 128 writes:

1. **Video off.** Read `0x3DA` to reset the attribute flip-flop, then write `0x00` to `0x3C0`. With the palette-address-source bit clear, the screen is blanked while registers change. (One write.)
2. **Sequencer.** Put the sequencer in synchronous reset (register 0 = 1), because the clock source is about to change; write the miscellaneous output register; write sequencer registers 1 to 4; release the reset with register 0's table value. (Thirteen writes.)
3. **CRTC.** Registers 0 to 7 ignore writes while bit 7 of register `0x11` is set, so the driver first writes `0x11` with that bit cleared (`0x8E & 0x7F = 0x0E` for mode 13h), then writes all 25 registers in order; the table's own value of `0x8E` goes back into `0x11` in passing and protects the lot again. (Fifty-two writes.)
4. **Graphics controller.** Registers 0 to 8. (Eighteen writes.)
5. **Attribute controller.** Reset the flip-flop with another read, write each of 21 registers as an index byte then a data byte, and write `0x20` to `0x3C0`, which sets the palette-address-source bit and turns video back on. (Forty-three writes.)
6. **The pixel mask.** `0xFF` to `0x3C6`, so every DAC entry is reachable. (One write.)

That is 1 + 13 + 52 + 18 + 43 + 1 = 128. Notice what is *not* in the sequence: the DAC's contents and the frame buffer. The caller loads a palette and clears the screen afterwards, because a direct set leaves whatever the previous mode left in the planes. The kernel's tour then reads the registers back through `read_mode_registers` and compares them with the table, printing `registers read back equal the table: true`, and `assert!`s it.

Mode 12h has one deliberate difference from a BIOS. A BIOS leaves the EGA-compatible attribute palette, which sends colour 6 to DAC entry `0x14` and colours 8 to 15 to entries `0x38` to `0x3F`. The Desk loads DAC entries 0 to 15 itself, so `MODE_12H.attr` is the identity (colour *n* is entry *n*), and `attribute_palette_identity` applies the same after a BIOS sets the mode on DOS. `MODE_03H` keeps the BIOS's mapping, and `text::TEXT_DAC` is the list of entries it points at.

### Where a pixel lives

In mode 13h with chain-4 a pixel is one byte, at `0xA0000 + y × 320 + x`, a surface of 64,000 bytes. `Linear8` is that view over an `MmioRegion` and also over a VBE linear frame buffer: it takes a `width`, a `height` and a [pitch](term:pitch), checks that a row of `pitch` bytes holds the width and that `pitch × height` fits the region, and then the address of a pixel is `y × pitch + x`. The pitch is the number of bytes from the start of one row to the start of the next, and it is not always the width: the information the BIOS reports for VBE mode `0x101` is 640 for 640 pixels, but 1,280 for a 640 by 480 mode at 16 bits per pixel (`spikes/s5-jsdos/evidence/vbe-jsdos.txt` is the listing the probe printed), and a planar mode has a pitch in bytes of one plane, 100 for 800 pixels. The driver reads the pitch; it never computes it.

Mode 12h is different, and `Planar16` (`planar.rs`) is the longest piece of the crate. The 640 by 480 picture is four [bit planes](term:bit-plane), each 80 bytes per row, all at `0xA0000`, and the same address in each plane holds one bit of eight pixels' colours: bit 0 of the colour in plane 0, up to bit 3 in plane 3. One CPU write goes through the graphics controller's logic into every plane the sequencer's map mask enables. `Planar16` sets the controller to write mode 0 with enable set/reset on all four planes: the colour goes into the set/reset register (GC register 0), the **bit mask** (GC register 8) selects which of the eight pixels of the byte change, and the *latches*, loaded by any read of that address, keep the others. So the value the CPU writes does not matter; the driver writes zero. A whole byte (mask `0xFF`) needs no read, and a horizontal line is two masked ends and a run of whole bytes. Reading the picture back goes the other way: the read map select (GC register 4) names one plane per read, so `get_pixel` makes four reads. `Planar16` leaves the controller in that state while it lives and puts set/reset off and the mask open when dropped. The cost is port traffic: a 640 by 480 mode 12h fill measured about 190 per second under QEMU's emulation, against about 8,300 for the 8-bit linear fill (`docs/D-NATIVE-VGA.md`, "Speed", emulated speed and not 486 numbers).

### The palette, the DAC and the fade

In the 256-colour modes a byte in the frame buffer is not a colour but an index into the [palette](term:palette): a table of 256 entries held in the DAC, each three six-bit numbers. `dac::set_palette` writes the start index to `0x3C8` and then red, green and blue for each entry through `0x3C9`; the index advances after each triple, so a whole palette is one index write and 768 data writes. Reading uses `0x3C7`. The colour type is `Rgb6`, six bits a channel; `from_rgb8` converts the nearest way (`(v × 63 + 127) / 255`: `0xFF, 0xAA, 0x40` becomes `63, 42, 16`) and `to_rgb8` repeats the top two bits into the bottom two so that 63 comes back as 255.

The Desk owns the palette and fades through the DAC, never through the frame buffer (`docs/PALETTE.md`), because only the DAC can change the whole screen's brightness with 768 writes instead of touching 307,200 bytes. A fade is `FADE_STEPS` (16) steps; step *i* shows the palette scaled by `(16 − i) / 16` going out, and the last is exactly black (or, going in, exactly the palette). `fade_out` and `fade_in` wait for vertical retrace before each step. `wait_vretrace` is bounded: it first waits until any retrace in progress ends and then until the next begins, reading bit 3 of `0x3DA`, at most `VRETRACE_POLLS` (a million) reads per phase, and a fade that times out writes anyway, counts the timeout in a `FadeReport`, and stops polling for the rest so a dead display does not cost a bound per frame.

> **From the stacks — The 256-colour VGA.** Wikipedia says IBM introduced its first VGA hardware in April 1987, with the PS/2 line of computers. It lists 320 by 200 in 256 colours as a standard graphics mode, "Mode 13h", and says each palette register holds a 3 by 6 bit RGB value, chosen from an 18-bit gamut of 262,144 colours.
> Source: [Video Graphics Array, Wikipedia](https://en.wikipedia.org/wiki/Video_Graphics_Array)

### Text mode and the way back

Setting a graphics mode by registers destroys what text mode needs. Plane 2 held the character generator (256 glyphs, each in a 32-byte slot of which the first 16 bytes are the 8 by 16 bitmap) and the DAC held the 16 text colours. Getting back without a BIOS therefore takes four steps, in the order the kernel's `restore_text` runs them: `set_mode_03h` (the 80 by 25 register image); `set_text_palette` (the 16 colours into entries `00` to `05`, `14`, `07` and `38` to `3F`, as the attribute palette expects); `text::load_font`, which sets the sequencer map mask to plane 2 only, the memory mode to sequential, and the graphics controller's read map, mode and memory-map bits so that the CPU can reach plane 2 at `0xA0000`, copies 4,096 bytes into 32-byte slots, and puts those registers back to their text values; and a cleared text buffer at `0xB8000`. The font has to come from somewhere, and the loader's job in [M4](chapter:m-loader) is to copy the BIOS's 8 by 16 font to `0x2000` before it leaves real mode, because after the switch nothing can ask the BIOS for it. If the font is missing the kernel restores the registers and says the characters will not be readable; the other choice was to halt.

The kernel's ordinary console, `kernel/src/vga.rs`, is a different and much smaller thing: an 80 by 25 writer straight to `0xB8000` in light grey on black, with scrolling, and it does not use `desk-vga` at all.

### VBE over the `Bios` trait

VBE is a set of BIOS functions on INT 10h with `AX = 4F00h` and up, and `desk-vga`'s `vbe.rs` implements the calls over the HAL's `Bios` trait, which is "one real-mode software interrupt with an optional buffer in low memory". The platform decides how the interrupt is made: on DOS it is DPMI function 0300h; in the kernel it is the real-mode trampoline of [M8](chapter:m-bioscalls); in a test it is `MockBios`, scripted replies. The `Vbe` type is unchanged across them.

`Vbe::info` (4F00h) puts the signature `VBE2` in a 512-byte buffer first, which asks for the VBE 2.0 layout, and `VbeInfo::parse` insists on `VESA` at the start, the version, the video memory in 64 KiB units, and two real-mode far pointers, to the OEM string and to the mode list. Those pointers are read with `Bios::read_low`, not assumed to be inside the buffer, because a BIOS may leave them in its own ROM. `mode_list` reads 16-bit mode numbers until `0xFFFF`, at most as many as the caller's array holds (the programs size it with `MAX_MODES`, 128). `Vbe::mode` (4F01h) fills a 256-byte block and `VbeMode::parse` takes from it the attribute word (offset 0), the pitch (16), the width and height (18 and 20), the planes, bits per pixel and memory model (24, 25 and 27) and the physical base of the linear frame buffer (40). `find` picks the first supported graphics mode of an exact size and depth in the packed-pixel or direct-colour model, with a linear frame buffer if asked. `set_mode` (4F02h) ORs `MODE_LINEAR` (`0x4000`) into the mode number when a linear buffer is wanted, and leaves bit 15, "keep memory", clear so the BIOS clears the screen. `set_window` (4F05h) is the banked fallback: wrapped and unit-tested, and never called, because the Desk has no banked driver.

The attribute word has the answer to the first question about any mode. `ATTR_SUPPORTED` is bit 0, `ATTR_GRAPHICS` bit 4, `ATTR_NO_BANKED` bit 6 and `ATTR_LINEAR` bit 7. `VbeMode::linear` needs both bit 7 *and* a non-zero physical base, which matters because a BIOS can set the one and not give the other.

<!-- explainer vbeattrs -->

The figure decodes three real lines from the probe run in js-dos (the browser spike), including a mode with no linear buffer and a machine option that removes it.

### The linear frame buffer on each track

A linear frame buffer is a card's video memory presented as one block of address space, normally in a PCI window near the top of the 32-bit space. Each track has to get a pointer to it, and they do it differently.

**Native.** The loader asks the BIOS while it still can ([M4](chapter:m-loader)): `video_setup` in `boot/stage2.asm` walks the mode list, walks the mode list for 8-bit packed-pixel modes with the linear bit and a non-zero base (the first 640 by 480 one wins at once; otherwise the largest area up to 1,280 wide, the width `checksum` can read back), sets it, and leaves a 32-byte video block at `0x820` that the boot information block points to.

<!-- figure bootinfo -->

The kernel's `video::init` reads that block, which is `repr(C)` and mirrors the loader's layout, only when the boot information version says it exists, and prints a line for what it found. On QEMU's standard card the line is `video: the loader set VBE 3.0 mode 0x0101: 640x480x8, pitch 640, memory model 4, 16384 KiB on the card`. Before the kernel draws it checks that the frame buffer overlaps nothing: not the first megabyte, not the kernel image, not any usable E820 region, which also covers the heap and the frame allocator. A buffer that fails the check is refused, and the kernel falls back to the register modes. One that passes is reserved, and the line says so: `video: mode 0x0101 frame buffer 0xfd000000-0xfd04b000 overlaps no RAM, heap or kernel; reserved`. The kernel then maps nothing: segments are flat, there is no paging, and `lfb_region` builds an `MmioRegion` at the physical address as a pointer. The size is `pitch × height`, here 640 × 480 = 307,200 = `0x4B000`, which is the difference of the two addresses in the line.

**Hosted.** Under DOS the picture is drawn in the same way but the buffer has to be mapped. `DosDisplay` (`spikes/d0-rust-dos/rt/src/hal.rs`) sets a VGA mode with INT 10h or a VBE mode with 4F02h and the linear bit, then maps the physical address from 4F01h with DPMI function 0800h (`dpmi::map_physical`), which returns a linear address reachable through the program's 4 GiB data segment, and releases it with 0801h on the way out. Mode 13h needs none of that: the buffer is in the first megabyte, which CWSDPMI maps at linear 0, so `rt/src/vga.rs` turns `0xA0000` into a pointer with the "near pointer" arithmetic. Under DOSBox Staging's S3 model the physical base of mode `0x101` is `0xC0000000`.

**The browser.** The same DOS program runs in js-dos, and `docs/S5-JSDOS.md` ("VBE and the linear frame buffer") is the record that the linear buffer works there too: `0x101` reports attributes `0x009b` and base `0xC0000000`, DPMI 0800h maps it, and what the program writes is what the canvas shows, zero of 307,200 pixels different from a computed pattern. The document's rules for the code follow from the differences it measured. Read the base from 4F01h and never hard-code it (js-dos reports `0xC0000000`, its DOSBox-X backend `0xE0000000`). Do not rely on more than 2 MiB of video memory. Find a mode by its size and depth, not its number. And keep a banked path, which works there too.

### The software cursor

A VGA has no hardware cursor, and the Desk draws its own (`docs/SPEC.md`). A [software cursor](term:software-cursor) is a sprite drawn into the frame buffer after saving the pixels under it, and erased by writing them back before it moves. `desk_vga::SoftCursor` is a 12 by 19 arrow (`X` outline, `o` fill, `.` transparent) with save-under over any `Canvas`, clipped at every edge, whose hot spot is the top-left pixel; `hide` restores, `show` moves. It is the cursor the DOS input program draws. The native kernel's `gfx.rs` has a second one of its own, `Cursor`, with the same twelve-by-nineteen shape and the same save-under idea; `docs/D-NATIVE-VGA.md` ("The cursor and the first window") describes it and does not mention the shared one. They are two implementations of the one idea. The later compositor replaces both with damage-aware drawing ([M13](chapter:m-pixels)), and the sprite and the clipping are what stay.

### The kernel's `video.rs` and the `gfx` tour

`video.rs` is the checking half and `gfx.rs` the drawing half of the kernel's graphics. In the `gfx` [test mode](term:test-mode) the kernel runs a fixed tour and prints a line at each step; at each stable picture it prints `gfx: ready NAME` and waits for a key, so that the run script can take a screenshot first and send the key after, with no timing involved. With a VBE buffer the tour is: draw the test picture on 640 by 480 by 8, read it back through `checksum` and compare it with the same picture drawn in memory (`gfx: vbe picture: frame buffer checksum 0x5bc65936, picture in memory 0x5bc65936`) and with the checksum of the DOSBox run on the DOS side (`0x5bc65936`, and a mismatch is an `assert!`, not a message); fade out and in with a pace taken from the kernel's own timer; measure fill and cursor speed; run the first window with the cursor, following the mouse and echoing the keys of [M11](chapter:m-input); *leave VBE*; set mode 13h by registers and check the DOSBox checksum `0x5c0f1f16`; and restore text. Without a VBE buffer (the loader was not asked, or the BIOS refused) it is 13h, then 12h with checksum `0xfaf8b758`, then the window in 12h, then text.

The checksum identity is the best evidence in the whole graphics story. The same scene drawn by the same driver gives the same Adler-32 on DOSBox's VGA model and QEMU's, for three modes, which is evidence that the register tables and the drawing code do not depend on either emulator's quirks. It is not evidence about a monitor.

<!-- figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png -->

The five screenshots are the VBE test picture, the same picture half-faded, mode 13h and mode 12h on the native kernel, and mode 13h on DOS as the program read it back from the frame buffer.

**Leaving VBE.** Entering a VBE mode is one BIOS call. Leaving it without the BIOS is card-specific. On QEMU's standard card a single write to the Bochs VBE enable register (index 4 at `0x1CE`, data at `0x1CF`) turns the mode off, and `leave_vbe` finds the card by reading the ID register (`0xB0C0` to `0xB0CF`). On a card without those registers the plain VGA register programming does not undo the card's extended registers, so the kernel asks the BIOS for text mode 3 through the trampoline. That fallback is the answer to the problem the spike for this chapter found on QEMU's Cirrus card, where the same programming left a garbled screen, and it is the reason the log chapter on the BIOS exists.

## Why it is this way

**Registers, and not only the BIOS.** The kernel can set mode 13h and 12h with no BIOS at all, so a native kernel that never grew a BIOS path would still have graphics. The tables are the cost: the project is responsible for values that a BIOS would otherwise supply. `docs/A-VGA-VBE.md` records the decision to set each mode both ways where both exist and read the registers back, to see what a BIOS programs differently.

**The loader asks VBE; the kernel does not.** Protected mode cannot call INT 10h, so VBE was first asked in the loader (`docs/D-NATIVE-VGA.md`, "The loader: VBE before protected mode"). The document says plainly that this made leaving VBE one-way until the trampoline arrived, and the trampoline closes it for any card.

**A `Bios` trait, not INT 10h calls.** `docs/A-VGA-VBE.md` ("HAL feedback") added `Bios` and `BiosRegs` to the HAL, which were not in the sketch, because VBE needs a call with a buffer the BIOS reads and writes and a way to read low memory. Putting that behind a trait is what lets the native platform implement VBE differently from DOS and lets the tests script it.

**The Desk's own palette, with an identity attribute map.** In 16 colours the Desk loads DAC entries 0 to 15 and wants colour *n* to be entry *n*; a BIOS's EGA-compatible mapping would put colours 6 and 8 to 15 elsewhere. This is the one place where the table deliberately differs from a BIOS, and the document lists it.

**A fade paced by the kernel's timer as well as by retrace.** The driver's fade waits for retrace; the kernel also paces its own fade, 16 steps of two ticks (20 ms), because QEMU's retrace bit cannot be trusted. The next section says why.

## What QEMU hides

Each of these is recorded in a document; the manual cites the section and does not add to it.

- **The retrace bit.** In QEMU the driver's retrace-synchronised fade of 16 + 16 frames finished in 0 ticks, all 32 frames "synchronised", which fits QEMU's VGA toggling the retrace bit on every read instead of timing it. The document adds that this is "as I understand its source; not checked here". On a real card the wait is real, and a card with no retrace bit would hit the bounded timeout (`docs/D-NATIVE-VGA.md`, "The palette and the fade" and "QEMU against real cards"). The transcript line `gfx: vbe: the driver's retrace-synchronised fade took 0 ticks for 16 + 16 frames (16 + 16 synchronised)` is the evidence.
- **Leaving VBE on a card that is not Bochs.** QEMU's standard card is a Bochs VBE card and leaves in one write; QEMU's Cirrus model needed the BIOS. On real S3, ET4000, Trident and Cirrus cards the extended registers are chip-specific (`docs/D-NATIVE-VGA.md`, "QEMU against real cards"). The BIOS fallback was verified on QEMU's Cirrus model only, not on a real card.
- **Monitor timing.** QEMU and DOSBox do not check CRTC timing, so the CRTC values "could be wrong in many ways and still produce the same picture"; a real monitor would show a rolling or offset picture (`docs/A-VGA-VBE.md`, "What DOSBox does and does not model" and "Expected differences on other hardware").
- **Banked modes and VBE 1.2.** Everything here assumes a linear frame buffer. Many 1990s cards, and every VBE 1.2 BIOS, offer only banked access through a 64 KiB window at `0xA0000` and function 4F05h, which "a 486-class machine with an ISA card will often be". The kernel's fallback is mode 12h by registers, and a banked 8-bit mode is not built (`docs/D-NATIVE-VGA.md`, "QEMU against real cards").
- **The frame buffer and the memory map.** QEMU's E820 does not list a PCI card's windows, so the overlap check against the map is a check against what the BIOS says; no test boots a machine where it fires (`docs/D-NATIVE-VGA.md`, "The frame buffer and the memory map" and "What was and was not verified").
- **One pitch field.** QEMU reports the same pitch in both the 2.0 and the 3.0 fields of the mode block, so the loader's path that prefers the VBE 3.0 linear pitch when it is non-zero is exercised by no test (`docs/D-NATIVE-VGA.md`, "The loader: VBE before protected mode").
- **The font pointer, and I/O delays.** The loader's call for the 8 by 16 font is a VGA BIOS function that an EGA-only BIOS may not have (`docs/D-NATIVE-VGA.md`, "QEMU against real cards"), and a few clone chips want a short delay between DAC accesses, which the driver does not make (`docs/A-VGA-VBE.md`, "Expected differences on other hardware").

## The code

| Where | What it holds |
|---|---|
| [`hal/drivers/vga/src/regs.rs`](repo:hal/drivers/vga/src/regs.rs) | The ports, [`ModeRegs`](sym:hal/drivers/vga/src/regs.rs::ModeRegs), the tables [`MODE_13H`](sym:hal/drivers/vga/src/regs.rs::MODE_13H), [`MODE_12H`](sym:hal/drivers/vga/src/regs.rs::MODE_12H) and [`MODE_03H`](sym:hal/drivers/vga/src/regs.rs::MODE_03H), [`set_mode_registers`](sym:hal/drivers/vga/src/regs.rs::set_mode_registers) and [`read_mode_registers`](sym:hal/drivers/vga/src/regs.rs::read_mode_registers) |
| [`hal/drivers/vga/src/dac.rs`](repo:hal/drivers/vga/src/dac.rs) | [`set_palette`](sym:hal/drivers/vga/src/dac.rs::set_palette), [`read_palette`](sym:hal/drivers/vga/src/dac.rs::read_palette), [`wait_vretrace`](sym:hal/drivers/vga/src/dac.rs::wait_vretrace), [`set_palette_scaled`](sym:hal/drivers/vga/src/dac.rs::set_palette_scaled), [`fade_out`](sym:hal/drivers/vga/src/dac.rs::fade_out) and [`fade_in`](sym:hal/drivers/vga/src/dac.rs::fade_in) |
| [`hal/drivers/vga/src/linear.rs`](repo:hal/drivers/vga/src/linear.rs) | [`Linear8`](sym:hal/drivers/vga/src/linear.rs::Linear8): the 8-bit surface for mode 13h and VBE |
| [`hal/drivers/vga/src/planar.rs`](repo:hal/drivers/vga/src/planar.rs) | [`Planar16`](sym:hal/drivers/vga/src/planar.rs::Planar16): mode 12h through the map mask, set/reset and bit mask; [`PITCH_640`](sym:hal/drivers/vga/src/planar.rs::PITCH_640) |
| [`hal/drivers/vga/src/canvas.rs`](repo:hal/drivers/vga/src/canvas.rs) | The [`Canvas`](sym:hal/drivers/vga/src/canvas.rs::Canvas) trait and the Adler-32 [`checksum`](sym:hal/drivers/vga/src/canvas.rs::checksum) |
| [`hal/drivers/vga/src/cursor.rs`](repo:hal/drivers/vga/src/cursor.rs) | [`SoftCursor`](sym:hal/drivers/vga/src/cursor.rs::SoftCursor) and the arrow, [`ARROW_ROWS`](sym:hal/drivers/vga/src/cursor.rs::ARROW_ROWS) |
| [`hal/drivers/vga/src/text.rs`](repo:hal/drivers/vga/src/text.rs) | The way back: [`load_font`](sym:hal/drivers/vga/src/text.rs::load_font), [`set_text_palette`](sym:hal/drivers/vga/src/text.rs::set_text_palette) and [`TEXT_DAC`](sym:hal/drivers/vga/src/text.rs::TEXT_DAC) |
| [`hal/drivers/vga/src/vbe.rs`](repo:hal/drivers/vga/src/vbe.rs) | [`Vbe`](sym:hal/drivers/vga/src/vbe.rs::Vbe), [`VbeInfo`](sym:hal/drivers/vga/src/vbe.rs::VbeInfo), [`VbeMode`](sym:hal/drivers/vga/src/vbe.rs::VbeMode), [`MODE_LINEAR`](sym:hal/drivers/vga/src/vbe.rs::MODE_LINEAR) and the attribute-bit constants |
| [`hal/hal/src/video.rs`](repo:hal/hal/src/video.rs) | [`Rgb6`](sym:hal/hal/src/video.rs::Rgb6), [`PixelLayout`](sym:hal/hal/src/video.rs::PixelLayout), [`ModeInfo`](sym:hal/hal/src/video.rs::ModeInfo), [`FrameBuffer`](sym:hal/hal/src/video.rs::FrameBuffer) and the [`Display`](sym:hal/hal/src/video.rs::Display) trait |
| [`kernel/src/video.rs`](repo:kernel/src/video.rs) | [`init`](sym:kernel/src/video.rs::init): the video block from the loader and the checks on the frame buffer; [`Lfb`](sym:kernel/src/video.rs::Lfb) |
| [`kernel/src/gfx.rs`](repo:kernel/src/gfx.rs) | The tour: [`tour`](sym:kernel/src/gfx.rs::tour), [`leave_vbe`](sym:kernel/src/gfx.rs::leave_vbe), [`restore_text`](sym:kernel/src/gfx.rs::restore_text), the paced fade and the kernel's own `Cursor` |
| [`kernel/src/vga.rs`](repo:kernel/src/vga.rs) | The kernel's text console at `0xB8000`; it does not use `desk-vga` |
| [`spikes/d0-rust-dos/rt/src/hal.rs`](repo:spikes/d0-rust-dos/rt/src/hal.rs) | [`DosDisplay`](sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay): the `Display` for DOS, with the mode set, the DPMI 0800h mapping and the clean-up |
| [`spikes/d0-rust-dos/rt/src/vga.rs`](repo:spikes/d0-rust-dos/rt/src/vga.rs) | Mode 13h by INT 10h and the frame buffer at `0xA0000` through the near-pointer arithmetic |
| [`hal/drivers/vga/tests/registers.rs`](repo:hal/drivers/vga/tests/registers.rs) | The 128-write test and the other register tests, against the model in [`hal/platform/mock/src/vga.rs`](repo:hal/platform/mock/src/vga.rs) |
| [`tests/native/expected-gfx-vbe.txt`](repo:tests/native/expected-gfx-vbe.txt) | The expected transcript of the VBE tour; [`expected-gfx-vbefail.txt`](repo:tests/native/expected-gfx-vbefail.txt) and [`expected-gfx-fallback.txt`](repo:tests/native/expected-gfx-fallback.txt) are the failed-VBE and register-only ones |

For depth: `docs/A-VGA-VBE.md` ([register tables](doc:docs/A-VGA-VBE.md#register-tables), [what DOSBox does not model](doc:docs/A-VGA-VBE.md#what-dosbox-does-and-does-not-model)), `docs/D-NATIVE-VGA.md` ([the boot information block](doc:docs/D-NATIVE-VGA.md#the-boot-information-block), [back to text mode](doc:docs/D-NATIVE-VGA.md#back-to-text-mode), [QEMU against real cards](doc:docs/D-NATIVE-VGA.md#qemu-against-real-cards)) and `docs/S5-JSDOS.md` ([VBE and the linear frame buffer](doc:docs/S5-JSDOS.md#vbe-and-the-linear-frame-buffer)).

## Worked example

**The mode 13h register walk.** The first eight writes of `set_mode_13h`, spelled out in the test as `(port, value)` pairs, are these, each followed by what it does.

| Port | Value | What it does |
|---|---|---|
| `3C0` | `00` | Video off (the attribute flip-flop was just reset by a read of `3DA`) |
| `3C4` | `00` | Select sequencer register 0 (reset) |
| `3C5` | `01` | Hold the sequencer in synchronous reset |
| `3C2` | `63` | Miscellaneous output: the 25.175 MHz clock, I/O at `3Dx`, RAM enabled |
| `3C4` | `01` | Select sequencer register 1 (clocking) |
| `3C5` | `01` | 8-dot characters |
| `3C4` | `02` | Select sequencer register 2 (map mask) |
| `3C5` | `0F` | All four planes writable |

The sequencer follows with register 3 (`00`) and register 4 (`0E`, chain-4) and the release of the reset (`03`). The last four writes of the 128 are `3C0 ← 14`, `3C0 ← 00` (the last attribute register, colour select, and its value), `3C0 ← 20` (video on) and `3C6 ← FF` (the pixel mask). In between, the CRTC unlock is `3D4 ← 11`, `3D5 ← 0E`, the value `8E` with its protect bit cleared. After the 128 writes the model asserts: miscellaneous `0x63`, sequencer register 4 `0x0E` ("chain-4 and no odd/even"), graphics controller register 5 `0x40` ("256-colour shift register") and no refused CRTC write.

A pixel at (10, 2) in mode 13h is the byte at `0xA0000 + 2 × 320 + 10`. The same pixel in mode 12h is not one byte: the host test `a_pixel_reads_to_load_the_latches_then_writes_under_a_one_bit_mask` draws `pixel(10, 2, 0x0A)` and expects exactly four port writes, `3CE ← 0` and `3CF ← 0A` (set/reset takes the colour, 10), then `3CE ← 8` and `3CF ← 20` (the bit mask is `0x80 >> (10 % 8) = 0x20`), and the one byte it touches in the window is at offset `2 × 80 + 10 / 8 = 161`, after a read of that byte to load the latches. Colour 10 is `1010` in binary, so the pixel's bit is set in planes 1 and 3 and clear in 0 and 2.

**The VBE attribute word `0x009b`.** The probe printed `mode 0x101: 640x480x8 model 4 attrs 0x009b lfb true pitch 640 phys 0xc0000000` in js-dos. In binary `0x9B` is `1001 1011`. Bit 7 is set: a linear frame buffer. Bit 4 is set: a graphics mode. Bit 0 is set: the mode is supported. Bit 6, the code's `ATTR_NO_BANKED`, is clear. Bits 1 and 3 are also set; no Desk code reads them and this manual does not name them. The planar mode in the same listing, `0x102` (800 by 600 in 4 planes), has `0x001b`: the same without bit 7, and a pitch of 100, the 800 pixels at eight to the byte. With the machine option `vesa_nolfb`, the same `0x101` reports `0x001b` and a base of `0x00000000`: the bit and the address both gone. 640 × 480 at one byte per pixel is 307,200 bytes, the same `0x4B000` as the native line.

## State of this layer

Verified here means that the record cites a test that runs in `tools/check.sh` or `tools/native-test.sh`, or a merged pull request whose CI passed. It means in QEMU, in DOSBox or on the host, never on a real VGA card or a real monitor.

```status
id: gfx.regs13
label: Mode 13h by register programming, 128 writes in the documented order
state: built-verified
evidence: test:hal/drivers/vga/tests/registers.rs::mode_13h_writes_the_documented_sequence, test:tests/native/expected-gfx-vbe.txt::gfx: 13h: frame buffer checksum 0x5c0f1f16, sym:hal/drivers/vga/src/regs.rs::MODE_13H
note: The tables were checked against DOSBox Staging's BIOS (docs/A-VGA-VBE.md, "Register tables"); the pictures read back right on QEMU's VGA too. No monitor, no real VGA chip.

id: gfx.regs12
label: Mode 12h by register programming, with the identity attribute palette
state: built-verified
evidence: test:hal/drivers/vga/tests/registers.rs::mode_12h_writes_the_documented_sequence, test:tests/native/expected-gfx-fallback.txt::gfx: 12h: frame buffer checksum 0xfaf8b758, sym:hal/drivers/vga/src/regs.rs::MODE_12H

id: gfx.planar
label: Planar drawing through the map mask, set/reset, bit mask and read map select
state: built-verified
evidence: test:hal/drivers/vga/tests/planar.rs::a_pixel_reads_to_load_the_latches_then_writes_under_a_one_bit_mask, test:tests/native/expected-gfx-fallback.txt::gfx: 12h: spike A's DOSBox checksum 0xfaf8b758, sym:hal/drivers/vga/src/planar.rs::Planar16
note: The mock's frame buffer is plain memory, so the planar write path is checked by the sequences and by two emulators, not by a model of the planes.

id: gfx.dac
label: The DAC palette load and read, and the 16-step fade through the DAC
state: built-verified
evidence: test:hal/drivers/vga/tests/dac.rs::a_fade_takes_sixteen_frames_and_ends_black_then_restores, test:tests/native/expected-gfx-vbe.txt::gfx: vbe: paced fade in, sym:hal/drivers/vga/src/dac.rs::fade_out

id: gfx.retrace
label: The retrace wait on a display whose retrace bit is real
state: built-unverified
evidence: sym:hal/drivers/vga/src/dac.rs::wait_vretrace, doc:docs/D-NATIVE-VGA.md#the-palette-and-the-fade
note: QEMU appears to toggle the bit on every read, so the wait succeeds at once there (docs/D-NATIVE-VGA.md, "The palette and the fade"); DOSBox models retrace timing and the bit toggled in all 64 waits (docs/A-VGA-VBE.md, "What DOSBox does and does not model"). The bounded timeout is tested on a model of a dead display, and a real card has never been asked.

id: gfx.text
label: Text mode restored without the BIOS: registers, palette and font in plane 2
state: built-verified
evidence: test:hal/drivers/vga/tests/text.rs::the_font_lands_in_32_byte_slots_and_the_registers_return_to_text_values, test:tests/native/expected-gfx-vbe.txt::gfx: text mode restored, test:tests/native/gfx-vbe.sums::text, sym:hal/drivers/vga/src/text.rs::load_font
note: The screenshot after the restore matches a reference checksum on QEMU. Real cards were not tried.

id: gfx.vbe
label: The VBE 2.0 layer over the Bios trait: information, modes, mode set, current mode, bank window
state: built-verified
evidence: test:hal/drivers/vga/tests/vbe.rs::set_mode_adds_the_linear_bit, test:hal/drivers/vga/tests/vbe.rs::find_picks_the_first_matching_linear_mode_and_skips_failures, test:spikes/a-vga-vbe/expected/all.txt::vbe: using 0x101, sym:hal/drivers/vga/src/vbe.rs::Vbe
note: The banked window call is wrapped and unit-tested only; no run has used it.

id: gfx.loader
label: The loader sets a VBE linear mode and the kernel checks the frame buffer against the memory map
state: built-verified
evidence: test:tests/native/expected-gfx-vbe.txt::stage2: vbe: mode set with the linear frame buffer, test:tests/native/expected-gfx-vbe.txt::overlaps no RAM, sym:kernel/src/video.rs::init
note: Verified on QEMU's standard card, and by the Cirrus run. The overlap refusal is never reached by any test.

id: gfx.overlap
label: The kernel refuses a frame buffer that overlaps RAM or the kernel
state: built-unverified
evidence: sym:kernel/src/video.rs::init, doc:docs/D-NATIVE-VGA.md#what-was-and-was-not-verified
note: The check runs on every boot and always passes; no test boots a machine where it fires.

id: gfx.vbefail
label: A refused VBE mode falls back to the register modes, with the same pictures
state: built-verified
evidence: test:tests/native/expected-gfx-vbefail.txt::the kernel falls back to the VGA register modes, test:tools/native-test.sh::gfx-vbefail, sym:boot/stage2.asm::video_setup

id: gfx.leave
label: Leaving VBE: the Bochs enable register, or the BIOS for any other card
state: built-verified
evidence: test:tests/native/expected-gfx-vbe.txt::gfx: leaving VBE: Bochs VBE registers, test:tools/native-test.sh::gfx-vbe-cirrus, sym:kernel/src/gfx.rs::leave_vbe
note: The Cirrus path is QEMU's Cirrus model. A real S3, ET4000, Trident or Cirrus card has not been asked.

id: gfx.lfb.dos
label: The linear frame buffer under DOS, mapped with DPMI 0800h
state: built-verified
evidence: test:spikes/a-vga-vbe/expected/all.txt::vbe 640x480x8: checksums match: true, sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay, doc:docs/A-VGA-VBE.md#results
note: DOSBox Staging with CWSDPMI only, as the anatomy page tags it.

id: gfx.lfb.browser
label: The linear frame buffer in js-dos in a browser
state: built-unverified
evidence: repo:spikes/s5-jsdos/evidence/vbe-jsdos.txt, doc:docs/S5-JSDOS.md#vbe-and-the-linear-frame-buffer
note: Run by hand in headless Chromium against js-dos 8.5.3 and recorded; CI does not run a browser. Only 640 by 480 by 8 was drawn.

id: gfx.cursor
label: The software cursor with save-under: the shared SoftCursor and the kernel's own
state: built-verified
evidence: test:hal/drivers/vga/src/cursor.rs::moving_restores_the_old_place_before_drawing_the_new, test:tools/native-test.sh::the cursor moved, sym:kernel/src/gfx.rs::Cursor
note: Two implementations of one idea; only the shared one is used by the DOS program.

id: gfx.banked
label: A banked 8-bit mode for cards with no linear frame buffer
state: planned
evidence: doc:docs/D-NATIVE-VGA.md#qemu-against-real-cards

id: gfx.hardware
label: A real VGA or VBE card and a monitor
state: planned
evidence: doc:docs/D-NATIVE-VGA.md#what-was-and-was-not-verified
```

The picture on the screen is {{state.gfx.regs13}} in mode 13h and {{state.gfx.regs12}} in mode 12h, and the VBE layer is {{state.gfx.vbe}}. The retrace wait is {{state.gfx.retrace}}, on purpose: QEMU's bit is not a timed one and DOSBox's has never met a real monitor.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

## Try it yourself

These commands run the driver's host tests and then the native kernel's graphics tour on QEMU's standard VGA card, the way `tools/native-test.sh` runs its `gfx-vbe` section: an image with the `gfx` test mode and a VBE request, the run script that takes the screenshots through the monitor, and the comparisons with the expected transcript and the reference checksums. They need a Rust toolchain, NASM, QEMU (`qemu-system-i386`) and Python; the scripts fall back to Docker for NASM and QEMU. Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: cargo; native run: bash, nasm, qemu-system-i386 (or Docker), python3
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ cd hal && cargo test -p desk-vga --test registers 2>&1 | grep -E "^test "
test mode_12h_writes_the_documented_sequence ... ok
test mode_13h_writes_the_documented_sequence ... ok
test the_attribute_flip_flop_is_reset_by_reading_status ... ok
test the_model_refuses_protected_crtc_writes ... ok
test the_identity_attribute_palette_is_programmed_without_touching_anything_else ... ok
test the_register_image_reads_back_equal ... ok
$ cd hal && cargo test -p desk-vga 2>&1 | grep -E "^test result: .* [1-9][0-9]* passed"
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ tools/native-image.sh --test-mode gfx --video vbe --out dist/gfx.img
dist/gfx.img: 1474560 bytes, kernel 173968 bytes in 340 sectors, test mode gfx, video vbe
$ tools/native-run.sh --timeout 60 --vga std --inject tests/native/gfx-vbe.inject --shots dist/shots dist/gfx.img > dist/gfx-run.txt
native-run: guest reported success
$ python3 -I tools/native-check.py tests/native/expected-gfx-vbe.txt dist/gfx-run.txt
transcript matches tests/native/expected-gfx-vbe.txt (88 lines)
$ python3 -I tools/native-shot.py check tests/native/gfx-vbe.sums dist/shots
native-shot: vbe-picture matches (640x480 9c399476)
native-shot: vbe-fade-half matches (640x480 7e7723f7)
native-shot: vbe-window-a matches (640x480 bd590d75)
native-shot: vbe-window-b matches (640x480 83be067f)
native-shot: vbe-window-c matches (640x480 1f104e82)
native-shot: mode-13h matches (640x400 22f676d7)
native-shot: text matches (720x400 23a1daee)
$ tr -d '\r' < dist/gfx-run.txt | grep -E "^(stage2: vbe 3|video: (the loader|mode)|gfx: (vbe picture: frame|leaving|13h: frame|text mode))"
stage2: vbe 3.0: mode 0x0101 640x480x8, pitch 640, frame buffer at 0xFD000000
video: the loader set VBE 3.0 mode 0x0101: 640x480x8, pitch 640, memory model 4, 16384 KiB on the card
video: mode 0x0101 frame buffer 0xfd000000-0xfd04b000 overlaps no RAM, heap or kernel; reserved
gfx: vbe picture: frame buffer checksum 0x5bc65936, picture in memory 0x5bc65936
gfx: leaving VBE: Bochs VBE registers (id 0xb0c5) switched off
gfx: 13h: frame buffer checksum 0x5c0f1f16, picture in memory 0x5c0f1f16
gfx: text mode restored: registers, palette and the 8x16 font reloaded
```

The screenshots land in `dist/shots/` as PPM files, and `tools/native-shot.py png` converts one for viewing. The six `test result` lines are the crate's unit tests (9) and its five test files. The `native-run` line is on the script's standard error stream.

**Exercises.** (1) Without running anything, count the writes of `set_mode_registers` by phase and check they sum to 128, then find each group in the test's `expected_writes`. (2) Predict what happens to the 16-step retrace-synchronised fade if the retrace bit never changed, using `wait_vretrace_bounded` and `fade`: how many frames wait, and what does `FadeReport` say? The test `a_fade_stops_polling_after_its_first_timeout` has the answer. (3) Decode the attribute word `0x00bb` by hand and say whether `VbeMode::linear` would accept the mode if its physical base were zero. (4) In a scratch branch, change one value of `MODE_13H.crtc` and say which of the tests above and which of the native checks would notice, and which would not (hint: see "What QEMU hides" on monitor timing).

```glossary
term: Palette
def: The table of 256 colours held in the VGA's DAC. In the 256-colour modes a pixel in the frame buffer is an index into it; each entry is three six-bit numbers.

term: Bit plane
def: One of the four layers of memory in the VGA's 16-colour modes. All four sit at the same addresses, and a pixel's colour is one bit from each plane at the same bit position.

term: Mode set
def: Putting the display into a video mode, either by writing the VGA's registers directly or by asking the BIOS (INT 10h, or VBE function 4F02h for the higher-resolution modes).

term: Software cursor
def: A mouse pointer the program draws itself into the frame buffer, saving the pixels under it and putting them back before it moves, because a VGA has no hardware cursor.

term: Pitch
def: The number of bytes from the start of one row of the frame buffer to the start of the next. It can be larger than the width of the picture, so a driver reads it from the mode and never assumes it.
```

```figures
id: m12-vga13
reuse: vga13
at: explainer vga13

id: m12-bootinfo
reuse: boot-info
at: figure bootinfo

id: m12-shot-vbe-picture
reuse: shot-d-vbe-picture
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png

id: m12-shot-vbe-fade
reuse: shot-d-vbe-fade
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png

id: m12-shot-mode13h
reuse: shot-d-mode13h
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png

id: m12-shot-mode12h
reuse: shot-d-mode12h
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png

id: m12-shot-dos-mode13h
reuse: shot-a-mode13h
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png

id: m12-vbe-attrs
reuse: vbe-attrs
at: explainer vbeattrs
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every status row was checked against the repository, and the host tests and the native VBE tour in the commands above were run on the host. Disagreements and notes found while writing: `docs/D-NATIVE-VGA.md` describes `Cursor` in `gfx.rs` and does not mention that `desk-vga` has a `SoftCursor` of the same shape that the kernel does not use (two implementations); `kernel/src/vga.rs`, which the plan lists under this chapter, is only the 80 by 25 text console and does not use `desk-vga`; and the document counts in `docs/A-VGA-VBE.md` ("82 tests in all, 47 of them new") are about the workspace at the time, and the crate now has 52 tests in six test binaries. The `vga13` and shot figures are reused from the log chapter on pixels.
