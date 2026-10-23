---
id: oneevent
n: 15
title: One event, two worlds
date: 2026-10-23
summary: The hosted track gets a keyboard and a mouse of its own, built so that the core receives exactly the events the native kernel produces, from the same decoder, even though the hardware underneath is reached in a completely different way.
prs: [15]
---
Chapter 11 gave the native kernel a keyboard and a mouse. Spike B (pull request #15, {{pr15.state}}, branch `spike/b-dos-input`, written on top of the VGA spike and containing the keyboard spike's branch) does the same on the DOS runtime, with one constraint that shapes everything: the Desk core must see the same `KeyEvent` and `MouseEvent` types on both tracks, produced by the same decoder, so it cannot tell which world it is in.

For the keyboard that is nearly free. Under DOS the BIOS has already set up the 8042 and turned its translation on, so the CPU reads scancode set 1 just as it does natively, and the handler, the 64-byte ring and the decoder are the *same code*. What differs is the platform around them. The DOS runtime hooks IRQ 1 through DPMI with a small assembly stub that saves registers, switches to an 8 KiB interrupt stack inside the program image (because Rust code assumes a local's address can be dereferenced through the data segment and the host's stack may not satisfy that), clears the direction flag and calls Rust. Before it changes the first vector it locks the whole image in memory with DPMI function 0600h, because a page fault inside an interrupt handler under CWSDPMI is fatal. The previous vector is saved, and put back at exit and in the panic handler before the report is printed.

The mouse is where the worlds really part. A DOS program does not talk to the PS/2 auxiliary port; it asks the mouse driver, through `INT 33h`. So the hosted platform *polls*: once per core iteration it asks for the motion since the last call (function 000Bh) and the button state (0003h), each a real-mode call through DPMI, and turns the mickeys into a `MouseEvent`. The core sums motion anyway, so one event per poll and one event per packet look the same to it. The two pipelines, row by row:

<!-- explainer pipelines -->

And the contract that holds them together, as the document tabulates it:

<!-- table dos-input-table.md -->

Polling has a price, and the spike found it at once. Under a virtual X server in Docker, `xdotool` moves and clicks DOSBox's window. Motion arrived exactly as scripted, one mickey to the pixel, and buttons arrived as `L`, `R` and `M`, but a quick click, whose release comes milliseconds after its press, produced no event: both happened between two polls. A click held for 0.3 seconds did. A human click lasts several frames, so this is a limit of a tight test loop more than of the design, but the fix is known: the driver's press and release counters (functions 0005h and 0006h), or a real-mode callback (000Ch), which the spike did not build.

<!-- figure shots img-b-screen.png -->

**Replace or chain.** By default the program installs its handler and the BIOS handler does not run. That is what the Desk wants, and it costs things, which the document tabulates one by one: keys typed in the program no longer reach DOS's type-ahead buffer; the BIOS's shift and lock flags go stale, so the decoder is seeded from them at start and the flags are put back as found at exit; the keyboard lights stop following Caps Lock; Ctrl+Break and DOS's own break check never see a key; and Ctrl+Alt+Delete is lost, so a program that hangs with the handler installed cannot be rebooted from the keyboard. A `chain` mode keeps the BIOS handler running and does the opposite; the same transcript results, and the type-ahead buffer ends with eight keys in it, which the program flushes at exit because a full buffer makes the BIOS beep.

**Overflow.** A full ring drops the newest byte and counts it, and a lost break code would leave a key held down. The platform notices that the dropped count moved, waits until the ring is empty, and resets the decoder's held keys. The DOS run stalls the core for six seconds while 40 `a` presses arrive: 80 bytes in, 16 dropped, 32 pairs decoded, one recovery, and the Escape that follows still ends the run.

> **From the stacks — Why a mouse counts in mickeys.** The unit in which a mouse reports motion, the smallest distance its sensor can resolve, is called a mickey. Wikipedia says it is named after Walt Disney's Mickey Mouse. DOS mouse drivers report motion this way through function 000Bh, and the Desk's DOS adapter passes those counts to the core unscaled, as the native PS/2 path does, leaving speed and acceleration to the core.
> Source: [Mickey (unit), Wikipedia](https://en.wikipedia.org/wiki/Mickey_(unit))

## The shelves

### Catalogued
- `INPUT.EXE` in headless DOSBox Staging 0.83 with keys typed by `AUTOTYPE` prints a key transcript that equals the one a host mock produces and that `spikes/b-dos-input/expected/keys.txt` records, in VBE 640 by 480, mode 13h and text mode, with the BIOS handler replaced and chained. The IRQ 1 vector is changed while the handler is installed and restored after exit, panic and `leave`. CI passed on the merge commit `{{pr15.head}}` of pull request #15, which is {{pr15.state}}.
- Negative runs behave: a missing key makes the comparison fail (19 events against 21), a wrong expected file is rejected, no Escape ends with status 4 and everything put back, a panic gives status 3 and reports the restored vector first.
- Under Xvfb in Docker (opt-in), the mouse motion, three buttons and the wheel come out as scripted over five runs in a row.
- The `desk-int33` crate passes 16 host tests, and {{pr15.hostTests}} tests pass in `hal/` at the merge commit of this pull request.

### Uncatalogued
- Real hardware, a real BIOS keyboard handler, and any mouse driver but DOSBox's built-in one: the unit of a mickey, its scaling and the sign of the wheel on real drivers are unchecked.
- A real 8042 keeping the byte in its output buffer so a chained BIOS handler can read it too: the technique chaining relies on, from general knowledge only.
- 86Box, QEMU and Bochs; DPMI hosts other than CWSDPMI; Windows DOS boxes.
- The real-mode callback (function 000Ch), and a click shorter than a poll.
- That the shared decoder behaves the same under both tracks beyond the transcripts shown: the two lines of transcript differ in formatting, and the events are compared by the host tests, not by one program running both.

### Wish list
- The press and release counters, or a callback, so that no click is lost.
- A last-resort chain for Ctrl+Alt+Delete only, so a hung program can still be rebooted.
- Driving the keyboard lights.
- A `Bios` buffer convention that reaches the INT 33h functions that need `ES:DX`.

```figures
id: pipelines
kind: explainer
track: shared
title: Two input pipelines, side by side
alt: A step-through of one key and one mouse move through the native and the hosted pipelines, row by row, marking which stages share code and which differ.
caption: Blue rows are shared code; amber rows differ.
at: explainer pipelines
thumb: steps

id: shot-img-b-screen
kind: screenshot
track: hosted
title: The DOS input program screen
alt: A dark 640 by 480 screen with three small rectangles at the top left and an arrow cursor near the middle, read back from the frame buffer.
caption: The screen after the mouse run, read back by the program.
at: figure shots img-b-screen.png
anchor: shot-img-b-screen
thumb: img:img/img-b-screen.png
```
