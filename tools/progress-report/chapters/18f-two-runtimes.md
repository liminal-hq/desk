---
id: runtimes
n: 23
title: Two runtimes
date: 2026-11-11
summary: A read of the Rust and DOS layers, three proposals for splitting the repository, and a comparison with Lieutenant Fizz 2, which forked Desk's runtime within days and has outgrown it; the decision is to bring its improvements home.
prs: [65, 71]
---
Chapter 8 proved that a DOS program could be Rust all the way down. Chapter 15 and the ones after it built on that runtime. This chapter is about what happened when someone else used it. It is a chapter about reading, not running: all three documents it covers are analyses of code, and none of them built or measured anything new except two commands, noted below.

**The analysis and the proposal.** Pull request #65 ({{pr65.merge}}) added two documents. `docs/RUST-DOS-INTEGRATION-ANALYSIS.md` follows each boundary from build input to shutdown (target, linker, packer, protected-mode entry, DPMI, DOS calls, allocation, interrupts, display and input, shell, loader, `std`, handoff) and marks every claim *observed* (visible in source or scripts), *recorded* (an earlier spike's run, not repeated) or *inferred*. Its largest finding is composition: the Rust-only runtime, the C-hosted module loader and the experimental `std` platform each work in their own setting, and nothing yet joins their lifecycle and failure contracts, so promoting `rt` is worthwhile but deleting the C path first would remove capabilities the Rust path has not shown. `docs/REPOSITORY-BREAKUP-PROPOSAL.md` sets out three ways to separate Desk the shell from its platform (two, four or eight repositories), recommends starting with the smallest and keeping the crate boundaries that allow the middle one later, and creates nothing. Both are suggestions: the roadmap records them as R82, done as documents.

**The fork.** Lieutenant Fizz 2 is the owner's other project: a deterministic game with a web presenter and a DOS one (`FIZZ2.EXE`). `docs/FIZZ2-COMPARISON.md` (#71, {{pr71.merge}}) records its history with Desk's runtime. Desk's pure-Rust runtime landed on 22 and 23 October and has not changed since. Fizz 2 first took it in a proof of concept on 26 October, started its own repository on 27 October, and copied the target specification, linker script, packer, stub route and several runtime files; since then it has rewritten the interrupt, timer, VGA, floating-point and memory code and added audio, probes, a PIT driver, a keyboard decoder and a text layer. Its runtime is 3,457 lines in `dos/rt/src/` against Desk's 2,304, figures taken from the comparison, not recounted. The first outside user of Desk's runtime forked it within days, because the runtime was a spike and was moving; the breakup proposal's warning against copied runtime trees describes this exactly.

**The executables have not diverged.** This is the part with a measurement. The comparison ran `file` and `cmp` on a built `SHELL.EXE` (107,520 bytes) and a built `FIZZ2.EXE` (391,680 bytes): both are DJGPP go32 v2.05 stubs with COFF images that autoload `CWSDPMI.EXE`; the first 2,048 bytes are byte-identical, so is `CWSDPMI.EXE` (21,325 bytes), the COFF headers match in flags and sections, the `.bss` is the same 271,360 bytes, and the target file is byte-identical. The divergence is everything above the packer.

<!-- figure go32-layout -->

> **From the stacks — The stub and the image.** A DJGPP program is a small real-mode "stub" in front of a 32-bit COFF image. Wikipedia's article says DJGPP can use up to 4 GB of RAM in pure DOS "when using a suitable DPMI host (e.g., CWSDPMI r7 or HDPMI32)", and that its stub and library "heavily rely on many 16-bit DOS and BIOS calls". The Desk's packer, `tools/elf2go32.py`, exists because the linker for Rust's DOS target produces an ELF file, and the stub wants COFF: the tool reads the ELF program headers and rearranges bytes into the three-section layout the stub requires, which the repository's authors found by reading the stub's code (`stubify -g` writes it out) and then confirmed by running the results. Nothing in the image beyond those 168 bytes of headers is read by the stub. For the Desk it means the "Rust-only" runtime still stands on DJGPP's stub and CWSDPMI, as the comparison notes, and that Fizz 2's copy of the packer differs from Desk's in two comment lines.
> Source: [DJGPP, Wikipedia](https://en.wikipedia.org/wiki/DJGPP)

**Where they differ, and who is better.** The comparison's table has twenty-six rows. In summary, as it judges them, with every Fizz 2 figure being Fizz 2's own and none reproduced:

- **Fizz 2 is ahead** on string-instruction `memset` and `memcpy` (it reports a 64,000-byte present falling from 3,589 to 599 µs in DOSBox), a reprogrammed PIT that keeps the BIOS clock right, sound (OPL2, OPL3, PC speaker and Sound Blaster code, never listened to, verified by write counts), hardware probes, safe settings writes with an INT 24h guard, a transcript buffer, benchmark discipline and structured CI.
- **Desk is ahead** on drivers written once against traits and tested on the host with a mock bus, on VBE and planar modes with damage tracking, on a mouse, on spurious-IRQ handling, and on running in more emulators.
- **Both** have the same clean-up list with its structural gaps, and the same unfixed findings from the d0 review.

**The decision.** On 11 November the owner decided that Desk's runtime is the canonical one, that Fizz 2's improvements come back into it, that Fizz 2 may adopt it later, and that nothing in Fizz 2 changes unless the owner asks. The roadmap records this as R131 to R140. None of it blocks the path to the first OS milestone, which is the native track.

| Item | What | Status |
|---|---|---|
| R131 | Decision: Desk's `rt` is canonical | planned (the decision is recorded; nothing is built) |
| R132 | String-instruction `memset` and `memcpy` | planned |
| R133 | Buffer the transcript until text mode returns | planned in the roadmap; built, as R127's fix, in #72 (chapter 20) |
| R134 | A reprogrammed PIT that keeps the BIOS clock right | planned |
| R135 | Sound drivers ported into `hal/drivers` | planned |
| R136 | Open decision: the finished-frame `present` against the framebuffer-lifetime gap | planned, for the owner |
| R137 | Benchmark and parity discipline | planned |
| R138 | Open decision: the float policy | planned, for the owner |
| R139 | Promote `rt` with Fizz 2's improvements merged in | planned |
| R140 | Fizz 2 adopts Desk's runtime | idea, later and only if the owner asks |

The deepest of these is R136, the design question the comparison turns on: Fizz 2's display takes a finished frame and holds no video pointer, which closes the framebuffer-lifetime gap the integration analysis found by construction, but Desk needs damage spans and planar modes, so the candidate is a presenter that owns the mapping and takes spans. The choice is the owner's; I record it as a decision waiting.

## The shelves

### Catalogued
- The documents exist and are merged: #65 ({{pr65.merge}}) and #71 ({{pr71.merge}}); the run on {{pr71.merge}} passed. The run on #65's merge commit shows failing: it is the package-mirror failure of chapter 22, which hit every run from 15:27 UTC, not anything in those documents.
- The comparison's own two measurements: `file` and `cmp` on the two executables, which it reports (I did not repeat them).
- The go32 layout above is checked against the packer's header comment and the comparison's sizes each time this page is built.

### Uncatalogued
- Every statement about Fizz 2: the comparison is a read of two code bases. Its figures (3,589 to 599 µs, 19,035 to 11,434 µs) are Fizz 2's and were not reproduced; its own later measurement of the same commit shows drawing at 9,888 µs on average against the documented 2,728, a reminder that DOSBox-only numbers drift.
- Licences: whether either side ships the DJGPP stub and CWSDPMI licence texts beside its executable was not determined, and the upstream licences of Fizz 2's derived sound code were not checked.
- `DESK.EXE`'s `file` output, which was not built for the comparison, and anything on real hardware, where both projects are emulator-only.
- The integration analysis's inferred claims, which by its own labels need a targeted experiment.

### Wish list
- R132 to R135, R137 and R139, and the two decisions (R136, R138); then, perhaps, R140.
- The repository breakup, if the owner chooses it; the proposal does not create anything.
- A FreeDOS image on 86Box with a 486 model, which the comparison suggests as the cheapest route to better x87 and ISA-VGA figures.

```figures
id: go32-layout
kind: diagram
track: hosted
title: The layout of a go32 executable
alt: A row of five blocks showing the file layout written by the packer: the 2,048-byte DJGPP stub, 168 bytes of COFF headers, the text section, the data section, and the bss, which exists only in memory.
caption: The stub, the headers and the sections, from the packer's own requirements.
at: figure go32-layout
thumb: svg
date: 2026-11-11
```
