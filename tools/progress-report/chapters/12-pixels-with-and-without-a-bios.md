---
id: pixels
n: 12
title: Pixels, with and without a BIOS
date: 2026-10-23
summary: One VGA and VBE driver written against the HAL, proved first on the DOS runtime and then reused by the native kernel, which has no BIOS to lean on, and the card on which leaving a graphics mode went wrong.
prs: [11, 14]
---
Two spikes belong in one chapter, because the second is the first reused. Spike A (pull request #11, {{pr11.state}}, branch `spike/a-vga-vbe-rust`) writes `desk-vga`, a driver that programs VGA modes 13h and 12h straight through the ports, loads and fades the palette, draws, and speaks VBE 2.0 through a small `Bios` trait, and proves it on the DOS runtime. Spike D (pull request #14, {{pr14.state}}, branch `spike/d-native-vga`, written on top of the keyboard spike and containing spike A's branch) runs the same driver from the bare-metal kernel, with a cursor and a first window, where nothing is left to call the BIOS with.

**Programming a VGA by hand.** A mode is a few dozen register values written in the right order, and the order is half the knowledge. The driver turns video off, holds the sequencer in reset while the clock register changes, unlocks the CRTC (register `11h` has a protect bit that makes registers 0 to 7 ignore writes), writes the CRTC, the graphics controller and the attribute controller, and turns video on. The step-through below runs that sequence for mode 13h, 320 by 200 in 256 colours, from the real table in `regs.rs`: exactly 128 port writes, then one colour loaded into the DAC and one pixel stored.

<!-- explainer vga13 -->

Mode 12h, 640 by 480 in 16 colours, is the same sequence with a different table and a stranger memory: four planes at the same addresses, written with set/reset registers and a bit mask. The palette is the Desk's own; fades go through the DAC, never the frame buffer. And the tables are the commonly published ones: they were checked in one way only, that DOSBox Staging's BIOS programs the same values for the same modes, apart from a difference in sequencer register 0 that was not investigated. A real monitor has never seen them, and QEMU does not check timing.

**VBE and the linear frame buffer.** VBE is the BIOS extension that lets a program ask for a high-resolution mode by number. A mode that has the *linear frame buffer* attribute can be written as one flat block of memory instead of through a 64 KiB window, which is what a protected-mode program wants. Spike A asks DOSBox's S3 model for mode `0x101` (640 by 480 by 8), finds it with a linear frame buffer at physical `0xC0000000`, maps it with DPMI function 0800h and draws. Three read-back checksums, one per mode, equal the same picture drawn into memory: `0x5c0f1f16` for mode 13h, `0xfaf8b758` for mode 12h and `0x5bc65936` for VBE.

**The loader asks the BIOS first.** The native kernel runs in flat protected mode with no V86 monitor, so it cannot call `INT 10h`. VBE can only be asked in real mode, so spike D teaches the loader to do it just before the switch: copy the BIOS's 8 by 16 font (the one thing the kernel cannot recover once text mode has been left), ask for the VBE controller block, walk the mode list for a packed-pixel 8-bit mode with the linear bit, set it, and write down what it did at fixed addresses. The kernel reads the blocks, checks the frame buffer against the memory map, and draws through the same `Linear8` that the DOS program used.

<!-- figure bootinfo -->

The result is the point of the exercise. The kernel's read-back checksum of the picture in VBE is `0x5bc65936`, the DOSBox number to the digit; its mode 13h and mode 12h pictures read back as `0x5c0f1f16` and `0xfaf8b758`. The same driver and the same scene give the same bytes on two different machine models. The pictures are below, as QEMU's monitor wrote them out with `screendump`: the VBE picture, the same picture at half intensity in the middle of a fade, mode 13h and mode 12h; the last is spike A's own read-back on DOS.

<!-- figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png -->

**The first window.** Once there are pixels the spike draws a window: an orange title bar with a close box, a plain body, a one-pixel border, over the horizon, and an arrow cursor of 12 by 19 pixels that saves what is under it, follows the PS/2 mouse of chapter 11 and is clamped to the screen. The keys typed are echoed in the window, with a caret. The test script moves the mouse with `mouse_move 100 60`, types, takes a screenshot through the monitor and compares it with a reference checksum, then moves again and checks the second screenshot differs.

<!-- figure shots img-d-vbe-window.png img-d-mode12h-window.png -->

Be exact about what this is. The window is three fills and a rectangle in `kernel/src/gfx.rs`: no window object, no clipping against other windows and no dirty rectangles. It is not drawn by a shared compositor, because there is none. The milestone the archive calls First Window needs the shared core on both tracks, and so far each track has its own demo.

**The way back, and the card that would not.** With no BIOS there is no `INT 10h` call to return to text mode, so the kernel reprograms the text registers, the sixteen text colours and the font in plane 2. After the register-programmed modes, and after VBE on QEMU's standard card and its VMware card, that works. After VBE on `-vga cirrus` it does not: the same register programming leaves a garbled screen.

<!-- figure shots img-d-text-restored.png img-d-cirrus.png -->

That is the most important thing the spike found, and the reason this chapter ends on a problem. A VBE mode leaves state in the card's own extended registers, which are chip-specific; on QEMU's standard card, leaving takes one write to the Bochs VBE enable register, and that is a fact about that card only. A kernel that sets VBE therefore has to know each chip in order to leave it, or ask the BIOS again (V86 or a real-mode trampoline), or reboot. Either VBE is a one-way trip, or the native kernel needs a real BIOS path. That is what spike n3 is for, and chapter 17 is its answer.

Two last facts, both from the spike's own measurements and both about an emulator, not a 486. A full-screen fill at 640 by 480 went from about 650 to about 8,300 a second under QEMU's translation when the driver stored words instead of bytes, and a retrace-synchronised fade finished instantly because QEMU appears to toggle the retrace bit on every read, so the kernel paces its fade with its own timer.

> **From the stacks — VESA and the linear frame buffer.** The VESA BIOS Extensions were defined in 1989 so that programs could use the many incompatible Super VGA cards through one interface. Version 2.0, dated November 1994, added linear frame buffer access and protected-mode banking. That is why the loader requires a mode with the linear bit and why it says "older than 2.0" when a BIOS reports an earlier version: a VBE 1.2 BIOS has no such mode, and the spike's own document expects a 486-class machine with an ISA card to offer only banked access, which is why mode 12h is the fallback.
> Source: [VESA BIOS Extensions, Wikipedia](https://en.wikipedia.org/wiki/VESA_BIOS_Extensions)

## The shelves

### Catalogued
- On the DOS runtime in headless DOSBox Staging 0.83 (`svga_s3`, `cputype = 486`), each stage reads its picture back with the checksum of the same picture in memory: mode 13h by registers and by `INT 10h`, mode 12h both ways, and VBE 640 by 480 by 8 with the linear frame buffer; a flipped pixel makes the check fail, and `vgaonly` has no VBE and is reported as such. CI passed on the merge commit `{{pr11.head}}` of pull request #11, which is {{pr11.state}}.
- On the native kernel in QEMU 11.1.2 (`-vga std`), the three pictures read back with spike A's checksums, a fade and a text-mode restore run, and every screenshot taken through the monitor matches its reference checksum; seven negative checks behave as described. CI passed on the merge commit `{{pr14.head}}` of pull request #14, which is {{pr14.state}}, and the screenshot checksums matched on CI's own QEMU.
- `cargo test` in `hal/` has {{pr11.hostTests}} tests at the tip of pull request #11 and {{pr14.hostTests}} at the tip of #14, which adds the text-mode restore and the faster fill; all pass in CI.

### Uncatalogued
- Real hardware: no real VGA chip, VBE BIOS or monitor. QEMU does not check CRTC timing, so the register values were never seen by one.
- 86Box and Bochs were not tried, and neither were VBE 1.2 or banked-only BIOSes, which the loader refuses and the kernel answers with mode 12h.
- Leaving VBE on any card but the Bochs one; on Cirrus it fails, as shown.
- Frame rates: the numbers above are emulated speed.
- BIOSes other than SeaBIOS's VGA BIOS, and the offsets of the VBE mode block, which were checked against what QEMU returns and not against the specification text.

### Wish list
- A native BIOS path, which spike n3 built, and which is merged (chapter 17), or a card-specific leave.
- A banked 8-bit mode for cards with no linear frame buffer.
- A fade that takes its pacing from a clock instead of assuming retrace works.
- A shared palette crate and one VBE mode parser fed the raw block by the loader.
- A compositor that draws windows into a back buffer and copies once per frame.

```figures
id: vga13
kind: explainer
track: shared
title: Mode 13h, register by register
alt: A step-through of the 128 port writes that set VGA mode 13h, followed by loading one colour into the DAC and storing one pixel.
caption: The documented register sequence for mode 13h, with the real values.
at: explainer vga13
thumb: steps

id: boot-info
kind: diagram
track: native
title: The loader hands over to the kernel
alt: Diagram of the loader asking the BIOS for a font and a VBE mode in real mode, writing a boot information block and a video block, then the kernel reading them in protected mode.
caption: What the loader asks the BIOS, and where it leaves the answer.
at: figure bootinfo
thumb: svg

id: shot-d-vbe-picture
kind: screenshot
track: native
title: The VBE test picture
alt: The native test picture in the VBE linear frame buffer: stepped purple to orange bands, a rainbow ramp and a box of fine lines with six colour swatches.
caption: Native, VBE linear frame buffer.
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png
anchor: shot-img-d-vbe-picture
thumb: img:img/img-d-vbe-picture.png

id: shot-d-vbe-fade
kind: screenshot
track: native
title: The half-faded palette
alt: The same test picture at about half intensity during a palette fade, every colour darker.
caption: The DAC fade at its half-way step.
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png
anchor: shot-img-d-vbe-fade
thumb: img:img/img-d-vbe-fade.png

id: shot-d-mode13h
kind: screenshot
track: native
title: Mode 13h on the native kernel
alt: The test picture in mode 13h on the native kernel, shown doubled to 640 by 400.
caption: Native, mode 13h by registers.
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png
anchor: shot-img-d-mode13h
thumb: img:img/img-d-mode13h.png

id: shot-d-mode12h
kind: screenshot
track: native
title: Mode 12h on the native kernel
alt: The test picture in mode 12h with sixteen colours: three bands of brown and orange, a ramp and the swatch box.
caption: Native, mode 12h by registers.
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png
anchor: shot-img-d-mode12h
thumb: img:img/img-d-mode12h.png

id: shot-a-mode13h
kind: screenshot
track: hosted
title: Mode 13h on DOS
alt: The test picture in mode 13h shown doubled, as the DOS program read it back from the frame buffer under DOSBox Staging.
caption: DOS, read back by the program.
at: figure shots img-d-vbe-picture.png img-d-vbe-fade.png img-d-mode13h.png img-d-mode12h.png img-a-mode13h.png
anchor: shot-img-a-mode13h
thumb: img:img/img-a-mode13h.png

id: shot-d-vbe-window
kind: screenshot
track: native
title: The first window with a cursor
alt: A dark window with an orange title bar reading Desk: first window, the typed text desk with a caret, and an arrow cursor, over a purple to orange horizon.
caption: The first window, drawn by the native kernel.
at: figure shots img-d-vbe-window.png img-d-mode12h-window.png
anchor: shot-img-d-vbe-window
thumb: img:img/img-d-vbe-window.png

id: shot-d-mode12h-window
kind: screenshot
track: native
title: The first window in 16 colours
alt: The same first window in 16 colours over a three-band brown and orange horizon, with the arrow cursor.
caption: The window through the planar driver.
at: figure shots img-d-vbe-window.png img-d-mode12h-window.png
anchor: shot-img-d-mode12h-window
thumb: img:img/img-d-mode12h-window.png

id: shot-d-text-restored
kind: screenshot
track: native
title: Text mode restored
alt: A black text screen with two white lines saying text mode was restored and the font reloaded.
caption: Text mode back after graphics, with no BIOS.
at: figure shots img-d-text-restored.png img-d-cirrus.png
anchor: shot-img-d-text-restored
thumb: img:img/img-d-text-restored.png

id: shot-d-cirrus
kind: screenshot
track: native
title: The Cirrus garble
alt: A screen of scattered coloured fragments of characters on black: unreadable text after leaving VBE on a Cirrus card.
caption: Leaving VBE on QEMU's Cirrus card, not restored.
at: figure shots img-d-text-restored.png img-d-cirrus.png
anchor: shot-img-d-cirrus
thumb: img:img/img-d-cirrus.png
```
