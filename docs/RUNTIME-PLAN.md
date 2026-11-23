# Runtime reunification: the build plan for R139

The plan for making `dos/rt` the union of R132 to R136, Fizz 2's probes, `text.rs`, the press-edge `InputLatch` and safe settings writes with an INT 24h guard (R139). The owner decided on 2026-11-11 that Desk's runtime is canonical and Fizz 2's improvements come back into it ([`FIZZ2-COMPARISON.md`](FIZZ2-COMPARISON.md), a read of two code bases and not a run). The promotion itself is done (R53, R148: the runtime is `dos/rt`, and R54's fixes are merged). What is left is the union.

As of 2026-11-23 T1 to T6 and T12 are built and merged (see the order table); T7 to T10, the sound drivers and the documents are not. The plan was drafted by reading Desk's code and Fizz 2's runtime (read only; its head was newer than the commit the comparison read), and it has not been reviewed by a second reader yet. Every claim about Fizz 2 is from that read, not from a run.

## Contents

- [Decisions](#decisions)
- [Rules for every task](#rules-for-every-task)
- [Order and parallelism](#order-and-parallelism)
- [The tasks](#the-tasks)
- [What could not be verified](#what-could-not-be-verified)

## Decisions

| Id | Question | Status |
|---|---|---|
| D1 | May Fizz 2's code be copied into Desk? The `AGENTS.md` rule makes Fizz 1 a reference only, and Fizz 2's probe and OPL design derive from it | **Decided 2026-11-22: no. Rewrite everything from the design.** Builders work from this plan and Desk's own code and do not read or copy Fizz 2's source. Where a task below says "from Fizz 2", it means the behaviour described here, written fresh, with its own tests |
| D2 | Can `MmioRegion::write_bytes` promise frame-buffer semantics, or should the fast copy be a new method? | **Decided 2026-11-22, the owner leaving the choice to the orchestrator: a new method, `copy_from`, for frame buffers only.** `write_bytes` stays byte-exact for register windows where the width of an access matters |
| D3 | The shape of the per-interrupt chain API (T4) | **Decided 2026-11-22: an `Action` return carried through `IrqHandler`.** Existing services keep `on_irq(&mut self)` and their trampolines return the `Chain` given at registration; a new `DecidingIrqService` returns an `Action` |
| D4 | Hosted timer rates, and whether the shell switches to them (T6) | **Decided 2026-11-22: `rt` offers 60 and 560 Hz as an opt-in, and the shell stays at 18.2 Hz**, as HAL.md says |
| D5 | The probe's x87 side effect (T8) | Open. Proposed: detect the x87 and restore the control word it found, which keeps the float policy (R138) open |
| D6 | Where `InputLatch` lives and who uses it first (T10) | Open. Proposed: `desk-hal::input`, over a `u32` mask, with host tests only |
| D7 | R136: which finished-frame present API | Open. The design note is `PRESENT-API.md` (merged, #105) |
| D8 | Is sound (R135) part of R139 being done? | Open. Proposed: no; the OPL, speaker and Sound Blaster drivers come after, with their own plan |

## Rules for every task

- A worktree off `origin/main`, the CI image for verification, an independent review before the PR, as in [`LOADER-PLAN.md`](LOADER-PLAN.md).
- Licence headers and Canadian English on every new file. No `#[unsafe(no_mangle)]` or other edition-2024 forms.
- The house rules for `dos/rt` and `hal`: no compare-and-swap (`hal/clippy.toml`); no floating point in `hal`, in drivers or in interrupt handlers; a one-line `SAFETY:` comment on every `unsafe`; `dos` names only `hal` (checked by `tools/layers`, see [`LAYERS.md`](LAYERS.md)).
- Pinned outputs stay the same: the shell goldens, `tests/native/expected*.txt` and the existing lines of `dos/expected/*.txt`. A task may add lines to a transcript, and must never change one.
- Every `cleanup::register` result is checked, and the resource is rolled back if registration fails.
- Every task that changes `dos/rt/src` updates the line and file count pinned in `tools/progress-report/tests/test_chapter_rt_toolchain.py`, so it also selects the `report` group.
- Groups to run: `rust`, `dos`, `shelldos` and `report`; add `native` and `compare` whenever `hal/hal` changes.
- Not to be carried over from Fizz 2 as it stands, because it breaks Desk's rules: port I/O marked `nomem` (breaks the `PortIo` ordering contract in [`HAL.md`](HAL.md)); an interrupt layer that drops spurious IRQ 7 and 15 handling; keyboard, ring-buffer and VGA code that duplicates `desk-i8042`, `SpscRing` and `desk-vga`; probes that write OPL and DSP registers through free functions (Desk puts drivers over `PortIo` with mock tests); an x87 probe that forces control word 027Fh (which would settle R138 by accident); and a non-volatile copy into the frame buffer (its measured gain comes from that).

## Order and parallelism

At most two code tasks run at once.

| Wave | Tasks | Depends on |
|---|---|---|
| 0 | R54 merged (done, #102) | none |
| 1 | T1 clean-up list (merged, #108) ∥ T5 accumulator and `Pacer` (merged, #107) ∥ T12 design note (merged, #105) | none |
| 2 | T2 `memset` and `memcpy` (merged, #111) ∥ T4 per-interrupt chain (merged, #110) | T1; D3 |
| 3 | T3 fast copy into device memory (merged, #112) ∥ T6 fast timer (merged, #113) | T2; T4, T5; D4 |
| 4 | T7 safe writes and INT 24h ∥ T8 probes (not started) | T1; D5 |
| 5 | T9 `text.rs` ∥ T10 `InputLatch` (not started) | T1, T8; D6 |
| 6 | T13 documents and the Manual (Gregory) | all of the above |
| later | S1 OPL driver ∥ S2 speaker and Sound Blaster drivers | T6; D8 |

T2 and T3 are in different waves on purpose, so that any change in a golden can be traced to one of them.

## The tasks

### T1. A clean-up list that keeps its order and refuses when full

Labels: `bug`, `dos`, `rust`. The shell already uses 6 of the clean-up list's 8 slots, and the fast timer and the INT 24h guard would bring it to 8. Worse, re-registering reuses a slot, which breaks last-in-first-out order: `DosDisplay::set_mode` forgets and re-registers `restore`, and if that lands below `flush_transcript` the transcript would print while graphics are still on ([`RUST-DOS-INTEGRATION-ANALYSIS.md`](RUST-DOS-INTEGRATION-ANALYSIS.md)).

- **Goal:** strict last-in-first-out order, so a re-registration moves to the top; 16 slots, compacted on `forget`; fix the callers in `rt` that ignore a full list (`vga::enter_mode13`, `DosDisplay::set_mode`, `timer::hook`).
- **Files:** `dos/rt/src/{cleanup,vga,hal,timer}.rs`; a new example `cleanup` in `dos/examples` with `dos/expected/cleanup.txt`; `dos/build.sh` and `dos/check.sh`.
- **Verify:** the example registers a, b and c, forgets a and registers a again, and the transcript must show the order a, c, b; a 17th registration is refused; `HW.EXE leave-hooked` and the shell-on-DOS run are unchanged. Negative check: a transcript with the order swapped must fail.
- **Size and risk:** about 150 lines. Changing the order could expose a hidden dependency in the spikes, so run the `dos` group in full.

### T2. String-instruction `memset` and `memcpy` (R132, part one)

Labels: `enhancement`, `performance`, `dos`.

- **Goal:** a strong `memset` (`rep stosd`) and `memcpy` (`rep movsd`) for the ELF build: head bytes up to the next 4-byte boundary, whole words, then the tail, with a short-run cut-off. `memmove` and `memcmp` stay in the stage-1 build only.
- **Files:** `dos/rt/src/mem.rs` and `lib.rs`.
- **Verify:** a `HELLO.EXE mem` mode compares both functions with a byte loop for offsets 0 to 7 and lengths 0 to 40 and prints one line, appended to `hello.txt`; the shell goldens and the a-vga-vbe and b-dos-input transcripts stay byte-identical; if `llvm-objdump` is in the image, check that `memcpy` in the linked `hello` contains `rep movsd`, so the strong symbol is the one that linked. Negative check: a wrong expected line must fail.
- **Limits:** no speed figure is claimed until R137 provides the measuring tools.

### T3. A fast copy into device memory (R132, part two; D2 decided)

Labels: `enhancement`, `performance`, `graphics`.

- **Goal:** a new `MmioRegion::copy_from`, for frame buffers only: the head, whole-word and tail split in pure Rust, then an `asm!` `rep movsd` for the word part on `target_arch = "x86"`, with the volatile loop on every other target. `write_bytes` is unchanged. Optionally `fill8` gets `rep stosd`.
- **Files:** `hal/hal/src/mmio.rs` (and `x86.rs` if the assembly goes there); the callers in `hal/drivers/vga/src/linear.rs` and `shell/present/src/{linear,planar}.rs`; the HAL.md "Performance" bullet.
- **The volatile question, answered for the new method:** at the compiler level an `asm!` block without `pure` or `nomem` is never removed or merged, which is at least as strong as volatile; at the bus level `rep movsd` makes aligned 32-bit stores in ascending order and reads no device memory, the same traffic `fill8` and `write32` already make. It is not safe for register windows where the width of an access matters, which is why it is a separate method. One assumption to check: `ES` equals `DS` under CWSDPMI and in the kernel (the runtime's interrupt stub already sets `ES`).
- **Verify:** host tests over a grid of offsets and lengths, copied in shape from `fill8`'s; cross builds for `dos-elf` and `i486-desk-none`; goldens unchanged in the shell-on-DOS run (depths 8 and 4), the a-vga-vbe spike and `native` and `compare`. Negative check: an out-of-bounds copy still returns `InvalidArgument`.
- **Risk:** this changes the native track too, and is not safe to assume on real ISA VGA cards.

### T4. A per-interrupt chain decision (R134, part one; D3)

Labels: `enhancement`, `dos`, `rust`.

- **Goal:** let a handler decide on each interrupt whether to send the end-of-interrupt or pass it to the BIOS, which a reprogrammed PIT needs.
- **Proposed shape:** the `IrqHandler` function pointer returns `Action { Eoi, Chain }`; `IrqService` and `SharedIrqService` keep `on_irq(&mut self)` and their trampolines return the `Chain` given at registration, so no existing driver changes; a new `DecidingIrqService` returns an `Action`. On the native track `register` still refuses `Chain::Previous`, and a returned `Chain` is treated as `Eoi`.
- **Files:** `hal/hal/src/irq.rs`, `hal/platform/mock/src/irq.rs`, `kernel/src/hal.rs` (dispatch), `dos/rt/src/irq.rs` (the stub tests the value returned in `eax`), HAL.md under "Who sends the end-of-interrupt". Keep Desk's spurious IRQ handling and `IrqRegistry`.
- **Verify:** mock tests with a handler that alternates `Eoi` and `Chain`; existing driver tests unchanged; `HW.EXE` and `native` unchanged. Negative check: a handler that returns `Chain` on a line with no previous owner is refused or treated as `Eoi`, with a test.

### T5. The BIOS-clock accumulator and the `Pacer` (R134, part two)

Labels: `enhancement`, `rust`.

- **Goal:** two pure pieces with no hardware access. In `desk-pit`, a `BiosChain` accumulator: add the divisor on each tick and chain when the sum reaches 65,536. In `desk-hal::time`, an integer `Pacer` that turns ticks into fixed steps with a catch-up cap.
- **Files:** `hal/drivers/pit/src/lib.rs` and `hal/hal/src/time.rs`. About 250 lines.
- **Verify:** over N ticks the number of chained interrupts equals N times the divisor over 65,536, give or take one, at divisors 19,886 and 2,131 (and 0, 1, 32,768 and 65,535); the pacer's whole-step, capping, re-synchronising and wrap-around tests pass; no 64-bit division in interrupt code (a review point). Negative checks by mutating each key rule.

### T6. A reprogrammed PIT in `rt` (R134, part three; D4)

Labels: `enhancement`, `dos`.

- **Goal:** `rt::timer::hook_rate(divisor)` over the DOS IRQ adapter, `desk-pit` and `BiosChain`, plus `ticks()`, `clocks()` (the PIT-latch stopwatch), `delay_us(us) -> bool` (which refuses while the PIT is reprogrammed) and `bios_elapsed` with the midnight wrap. On unhook and from the clean-up list the vector goes back first and then the BIOS rate. Keep the d0 `hook()` so `HW.EXE`'s pinned lines stand.
- **Files:** `dos/rt/src/timer.rs`, `dos/examples/src/hw.rs`, `dos/expected/hw.txt` (new lines only), `dos/check.sh`.
- **Verify:** `HW.EXE fast` at 60 Hz for one BIOS second expects 59 to 61 ticks with the BIOS counter advancing by 18 to 19; after unhooking the BIOS counter advances 18 to 19 per second again; `delay_us` returns false while hooked; a panic and `fast-leave-hooked` both restore the rate. Negative check: a run with the accumulator disabled must fail the BIOS line. DOSBox timing is loose, so the ranges are wide.

### T7. Safe settings writes and an INT 24h guard

Labels: `enhancement`, `dos`.

- **Goal:** `dos::rename` (AH=56h); `FailCritical`, a 3-byte real-mode `mov al,3; iret` handler, installed only if the clean-up registration succeeds; `save_replacing(name, bytes)` and `read_recovering(name)`: write the new file, read it back and compare, delete the old, then rename. The temporary name is derived from the 8.3 stem (`DESK.INI` becomes `DESK.NEW`), not fixed.
- **Verify:** save then read back; a crash simulated after the delete is recovered from the `.NEW` file; the INT 24h vector is checked before, during and after; the stub called through `simulate_int(0x24)` returns AL=3. Not verified: a real critical error (a read-only DOSBox mount gives DOS error 5, which is not one), so it cannot be provoked headless.

### T8. Platform probes (D5)

Labels: `enhancement`, `dos`.

- **Goal:** `rt::probe`: video class through `DosBios`, processor family by the EFLAGS AC and ID bits (CPUID is never run), x87 present with the control word restored as found, free memory through DPMI 0500h, and the joystick through `RawPorts`. The OPL and Sound Blaster probes belong to S1 and S2, and the mouse to `desk-int33`.
- **Verify:** under DOSBox the transcript reads VGA, 486 and x87 present, with memory printed only as "in range"; `ctx.txt` is unchanged because the control word stays as found.

### T9. Text screen

Labels: `enhancement`, `dos`. `rt::text`: enter 80 by 25 text mode (mode 3, or mode 7 on a monochrome adapter chosen from T8's video class), hide the cursor, write and read cells through an `MmioRegion` over B8000h or B0000h, poll keys through INT 16h, with checked clean-up. Verified by a test screen written, read back and digested into a transcript.

### T10. `InputLatch` (D6)

Labels: `enhancement`, `input`. A press-edge latch over a `u32` mask with `set` and `frame`, so a tap shorter than one tick still counts as a press, in `hal/hal/src/input.rs`. Verified by host tests; an `AUTOTYPE` example is optional.

### T12. The design note for R136 (no code)

Labels: `documentation`, `design-docs`, `graphics`. `PRESENT-API.md` (merged, #105): the finished-frame present API against the framebuffer-lifetime gap, with its options and a recommendation, for the owner to decide before R79. The code comes afterwards and is planned separately.

### S1 and S2. Sound drivers (R135; D8, after T6)

S1: an OPL driver (register writes with the status-read settling, the timer-test probe, OPL3 detection, and putting register 0x105 back to 0). S2: Sound Blaster (DSP reset and version, `BLASTER` parsing) and PC speaker (PIT channel 2 and port 0x61). All over `PortIo`, tested against the mock bus, with no `unsafe`, verified by write counts only (nobody can listen to a headless run). The 560 Hz audio service and the effects and machine steppers get their own plan.

### T13. Documents and the Manual

Labels: `documentation`. Gregory brings the roadmap rows (R132, R134, R139 to done or in progress with PR numbers, R136 recording the decision), [`FIZZ2-COMPARISON.md`](FIZZ2-COMPARISON.md) (a status line), HAL.md (interrupts, time and memory-mapped regions), [`D0-RUST-DOS.md`](D0-RUST-DOS.md) (the runtime's new modules) and the Manual's runtime chapters up to date; the Manual's runtime chapter and chapter 18f still quote the older source size.

## What could not be verified

- Nothing was built or run, and none of Fizz 2's measurements were reproduced.
- Whether the strong `memcpy` beats `compiler_builtins` in Desk's link, and whether `rep movsd` is safe on real ISA VGA cards.
- The behaviour of a real INT 24h critical error, probe results on hardware, and any sound.
- The licences upstream of Fizz 1's `fizz-dos` (moot under D1, since nothing is copied).
