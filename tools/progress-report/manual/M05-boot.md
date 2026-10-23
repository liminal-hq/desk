---
id: m-boot
n: M5
part: II
title: The boot, end to end
summary: Power-on to boot complete, one transcript line at a time; every test mode of the kernel and the file that checks it; how a run ends and what its exit status means.
date: 2026-10-23
reviewed: 44f0265
watches: [kernel/src/main.rs, kernel/src/exit.rs, tests/native/, tools/native-run.sh, tools/native-check.py, tools/native-test.sh]
log: [boot, emulators]
anatomy: [desk:boot, desk:kernel, desk:emu]
---
## What it is

This chapter follows one boot of the native image from the moment the machine starts to the line `boot complete`, using the real transcript of a run as the thread. The [previous chapter](chapter:m-loader) explained the loader that runs first; here the loader has finished, the kernel is running, and every line it prints is a small proof that something works. The chapter is also the guide to how those proofs are made to count: the [expected transcripts](term:expected-transcript), the [test modes](term:test-mode) that make the kernel fail on purpose, and the way a run ends, so that a script can tell success from failure without a person watching.

On the map of the system ([M1](chapter:m-map)) the chapter covers three boxes of the native track: the loader (`boot`, which wrote the first lines), the kernel (`kernel`, which wrote the rest) and the emulator that runs both (`emu`). The kernel's parts are each taught in their own chapters ([M6](chapter:m-interrupts) for the tables, interrupts and timer, [M7](chapter:m-memory) for memory and the x87, [M8](chapter:m-bioscalls) for calling the BIOS) and this chapter says only what each line shows and where it is printed. After it you should be able to read a boot transcript and say which part of the system wrote each line, know every test mode and the file that checks it, add a mode of your own, and read an exit status.

## How it works

### Everything before the first serial line

The BIOS runs its self-test, finds the floppy and loads the boot sector, which prints `Desk boot` and a dot for each read of the floppy. Those are on the screen only; the serial port does not exist yet. A headless test records nothing of stage 1. The transcript begins when stage 2 starts the serial port ([M4](chapter:m-loader)), with a blank line, because stage 2's first message begins with a line break.

### The loader's lines

| Transcript line | Written by | What it shows |
|---|---|---|
| `stage2: loader running` | `boot/stage2.asm`, start of `entry` | Stage 1 read stage 2 and jumped to it, and the serial port works |
| `stage2: a20 already enabled` | the [A20](term:a20) step | The gate was open before the loader touched it, as QEMU leaves it (the other two lines say `by the fast gate` and `by the keyboard controller`) |
| `stage2: memory map from E820, entries: 6` | the memory map step | The first BIOS call worked, and returned six entries |
| `stage2: font: 8x16 copied from the BIOS` | `load_font` | The kernel will be able to restore text mode |
| `stage2: vbe: not requested, the screen stays in text mode` | `video_setup` | The image's video byte is 0; a graphics image prints the mode it chose here instead |
| `stage2: entering protected mode` | just before `lgdt` | The last [real-mode](term:real-mode) line; interrupts are about to go off, and the switch to [protected mode](term:protected-mode) follows |

### From `_start` to `kmain`

Stage 2 jumps to the first byte of the kernel at `0x100000`, in 32-bit protected mode, with interrupts off, flat segments, and the address of the boot information block in `EAX`. The first byte is `_start`, a few instructions of assembly in `kernel/src/main.rs`:

1. `cli` and `cld`: interrupts stay off, and string instructions count up.
2. Park `EAX` in `EBX`, because the next loop needs `EAX`.
3. Point `ESP` at the top of a 64 KiB stack the kernel reserved in its own `.bss`. The loader's stack at `0x7000` is not used again.
4. Clear [`.bss`](term:bss), from `__bss_start` to `__bss_end`, with `rep stosd`. The flat image on the floppy contains no `.bss` (it holds only zeros, so it is not stored), but Rust code assumes its statics start at zero.
5. Push the saved address as the argument and call `kmain`. If `kmain` ever returned (it is declared never to), the code that follows halts.

[`kmain`](sym:kernel/src/main.rs::kmain) then runs the whole bring-up in a fixed order, printing one line for each step. Everything it prints goes both to the serial port and to the VGA text screen. The order matters, because each step needs the one before it: the console first (nothing can be reported without it), then the tables that make faults and interrupts survivable, then the timer, then memory, because the heap needs frames and the demonstrations need the heap.

### The kernel's lines

| Transcript line | Written by | What it shows |
|---|---|---|
| `Desk native kernel 0.1.0 (i486)` | `kmain`, after `serial::init` and `console::init` | The kernel is running and the serial port and screen both work; the number is the crate's version |
| `boot: drive 0x00, memory map from E820 (6 entries)` | `kmain`, from the boot information block | The magic number `DESK` matched (a wrong one is a panic), and the kernel read the loader's fields: drive 0 and a map of six entries from E820 |
| `gdt: loaded, flat 4 GiB code and data segments` | `gdt::init` | The kernel's own descriptor table replaced the loader's ([M6](chapter:m-interrupts)) |
| `idt: 32 exception gates and the timer gate installed` | `idt::init` | A fault now reaches a handler instead of resetting the machine |
| `pic: remapped to 0x20 and 0x28, only IRQ0 unmasked` | `pic::init` | Hardware interrupts no longer collide with CPU exceptions, and only the timer's line is open |
| `pit: channel 0 at 100 Hz` | `pit::init(100)` | The timer chip is programmed for a tick every 10 ms |

Then memory ([M7](chapter:m-memory)). `memory::init` builds the frame bitmap from the map, and `memory::report` prints the map and three totals. In the recorded run the map is:

```text
  map 0x00000000-0x0009fc00 usable
  map 0x0009fc00-0x000a0000 reserved
  map 0x000f0000-0x00100000 reserved
  map 0x00100000-0x00fe0000 usable
  map 0x00fe0000-0x01000000 reserved
  map 0xfffc0000-0x100000000 reserved
```

These are the six lines the expected transcript replaces with a lone `...`, because they depend on the BIOS; the checks that follow them do not.

| Transcript line | Written by | What it shows |
|---|---|---|
| `memory: usable RAM 15871 KiB` | `memory::report` | The sum of the usable entries as the BIOS gave them, 639 + 15,232 KiB (the worked example of [M4](chapter:m-loader)) |
| `memory: kernel image 0x100000-0x13c000` | `memory::report` | The kernel starts where the loader put it, and with its `.bss` and stack it ends 245,760 bytes later |
| `memory: 3748 frames of 4 KiB free after reservations` | `memory::report` | What is left after the first megabyte and the kernel are reserved |
| `frames: alloc and free of two frames ok` | `kmain` | Two frames were allocated, distinct and 4 KiB aligned, freed, and the count returned to what it was |
| `bios: real-mode trampoline at 0x10000, bounce buffer 0x11000 (8192 bytes), stack 0x13000` | `bios::init` | The kernel copied its real-mode trampoline over the staging copy of itself, which is no longer needed ([M8](chapter:m-bioscalls)) |
| `heap: 4096 KiB at 0x13c000` | `heap::init` | A 4 MiB heap was carved out just after the kernel |
| `video: boot info version 1` | `video::init` | The loader's block is new enough to carry the video fields |
| `video: 8x16 font, 4096 bytes at 0x2000` | `video::init` | The font the loader copied is where it said |
| `video: the loader did not try VBE (the screen is in text mode)` | `video::init` | The video block says state 0, "not tried" ([M12](chapter:m-graphics)) |
| `alloc: Vec sum 385, String "desk-native", format "desk-native:10:0x0181"` | `kmain` | The heap works: a `Vec` of the first ten squares sums to 385, a `String` grows, and `format!` builds a string |
| `heap: 4096 KiB free after the demo` | `kmain` | Everything allocated was freed, and the whole 4 MiB is free again |
| `timer: waited for 10 ticks with hlt, elapsed 10 ticks` | `timer_demo` | Interrupts are on, IRQ 0 arrives, and `hlt` returns on each tick; 10 ticks at 100 Hz is 100 ms |
| `fpu: control word 0x037f, sqrt(2) = 1.414214, H(10) = 2.928968` | `fpu::demo` | The x87 is enabled and its control word is the default, and a square root and a harmonic sum compute |
| `fpu: results check out` | `fpu::demo` | The results were compared with the expected values |
| `input: 8042 dual channel, translation on` | `input::init` | The keyboard controller answers, and has a second channel for a mouse |
| `input: keyboard ready, scancode set 1 translated by the controller` | `input::init` | The keyboard is in the mode the decoder expects ([M11](chapter:m-input)) |
| `input: mouse ready, id 3 (4-byte packets)` | `input::init` | A PS/2 mouse with a wheel was found |
| `input: IRQ 1 and IRQ 12 handlers registered, controller interrupts on` | `input::init` | Keys and mouse motion now arrive by interrupt |
| `boot complete` | `kmain`, then `exit::success` | Nothing failed; the kernel ends the run |

The timer deserves a note, because it is the one place a mistake would hang the boot instead of failing it. `timer_demo` enables interrupts and then waits for the first tick in a bounded spin, `while pit::ticks() == 0`, giving up with a panic after 200 million turns. Only then does it use `hlt`, which stops the processor until an interrupt, in a loop for 10 more ticks. If the timer were silent, `hlt` would wait for ever, and the run would end only by the watchdog described below; the spin turns that into a message.

The step-through below replays the same transcript beside the memory map as it fills. It is the same data as the tables above, and it refuses to build if the transcript stops matching `tests/native/expected.txt` or if the usable-RAM line is not the sum of the map.

<!-- explainer boot -->

The screen of the same run (the kernel writes each line to the VGA text screen as well as to the serial port) looks like this.

<!-- figure qemu -->

### The test mode byte

The byte patched into the image at stage 2 offset 3 travels in the boot information block to the kernel, which turns it into a `TestMode`. After the demonstrations, and before the input section, `run_test_mode` does what the mode says, and `input_section` runs the input and graphics modes. A normal boot is mode 0 and does neither. These are all the modes, in the order of the enum in `kernel/src/main.rs`; the second column is the name `tools/native-image.sh --test-mode` takes.

| No. | Name | `TestMode` | What the kernel does | Checked by | Run ends |
|---|---|---|---|---|---|
| 0 | `none` | `Normal` | Nothing extra | `expected.txt`, and the four loader variants | `0` |
| 1 | `div0` | `DivideError` | Divides by zero | `expected-div0.txt` | `1` |
| 2 | `gp` | `GeneralProtection` | Loads [selector](term:selector) `0x28`, beyond the [GDT](term:gdt) | `expected-gp.txt` | `1` |
| 3 | `ud` | `InvalidOpcode` | Executes `ud2` | `expected-ud.txt` | `1` |
| 4 | `panic` | `Panic` | Calls `panic!` | `expected-panic.txt` | `1` |
| 5 | `hang` | `Hang` | Stops with interrupts off | the watchdog; no expected file | `124` |
| 6 | `notick` | `NoTick` | Masks the timer before waiting for it | `expected-notick.txt` | `1` |
| 7 | `input` | `Input` | Waits for injected keys and mouse motion, until Escape | `expected-input.txt` and `input.inject` | `0` |
| 8 | `input-masked` | `InputMasked` | As `input`, with IRQ 1 left masked | `expected-input-masked.txt` | `1` |
| 9 | `input-idle` | `InputIdle` | Waits for input that never comes | the watchdog; `input-idle.inject`; no expected file | `124` |
| 10 | `gfx` | `Gfx` | The graphics tour | `expected-gfx-vbe.txt`, `expected-gfx-fallback.txt`, `expected-gfx-vbefail.txt` and screenshots | `0` |
| 11 | `bios` | `Bios` | Calls the BIOS from protected mode | `expected-bios.txt` and screenshots | `0` |
| 12 | `bios-solo` | `BiosSolo` | The BIOS scenarios without typed keys | no script in this repository runs it | |
| 13 | `shell` | `Shell` | The first window's scenario, frame by frame | `expected-shell-8.txt`, `expected-shell-4.txt` and screendumps | `0` |
| 14 | `shell-corrupt` | `ShellCorrupt` | `shell`, then one pixel of the screen changed | the screendump check must reject it | `0` |

The names, the numbers and the run lines are the ones in the code; the build checks the table against `kernel/src/main.rs`. Mode 12 is an honest gap: `docs/N3-BIOS-PATH.md` records that it was run on Bochs by hand, but nothing in `tools/` or the CI runs it.

### How a run ends

A kernel that has finished has nothing to return to, so it has to tell the outside world how it went. In QEMU it writes to a device made for the purpose, the [debug-exit port](term:debug-exit-port).

> **From the stacks — The exit that QEMU provides.** QEMU's `isa-debug-exit` device is implemented in `hw/misc/debugexit.c`. A write to its I/O port makes QEMU call `exit((val << 1) | 1)`, so the value the guest writes becomes the process's exit status, odd by construction and never zero. The source defaults the port to `0x501` with a size of 2; the Desk's run script puts it at `0xf4`. On a machine with no such device the write goes nowhere, which is why the kernel halts after it. The source says nothing about why the formula has the shift.
> Source: [hw/misc/debugexit.c at QEMU v9.0.0](https://qemu.googlesource.com/qemu/+/refs/tags/v9.0.0/hw/misc/debugexit.c)

`kernel/src/exit.rs` writes `0x10` to port `0xF4` for success and `0x11` for failure, then halts with interrupts off. QEMU exits with `(0x10 << 1) | 1 = 33` or `(0x11 << 1) | 1 = 35`, and `tools/native-run.sh` maps what QEMU did to a status of its own:

| `native-run.sh` exit | Meaning |
|---|---|
| 0 | The guest wrote success (QEMU exited with 33) |
| 1 | The guest wrote failure (35): a panic, an exception dump or a failed assertion |
| 2 | QEMU exited some other way, for example a reset or triple fault with `-no-reboot` |
| 124 | The [watchdog](term:watchdog) ended the run: the script runs QEMU under `timeout`, 20 seconds by default and 5 for the tests that expect to be killed |
| 3 | Neither QEMU nor Docker is available, or the image (or the injection script) does not exist |

Both loader stages use the same trick: `die` in stage 1 and stage 2 writes `0x11` before halting, so a boot that never reaches the kernel is a failure too. The watchdog is what turns "the kernel stopped and said nothing" into a result.

### What each negative check makes the kernel print

A test that can only pass proves little, so `tools/native-test.sh` also runs the kernel in ways that must fail, called [negative checks](term:negative-check), and demands the specific failure. The expected files for these start with a lone `...` (the normal boot lines, whatever they are), then the line the mode prints, then the dump or message that proves the failure was the intended one. Every deliberate failure is below, with the mode that causes it and what proves it.

<!-- explainer m-negatives -->

## Why it is this way

- **A transcript, not a debugger.** The test is a comparison of the kernel's serial output with an expected file, because that is what a headless run can produce and what a person can read. The expected files carry patterns for the parts that vary: `{N:lo..hi}` for a number in a range, `{HEX}` for an address, `{BITS:mask=value}` for a register whose other bits depend on the BIOS, `{ANY}`, and `...` for any number of lines (`docs/NATIVE-BOOT.md`, "The checks"; `tools/native-check.py`). The matcher is a search over the lines, so a pattern may skip lines only where it says so.
- **One kernel, many behaviours.** The test mode is a byte in the image, not a build flag, so a single kernel binary serves all fifteen modes and an image is made by patching one byte (`tools/native-image.sh`). A kernel that carries code to misbehave deliberately is a cost, accepted so that the negative checks run the real boot path.
- **Prove that the check can fail.** The repository's rule is that a bug fix needs a check that fails before the fix (`AGENTS.md`, "Testing"). The same idea is applied to the checker itself: `tests/native/wrong-expected.txt` must be rejected by `tools/native-check.py`.
- **Exit through a device.** Reporting success or failure by an exit status is what lets `tools/check.sh` and the CI treat a boot like any other test. A boot with no such device (real hardware, Bochs) ends by the kernel halting, and the harness decides by the transcript or the watchdog instead; the Bochs and 86Box runners return the same statuses ([M20](chapter:m-emulators)).
- **Bounded waits.** Every place the boot could wait for hardware that might not answer has a limit: stage 1's retries, the loader's A20 and keyboard-controller waits, the first-tick spin. A boot that cannot continue prints why and stops, instead of hanging until the watchdog.
- **A known flaw in a negative check.** `tests/native/wrong-expected.txt` is described in `docs/NATIVE-BOOT.md` as differing in one number. It is in fact an older copy of the transcript that lacks several lines the loader and kernel now print, so it is rejected at its fifth line, before it reaches the number. The checker still says no, and the worked example shows a version that differs in one number only. Refreshing the file is a change to a test, not to this chapter.

## The code

| Where | What it holds |
|---|---|
| [`kernel/src/main.rs`](repo:kernel/src/main.rs) | `_start` (the assembly stub), [`kmain`](sym:kernel/src/main.rs::kmain) (the order of the bring-up), [`TestMode`](sym:kernel/src/main.rs::TestMode), [`run_test_mode`](sym:kernel/src/main.rs::run_test_mode), [`timer_demo`](sym:kernel/src/main.rs::timer_demo), [`input_section`](sym:kernel/src/main.rs::input_section) and the panic handler |
| [`kernel/src/exit.rs`](repo:kernel/src/exit.rs) | [`success`](sym:kernel/src/exit.rs::success) and [`failure`](sym:kernel/src/exit.rs::failure): the write to port `0xF4` |
| [`kernel/src/console.rs`](repo:kernel/src/console.rs) | `println!` to the serial port and the screen |
| [`tools/native-run.sh`](repo:tools/native-run.sh) | Boots an image in QEMU as a 486 and maps its exit status |
| [`tools/native-check.py`](repo:tools/native-check.py) | Compares a transcript with an expected file and its pattern language |
| [`tools/native-test.sh`](repo:tools/native-test.sh) | The whole check: loader variants, the normal boot, every negative check and the input, graphics, shell and BIOS runs |
| [`tools/native-image.sh`](repo:tools/native-image.sh) | `--test-mode NAME` patches the byte |
| [`tests/native/expected.txt`](repo:tests/native/expected.txt) | The normal boot's expected transcript |
| [`tests/native/`](repo:tests/native/) | All the other expected transcripts, the injection scripts and `wrong-expected.txt` |
| [`tools/progress-report/media/native-serial.txt`](repo:tools/progress-report/media/native-serial.txt) | The recorded run that the step-through replays |

For depth: `docs/NATIVE-BOOT.md` ([boot flow](doc:docs/NATIVE-BOOT.md#boot-flow), [the checks](doc:docs/NATIVE-BOOT.md#the-checks), [running it and the CPU model](doc:docs/NATIVE-BOOT.md#running-it-and-the-cpu-model)).

## Worked example

**A divide error, from the instruction to the exit status.** In mode 1 the kernel prints `test: dividing by zero` and executes `div` with a zero divisor. The processor raises exception 0. The kernel's IDT sends it to a stub, which saves the registers and calls one Rust handler ([M6](chapter:m-interrupts)). The handler prints a dump and ends the run with the failure value. This is the transcript from a run on 2026-10-23, after the normal lines:

```text
test: dividing by zero

EXCEPTION 0x00: divide error
error code 0x00000000
EIP 0x0010d125  CS 0x0008  EFLAGS 0x00010246
EAX 0x00000001  EBX 0x00000500  ECX 0x00000000  EDX 0x00000000
ESI 0x00000001  EDI 0x0000000a  EBP 0x0013aff4  ESP 0x0013af04
CR0 0x00000033  CR2 0x00000000  CR3 0x00000000
halting with a failure status
```

Each register line tells a story. `EAX` is 1 and `ECX` 0, which are the dividend's low half and the divisor the test loaded. `CS` is `0x0008`, the kernel's code selector. `EIP` is inside the kernel, above `0x100000`. `EBX` is `0x00000500`, the boot information block's address still held from `_start`. [`CR0`](term:cr0) is `0x33`: protection enable (bit 0), monitor coprocessor (bit 1), extension type (bit 4, always set on a 486) and numeric error (bit 5), with the cache-disable bits clear. The expected file, `tests/native/expected-div0.txt`, does not pin the values that depend on the build: it matches `EIP {HEX}`, `EAX {HEX}`, and says `ECX 0x00000000  EDX 0x00000000` exactly, because those are the ones the test itself arranged; and it accepts the cache bits of CR0 either way with `{BITS:0x9fffffff=0x00000033}`, because a BIOS decides them.

The kernel then writes `0x11` to port `0xF4`. QEMU exits with 35, `native-run.sh` returns 1, and `native-test.sh` wants exactly that: exit 1 and a transcript that matches. Both conditions must hold.

**A wrong expectation that differs in one number.** The checker's refusal is easy to see by changing a single number of the real expected file and checking the real transcript against it:

```try
needs: host: bash, nasm, cargo, qemu-system-i386, sed
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-build.sh > /dev/null && tools/native-image.sh > /dev/null && tools/native-run.sh --serial dist/serial.txt > /dev/null; echo "exit $?"
native-run: guest reported success
exit 0
$ tr -d '\r' < dist/serial.txt > dist/serial.clean; python3 -I tools/native-check.py tests/native/expected.txt dist/serial.clean
transcript matches tests/native/expected.txt (38 lines)
$ sed 's/elapsed 10 ticks/elapsed 11 ticks/' tests/native/expected.txt > dist/wrong.txt; python3 -I tools/native-check.py dist/wrong.txt dist/serial.clean; echo "exit $?"
transcript does not match dist/wrong.txt
  expected line 26: timer: waited for 10 ticks with hlt, elapsed 11 ticks
  actual line   31: timer: waited for 10 ticks with hlt, elapsed 10 ticks
exit 1
```

The checker reports the first line it could not match, with both sides. The two line numbers differ because the lone `...` after the `pit:` line absorbed the six map lines in the transcript and the one line the expected file has for them. A pattern changes the answer: `{N:1..32}` would accept any number from one to 32.

## State of this layer

```status
id: boot.kmain
label: kmain's bring-up reaches boot complete, in order, on QEMU as a 486
state: built-verified
evidence: pr:6, test:tests/native/expected.txt::boot complete, sym:kernel/src/main.rs::kmain
note: Every line of the transcript is held to tests/native/expected.txt. The BIOS-dependent lines are patterns.

id: boot.checker
label: The transcript checker and its pattern language
state: built-verified
evidence: test:tools/native-test.sh::wrong-expected.txt, test:tools/native-check.py::compile_line, sym:tools/native-check.py::line_matches

id: boot.faults
label: A divide error, a general protection fault and an invalid opcode each end the run with a dump
state: built-verified
evidence: test:tests/native/expected-div0.txt::EXCEPTION 0x00: divide error, test:tests/native/expected-gp.txt::EXCEPTION 0x0d: general protection fault, test:tests/native/expected-ud.txt::EXCEPTION 0x06: invalid opcode

id: boot.panic
label: A panic and a masked timer each end the run with a message
state: built-verified
evidence: test:tests/native/expected-panic.txt::deliberate panic for the negative check, test:tests/native/expected-notick.txt::the timer did not tick

id: boot.watchdog
label: The watchdog ends a hung boot with exit 124
state: built-verified
evidence: test:tools/native-test.sh::hang, sym:tools/native-run.sh::timeout

id: boot.exit
label: Exit through isa-debug-exit, with statuses 33 and 35 mapped to 0 and 1
state: built-verified
evidence: sym:kernel/src/exit.rs::FAILURE, sym:tools/native-run.sh::guest reported success, test:tools/native-test.sh::run boot

id: boot.input
label: The input test modes: injected keys, a masked line and a silent line
state: built-verified
evidence: test:tests/native/expected-input-masked.txt::input: no Escape within 500 ticks, test:tools/native-test.sh::input-idle
note: Run on QEMU only, with the keys sent through its monitor.

id: boot.corrupt
label: A screen with one wrong pixel is rejected by the screendump check
state: built-verified
evidence: sym:kernel/src/main.rs::ShellCorrupt, test:tools/native-test.sh::shell-corrupt

id: boot.solo
label: The bios-solo test mode
state: built-unverified
evidence: sym:kernel/src/main.rs::BiosSolo, doc:docs/N3-BIOS-PATH.md#how-it-is-tested
note: Run on Bochs by hand and recorded in the document; no script in this repository runs it.

id: boot.wrong
label: wrong-expected.txt rejects a transcript for the reason its documentation gives
state: built-unverified
evidence: sym:tests/native/wrong-expected.txt::elapsed 11 ticks, doc:docs/NATIVE-BOOT.md#the-checks
note: The file is rejected, but at its fifth line, not at the number; see "Why it is this way".

id: boot.others
label: The same boot in Bochs and 86Box
state: built-unverified
evidence: pr:16, doc:docs/NATIVE-EMULATORS.md#results
note: Run by hand; CI does not run them. The chapter on the emulators has the table.

id: boot.hardware
label: A boot on a real machine
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified
```

The kernel's bring-up and its failures are {{state.boot.kmain}} in QEMU; the two other emulators are {{state.boot.others}}, and a real machine is {{state.boot.hardware}}. The step-through's transcript is a recorded run, the same as the one CI compares; it is not evidence about any other machine.

## Try it yourself

The build, the run and the checker are in the example above. These commands run each negative mode by hand and show the statuses this chapter describes. They need the same tools as the loader chapter.

```try
needs: host: bash, nasm, cargo, qemu-system-i386
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ tools/native-image.sh --test-mode gp --out dist/n-gp.img > /dev/null; tools/native-run.sh dist/n-gp.img 2>&1 | tr -d '\r' | sed -n '/^test:/,$p'
test: loading a segment selector beyond the GDT

EXCEPTION 0x0d: general protection fault
error code 0x00000028
EIP 0x0010d0dc  CS 0x0008  EFLAGS 0x00010212
EAX 0x00000028  EBX 0x00000500  ECX 0x0012066f  EDX 0x000b8000
ESI 0x00000002  EDI 0x0000000a  EBP 0x0013aff4  ESP 0x0013af04
CR0 0x00000033  CR2 0x00000000  CR3 0x00000000
halting with a failure status
native-run: guest reported failure
$ tools/native-image.sh --test-mode hang --out dist/n-hang.img > /dev/null; tools/native-run.sh --timeout 5 dist/n-hang.img > /dev/null 2>&1; echo "exit $?"
exit 124
$ tools/native-image.sh --test-mode notick --out dist/n-nt.img > /dev/null; tools/native-run.sh dist/n-nt.img 2>&1 | tr -d '\r' | tail -5
test: timer masked on purpose

KERNEL PANIC: panicked at src/main.rs:209:13:
the timer did not tick
native-run: guest reported failure
```

**Exercises.** (1) Without running it, say what `native-run.sh` returns for a kernel that panics in `timer_demo` and why the transcript's last line will not be `boot complete`; then run `--test-mode notick` and check. (2) Add a mode of your own in a scratch branch: a variant of `TestMode`, a name and number in the `case` of `tools/native-image.sh`, the behaviour in `run_test_mode`, an expected file, and a line in `tools/native-test.sh`; then read the negative-checks figure's rules and say which of them your change would trip if you forgot a step. (3) Change the usable-RAM range in `tests/native/expected.txt` so that it excludes 15,871 and say which line the checker reports.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Test mode
def: A byte patched into the floppy image that makes the kernel do something extra after its normal bring-up, usually to fail on purpose so that a check can prove it notices.

term: Debug-exit port
def: QEMU's isa-debug-exit device, which the kernel writes at I/O port 0xF4 to end the run. QEMU exits with the value shifted left once, with the low bit set.

term: Watchdog
def: The timeout the run script puts around QEMU, so that a kernel that stops without reporting still ends the run, with exit status 124.

term: Negative check
def: A test that must fail in the stated way: a kernel that misbehaves on purpose, or an expected transcript that is wrong, to prove the check is able to say no.

term: Expected transcript
def: A file of lines, with patterns for the parts that vary, that a boot's serial output must match line by line. The checker is tools/native-check.py.

term: bss
def: The part of a program's memory that starts as zeros and is not stored in the image. The kernel's start-up code clears it before any Rust runs.
```

```figures
id: m5-boot
reuse: boot-stepper
at: explainer boot

id: m5-qemu
reuse: qemu-boot
at: figure qemu

id: m5-negatives
kind: explainer
track: native
title: The negative checks
alt: A step-through of the deliberate failures of the native boot test, each with the test mode that causes it, the lines that prove it and the exit status the run must have.
caption: Nine failing modes and the wrong expected transcript, each with its proof, read from the kernel and the test script.
at: explainer m-negatives
thumb: steps
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The transcript walk covers every line of `media/native-serial.txt`, the mode table covers every `TestMode`, and the commands above were run on the host. Found while writing: `tests/native/wrong-expected.txt` does not differ "in one number" as `docs/NATIVE-BOOT.md` says (see "Why it is this way"), and no script runs the `bios-solo` mode.
- 2026-10-23: linked the processor terms that M2 now defines (A20, real and protected mode, selector, GDT, CR0) at their first natural use. No claim changed.
