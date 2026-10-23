---
id: m-input
n: M11
part: IV
title: Drivers II: keyboard and mouse
summary: The 8042 and its set-up sequences, scancode set 1 and the decoder, the PS/2 mouse packet, the event contract shared by both tracks, the lock-free rings, the kernel's input loop, the DOS side and the injection tests.
date: 2026-10-23
reviewed: 44f0265
watches: [hal/drivers/i8042/, hal/drivers/kbd/, hal/drivers/int33/, hal/hal/src/input.rs, hal/hal/src/ring.rs, kernel/src/input.rs, spikes/d0-rust-dos/rt/src/irq.rs, tests/native/, tools/native-inject.py]
log: [input, oneevent]
anatomy: [desk:drivers, desk:live, dos:drivers, dos:kbd, dos:mouse]
---
## What it is

This chapter is about the two ways a person talks to the Desk: a keyboard and a mouse. Everything the Desk's core knows about them arrives as one of two small values, a key event or a mouse event, and this chapter follows those values back to the wires. On the native track ([M5](chapter:m-boot) boots it) the kernel talks to the keyboard controller itself. On the hosted track, under DOS, the keyboard is read the same way but the mouse belongs to a driver that the program asks. The two tracks end in the same [input event](term:input-event) types and the same decoder, which is the point of the design, and the places where they part are the content of the second half of the chapter.

In the map of the system ([M1](chapter:m-map)) this is the input half of the HAL's drivers: the box [`drivers`](chapter:anatomydesk#box-desk-drivers) on the native anatomy page and [`drivers`](chapter:anatomydos#box-dos-drivers), [`kbd`](chapter:anatomydos#box-dos-kbd) and [`mouse`](chapter:anatomydos#box-dos-mouse) on the DOS page. It builds on [M3](chapter:m-devices), which introduced the [8042](term:8042), [scancodes](term:scancode) and the three-byte mouse report, and on [M9](chapter:m-hal), which defined the traits the drivers are written against. It does not repeat the hardware facts in M3; it shows how the code uses them. The box [`live`](chapter:anatomydesk#box-desk-live), live input into the shell, is the one place the manual stops short, and the state table says where.

After this chapter you should be able to read the kernel's `input:` lines and say which register write produced each, take a run of scancode bytes and decode it by hand into the key events and the transcript lines the kernel prints, explain why an interrupt handler may push into a [lock-free ring](term:lock-free-ring) without a lock, say what is shared between the native and the DOS input paths and what is not, and say which of the project's input claims have run and which have not. Nothing here has run on a real PC. The wires were only ever QEMU's, and on DOS DOSBox's.

## How it works

### The controller's set-up, in order

The driver crate is `desk-i8042`. Its first layer is the controller: two ports (`0x60` for data, `0x64` for the status register on a read and a command on a write), a handful of status bits, and a table of command bytes, all named constants in `hal/drivers/i8042/src/lib.rs`. Every exchange with the chip is a bounded wait on a status bit. `POLLS` is 100,000 status reads for an ordinary reply and `SLOW_POLLS` a million for the slow ones (a keyboard's self-test after a reset); when the count runs out the call returns `Error::Timeout` instead of hanging, which matters because a floating bus reads all ones and makes the input buffer look permanently full. Waiting for a *reply* also looks at status bit 5, the source bit: a byte from the other device is read and dropped, so a key pressed while the mouse is resetting cannot be taken for the mouse's acknowledgement.

The kernel calls three set-up functions in turn, and each prints a line that appears in the transcript.

1. **The controller (`I8042::init`).** Disable both ports (`0xAD`, `0xA7`); flush up to `FLUSH_LIMIT` (64) stray bytes; read the configuration byte (`0x20`), clear its two interrupt-enable bits, set the translate bit (`0x40`) and write it back (`0x60`); run the self-test (`0xAA`) and insist on the answer `0x55`; write the configuration byte again, because some controllers reset it during the test; enable the second port (`0xA8`) and read the configuration byte to see whether its clock-disable bit (bit 5) cleared, which is how a two-port controller is told from a one-port one, and disable it again; test the keyboard port (`0xAB`) and, if there is a second channel, the mouse port (`0xA9`), each answering `0x00`; enable both ports and flush. The function reports `dual_channel` and `translation`, and the kernel prints `input: 8042 dual channel, translation on`. The output port, which carries the A20 gate in bit 1 ([M2](chapter:m-cpu)), is never touched: `controller_init_never_touches_the_output_port_where_the_a20_gate_lives` asserts it.
2. **The keyboard (`init_keyboard`).** Stop scanning (`0xF5`), reset (`0xFF`), expect `0xFA` then `0xAA`, flush, and make sure the processor will see scancode set 1: if the controller's translation bit read back as set, nothing more is needed, and otherwise the driver asks the keyboard to switch to set 1 (`0xF0 0x01`) and checks by asking which set it is in (`0xF0 0x00`, accepting `1` or the translated `0x43`). Then start scanning (`0xF4`). A reply of `0xFE` means "send that again" and is retried up to `RESEND_LIMIT` (3) times; a first command that is never answered means no keyboard (`Error::NoDevice`). The line is `input: keyboard ready, scancode set 1 translated by the controller`.
3. **The mouse (`init_mouse`).** Every byte goes behind the `0xD4` prefix, which sends the next data byte to the second port. Reset, then read the self-test result and the device ID, set defaults (`0xF6`), then the wheel probe: set the sample rate (`0xF3`) to 200, 100 and 80 in turn and read the ID (`0xF2`); an ID of 3 means a wheel and four-byte reports. The sample rate goes back to 100 and streaming is turned on (`0xF4`). A failed probe leaves a plain three-byte mouse and is not an error. The line is `input: mouse ready, id 3 (4-byte packets)`.

Interrupts come last. Only after the handlers exist does `set_interrupts` read the configuration byte again and set bit 0 (IRQ 1) and bit 1 (IRQ 12), so no interrupt can reach a handler that is not there. The order is the one the OSDev recipes use, and `docs/C-INPUT.md` ("The set-up sequences") says plainly that those details were written from memory of the usual references and not re-checked against the originals.

> **From the stacks — The PS/2 port.** The PS/2 port was introduced in 1987 with IBM's Personal System/2 series of personal computers. Wikipedia describes it as the port used for connecting keyboards and mice to a PC compatible computer system.
> Source: [PS/2 port, Wikipedia](https://en.wikipedia.org/wiki/PS/2_port)

### Scancodes and the decoder

A keyboard speaks scancode set 2 after a reset; the controller translates that into set 1 on the way through when its translate bit is on, so the processor reads set 1, and set 1 is what the decoder understands. In set 1 a [make code and break code](term:make-code-and-break-code) are the same number: the make code is the key's number (`0x1E` is A), the break code is that number with bit 7 set (`0x9E`). Keys the 101-key layout added send the prefix `0xE0` first, so `0x4B` is keypad 4 and `E0 4B` is the left arrow. The decoder is `desk_kbd::Decoder`, and its whole interface is `feed(byte) -> Option<KeyEvent>`: it is a pure function over a small state machine (`Idle`, after `E0`, or part-way through the Pause sequence), with no ports and no interrupts, which is why 24 host tests can exercise it on a laptop.

The unusual cases are the reason a decoder is more than a table lookup.

- **Pause** sends the six bytes `E1 1D 45 E1 9D C5` on press and nothing on release, so the decoder walks a five-byte tail and reports one `pressed` event; a broken sequence is dropped and the offending byte read afresh.
- **The fake shifts.** When Shift or Num Lock is down, a keyboard wraps Print Screen, Insert and the cursor block in `E0 2A`, `E0 AA`, `E0 36` and `E0 B6`. These are not keys. The decoder swallows them so that they cannot release a real Shift: the test `fake_shifts_do_not_release_a_real_shift` holds Shift, types Insert and checks Shift is still down.
- **Protocol bytes** (`0x00`, `0xFA`, `0xFE`, `0xFF`: overrun, acknowledge, resend, error) carry no key. The handler queues `0xFF` in place of a byte the link flagged bad, which makes the decoder drop a half-received `E0` or Pause sequence.
- **Modifiers and locks.** The decoder keeps left and right Shift, Ctrl, Alt and Gui separately, so two Shift keys hold Shift until both are up, and it reports the modifier state *after* the event, which means that pressing Shift reports `SHIFT` set and releasing it reports it clear. Lock keys toggle on press only and ignore typematic repeats (a held key repeats its make code), so the decoder remembers whether the lock key itself is down.
- **Unknown codes** become `KeyCode::Unknown { extended, scancode }` rather than disappearing.

The second half of the crate is `char_for(code, modifiers)`, a small US key map: Shift selects the upper legend, Caps Lock flips letters only, the keypad types digits with Num Lock on and Shift off, and Control turns a letter into its control code (`Ctrl+A` is `0x01`).

### The mouse packet

Section after section of the hardware is in M3; here is what the code does with it. A [PS/2 mouse packet](term:ps2-mouse-packet) is three bytes, or four with a wheel. `MouseIrq::feed` takes the stream one byte at a time, because the interrupt cannot wait for the rest. While no packet is in progress, a byte whose bit 3 is clear cannot be a first byte, so it is dropped and counted in `mouse_resyncs`; that is how a stream that lost a byte finds its feet again. When the last byte arrives `decode` builds the event: it refuses the packet if either overflow bit (`0xC0`) is set, because the delta is then meaningless; it takes each delta as a nine-bit two's complement number (the byte minus 256 when the sign bit in byte 0 is set); it negates the vertical delta, since the device counts upward as positive and the screen counts downward; it takes the buttons from bits 0 to 2; and it takes the fourth byte as a signed wheel count, but only if a wheel was negotiated. A byte read with a parity or timeout flag abandons the packet in progress. None of this can be seen from QEMU, which never loses a byte; the resynchronisation is a host test only, and the chapter's state table says so.

### The event contract

The types live in `hal/hal/src/input.rs`, in the HAL crate, so that drivers and core share them without either depending on the other. A `KeyEvent` is `{ code, pressed, modifiers }`, where `KeyCode` names a physical key on a US layout, independent of any scancode set. A `MouseEvent` is `{ dx, dy, buttons, wheel }`: right is positive `dx`, down the screen is positive `dy`, `buttons` is bit 0 left, bit 1 right and bit 2 middle and is the state *now* rather than the changes, and `wheel` is positive towards the user. The trait the core drains is `InputSource` with `poll_key`, `poll_mouse` and `has_mouse`, which is true by default and false on a DOS machine with no mouse driver, so the core can run keyboard-only. The core never learns whether a byte came from a PS/2 port or a driver.

The document `docs/C-INPUT.md` asked for one change to this contract and made it: `KeyEvent` carries the modifier state, and `MouseEvent` gained `wheel`, because without it the wheel probe would negotiate four-byte packets and throw the fourth byte away.

### The hand-off through lock-free rings

The interrupt handlers do the least possible. `KbdIrq` (IRQ 1) reads port `0x60` and pushes the raw byte into an `SpscRing<u8, 64>`; `MouseIrq` (IRQ 12) assembles a packet and pushes one `MouseEvent` into an `SpscRing<MouseEvent, 32>`. Nothing is decoded in the handler. The decoding happens in `Ps2Input`, the core's end, which pops the keyboard ring through the decoder and pops the mouse ring directly.

A [lock-free ring](term:lock-free-ring) is a queue with exactly one writer and exactly one reader, where neither ever waits for the other. Here the buffer's size is a power of two, two indices count up forever, and only the producer writes `head` while only the consumer writes `tail`. The slot is the index masked with `N - 1`, and the length is `head - tail` even across the wrap of the counter, which `the_indices_wrap` tests by starting both at `usize::MAX`. Three rules make it safe in the places a kernel needs it:

- **No lock.** An interrupt that tried to take a lock the interrupted code already held would deadlock, so there is none. A full ring drops the new item and counts it (`dropped`); a handler must never wait.
- **No compare-and-swap.** The 486 has `CMPXCHG`, but DOSBox's `cputype = 486` rejects it (`docs/B-DOS-INPUT.md`), and the HAL's `hal/clippy.toml` bans it. On x86 an acquire load and a release store are plain `mov` instructions, and the orderings only stop the compiler from moving the slot access across the index update.
- **The ends are handed out once.** `split` returns the producer and consumer the first time and `None` after, so a second producer does not compile. The producer goes into a handler's state and the consumer stays with the core.

Two details keep the two interrupt lines from corrupting each other. Each handler reads at most `MAX_BYTES_PER_IRQ` (8) bytes per interrupt, a bound on its work, and each reads the status register's source bit first, so IRQ 1 never takes a mouse byte and IRQ 12 never takes a key. The design document's contract had said only "read a byte if status bit 0 is set", which can steal the other device's byte when both lines are pending; the handlers here check bit 5 as well, and `docs/C-INPUT.md` recommends the contract text say so. The platform, not the handler, sends the [end-of-interrupt](term:eoi) after the handler returns: the slave and then the master for IRQ 12.

### The kernel's side

`kernel/src/input.rs` is the native platform's whole share. `init` runs the three set-ups, prints their lines, splits the two static rings, boxes the two handlers (`Box::leak`, from the heap, allocated before interrupts are on), registers them for IRQ 1 and IRQ 12 with the [IRQ registry of M6](chapter:m-interrupts), flushes the controller once more and turns the controller's interrupts on. A missing device is reported and skipped, not fatal: a machine without a working 8042 still boots, without input. The flush before enabling matters: a byte left in the output buffer might raise no interrupt on a controller that already had it when interrupts were enabled, so it is dropped so that the next byte raises a fresh edge.

`run` is the loop the `input` [test mode](term:test-mode) uses. It drains the mouse ring and prints each event as `mouse: dx=10 dy=-5 buttons=0 wheel=0`, drains the key ring and prints each as `key: A down mods=- char='a'`, ends when Escape is released, and idles with `hlt` when both rings are empty. The idle step tests the rings with interrupts off and then runs `sti; hlt`: `sti` takes effect after the next instruction, so an interrupt that arrives in between wakes the `hlt` instead of being missed. The loop gives up with a panic if Escape does not come within a tick bound. In a normal boot only the set-up runs and prints its four `input:` lines before `boot complete` ([M5](chapter:m-boot)). The graphics tour of [M12](chapter:m-graphics) reuses the same `Ps2Input` for its cursor and typing.

### The DOS side

Under DOS the keyboard path is the same code with a different platform around it, and the mouse is a different device interface altogether.

**The keyboard.** The BIOS has already set the 8042 up, with both ports enabled and translation on, so the program does not touch the controller at all. What differs is how the handler is installed and who acknowledges the interrupt. The runtime's `rt::irq` (`spikes/d0-rust-dos/rt/src/irq.rs`) is a registry for all sixteen lines made of one small assembly stub per line, and the stub, which the DPMI host calls through the [protected-mode](term:protected-mode) vector, does this in order: save the registers it clobbers; load the flat data selector (read through `CS`, because `DS` cannot be trusted yet); switch to an 8 KiB interrupt stack inside the program image, saving the interrupted `SS:ESP` and restoring it with `lss`; clear the direction flag; and call `rt_irq_dispatch(line)`, which looks up the registered handler, runs it, and for a line the Desk owns sends the end-of-interrupt itself through `desk-pic`. A spurious IRQ 7 or 15 runs no handler and gets `eoi_spurious`. The stub returns with `iretd`, or for a chained line jumps to the previous handler with `jmp fword ptr`. Before the first vector changes, `register` locks the whole image with DPMI function 0600h, because a page fault in a handler is fatal; the handler's state therefore lives in a `Slot`, a `static` filled once, rather than in a `Box::leak`, which would sit in unlocked heap memory. The previous vector is saved, restored by `unregister`, and restored again by a clean-up function on the runtime's list that runs at exit and in the panic handler.

Replacing the BIOS handler (`Chain::None`), the default, has costs that `docs/B-DOS-INPUT.md` tabulates one by one and that matter to anyone who reads this chapter for the trade-off. Keys typed in the program never reach DOS's type-ahead buffer, which is wanted. The BIOS's shift and lock flags at `0040:0017` go stale, so `bios_kbd::locks` seeds the decoder (`Decoder::with_locks`) from them at start, and the flags are put back as found at exit. The keyboard lights stop following Caps Lock. Ctrl+Break and DOS's own break check never see a key. And Ctrl+Alt+Delete is lost, so a program that hangs with the handler installed cannot be rebooted from the keyboard. A `chain` mode (`Chain::Previous`) does the opposite and works in DOSBox. A full ring on DOS has one extra step: dropped bytes can lose a break code, which would leave a Shift stuck down, so `DosInput::poll_key` notices that the dropped count moved, waits for the ring to empty so that the hole is behind the decoder, resets the decoder's held keys (the locks stay) and counts a recovery.

**The mouse.** A DOS program does not talk to the PS/2 auxiliary port; it asks the mouse driver, whose software interrupt is INT 33h. `desk-int33` wraps the calls through the HAL's `Bios` trait (under DPMI a real-mode call, function 0300h, [M8](chapter:m-bioscalls) has the native analogue). `Mouse::detect` first reads the INT 33h vector from the real-mode table at `0000:00CC`; zero, or a vector that points at an `IRET` (`0xCF`), is the classic "no driver", and no call is made. Otherwise function 0000h resets the driver and reports the buttons, and function 0011h asks whether it has the CuteMouse wheel API (the answer `574Dh`, "WM"). `MouseInput::poll`, once per core iteration, asks for the motion since the last call (000Bh, in mickeys, signed, and the counters then reset) and the buttons now (0003h), and builds a `MouseEvent`. The units are the PS/2 counts one for one at default sensitivity; the signs are right and down positive. The driver's own position is never used: the core integrates the deltas itself, exactly as on the native track, so that the two behave the same at the screen's edges.

Polling gives the DOS mouse two behaviours the native path does not have. One event per poll carries the *sum* of the motion since the last call, not one event per packet, which a core that integrates deltas cannot tell apart. And a button pressed and released between two polls is not seen at all: under a virtual X server a quick `xdotool click` produced no event in DOSBox, while a press held for 0.3 seconds did. A human click lasts several frames, but the limit is real, and the known fixes are the driver's press and release counters (functions 0005h and 0006h) or a real-mode callback (000Ch), neither built.

The mouse path on DOS is tagged as the anatomy page tags it: [`mouse`](chapter:anatomydos#box-dos-mouse) is *unverified*, because the first-window run scripts the pointer as events and never moves a mouse. The adapter is covered by host tests against `Int33Model`, and was run against DOSBox's built-in driver under a virtual X server in an opt-in Docker check, but no real driver and no real mouse has been tried, and nothing in the checked-in first-window run reaches it.

<!-- explainer input -->

The figure above replays the recorded run of `tests/native/input.inject`: each injected command, the bytes in the ring and the decoded lines. It is built from the transcript and a replica of the decoder, and refuses a transcript the replica cannot reproduce.

<!-- explainer pipelines -->

The figure above steps one key and one mouse move down both pipelines. Where a stage is shared code it is marked, and the point is how few of the rows differ: the handler, the ring and the decoder are the same crates on both sides.

## Why it is this way

**One decoder and one handler for two tracks.** The hosted platform could have called the BIOS's `INT 16h`, and the native kernel could not, so the project could have grown two decoders. It did not. `docs/B-DOS-INPUT.md` states the constraint at the top: the core must see the same `KeyEvent` and `MouseEvent` types from the same decoder on both tracks. That is why `KbdIrq` is the handler on both and why `Decoder` has no port access.

**Handlers move bytes; the core decodes.** `docs/HAL.md` ("Concurrency") sets the rule for interrupt code: bounded work, no allocation, no lock. The keyboard handler follows it to the letter and only pushes a byte. The mouse handler cannot, because a packet has to be assembled as bytes arrive, so it does the minimum and pushes one finished event.

**A ring with no compare-and-swap.** The alternatives were a spin lock, which an interrupt could deadlock on, and an atomic counter built on `CMPXCHG`, which the 486 has and DOSBox's 486 mode refuses. Loads and stores alone, with a single writer for each index, work on every x86 and cost nothing.

**Polling the DOS mouse.** `docs/B-DOS-INPUT.md` ("Polling or a callback") chose polling over a real-mode callback because it is a function that is called: the same `MouseInput` runs against `Int33Model` on the host, and the callback would need a register-structure protocol, a locked handler reachable from real mode and a second path to test. The price is the lost quick click, written down.

**Replace, not chain, on DOS.** The BIOS handler is replaced because Desk wants the keys, and the costs are listed in the document rather than hidden. The decision for each row is in its table; the one that hurts is Ctrl+Alt+Delete, and a last-resort chain for that one combination is possible and unbuilt.

**Why the transcript formats differ.** The native kernel prints `key: A down mods=- char='a'`; the DOS program and its host mock print `key A down mods=none` (`desk_int33::transcript`). The comment in `transcript.rs` says the same functions format events on the host and under DOS "so one expected file can serve both", which is true of those two and not of the native kernel. The two formats were made at different times for different checks. The log chapter for the DOS run says so too: the shared decoder is compared by the host tests, not by one program running both.

## The code

| Where | What it holds |
|---|---|
| [`hal/drivers/i8042/src/lib.rs`](repo:hal/drivers/i8042/src/lib.rs) | The ports, [`status`](sym:hal/drivers/i8042/src/lib.rs::status), [`command`](sym:hal/drivers/i8042/src/lib.rs::command) and [`config`](sym:hal/drivers/i8042/src/lib.rs::config) constants, the bounded waits ([`POLLS`](sym:hal/drivers/i8042/src/lib.rs::POLLS), [`SLOW_POLLS`](sym:hal/drivers/i8042/src/lib.rs::SLOW_POLLS)) and [`I8042`](sym:hal/drivers/i8042/src/lib.rs::I8042): `init`, `init_keyboard`, `init_mouse`, `set_interrupts` |
| [`hal/drivers/i8042/src/irq.rs`](repo:hal/drivers/i8042/src/irq.rs) | [`KbdIrq`](sym:hal/drivers/i8042/src/irq.rs::KbdIrq), [`MouseIrq`](sym:hal/drivers/i8042/src/irq.rs::MouseIrq), [`decode`](sym:hal/drivers/i8042/src/irq.rs::decode) for one packet and the [`IrqStats`](sym:hal/drivers/i8042/src/irq.rs::IrqStats) counters |
| [`hal/drivers/i8042/src/input.rs`](repo:hal/drivers/i8042/src/input.rs) | [`Ps2Input`](sym:hal/drivers/i8042/src/input.rs::Ps2Input): the core's end of both rings and the decoder; the native `InputSource` |
| [`hal/drivers/i8042/src/tests.rs`](repo:hal/drivers/i8042/src/tests.rs) | 47 host tests against a model of the controller: wire sequences, timeouts, packets, resynchronisation, the whole path from a byte to a `KeyEvent` |
| [`hal/drivers/kbd/src/lib.rs`](repo:hal/drivers/kbd/src/lib.rs) | [`Decoder`](sym:hal/drivers/kbd/src/lib.rs::Decoder), the two key tables [`plain_key`](sym:hal/drivers/kbd/src/lib.rs::plain_key) and [`extended_key`](sym:hal/drivers/kbd/src/lib.rs::extended_key), and [`char_for`](sym:hal/drivers/kbd/src/lib.rs::char_for) |
| [`hal/hal/src/input.rs`](repo:hal/hal/src/input.rs) | [`KeyCode`](sym:hal/hal/src/input.rs::KeyCode), [`KeyEvent`](sym:hal/hal/src/input.rs::KeyEvent), [`MouseEvent`](sym:hal/hal/src/input.rs::MouseEvent), [`Modifiers`](sym:hal/hal/src/input.rs::Modifiers) and the [`InputSource`](sym:hal/hal/src/input.rs::InputSource) trait |
| [`hal/hal/src/ring.rs`](repo:hal/hal/src/ring.rs) | [`SpscRing`](sym:hal/hal/src/ring.rs::SpscRing) with its [`Producer`](sym:hal/hal/src/ring.rs::Producer) and [`Consumer`](sym:hal/hal/src/ring.rs::Consumer) ends |
| [`kernel/src/input.rs`](repo:kernel/src/input.rs) | [`init`](sym:kernel/src/input.rs::init) (set-up, rings, handlers), [`run`](sym:kernel/src/input.rs::run) (drain and print), [`idle`](sym:kernel/src/input.rs::idle) (`sti; hlt`) and the negative-check [`Fault`](sym:kernel/src/input.rs::Fault) |
| [`hal/drivers/int33/src/lib.rs`](repo:hal/drivers/int33/src/lib.rs) | [`Mouse`](sym:hal/drivers/int33/src/lib.rs::Mouse) over the INT 33h functions in [`func`](sym:hal/drivers/int33/src/lib.rs::func), with `detect` |
| [`hal/drivers/int33/src/input.rs`](repo:hal/drivers/int33/src/input.rs) | [`MouseInput`](sym:hal/drivers/int33/src/input.rs::MouseInput) (mickeys to events) and [`DosInput`](sym:hal/drivers/int33/src/input.rs::DosInput), the hosted `InputSource` |
| [`hal/drivers/int33/src/transcript.rs`](repo:hal/drivers/int33/src/transcript.rs) | [`write_key`](sym:hal/drivers/int33/src/transcript.rs::write_key) and [`write_mouse`](sym:hal/drivers/int33/src/transcript.rs::write_mouse): the text form of an event on the DOS side |
| [`spikes/d0-rust-dos/rt/src/irq.rs`](repo:spikes/d0-rust-dos/rt/src/irq.rs) | [`DosIrq`](sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq), the stub per line, `rt_irq_dispatch` and [`Slot`](sym:spikes/d0-rust-dos/rt/src/irq.rs::Slot) |
| [`spikes/b-dos-input/prog/src/main.rs`](repo:spikes/b-dos-input/prog/src/main.rs) | `INPUT.EXE`, the spike's program; [`shell/dos/src/main.rs`](repo:shell/dos/src/main.rs) is the hosted shell that uses `DosInput` |
| [`hal/platform/mock/src/ps2.rs`](repo:hal/platform/mock/src/ps2.rs) and [`int33.rs`](repo:hal/platform/mock/src/int33.rs) | The models the host tests run against: an 8042 with a keyboard and a mouse, and a DOS mouse driver |
| [`tools/native-inject.py`](repo:tools/native-inject.py) | The harness that plays [`tests/native/input.inject`](repo:tests/native/input.inject) into QEMU's monitor |
| [`tests/native/expected-input.txt`](repo:tests/native/expected-input.txt) | The expected transcript of the input run; [`expected-input-masked.txt`](repo:tests/native/expected-input-masked.txt) is the negative one |

For depth: `docs/C-INPUT.md` ([the set-up sequences](doc:docs/C-INPUT.md#the-set-up-sequences), [the interrupt hand-off](doc:docs/C-INPUT.md#the-interrupt-hand-off), [QEMU against real hardware](doc:docs/C-INPUT.md#qemu-against-real-hardware)) and `docs/B-DOS-INPUT.md` ([the keyboard under DPMI](doc:docs/B-DOS-INPUT.md#the-keyboard-irq-1-under-dpmi), [the mouse](doc:docs/B-DOS-INPUT.md#the-mouse-int-33h), [what DOSBox models](doc:docs/B-DOS-INPUT.md#what-dosbox-models-and-what-real-hardware-may-do-differently)).

## Worked example

**Shift, A, and the cursor-up key.** The host test `keys_travel_from_the_device_through_the_interrupt_and_the_ring_to_the_core` puts eight bytes into the model of the controller and checks what comes out of the core's end:

| Byte | State before | What the decoder does | Event | Modifiers after |
|---|---|---|---|---|
| `2A` | Idle | A make code: `0x2A` is left Shift | `LeftShift` down | `SHIFT` |
| `1E` | Idle | A make code: `0x1E` is A | `A` down | `SHIFT` |
| `9E` | Idle | Bit 7 set, `0x9E & 0x7F = 0x1E` | `A` up | `SHIFT` |
| `AA` | Idle | Bit 7 set, `0x2A`: left Shift | `LeftShift` up | none |
| `E0` | Idle | The extended prefix | (none yet) | none |
| `48` | Extended | `0x48` after `E0` is Up | `Up` down | none |
| `E0` | Idle | The extended prefix | (none yet) | none |
| `C8` | Extended | Break of `0x48` | `Up` up | none |

That is the test's six events, in order. The injected key sequence in `tests/native/input.inject` begins with `sendkey a` and `sendkey shift-a`, and the transcript the kernel printed for the second of them is in `tests/native/expected-input.txt`, line for line:

```
key: LeftShift down mods=shift
key: A down mods=shift char='A'
key: A up mods=shift
key: LeftShift up mods=-
```

The four lines are the first four rows. The `char='A'` is `char_for(A, SHIFT)`: Shift flips the letter. The `mods=-` is what `modifier_names` prints for an empty set. One honest caveat: the real run does not log the bytes on the wire, only the events, so that the bytes `2A 1E 9E AA` belong to the host test and the lines to the QEMU run, and the match between them is by the decoder's tables. A check in this chapter's tests decodes the test's bytes with a small replica of those tables and compares the result with the transcript lines.

**Ctrl+Alt+Delete.** The decoder's test `ctrl_alt_delete_reports_all_three_with_the_chord_state` feeds `1D 38 E0 53 E0 D3 B8 9D`: Ctrl down (`1D`), Alt down (`38`), Delete down (`E0 53`), Delete up (`E0 D3`), Alt up (`B8`), Ctrl up (`9D`). The transcript's six lines for the same chord show the modifier state after each event, `ctrl`, then `ctrl+alt`, `ctrl+alt` and `ctrl+alt` for the Delete press and release, then `ctrl` and `-`, which is why the Alt release shows `mods=ctrl`.

**A mouse report.** The test `mouse_packets_become_events_and_the_acknowledgement_covers_the_slave` feeds nine bytes: `09 0A 04`, `3A FF FE`, `08 00 00`. The first is the left button (`0x09`: bit 0 and the always-set bit 3) with dx 10 and device dy 4, which becomes dx +10, dy −4, buttons 1. The second, `0x3A`, is `0011 1010`: the right button (bit 1), bit 3, and both sign bits, so `0xFF` is −1 and `0xFE` is −2 and the event is dx −1, dy +2, buttons 2 (the Y delta was −2 on the device and is negated). The third is a report with nothing held and nothing moved. In the QEMU run the monitor command `mouse_move 10 -5` came out as `mouse: dx=10 dy=-5 buttons=0 wheel=0`, and `mouse_button 1` as `mouse: dx=0 dy=0 buttons=1 wheel=0`; the bytes on the wire were again not logged, but a four-byte packet with an ID of 3 is what the kernel printed it was reading.

## State of this layer

Verified here means that the record cites a test that runs in `tools/check.sh` or `tools/native-test.sh`, or a merged pull request whose CI passed. It means in QEMU, in DOSBox or on the host, never on a real machine. Nothing here has touched a real 8042, a real keyboard or a real mouse.

```status
id: input.8042
label: The 8042 set-up: both ports, self-test, configuration byte, port tests, interrupts last
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::controller_init_runs_the_documented_sequence_and_reports_what_it_found, test:tests/native/expected-input.txt::input: 8042 dual channel, sym:hal/drivers/i8042/src/lib.rs::I8042
note: Verified against a model of the controller and QEMU's. A real or USB-emulated controller may not stick the translation bit or answer the self-test as expected.

id: input.keyboard
label: Keyboard reset and enable, scancode set 1 by translation or by asking the keyboard
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::keyboard_init_resets_and_enables, test:hal/drivers/i8042/src/tests.rs::without_translation_the_keyboard_is_switched_to_set_1, test:tests/native/expected-input.txt::input: keyboard ready
note: Only the translated path runs in QEMU; the switch-to-set-1 path is a host test only.

id: input.mouse
label: Mouse reset, defaults and the wheel probe (200, 100, 80, then the ID)
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::the_wheel_probe_sends_200_100_80_and_reads_the_id, test:tests/native/expected-input.txt::input: mouse ready, sym:hal/drivers/i8042/src/lib.rs::init_mouse

id: input.decoder
label: The scancode set 1 decoder: extended keys, Pause, fake shifts, modifiers and locks
state: built-verified
evidence: test:hal/drivers/kbd/src/lib.rs::the_pause_sequence_is_one_press_and_no_release, test:hal/drivers/kbd/src/lib.rs::fake_shifts_do_not_release_a_real_shift, test:tests/native/expected-input.txt::key: LeftAlt down mods=ctrl+alt, sym:hal/drivers/kbd/src/lib.rs::Decoder
note: Typematic repeat, the lock-key lights and a real keyboard's timing were not exercised; QEMU's sendkey does not repeat.

id: input.packet
label: The PS/2 mouse packet: nine-bit deltas, flipped Y, buttons, wheel and overflow
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::packets_decode_with_sign_extension_and_a_flipped_y, test:tests/native/expected-input.txt::mouse: dx=-20 dy=7 buttons=0 wheel=0, sym:hal/drivers/i8042/src/irq.rs::decode

id: input.resync
label: Resynchronising a mouse stream that lost a byte
state: built-unverified
evidence: test:hal/drivers/i8042/src/tests.rs::a_lost_byte_costs_a_packet_or_two_and_then_the_stream_is_back_in_step, sym:hal/drivers/i8042/src/irq.rs::MouseIrq
note: Runs on the host only, where the test drops a byte on purpose. QEMU never loses one, so nothing in an emulator has exercised it, and the host test is not an emulator run.

id: input.ring
label: The single-producer, single-consumer ring: no lock, no compare-and-swap, drops and counts when full
state: built-verified
evidence: test:hal/hal/src/ring.rs::a_producer_thread_and_a_consumer_thread_agree, test:hal/hal/src/ring.rs::the_indices_wrap, test:tests/native/expected-input.txt::input: dropped keys 0, sym:hal/hal/src/ring.rs::SpscRing
note: A thread stands in for the interrupt on the host. The ordering argument rests on x86 semantics and is not proved on a multiprocessor, which the project does not target.

id: input.handlers
label: The IRQ 1 and IRQ 12 handlers: a byte or a packet into a ring, the source bit checked
state: built-verified
evidence: test:hal/drivers/i8042/src/tests.rs::a_keyboard_interrupt_does_not_steal_a_mouse_byte_or_the_reverse, test:hal/drivers/i8042/src/tests.rs::a_masked_irq_1_delivers_no_keys, sym:hal/drivers/i8042/src/irq.rs::KbdIrq

id: input.kernel
label: The kernel's input loop: init, drain, print, and sleep with sti and hlt
state: built-verified
evidence: test:tests/native/expected-input.txt::input: escape released after 26 key events and 8 mouse events, test:tools/native-test.sh::input-missing-event, sym:kernel/src/input.rs::run
note: QEMU only. The negative runs are in tools/native-test.sh: a removed injected key is rejected by the comparison.

id: input.masked
label: The negative checks: IRQ 1 left masked (a panic) and an idle input (the watchdog)
state: built-verified
evidence: test:tools/native-test.sh::input-masked, test:tools/native-test.sh::input-idle, sym:kernel/src/input.rs::Fault

id: input.inject
label: Input injected through the QEMU monitor and compared with a transcript
state: built-verified
evidence: sym:tools/native-inject.py::main, repo:tests/native/input.inject, test:tests/native/expected-input.txt::input: ready
note: QEMU folds a mouse press and release that arrive before the guest reads the previous packet, which is why the script pauses after each mouse command.

id: input.dos.kbd
label: The keyboard on DOS: a protected-mode IRQ 1 stub, the shared handler, ring and decoder
state: built-verified
evidence: test:hal/drivers/int33/src/tests.rs::the_keys_the_dos_program_is_sent_decode_to_the_expected_transcript, test:spikes/b-dos-input/expected/keys.txt::key A down mods=none, sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq
note: Verified in DOSBox Staging with typed keys, as the anatomy page tags it. The BIOS keyboard handler, CWSDPMI's stack behaviour and a real 8042 were not tried.

id: input.dos.mouse
label: The mouse on DOS through INT 33h, polled once per core iteration
state: built-unverified
evidence: sym:hal/drivers/int33/src/lib.rs::Mouse, sym:hal/drivers/int33/src/input.rs::MouseInput, doc:docs/B-DOS-INPUT.md#the-mouse-int-33h
note: Not verified in the first-window run, as the anatomy page tags it: that run scripts the pointer as events and never moves a mouse. Host tests against a model and an opt-in Docker run under a virtual X server exist; no real driver or mouse has been tried, and a click shorter than a poll is lost.

id: input.live
label: Live input reaching the shell on both tracks
state: built-unverified
evidence: repo:docs/FIRST-WINDOW.md, doc:docs/B-DOS-INPUT.md#results
note: Keys were typed live into the focused field on DOS only. The mouse reached the shell on neither track, and nothing live reached the native shell.

id: input.hardware
label: A real 8042, USB legacy emulation, laptop touchpads, key repeat and the lock-key lights
state: planned
evidence: doc:docs/C-INPUT.md#qemu-against-real-hardware
```

The keyboard and the QEMU mouse path are {{state.input.kernel}} in QEMU, and the controller set-up is {{state.input.8042}} against the model of the controller and QEMU's. The DOS keyboard is {{state.input.dos.kbd}} in DOSBox. The DOS mouse is {{state.input.dos.mouse}}, and that is on purpose.

## What a real machine may do differently

`docs/C-INPUT.md` ("QEMU against real hardware") and `docs/B-DOS-INPUT.md` ("What DOSBox models") list these, and each is a guess about hardware, not a measurement.

- **USB legacy emulation.** On a PC whose keyboard and mouse are USB, the BIOS or a chipset's system-management code presents an 8042 so old software works. Such emulation may leave out the self-test, the auxiliary port or the acknowledgements, or have configuration bits that do not stick. The set-up reports each failure and carries on without the device rather than hanging, but the sequences have met only the model and QEMU.
- **Translation that will not stick.** Then the driver asks the keyboard for set 1. Some keyboards do not support it, and the check afterwards is reported but the decoder would then be fed set 2 bytes.
- **Mice that do not acknowledge.** Some devices omit the `0xFA` or answer `0xFE`. Resends are retried, and a mouse that never acknowledges fails the set-up with a timeout and is not used.
- **Timing.** QEMU's devices answer instantly; a real keyboard's self-test can take up to about a second. The bound is a million status reads, which on an ISA bus is meant to be enough and is not measured.
- **Edge-triggered lines.** If the controller raises IRQ 1 or 12 while the line is masked, the edge can be lost and the byte sits in the output buffer until the next arrives. The set-up flushes before unmasking and the handler drains up to eight bytes, which makes a stale byte unlikely to wedge the stream. The BIOS-call trampoline of M8 closes IRQ 1 and IRQ 12 for the same reason and polls the 8042 by hand after a call.
- **Touchpads, hot-plugging, lights.** Laptop touchpads may need vendor sequences for more than basic motion; hot-plugging a PS/2 mouse would put its `0xAA 0x00` start-up bytes in the stream as the start of a packet; the lock-key lights are not driven. For DOS there is more: mouse drivers differ in the meaning of a mickey, in the sign of the wheel and in the signedness of the arguments to function 0004h, and a BIOS keyboard handler may not tolerate a second read of port `0x60` in chain mode. DOSBox's own driver is the only one that has been run.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

## Try it yourself

These commands run the input drivers' host tests and then the native kernel's input test mode, the way `tools/native-test.sh` does in its `run_input` function: build the kernel and an image with the `input` test mode, play `tests/native/input.inject` into QEMU's monitor with `tools/native-run.sh --inject`, and compare the transcript with the expected file. They need a Rust toolchain, NASM and QEMU (`qemu-system-i386`); the scripts fall back to Docker for NASM and QEMU. Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: cargo (host tests); native run: bash, nasm, qemu-system-i386 (or Docker)
ran: 2026-10-23 on the host, QEMU 11.1.2, NASM 3.02
$ cd hal && cargo test -p desk-i8042 -p desk-kbd -p desk-int33 2>&1 | grep -E "^test result: .* [1-9][0-9]* passed"
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cd hal && cargo test -p desk-i8042 keys_travel 2>&1 | grep -E "^test "
test tests::keys_travel_from_the_device_through_the_interrupt_and_the_ring_to_the_core ... ok
$ cd hal && cargo test -p desk-hal ring 2>&1 | grep -E "^test "
test ring::tests::items_come_out_in_order_and_a_full_ring_drops ... ok
test ring::tests::the_indices_wrap ... ok
test ring::tests::a_producer_thread_and_a_consumer_thread_agree ... ok
$ tools/native-build.sh
dist/native/kernel.bin: 173968 bytes, load address 0x100000
$ tools/native-image.sh --test-mode input --out dist/native-input.img
dist/native-input.img: 1474560 bytes, kernel 173968 bytes in 340 sectors, test mode input, video text
$ tools/native-run.sh --timeout 30 --inject tests/native/input.inject dist/native-input.img > dist/input-run.txt
native-run: guest reported success
$ tr -d '\r' < dist/input-run.txt | grep -E "^(input|mouse): " | sed -n 1,7p
input: 8042 dual channel, translation on
input: keyboard ready, scancode set 1 translated by the controller
input: mouse ready, id 3 (4-byte packets)
input: IRQ 1 and IRQ 12 handlers registered, controller interrupts on
input: ready
mouse: dx=10 dy=-5 buttons=0 wheel=0
mouse: dx=-20 dy=7 buttons=0 wheel=0
$ tr -d '\r' < dist/input-run.txt | grep -E "^input: (escape|dropped)"
input: escape released after 26 key events and 8 mouse events
input: dropped keys 0, dropped mouse events 0, mouse resyncs 0
$ python3 -I tools/native-check.py tests/native/expected-input.txt dist/input-run.txt
transcript matches tests/native/expected-input.txt (75 lines)
```

The three `test result` lines are `desk-i8042` (47 tests), `desk-int33` (16) and `desk-kbd` (24); the documents say 46, 16 and 22 (see Revisions). The `native-run` line is on the standard error stream of the script, shown here with the command. The 26 key events and 8 mouse events are the injected sequence of `tests/native/input.inject`.

**Exercises.** (1) Without running anything, decode `1D 38 E0 53 E0 D3 B8 9D` into events and modifier states, then find your six lines in `tests/native/expected-input.txt`. (2) The packet `3A FF FE` has the right button and both sign bits set; compute dx, dy and buttons, then find the same numbers in the host test. (3) In a scratch branch, remove the `ctrl-alt-delete` line from `tests/native/input.inject`, predict whether the run exits 0, and whether `native-check.py` accepts its transcript; the `input-missing-event` run in `tools/native-test.sh` does exactly this. (4) Predict what the DOS program prints for the same shift-A, then compare with `spikes/b-dos-input/expected/keys.txt`, whose `mods=none` and `mods=SHIFT` spellings differ from the kernel's `mods=-` and `mods=shift`.

```glossary
term: Make code and break code
def: In scancode set 1, the byte a keyboard sends when a key goes down (the make code, the key's number) and the byte it sends when the key comes up (the break code, the same number with bit 7 set).

term: Input event
def: One of the two values the Desk's core receives from a keyboard or mouse: a KeyEvent (a key code, whether it went down, and the modifier state after) or a MouseEvent (dx, dy, the buttons now and the wheel). Both tracks produce the same two types.

term: Lock-free ring
def: A fixed-size queue with one writer and one reader, whose two indices are each written by only one side, so neither side ever waits or takes a lock. The Desk uses it to pass bytes and events from an interrupt handler to the core.

term: PS/2 mouse packet
def: The three bytes (four with a wheel) a PS/2 mouse sends for each report: the buttons, the two sign bits and two overflow bits, then the low eight bits of each movement.
```

```figures
id: m11-input-replay
reuse: input-replay
at: explainer input

id: m11-pipelines
reuse: pipelines
at: explainer pipelines
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every status row was checked against the repository, and the host tests and the native input run in the commands above were run on the host. Disagreements found between the documents and the code: `docs/C-INPUT.md` counts 46 tests for the controller driver and 22 for the decoder, and the tree has 47 and 24 (`cargo test` prints them); and `desk_int33::transcript`'s header says one expected file serves "both" the host and the DOS run, which is true of those two, while the native kernel prints a different text form (`key: A down mods=- char='a'`). The wire bytes of the QEMU run are not logged; the worked example's bytes come from host tests.
