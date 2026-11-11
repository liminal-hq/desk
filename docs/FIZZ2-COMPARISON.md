# Fizz 2 and Desk: where the runtimes diverged

A comparison of Desk's DOS runtime with the one in Lieutenant Fizz 2, which began as a copy of it. It records what Fizz 2 is, how it relates to Desk, where the two now differ, what Desk should take from it, and what Desk learned that Fizz 2 could take back. The roadmap items that follow from it are R131 to R140 ([`ROADMAP.md`](ROADMAP.md#future-runtime-reunification)).

## Contents

- [Method and evidence](#method-and-evidence)
- [What Fizz 2 is](#what-fizz-2-is)
- [How the two relate](#how-the-two-relate)
- [The decision](#the-decision)
- [The executables have not diverged](#the-executables-have-not-diverged)
- [Divergence table](#divergence-table)
- [Learnings for Desk](#learnings-for-desk)
- [What Fizz 2 could take back from Desk](#what-fizz-2-could-take-back-from-desk)
- [Where Fizz 2 contradicts a Desk document](#where-fizz-2-contradicts-a-desk-document)
- [Open questions](#open-questions)
- [What is unverified](#what-is-unverified)

## Method and evidence

This is a read of two code bases, not a run. Nothing was built, measured or executed for it, except that `file` and `cmp` were run on two built executables (see [the executables](#the-executables-have-not-diverged)). Every figure for Fizz 2 is Fizz 2's own, taken from its documents and snapshots; where this document restates one it says so. Every claim about Desk was checked against the tree at the time of writing; the few that were not are listed under [What is unverified](#what-is-unverified).

The sources are Desk at `b928309` (`main`), and Fizz 2 (`ScottMorris/lieutenant-fizz-2`) at `origin/main` `46afd6c`, dated 2026-11-06, 650 commits. A later measuring branch of Fizz 2 (`docs/progress-measured`) re-measured that commit on 2026-11-10; its snapshot is `tools/progress-report/snapshots/measured.json` there. Fizz 2's two published artefacts (its Architecture page and Progress Report) were not read; its repository was enough. Everything DOS-side in Fizz 2 is verified only in DOSBox Staging 0.83 (its `docs/PLATFORM.md#not-verified`), and so is most of Desk's hosted track. Nothing on either side has run on real hardware.

## What Fizz 2 is

Lieutenant Fizz Episode 2 is one deterministic game with two first-class presenters, Web (WASM) and DOS (`FIZZ2.EXE`), built in parallel from one `f64` simulation at a fixed 60 Hz. A state hash is the arbiter: native x86_64, WASM and DOS in headless DOSBox must all print the same replay transcript (`PARITY ok ec9195c222b626f2` at `46afd6c`, per the measured snapshot). The rules port Lieutenant Fizz 1's line for line and are proved against Fizz 1's own traces bit for bit (22 tests).

On DOS it has:

- mode 13h drawing with a retrace wait
- a 60 Hz or 560 Hz PIT timer that keeps the BIOS clock right
- OPL2, OPL3 (a stereo pair) and PC speaker music and effects, driven from the timer interrupt
- hardware probes and a start-up check screen
- a settings file with safe writes and an INT 24h guard
- a frame-rate governor
- a `/BENCH` mode with a PIT stopwatch

Milestone 1, a playable vertical slice, is on `main`. A pack format (`fizz-pack`, a `LEVL` section) is partly in. Its runtime is 3,457 lines in `dos/rt/src/`; Desk's is 2,304 in `spikes/d0-rust-dos/rt/src/`.

## How the two relate

Desk's pure-Rust DOS runtime (`spikes/d0-rust-dos`) landed on 2026-10-22 and 2026-10-23. Its last change is dated 2026-10-23 (the merge `58eb810`; the last non-merge change is `ec86c13`). It has not changed since.

Fizz 2 first took it in a proof of concept (`fizz-2-dos-poc1`, tag `poc-1`, 2026-10-26). That proof's findings (`docs/POC-1-FINDINGS.md` in Fizz 2) said the DOS platform layer was Desk's HAL and `dos-rt`, not a separate runtime. Fizz 2's own repository began on 2026-10-27 and took the other course: `docs/PLATFORM.md` says `fizz-hal` mirrors the shapes of Desk's HAL and is written there, "not vendored and not a path dependency, so the real Desk crates can replace it later". It copied Desk's target specification, linker script, packer, the DJGPP stub and CWSDPMI route, and the runtime files `start`, `dpmi`, `dos`, `heap`, `env`, `panic`, `cleanup` and `io` (the provenance table in `docs/PLATFORM.md#provenance`). Since then it has rewritten `irq`, `timer`, `vga`, `fpu` and `mem` and added `audio`, `probe`, `pit`, `kbd`, `text`, `ring` and `port`.

The first external consumer of Desk's runtime therefore forked it within days, because the runtime was a spike and was moving. The shared-driver intent in [`DIRECTION.md`](DIRECTION.md) ("Drivers written once … so Lieutenant Fizz Episode 2 on DOS … can share them") and the warning in [`REPOSITORY-BREAKUP-PROPOSAL.md`](REPOSITORY-BREAKUP-PROPOSAL.md) against copied active runtime trees both describe exactly this risk.

## The decision

On 2026-11-11 the owner decided: bring Fizz 2's improvements back into Desk, and keep Desk's runtime as the general, canonical one. Fizz 2 may adopt it later. Two copies coexist for a while, and nothing in Fizz 2 changes unless the owner asks. This document is therefore about moving work into Desk, not about changing Fizz 2. The roadmap records the decision as R131.

## The executables have not diverged

At the executable level the two have not diverged. `file` on both reported the same kind of file:

```
SHELL.EXE  MS-DOS executable, COFF for MS-DOS, DJGPP go32 v2.05 DOS extender (stub), autoload "CWSDPMI.EXE"   107,520 bytes
FIZZ2.EXE  MS-DOS executable, COFF for MS-DOS, DJGPP go32 v2.05 DOS extender (stub), autoload "CWSDPMI.EXE"   391,680 bytes
```

The first 2,048 bytes (the stub) are byte-identical (`cmp`), and so is `CWSDPMI.EXE` (21,325 bytes). Both COFF headers have flags `0x107`, three sections (`.text` at `0x1000`, `.data` and `.bss` 512-byte aligned) and the same 271,360-byte `.bss`. Fizz 2's `dos/dos-elf.json` is byte-identical to Desk's `spikes/d0-rust-dos/dos-elf.json`; `dos/link.ld` is recorded as copied unchanged; `tools/elf2go32.py` differs only in two comment lines; and both CI files pin the same `ci-dos` image digest. The `SHELL.EXE` used was a build from a FreeDOS run, and the `FIZZ2.EXE` came from Fizz 2's measuring worktree.

Desk's other hosted executable, `DESK.EXE` from `tools/build.sh`, is linked with gcc together with the C layer, so it looks different. It was not built for this comparison, so its `file` line is unverified. The real divergence is everything above the packer.

## Divergence table

"Better" names the side whose approach Desk should prefer, and why.

| Area | Desk | Fizz 2 | Better, and why |
|---|---|---|---|
| Target, linker, packer, stub, extender | `spikes/d0-rust-dos/dos-elf.json`, `link.ld`, `tools/elf2go32.py`, DJGPP stub, CWSDPMI | Identical copies (`dos/dos-elf.json`, `dos/link.ld`, `tools/elf2go32.py`) | **Same**, but there are now two copies. The permissive `parse_elf` finding of the [integration analysis](RUST-DOS-INTEGRATION-ANALYSIS.md) applies to both, and a fix in one will not reach the other |
| Native kernel target | `kernel/i486-desk-none.json` | None; Fizz 2 has no native track | Not comparable |
| Edition and toolchain | Edition 2021, Rust 1.98.0 | Edition 2024 (`#[unsafe(no_mangle)]`), Rust 1.98.0 | Fizz 2 is slightly more current; not important |
| Where the runtime lives | `spikes/d0-rust-dos/rt`, a path dependency of `shell/dos`; promotion is R53 | `dos/rt` (`fizz-dos-rt`) in its own `dos/` workspace, kept out of the root workspace so host checks never need `rust-src` | **Fizz 2.** It has a real product consumer. Desk's `shell/dos` is a separate workspace too, but its runtime is still a spike |
| HAL shape | A lower edge (`PortIo`, IRQ registry) and an upper edge; shared drivers generic over `PortIo` and tested against a mock bus ([`HAL.md`](HAL.md)); `rt` uses `desk-pic`, `desk-vga`, `desk-i8042` and `desk-int33` | `fizz-hal` has an upper edge only (clock and `Pacer`, input and `Set1Decoder`, display, `SpscRing`). `dos/rt` does not depend on it; drivers are free functions over `port::inb` and `port::outb` | **Desk for drivers**: register sequences are written once and tested on the host. Fizz 2's OPL, speaker, PIT and probe code has no port-level mock tests; only its pure models (`fizz-machine`) are host-tested. **Fizz 2 for the display edge** (next row) |
| Display contract | `Display` hands out an owned `FrameBuffer` or `MmioRegion` that can outlive the mode (integration analysis section 3, an open soundness gap) | `Display::present(pixels, pitch)` takes the finished frame; the HAL holds no video pointer and no `unsafe` (a "Deliberate deviation from Desk" in `docs/PLATFORM.md`) | **Fizz 2.** It closes that gap by construction. Desk needs damage-span presents, so it should borrow the idea, not copy it (R136) |
| Fills and copies to video memory | `MmioRegion::write_bytes` does one volatile byte store per pixel (`hal/hal/src/mmio.rs`, lines 74 to 84). The shared crates' `fill` and `copy_from_slice` end in `compiler-builtins-mem` loops | `dos/rt/src/mem.rs` overrides `memset` and `memcpy` with `rep stosd` and `rep movsd`; the strong symbol beats the weak one in `compiler_builtins`, so the portable crates stay `forbid(unsafe_code)` | **Fizz 2, measured** by Fizz 2 in DOSBox at 26,800 cycles: drawing from 19,035 to 11,434 µs, and the 64,000-byte present from 3,589 to 599 µs (`docs/PLATFORM.md`, "Drawing changes"). DOSBox does not model the ISA bus, so on real hardware the gain is fewer instructions, not a measured bus speed-up |
| Timer | Hosted: no PIT reprogramming; the shell paces on the 18.2 Hz BIOS count (`shell/dos/src/main.rs`, `elapsed_ticks`); `rt/src/timer.rs` is a counting stub that always chains. Native: 100 Hz through `desk_pit` | PIT at 60 Hz (divisor 19886) or 560 Hz (2131) with a 16-bit accumulator that chains to the BIOS every 65,536 input clocks, so DOS time stays right. Restored on exit. An integer `Pacer` with a catch-up cap | **Fizz 2.** Desk has no fine hosted clock. Fizz 2 also found and fixed a midnight-wrap bug in CI (its `PLATFORM.md`) |
| IRQ dispatch | A registry over `desk_hal::IrqRegistry`. `Chain` is fixed when the line is registered (`hal/hal/src/irq.rs`: `None` or `Previous`); spurious IRQ 7 and 15 are handled | The handler returns `Action::Eoi` or `Action::Chain` on each interrupt; all state is atomics with no `static mut`; spurious IRQs are not detected | **Mixed.** A reprogrammed PIT has to decide per interrupt, which Desk's API cannot express. Desk's spurious-IRQ handling is more complete |
| Clean-up list | 8 slots, first free slot, run in reverse slot order; some callers ignore `false` (integration analysis section 2) | A byte-identical file. Callers in `irq`, `kbd`, `timer` and `audio` check the result; `vga.rs`, `text.rs` and the INT 24h guard in `dos.rs` still ignore it | **Fizz 2, slightly**, but it inherited every structural gap in section 2 (fallible registration, last-in-first-out order broken by slot reuse) |
| The d0 review findings (R54) | Unfixed | Unfixed and inherited: `die` has no recursion guard, and the heap sums are not saturating | Neither |
| Video modes | Mode 13h, VBE 640×480×8 linear (DPMI 0800h mapping) and 640×480×4 planar; a software cursor in `desk-vga`; damage tracking | Mode 13h only; a full-frame 320×200 redraw every frame, then one copy; the retrace wait bounded at 1,000,000 reads a phase | **Desk** for breadth (the desktop needs 640×480). For a full-screen game Fizz 2's whole-frame approach is simpler and measured. Both reach A0000h by the same near-pointer trick, which holds only on CWSDPMI-style hosts |
| Retrace wait | `desk_vga::wait_vretrace`, two phases, bounded, in a shared and tested driver | The same method, but in `rt/src/vga.rs` and not shared | **Desk**, because it is shared and tested |
| Input | IRQ 1 through `desk-i8042`; the mouse through INT 33h (`desk-int33`); BIOS flag state in `bios_kbd.rs` | IRQ 1 in `rt/src/kbd.rs`, which restores the `0x417` lock bits and clears `0x418`; Set 1 decoding in `fizz-hal`; an `InputLatch` that makes a tap last at least one tick; no mouse; joystick only probed | **Desk** for the device set, **Fizz 2** for the press-edge latch, which fixed dropped-sound and diagonal-input bugs in the proof of concept (`POC-1-FINDINGS.md`) |
| Floating point | `shell/` forbids floating-point maths | The rules use `f64` on the x87 at 53 bits (control word `0x027F`, set before the first tick; `FNINIT` leaves `0x037F`). `libm` is pinned at `=0.2.16` with no `exp` family; subnormals snap below 2^-1000. Presenters, HAL and `rt` deny `clippy::float_arithmetic`; interrupt handlers never touch the x87 | **Each fits its job.** Desk's integer rule is right for a shell. Fizz 2 shows what an app on Desk would need for `f64` to stay deterministic (R138) |
| Sound | None (R81 and R62 are planned; R116 is the Audio Player) | `rt/src/audio.rs` (800 lines): a 560 Hz service with OPL2 channel 5, an OPL3 stereo pair (channels 5 and 14) and the PC speaker (PIT channel 2 at 140 Hz); music and effects from one handler; live `replan`; `stop` on the clean-up list puts `0x105` back. Probes: OPL timer test, OPL3, dual OPL2, DSP reset and version, `BLASTER` parsing. No DMA or PCM | **Fizz 2** by a wide margin. Caveat: nobody has listened to the DOS output; the evidence is write counts and a non-silent DOSBox recording (`PLATFORM.md`, "Not verified") |
| Hardware probing | None in the hosted track | `rt/src/probe.rs`: video class, mouse (only if the INT 33h vector is non-null, because a null vector hangs DOS), joystick, OPL, Sound Blaster, x87, CPU family by the EFLAGS AC and ID bits (no CPUID) and DPMI 0500h free pages. Every wait is bounded (`docs/DOS-CHECK.md`) | **Fizz 2.** Inspector (R121) and Settings (R115) need exactly this |
| Assets | `.DFN` fonts loaded as files beside `SHELL.EXE` | Generated `const` data today; Fizz Asset Packs are specified in `docs/FAP.md`: a version, a schema-derived compatibility number, a per-section checksum and codec, raw deflate, and a stored stem that must match the file name | **Fizz 2's pack design** is the more thorough format specification. Worth reading for R48 |
| Console output | `println!` in interactive mode draws over the graphics screen (R127) | The transcript goes into a fixed 8,192-byte buffer, "so nothing is printed (and no DOS call made) while the screen is in graphics mode", and is printed after the return to text mode (`dos/fizz2/src/main.rs`, the `Transcript` type) | **Fizz 2.** This is the fix for R127, already in use |
| Settings files and DOS errors | Nothing is written yet (`DESK.INI` is R115) | Write `.NEW`, read it back and compare, delete, rename; reading recovers from `.NEW`. INT 24h is answered "Fail" by a 3-byte real-mode handler on the clean-up list (`docs/PLATFORM.md#safe-writes-and-int-24h`) | **Fizz 2** |
| Command line | Modes in `shell/dos/src/main.rs` | One table drives both the parser and `/?`; the help text is a golden file; an unknown switch exits 2 (`crates/fizz-machine/src/switches.rs`) | **Fizz 2** |
| Determinism and testing | Golden frames; hosted and native frames match; QEMU, Bochs and 86Box for native; DOSBox `cpu_cycles = max` for hosted | A golden replay transcript compared across native, WASM and DOS; DOS frame digests compared with the host (`BENCH FRAME`); a `memsize` ladder from 1 to 16; six sound-card configurations each checked for write counts; check-screen cells read back from B8000h | **Both strong, in different ways.** Desk covers more emulators; Fizz 2 checks more inside DOSBox |
| Performance measurement | None hosted (the integration analysis lists measurements as a next step) | `/BENCH`: a PIT-latch stopwatch with its reading cost subtracted, a `dec/jnz` calibration loop asserted within 10% of the configured cycles, `DOS_CYCLES` for a fixed rate, and a guard: its `dosrun.sh` exits 125 on any `Invalid '` DOSBox setting | **Fizz 2.** It also caught a real trap: `fixed 26800` is silently rejected by DOSBox Staging 0.83, which then runs at 3,000. Desk's `tools/dosrun.sh` only uses `cpu_cycles = max` and has no such guard |
| CI | One job on the pinned `ci-dos` image; installs QEMU and NASM with `apt`, which has failed on a network error | `check` and `parity` jobs, `ci-scope.sh` path-scoped DOS checks on pull requests, a nightly full gate, cancel-in-progress | **Fizz 2** for structure. Desk's `apt` step is its flakiest |
| Size | `SHELL.EXE` 107,520 bytes | `FIZZ2.EXE` 391,680 bytes; a budget of 1,100,000 checked by a `SIZE` line | Fizz 2 tracks a budget; Desk does not |
| Licences of vendored tools | `vendor/` is git-ignored; tools come from the image | The same, with an explicit note that only the DJGPP stub and CWSDPMI are used at run time and are data (`PLATFORM.md`, Toolchain) | **Same.** Neither ships a DJGPP or CWSDPMI licence text beside the executable that was found. Unverified |

## Learnings for Desk

In order of value. Each has a roadmap item.

1. **Replace `memset`, `memcpy` and the MMIO byte copy with string-instruction versions in `rt`** (R132). Fizz 2 measured a present about six times faster and drawing about 40% faster from one file, `dos/rt/src/mem.rs`, without touching its `forbid(unsafe_code)` crates. Desk already holds the hypothesis in [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md) (`rep stosd` "would be the same traffic with fewer" instructions) and in the optimisation table of the integration analysis ("Improve linear copies"); Fizz 2 is the measurement that table asks for. For Desk it means a `mem.rs` in the promoted `rt` (today it exists "for the stage 1 build only", `rt/src/lib.rs`), a `write_bytes` that uses a `rep movsd` path for aligned runs, and the native kernel too. The gain in DOSBox is in counted instructions; ISA bus cost is unmeasured on both sides.
2. **Fix R127 the Fizz 2 way** (R133). Collect the transcript in a fixed buffer, print nothing while in graphics mode, and flush after the clean-up list has restored text mode. It is a small change, already proved, and it keeps the checks that read the `shell: mark …` lines working.
3. **Build a real hosted timer from Fizz 2's design** (R134). Reprogram the PIT, keep the BIOS clock right with an accumulator, use an integer `Pacer` with a catch-up cap, restore the divisor on exit, and decide per interrupt whether to chain. It needs one change to Desk's HAL: `Chain` decided per interrupt (an `Action` return), not at registration, which contradicts the shape in `hal/hal/src/irq.rs`. Keep Fizz 2's rule that `delay_us` refuses to run while the PIT is reprogrammed.
4. **Sound: port Fizz 2's audio service into Desk's driver model rather than writing one fresh** (R135). Port `audio.rs`, the OPL and speaker steppers in `fizz-sfx`, the device resolver and OPL3 router in `fizz-machine`, and the probes in `probe.rs`, as `hal/drivers/opl`, `hal/drivers/sb` and `hal/drivers/speaker` over `PortIo`, with mock tests. The lessons come with it: interrupt-time register writes with status-read settling (6 and 35 reads), channel reservation (5 and 14 for effects), putting `0x105 = 0` back on exit or an OPL3 stays in OPL3 mode, bounding every device wait, and never calling INT 33h through a null vector. It has no DMA or PCM, so WAV and VOC playback (R116) still needs new work, and "verified" there means write counts in DOSBox, not sound. The same probes feed R121 and R115.
5. **Close the framebuffer-lifetime gap** (R136). Fizz 2's `Display::present(pixels, pitch)` holds no video pointer in the HAL, which closes section 3 of the integration analysis by construction. Desk cannot take a whole frame per present (damage spans, planar modes), but a presenter could be a session that owns the mapping and takes `present_span(y, x, &[u8])`, so no `MmioRegion` escapes. That touches `hal/hal/src/video.rs`, `shell/present/`, R53 and R58, and has to land before apps can switch modes (R79). It is a design decision, not a copy.
6. **Take the measurement discipline before any optimisation** (R137). Use plain-number `cpu_cycles`, the `Invalid '` guard, the calibration loop asserted within 10%, the PIT-latch stopwatch with its reading cost subtracted, and frame digests taken outside timed sections (Fizz 2's `tools/dosrun.sh` and `docs/PLATFORM.md#how-bench-measures`). Step 4 of the integration analysis ("Measure representative workloads") has no tooling yet. Fizz 2's first round of figures was taken at the wrong speed without anyone noticing, which is the warning that comes with the tooling.
7. **If apps can use floats, decide the x87 contract** (R138). Fizz 2 sets the control word to `0x027F` before app code runs, keeps interrupt handlers off the x87, pins a software `libm` without the `exp` family on non-SSE targets, and snaps subnormals so hashes match SSE2. Desk's `rt/src/fpu.rs` has `set_double_precision`, but nothing makes it policy. This affects R77 and R79 (who owns the x87 state, and whether the native scheduler saves it per task) and [`SDK.md`](SDK.md).
8. **Safe settings writes and an INT 24h guard before `DESK.INI` (R115) and Files (R112).** Write `.NEW`, read it back, delete, rename, recover on read, and install a "Fail" critical-error handler on the clean-up list. Without it a write-protected floppy prints "Abort, Retry, Fail?" over the desktop. This is folded into the merge of runtime work (R139) rather than given its own item.
9. **Evidence on the repository breakup (R82).** The first external consumer forked the runtime within days, and two copies of `cleanup.rs`, `dpmi.rs`, `dos.rs`, `heap.rs` and the packer now diverge. Fizz 2 cannot swap back to Desk's runtime until it is promoted (R53) and stable, and Fizz 2's work (timer, audio, probes, `mem.rs`, `text.rs`) should be merged into it so that any later swap goes the right way (R139, R140). R52 ("Fizz Episode 2 as a native Rust DOS game") is in effect done outside Desk.
10. **Smaller ones worth copying.** A switch table that drives both the parser and the help text, with a golden `/?`. The check screen read back from B8000h as a golden. A `memsize` ladder in CI to back up the 8 MB claim in [`SPEC.md`](SPEC.md) with a floor measurement. A `SIZE` and budget line. Path-scoped pull request checks plus a nightly full gate. The `InputLatch` press-edge model for any timed app. And, for R48, the compatibility number and the stem-matches-file-name check from `docs/FAP.md`.

The sound and timer code is Fizz 2's own and the same author's, so reuse needs a `NOTICE` entry but no new licence analysis; see [What is unverified](#what-is-unverified) for what was and was not determined.

## What Fizz 2 could take back from Desk

These are for the owner's later call; nothing in Fizz 2 changes unasked.

- **Shared drivers with mock tests** (`hal/drivers/*`, `hal/platform/mock`). Fizz 2's OPL, speaker, PIT and probe register code is tested only in DOSBox.
- **The integration analysis findings apply word for word to Fizz 2's copies.** Section 2: `vga.rs`, `text.rs` and the INT 24h guard ignore a full clean-up list, and slot reuse breaks last-in-first-out order. Section 6: the permissive ELF parser. R54: no recursion guard in `die`, unchecked heap sums.
- **Spurious IRQ 7 and 15 handling**, which Fizz 2 dropped in its `irq.rs` rewrite.
- **More than one emulator.** Fizz 2's largest stated gap is the x87 cycle cost, which DOSBox charges like any other instruction. Desk already runs 86Box, Bochs and QEMU ([`NATIVE-EMULATORS.md`](NATIVE-EMULATORS.md)); a FreeDOS image on 86Box with a 486 model is the cheapest way to a better x87 and ISA-VGA figure for `/BENCH`, and it would test the DPMI and INT 24h paths Fizz 2 lists as unverified.
- **VBE and DPMI 0800h mapping** (`rt/src/hal.rs`, `dpmi::map_physical`), if Fizz 2 ever wants 640×480 or a host other than CWSDPMI. The near-pointer A0000h trick both use is a CWSDPMI assumption.

## Where Fizz 2 contradicts a Desk document

- [`HAL.md`](HAL.md)'s IRQ shape: `Chain` fixed at registration is not enough for a reprogrammed PIT.
- [`DIRECTION.md`](DIRECTION.md) and the breakup proposal assume one shared runtime; there are two.
- R81 implies Fizz needs Desk to build sound and a timer first; Fizz 2 built both.
- The integration analysis's remark that the Rust-only runtime "works in its own setting" understates it: a 390 KB game with sound, files and probes runs on it.

## Open questions

- **Does a `rep movsd` into video memory meet the `MmioRegion` volatile contract?** Desk's contract is volatile byte stores; whether a string move meets it on every card Desk targets is not established (R132).
- **The float policy** (R138): will Desk's SDK promise 53-bit precision, and will the native scheduler save x87 state per task? This is the owner's decision.
- **When Fizz 2 adopts Desk's runtime** (R140): the owner's call, after Desk's runtime has absorbed the work above and been promoted.
- **R52 and R81:** whether they are marked as satisfied outside Desk by Fizz 2, or re-scoped as "upstream Fizz 2's timer and sound into `hal/`". The roadmap keeps both as they were and puts the re-scoping in R135 and R134.

## What is unverified

- Whether either side ships the DJGPP stub and CWSDPMI licence texts beside its executable.
- `DESK.EXE`'s `file` output (it was not built).
- Anything about real hardware: both projects are emulator-only.
- Fizz 2's licence terms: its `NOTICE` and `Cargo.toml` state Apache-2.0 OR MIT, the same as Desk's, with the same copyright holder. Its sound-effect converter is adapted from `lieutenant-fizz-dos` (MIT, the same author) and relicensed; its `crates/fizz-phys` derives from Lieutenant Fizz 1's `lf-sim` crate (Apache-2.0 OR MIT); the `libm` crate 0.2.16 (MIT) is linked into the game. This reading is from the files named; it is not legal advice and the upstream licences of those sources were not themselves checked. Porting code into Desk needs `NOTICE` entries for anything derived from `lieutenant-fizz-dos`.
- Fizz 2's current `/BENCH` against the tables in its `PLATFORM.md`: the 2026-11-10 snapshot shows drawing at 9,888 µs on average (the slowest sky frame at 16,155 µs, close to the 16,667 µs budget), against the documented 2,728. That is Fizz 2's own concern, but it is a reminder that DOSBox-only numbers drift fast. The measured figures quoted in this document (3,589 to 599 µs, 19,035 to 11,434 µs) are Fizz 2's documented figures and were not reproduced.
- Why Fizz 2's `fizz-2-wt` directory exists: it is an empty, non-git directory.
- Most Desk file and line references were rechecked against `b928309` (`hal/hal/src/mmio.rs`, `hal/hal/src/irq.rs`, `rt/src/lib.rs`, `shell/dos/src/main.rs`, `tools/dosrun.sh`). Not rechecked: the spurious-IRQ handling in `rt/src/irq.rs`, the clean-up callers that ignore `false`, the bounds of the retrace wait, the exact wording of the `D-NATIVE-VGA.md` quotation, and the CI row's report that the `apt` step failed reaching `archive.ubuntu.com`.
