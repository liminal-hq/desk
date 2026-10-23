---
id: browser
n: 14
title: The browser too
date: 2026-10-23
summary: Spike s5 ran every DOS program the repository builds inside js-dos 8 in a browser, and asked the one question the spec had left open: does the VBE linear frame buffer work there?
prs: [13]
---
The hosted track has a second ambition besides old PCs: a demo that anyone can open from a link. `docs/OPEN-QUESTIONS.md` had set a condition on it: ship the browser build with DOSBox only, unless a spike showed the VESA linear frame buffer working in js-dos. Pull request #13 ({{pr13.state}}, branch `spike/s5-jsdos`, built on `main`) is that spike, run in headless Chromium 154 on Linux against js-dos 8.5.3 and compared line by line with DOSBox Staging 0.83 running the same programs.

The answer is **yes**, and it is a stronger yes than the question needed. Every program the repository builds for DOS runs unchanged in the browser with `CWSDPMI.EXE` beside it (or the self-contained stub build): the hosted skeleton, the d0 programs, and the s1 module demo including its recovered panic and caught divide fault. Their transcripts are byte for byte the ones the native harness expects. Each row passed in all three of three full runs of 18 scenarios, about 44 seconds for the suite, with a cold browser profile each time.

<!-- table jsdos-results.md -->

**The question itself.** The probe asks the emulated BIOS for the mode list, and for each mode the attribute word. Bit 7 is the linear frame buffer. In js-dos mode `0x101`, 640 by 480 at 8 bits, reports `0x009b`, the bit is set, and the physical base is `0xC0000000`; setting the mode with `4F02h` works, DPMI function 0800h maps the buffer under CWSDPMI, and the pattern the program writes there is what the page's canvas shows: zero of 307,200 pixels differ from a pattern the page computes for itself. The banked path through `4F05h` gives the identical picture. Here are three real lines from the probe, decoded.

<!-- explainer vbeattrs -->

<!-- figure shots img-s5-vbe.png -->

Four rules for the browser build come out of the differences the spike measured, and each is a rule for the code, not for the emulator. Read the frame buffer's address from `4F01h` and never hard-code it (js-dos reports `0xC0000000`, its DOSBox-X backend `0xE0000000`). Do not rely on more than two mebibytes of video memory (js-dos reports 2,048 KiB where Staging reports 4,096). Do not rely on the mode list: js-dos lists 47 modes and Staging 59, and only the five standard 8-bit VESA numbers are in both, so find a mode by its size and depth. And keep a banked path, because it works too and is what the `vesa_nolfb` setting leaves.

Sound was tested the only way a headless run can, by measuring the pitch of the samples the emulator produces: an OPL2 sine at 440 Hz measured 440, a Sound Blaster DMA square wave of 400 Hz measured 400, and the PC speaker's 440 Hz measured 440.1. The Sound Blaster's DSP answered, the DMA reached terminal count and IRQ 7 went pending. Nothing was audible to anyone, and the measurement is taken before the browser's audio path, so it checks the emulation and not the playback.

What the browser build is, then: DOSBox with CWSDPMI running inside the emulated DOS, so the runtime contract of chapter 8 holds as it stands. js-dos 8's backend is a fork of the original DOSBox compiled to WebAssembly; its exact revision is not recorded in the files, so the spike could say only that it reports itself as "jsdos". That means a result from Staging is not automatically a result from js-dos, and the spike re-ran everything rather than assume. Which also means the finding that Staging's `cputype = 486` rejects `CMPXCHG` was *not* re-checked there.

> **From the stacks — DOS in a tab, before js-dos.** TechCrunch reported in January 2015 that the Internet Archive was hosting 2,296 MS-DOS games, playable in a browser, powered by EM-DOSBOX, an independent project that compiles DOSBox for the web and focuses solely on DOS. As the story is usually told, that collection is what made running DOS programs on a web page seem ordinary. js-dos is a different project, whose DOSBox backend the spike measured as a fork compiled to WebAssembly; I have not traced the lineage between the two and do not claim one.
> Source: [TechCrunch, 9 January 2015](https://techcrunch.com/2015/01/09/internet-archive-brings-oregon-trail-prince-of-persia-lemmings-and-2200-other-ms-dos-games-to-your-browser)

## The shelves

### Catalogued
- The 18 scenarios pass in headless Chromium 154 with js-dos 8.5.3, three clean runs, and the DOSBox-X backend once (it passed all but one scenario, which did not repeat); transcripts equal the native ones and the 640 by 480 by 8 pictures equal a computed reference exactly, through the linear frame buffer and through the bank window. CI passed on the merge commit `{{pr13.head}}` of pull request #13, which is {{pr13.state}}.
- With `vesa_nolfb` set, no mode has the linear bit and the base address is zero; the probe says so and stops.
- Pitch measurements of the OPL2, the Sound Blaster and the PC speaker, from the emulator's own sample stream.

### Uncatalogued
- Browsers other than Chromium, other operating systems, touch and phones: no Firefox, no Safari, no mobile.
- Hearing anything: the playback path, the prebuffer and the choice between OPL emulators on music were not checked.
- The Desk itself: only the skeleton and the spike programs ran, so mouse capture and keyboard focus in the page are untested.
- Other VBE modes than 640 by 480 by 8, and palette fades in the browser.
- The DOSBox revision inside js-dos, and a page served from a real host with its own caching and MIME types.
- Performance on a slow device, and the browser demo's own frame rate, which depends on a compositor that does not exist.

### Wish list
- A `.jsdos` bundle of the hosted Desk, from the first release that has one.
- A recorded reference tune compared by envelope or spectrum, to tell the OPL emulators apart.
- A page served from a real host, with the keyboard and mouse checked in it.

```figures
id: vbe-attrs
kind: explainer
track: hosted
title: Reading a VBE mode attribute word
alt: A step-through of three real lines from the js-dos VBE probe, decoded bit by bit to show which modes have a linear frame buffer.
caption: Bit 7 is the linear frame buffer, and one option takes it away.
at: explainer vbeattrs
thumb: steps

id: shot-img-s5-vbe
kind: screenshot
track: hosted
title: The VBE pattern in js-dos
alt: A 640 by 480 test pattern of a smooth colour gradient above a fine woven diagonal pattern, as the browser page returned it.
caption: Drawn through the linear frame buffer, no pixel differs.
at: figure shots img-s5-vbe.png
anchor: shot-img-s5-vbe
thumb: img:img/img-s5-vbe.png
```
