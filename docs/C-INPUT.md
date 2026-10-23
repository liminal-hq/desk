# Spike C: keyboard and PS/2 mouse in the native kernel

Spike C (n1) puts the keyboard and the PS/2 mouse into the native kernel as drivers written once against the HAL ([`HAL.md`](HAL.md)): the 8042 controller, the keyboard, the mouse, the hand-off from the interrupt handlers to the kernel's loop, and a QEMU check that injects real key presses and mouse motion and compares the decoded events. The code is `hal/drivers/i8042`, `hal/drivers/kbd`, the model in `hal/platform/mock/src/ps2.rs` and, for the kernel, `kernel/src/hal.rs` and `kernel/src/input.rs`.

## Contents

- [Result](#result)
- [Method](#method)
- [The 8042 controller](#the-8042-controller)
- [The set-up sequences](#the-set-up-sequences)
- [The event contracts](#the-event-contracts)
- [The scancode decoder](#the-scancode-decoder)
- [The mouse packet](#the-mouse-packet)
- [The interrupt hand-off](#the-interrupt-hand-off)
- [The kernel side](#the-kernel-side)
- [How it is tested](#how-it-is-tested)
- [QEMU against real hardware](#qemu-against-real-hardware)
- [HAL feedback](#hal-feedback)
- [What was and was not verified](#what-was-and-was-not-verified)

## Result

Both devices work in QEMU as a 486: the kernel brings up the controller, resets the keyboard and the mouse (finding an IntelliMouse wheel), takes IRQ 1 and IRQ 12 through a registry for all sixteen lines, and a headless run that types `a`, `Shift+A`, `1`, `Shift+1`, Enter, keypad Enter, Up, Ctrl+Alt+Delete and Escape and moves and clicks the mouse through the QEMU monitor shows every event decoded in the serial transcript. The kernel's serial, PIC and PIT code now runs on the shared drivers (migration step 1 of the HAL plan), with the existing transcript lines unchanged.

An excerpt of the transcript of that run (the full expectation is `tests/native/expected-input.txt`):

```
input: 8042 dual channel, translation on
input: keyboard ready, scancode set 1 translated by the controller
input: mouse ready, id 3 (4-byte packets)
input: IRQ 1 and IRQ 12 handlers registered, controller interrupts on
input: ready
mouse: dx=10 dy=-5 buttons=0 wheel=0
mouse: dx=-20 dy=7 buttons=0 wheel=0
mouse: dx=0 dy=0 buttons=1 wheel=0
key: A down mods=- char='a'
key: LeftShift down mods=shift
key: A down mods=shift char='A'
key: Num1 down mods=shift char='!'
key: KpEnter down mods=- char='\n'
key: Up down mods=-
key: LeftCtrl down mods=ctrl
key: LeftAlt down mods=ctrl+alt
key: Delete down mods=ctrl+alt
key: Escape up mods=-
input: escape released after 26 key events and 8 mouse events
```

## Method

1. Write the drivers as `no_std` crates over `PortIo` with no `unsafe`, and test every path on the host against a model of the 8042 with a keyboard and a mouse behind it (`desk-hal-mock::ps2`), plus short scripted port sequences where the exact bytes on the wire matter.
2. Put the platform-specific parts in the kernel: the real port bus, the IDT stubs and the interrupt registry.
3. Run the kernel in headless QEMU, wait for it to say its handlers are live, and send input through the QEMU monitor (`sendkey`, `mouse_move`, `mouse_button`) from `tools/native-inject.py`, then compare the transcript.

The details below were written from memory of the usual references, not re-checked against the originals in this spike: the OSDev wiki pages "8042 PS/2 Controller", "PS/2 Keyboard" and "PS/2 Mouse", the IBM PS/2 technical references, and the behaviour of Linux's `i8042` and `psmouse` drivers and of QEMU's `hw/input/pckbd.c` and `ps2.c`. Anything the tests cannot show is hedged where it appears.

## The 8042 controller

Two ports: `0x60` is the data port and `0x64` is the status register (read) and the command register (write).

| Status bit | Meaning |
|---|---|
| 0 | Output buffer full: a byte is waiting at `0x60` |
| 1 | Input buffer full: the controller has not taken the last byte written; wait before writing |
| 2 | System flag, set by the controller's self-test |
| 3 | The last write was a command (`0x64`) rather than data (`0x60`) |
| 5 | The byte in the output buffer came from the auxiliary (mouse) port (on PS/2 controllers; the original AT controller used it for a transmit timeout) |
| 6 | Timeout error on the device link |
| 7 | Parity error on the device link |

| Command (`0x64`) | Does |
|---|---|
| `0x20` | Read the configuration byte |
| `0x60` | Write the configuration byte (next byte to `0x60`) |
| `0xA7` / `0xA8` | Disable / enable the auxiliary port |
| `0xA9` | Test the auxiliary port (`0x00` passes) |
| `0xAA` | Self-test (`0x55` passes) |
| `0xAB` | Test the keyboard port (`0x00` passes) |
| `0xAD` / `0xAE` | Disable / enable the keyboard port |
| `0xD4` | Send the next byte written to `0x60` to the mouse |

The configuration byte: bit 0 enables IRQ 1 for the keyboard, bit 1 enables IRQ 12 for the mouse, bit 2 is the system flag (leave it), bit 4 holds the keyboard clock low (port disabled), bit 5 the mouse clock, and bit 6 turns on the translation of the keyboard's scancode set 2 to set 1.

The driver never uses the output-port commands (`0xD0`, `0xD1`) or the reset pulse (`0xFE`). Bit 1 of the output port is the A20 gate, and bit 0 must stay set or the machine resets; the boot loader's keyboard-controller A20 method (`-DA20_TEST=2`) uses that port, the kernel does not. A host test asserts that controller set-up issues neither command and leaves the model's output port as it was.

Every wait on a status bit is bounded: `POLLS` (100,000 reads, about 100 ms of ISA reads, assuming roughly 1 µs each — an estimate) for an ordinary command and reply, `SLOW_POLLS` (1,000,000) for the replies that take long on real parts (a keyboard's self-test after reset, the controller self-test). A bound that runs out returns `Error::Timeout`; nothing hangs. A floating bus reads all ones, so the input buffer looks permanently full and the first command times out, which a host test covers.

## The set-up sequences

Controller (`I8042::init`), with the controller's interrupts off throughout:

1. `0xAD`, `0xA7`: disable both ports.
2. Flush the output buffer (at most 64 bytes, so a device that streams forever cannot trap the set-up).
3. `0x20`, then `0x60` with interrupts off, translation on and the clock-disable bits clear.
4. `0xAA` self-test, expect `0x55`; write the configuration byte again, because some controllers reset it.
5. `0xA8` then `0x20`: if bit 5 is clear the controller has a second channel; `0xA7` again.
6. `0xAB` (and `0xA9` if dual channel) port tests, expect `0x00`.
7. `0xAE` and `0xA8`: enable the ports. Flush.

The order follows the usual OSDev recipe. Interrupts are turned on last (`set_interrupts`), after the devices are set up and the handlers are registered, so no interrupt arrives to a handler that is not there.

Keyboard (`init_keyboard`): `0xF5` (stop scanning), `0xFF` (reset), expect `0xFA` then `0xAA` (self-test passed; `0xFC` or `0xFD` is a failure), flush, `0xF4` (start scanning). Each command byte is retried when the keyboard answers `0xFE` (resend), up to three times, and a reply that is neither `0xFA` nor `0xFE` is an error. A timeout on the first command means no keyboard (`Error::NoDevice`). The keyboard's own typematic rate and delay are left at the defaults the reset gives (about 500 ms and 10.9 characters a second, by the usual figures); the BIOS's choice is lost on reset, and nothing here reprograms it.

Scancode set: the keyboard speaks set 2 after a reset. The controller translates set 2 to set 1 when configuration bit 6 is set, so **the CPU reads set 1**, which is what `desk-kbd` decodes. If the bit does not read back as set (a controller that cannot translate), the driver asks the keyboard for set 1 (`0xF0 0x01`) and checks with `0xF0 0x00`; `KeyboardInfo` says which happened. Whether a real keyboard answers the query with `1` or with the translated `0x43` is a detail the check accepts both of; it is exercised only against the model.

Mouse (`init_mouse`), every byte after a `0xD4` prefix: `0xFF` reset, expect `0xFA`, `0xAA`, `0x00` (a mouse that sends `0xAA` without the `0xFA` is accepted), `0xF6` set defaults, then the IntelliMouse probe (sample rate `0xF3` with 200, 100, 80, then `0xF2` to read the ID; ID 3 means a wheel and four-byte packets), the sample rate back to 100, and `0xF4` enable streaming. A failed probe leaves a plain three-byte mouse. A byte from the other device during the set-up (a key pressed while the mouse resets) is read and dropped by looking at status bit 5, so it is never mistaken for a reply.

## The event contracts

Defined in `desk-hal::input` ([`HAL.md`](HAL.md), "Input"):

- `KeyEvent { code: KeyCode, pressed: bool, modifiers: Modifiers }`. `KeyCode` is a Desk enum of physical keys on a US layout, independent of the scancode set. `modifiers` is the state **after** the event: pressing Shift reports it with `SHIFT` set. Lock keys (Caps, Num, Scroll) toggle on press only and ignore typematic repeats. Pause has no release code, so it produces one `pressed` event.
- `MouseEvent { dx: i16, dy: i16, buttons: u8, wheel: i8 }`. Right is positive `dx`, down the screen is positive `dy` (the device reports up as positive, so the driver negates it), buttons are bit 0 left, bit 1 right, bit 2 middle and are the state now, not changes, and `wheel` is positive towards the user.
- `InputSource` with `poll_key` and `poll_mouse`, implemented by `desk_i8042::Ps2Input`.

`desk_kbd::char_for(code, modifiers)` is the small US key map from keys to characters: Shift selects the upper legend, Caps Lock flips letters only, the keypad types digits with Num Lock on and Shift off, Ctrl turns a letter into its control code.

## The scancode decoder

`desk_kbd::Decoder::feed(byte) -> Option<KeyEvent>`, pure, scancode set 1. A make code is the key's number; a break code has bit 7 set. `0xE0` prefixes the keys the 101-key layout added (right Ctrl and Alt, the cursor block and the six above it, keypad `/` and Enter, Print Screen, Windows and menu keys); `E0 46` is Ctrl+Pause (Break). Pause sends `E1 1D 45 E1 9D C5` on press and nothing on release. The fake shifts the keyboard wraps around Print Screen, Insert and the cursor block (`E0 2A`, `E0 AA`, `E0 36`, `E0 B6`) are swallowed and do not disturb the real Shift state. `0x00`, `0xFA`, `0xFE` and `0xFF` in the stream (overrun, ACK, resend) carry no key. A broken Pause sequence is dropped and its last byte read afresh. An unknown code gives `KeyCode::Unknown { extended, scancode }` rather than being lost. Host tests cover every plain code, the whole extended block, Print Screen, the Pause sequence (whole, split, broken), the modifier and lock logic, and the key map.

## The mouse packet

Three bytes (four with a wheel): byte 0 has the left, right and middle buttons in bits 0 to 2, bit 3 always set, the X and Y sign bits in bits 4 and 5 and the X and Y overflow bits in bits 6 and 7; byte 1 is the low eight bits of the X delta and byte 2 of the Y delta, so the deltas are nine-bit two's complement with the sign in byte 0; byte 3 is a signed wheel count.

- A packet with an overflow bit set is consumed and produces no event (the delta is meaningless); it is counted.
- **Resynchronisation.** While no packet is in progress, a byte without bit 3 cannot be a first byte and is dropped and counted, until one with bit 3 arrives. This recovers from a lost byte within a few packets (a host test drops a byte and checks the stream is back in step; the packet that straddles the loss is garbage, and one more may be). It cannot detect a loss when the shifted bytes happen to look like a valid first byte, and there is no timeout-based resynchronisation, because a handler has no clock. QEMU never loses a byte, so only the host tests exercise this.
- A byte flagged with a parity or timeout error abandons the packet in progress.

## The interrupt hand-off

The handlers only move bytes ([`HAL.md`](HAL.md), "Concurrency"):

- `KbdIrq` (IRQ 1): while the status register shows output full from the keyboard (bit 5 clear), read `0x60` and push the raw byte into a `SpscRing<u8, 64>`. Nothing is decoded.
- `MouseIrq` (IRQ 12): the same for the mouse (bit 5 set), assembling the packet byte by byte, because that cannot wait, and pushing one `MouseEvent` per packet into a `SpscRing<MouseEvent, 32>`.
- Each handler reads at most eight bytes per interrupt, a bound on its work. Looking at bit 5 means IRQ 1 never takes a mouse byte and the reverse, which would corrupt a packet or a scancode sequence when both lines are pending.
- A full ring drops the new item and counts it in the ring (`dropped`); the handler never waits.
- The kernel registers each handler as an `IrqHandler` (a C-ABI function pointer plus a context pointer, built by `from_static` from a leaked `Box`, allocated before the interrupts are enabled, never in the handler), installs the IDT gate, and **sends the end-of-interrupt itself** after the handler returns: the slave and then the master for IRQ 12. A spurious IRQ 7 or 15 runs no handler and gets `eoi_spurious` (nothing for 7, the master only for 15).
- Unmasking IRQ 12 on the slave also unmasks the cascade (IRQ 2), which `desk-pic` does for any slave line.

**Why it is safe.** The producer end of each ring lives only in its handler and the consumer end only in the kernel's loop (`split` hands each out once, so a second producer does not compile). The ring's indices are 32-bit words: the producer alone writes `head`, the consumer alone writes `tail`, with release stores and acquire loads, which are plain `mov`s on x86; there is no compare-and-swap (the 486 has it but DOSBox's 486 mode rejects it, and `hal/clippy.toml` bans it), no 64-bit atomic and no lock the interrupted code could hold. The statistics counters use `fetch_add` (`lock xadd`). An interrupt gate clears IF on entry, so a handler is never interrupted by another handler or by itself. The loop idles safely: it tests the rings with interrupts off and then runs `sti; hlt` — the `sti` takes effect after the next instruction, so an interrupt arriving in between wakes the `hlt` rather than being missed.

## The kernel side

`kernel/src/hal.rs` holds the one `unsafe` step (`RawPorts::new`, "the kernel runs in ring 0"), the statics for the PIC, the 16550 and the PIT clock, and the `IrqTable` registry. `kernel/src/idt.rs` has one assembly stub per hardware line (vectors `0x20` to `0x2F`) that calls `irq_dispatch(line)`; the gate for a line is installed when a handler is registered (IRQ 0's at start-up as before), so the existing line "idt: 32 exception gates and the timer gate installed" stays true. `serial.rs`, `pic.rs` and `pit.rs` became thin layers over `desk-serial`, `desk-pic` and `desk-pit`; `port.rs` is gone. `vga.rs` was left alone: moving it to a `desk-vgatext` driver is optional in the plan and unrelated to input.

`kernel/src/input.rs` sets up the controller and devices, prints one line per step, and in the test modes runs the loop: drain the mouse ring, drain the key ring (decoding), print each event, `hlt` when both are empty, and give up with a panic if Escape is not released within a tick bound. In a normal boot it only sets up and prints the four `input:` lines before `boot complete`. A missing device is reported and skipped.

## How it is tested

**Host (`cargo test` in `hal/`).** The model `I8042Model` implements the status register, command and data ports, the configuration byte, the self-test and port tests, the output buffer with its source bit, and the two devices' command sets, with knobs for an absent device, `0xFE` resends, a device that never acknowledges, a failing self-test, a controller without translation or a second channel, and a controller whose input buffer is full for a number of reads or for ever. 46 tests cover: the exact bytes on the wire for the main handshakes (scripted), the documented sequence of controller commands, that the output port is never touched, every timeout path, the resend limit, a missing device, the wheel probe, a stray byte from the other device, packet decoding (sign extension, the nine-bit extremes, buttons, overflow, wheel), resynchronisation, a ring that fills and drops, indices that wrap over many rounds, and the whole path from a scancode in the model through `MockIrq`, the handler, the ring and `Ps2Input` — including a masked IRQ 1 delivering nothing until it is unmasked. The decoder has 22 more.

**QEMU (`tools/native-test.sh`).** `tools/native-run.sh --inject SCRIPT` puts the QEMU monitor on a Unix socket and `tools/native-inject.py` plays `tests/native/input.inject` into it once the kernel prints `input: ready`. The mouse commands are followed by short pauses: without them QEMU folded a button press and release that arrived before the guest read the previous packet, giving 6 to 8 events across runs (the kernel's drop and resync counters stayed at zero, so the folding is on the QEMU side); with them eight runs in a row gave the same eight events. Escape ends the run; the transcript is compared with `tests/native/expected-input.txt`. Negative checks: the same kernel with the `ctrl-alt-delete` line removed from the script still exits 0, but its transcript is rejected by the comparison; `input-masked` leaves IRQ 1 masked, the mouse events arrive and no key does, and the kernel panics after 3 seconds of ticks with the expected message (exit 1); `input-idle` waits for input that never comes and the 5-second watchdog ends it (exit 124). A malformed mouse packet cannot be sent through the monitor, so resynchronisation is a host test only. The whole input section adds about 10 seconds.

## QEMU against real hardware

What QEMU models, and what a real machine may do differently. None of the following was tested on hardware.

- **USB legacy emulation.** On a PC whose keyboard and mouse are USB, the BIOS (or a chipset's SMM code) emulates an 8042 so old software works. Such emulation can be incomplete: ports that never ACK, a missing self-test, no auxiliary port, bits in the configuration byte that do not stick, or a controller that does not answer `0xAA` as expected. The set-up reports each failure and carries on without the device rather than hanging, but the sequences have only met QEMU and the model.
- **Keyboards that start in set 2 without translation.** The controller's translation bit may not stick; the driver then asks the keyboard for set 1. Some keyboards do not support set 1 at all; the check afterwards (`set1: false` in `KeyboardInfo`) is reported but the decoder would then be given set 2 bytes. Not handled further.
- **Mice that do not acknowledge.** Some devices omit the `0xFA` after some commands, or answer a command with `0xFE`. Resends are retried; a mouse that never acknowledges fails the set-up with a timeout and is not used. A mouse that streams before it is enabled would leave bytes in the buffer, which the flush after the controller set-up drops (up to 64 bytes).
- **The A20 gate** sits on the 8042's output port, bit 1. This driver does not touch the output port, and its host test asserts it. A real BIOS or the boot loader may have used the port; the kernel does not depend on it afterwards.
- **Laptop touchpads and trackpoints** often sit behind the auxiliary port and may need vendor sequences for more than basic relative motion (Synaptics and ALPS pads speak an extended protocol after a magic knock). In the default mode they usually emulate a standard mouse, which is all this driver asks for; this is from general knowledge, not tested.
- **Timing.** QEMU's devices answer instantly. A real keyboard takes up to about a second for its self-test; the bound is 1,000,000 status reads, which on an ISA bus is meant to be enough but is not measured.
- **Edge-triggered IRQs.** If the controller raises IRQ 1 or 12 while the line is masked, the edge may be lost; the byte stays in the output buffer until the next byte arrives or the line is unmasked and the controller raises it again. The set-up flushes the buffer before unmasking, and the handler drains up to eight bytes per interrupt, which makes a stale byte unlikely to wedge the stream; a real machine could still behave differently.
- **Typematic repeat** is whatever the keyboard does; QEMU's `sendkey` does not repeat, so repeat events are not exercised.
- **PS/2 wheel mice with five buttons** (ID 4) are not probed. Only ID 3 is.
- **Key LEDs** (the `0xED` command) are not driven; the decoder tracks the lock state but nothing shows it.

## HAL feedback

Changes made to the HAL in this branch, small and compatible:

- `KeyEvent` is `{ code, pressed, modifiers }` rather than the sketch's `{ key, pressed }`: the owner's brief asked for modifiers in the event and the name `code`. `Modifiers` is a small bit set in `desk-hal::input`.
- `MouseEvent` gains `wheel: i8`. Without it the wheel probe would negotiate four-byte packets and then throw the fourth byte away.
- `KeyCode`, `Modifiers`, `KeyEvent`, `MouseEvent` and `Scancode` all live in `desk-hal::input` (the types the drivers and the core share), the decoder in `desk-kbd`.

Recommendations, not changes:

- The contract says the keyboard handler reads a byte "if status bit 0 is set". With both lines pending that can steal the other device's byte; the handlers here also test the source bit (status bit 5). The contract text should say so.
- `InputSource` has no way to ask "is there anything" without popping, which a `hlt` loop needs (check with interrupts off, then `sti; hlt`). `Ps2Input::is_empty` exists; if the core's loop is to be written against the trait, the trait wants an `is_empty` or a `wait` the platform implements.
- `IrqHandler::from_static` needs a `&'static mut`. The kernel leaks a `Box`, which is fine once the heap is up, but a kernel that wants input before the heap would use `from_raw` over a static; a `StaticCell`-style helper in the HAL would avoid each platform writing that.
- Mouse overflow packets are dropped rather than clamped, as the contract says. A fast flick on real hardware may overflow often enough that clamping would feel better; undecided.
- The platform's `IrqRegistry` needed a table the stubs read in interrupt context. `kernel::hal::IrqTable` is a candidate for the HAL's mock-tested shared code, but its only interesting part is the `unsafe` table access, which is platform-specific, so it stayed in the kernel.

## What was and was not verified

Verified, on the development machine:

- `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` in `hal/`; the clippy build for the kernel's target and the `cargo check` for the pure-Rust DOS target, including the new crates.
- `tools/native-test.sh` with QEMU as a 486: the existing checks with their transcripts unchanged except the four new `input:` lines in the normal boot, and the new keyboard and mouse section as described above.

Not verified:

- Anything on real hardware: a real 8042, a USB-legacy-emulated controller, a laptop touchpad, a wheel mouse, a keyboard with its own timing. 86Box and Bochs (spike G) are not used.
- The Docker backend of `tools/native-run.sh` with `--inject` (it mounts the socket directory; not run).
- Mouse wheel motion (QEMU's monitor cannot send it): the wheel path is tested on the host only, though the wheel negotiation and four-byte packets run in QEMU with every packet.
- Resynchronisation after a real lost byte, key repeat, and key-up events for a key held while the kernel was not yet reading.
- The sources listed under [Method](#method) were not re-read in this spike.
