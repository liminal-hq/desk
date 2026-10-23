# B: the keyboard and the mouse on the DOS runtime

Spike B (`s3-mouse` in the older numbering): the keyboard through a protected-mode IRQ 1 handler under DPMI and the mouse through the DOS mouse driver's INT 33h, from Rust on the pure-Rust DOS runtime (`spikes/d0-rust-dos/rt`), built from the shared HAL drivers so that the hosted platform hands the Desk core the same `KeyEvent` and `MouseEvent` types, from the same decoder, as the native kernel does (spike C, [`C-INPUT.md`](C-INPUT.md)).

> **Status.** *Built and verified:* the INT 33h driver crate `desk-int33` and its model, 16 host tests including the whole DOS input path (mock 8042, IRQ 1 service, ring, shared decoder) against the transcript the DOS run is also checked against; `INPUT.EXE` in DOSBox Staging 0.83 with keys typed in by `AUTOTYPE` (VBE 640×480×8, mode 13h, text mode, chained and replaced BIOS handler, ring overflow, no mouse driver, a panic, a missing key, no Escape); the mouse moved, clicked and scrolled by `xdotool` over DOSBox's SDL window under Xvfb in Docker (opt-in). *Not verified:* real hardware, any mouse driver but DOSBox's built-in one, a real BIOS keyboard handler, 86Box, QEMU or Bochs, DPMI hosts other than CWSDPMI, Windows DOS boxes, the real-mode callback (function 000Ch).

## Contents

- [What was built](#what-was-built)
- [Method](#method)
- [The keyboard: IRQ 1 under DPMI](#the-keyboard-irq-1-under-dpmi)
- [The mouse: INT 33h](#the-mouse-int-33h)
- [The event contract shared with the native kernel](#the-event-contract-shared-with-the-native-kernel)
- [The program](#the-program)
- [How it is tested](#how-it-is-tested)
- [Results](#results)
- [What DOSBox models and what real hardware may do differently](#what-dosbox-models-and-what-real-hardware-may-do-differently)
- [What the native platform must keep in step](#what-the-native-platform-must-keep-in-step)
- [HAL feedback](#hal-feedback)
- [Commands](#commands)
- [What was and was not verified](#what-was-and-was-not-verified)

## What was built

| Piece | Where | What it is |
|---|---|---|
| `desk-int33` | `hal/drivers/int33` | `Mouse` (INT 33h through the `Bios` trait: reset and detect, show and hide, position, set position, ranges, motion counters, wheel API, storage size, driver type), `MouseInput` (mickeys and buttons to `MouseEvent`), `DosInput` (the hosted `InputSource`: scancode ring, shared decoder, optional mouse), and `transcript` (one line of text per event) |
| `Int33Model` | `hal/platform/mock/src/int33.rs` | A DOS mouse driver for the host tests: no driver, a vector that points at an `IRET`, two buttons, a wheel; counters that a read clears, ranges and clamping |
| `SoftCursor` | `hal/drivers/vga/src/cursor.rs` | The arrow sprite with save-under over any `Canvas`, clipped at every edge |
| `Decoder::with_locks` | `hal/drivers/kbd` | A decoder that starts with the lock keys in the state the firmware shows |
| `InputSource::has_mouse` | `hal/hal/src/input.rs` | The core's way to ask whether to run keyboard-only |
| `rt::irq` | `spikes/d0-rust-dos/rt/src/irq.rs` | `DosIrq: IrqRegistry` for IRQ 0 to 15: a stub per line, a locked interrupt stack, the end-of-interrupt through `desk-pic`, chaining, restore on exit and on panic; `Slot` for interrupt-side state in the locked image |
| `rt::bios_kbd` | `spikes/d0-rust-dos/rt/src/bios_kbd.rs` | The BIOS keyboard state in the data area: lock flags, the type-ahead buffer |
| Program | `spikes/b-dos-input` | `INPUT.EXE`, its build, `check.sh` (headless, always on) and `check-xvfb.sh` (the mouse, opt-in) |
| Tools | `tools/dosrun.sh`, `tools/docker/Dockerfile.dos-xvfb` | `DOS_AUTOTYPE`, `DOS_CONF` and `DOS_X11` for the runner; the image with Xvfb and `xdotool` |

## Method

The hosted platform and the native kernel share the scancode path completely: the same handler (`desk_i8042::KbdIrq`, which reads port 0x60 and pushes raw bytes into an `SpscRing<u8, 64>`), the same decoder (`desk_kbd::Decoder`, scancode set 1 to `KeyEvent`), the same types. What differs is the platform around it: how the handler is installed, who acknowledges the interrupt, and the controller's set-up (which DOS has already done). The mouse is a different device interface: DOS programs do not talk to the PS/2 auxiliary port, they ask the mouse driver, so `desk-int33` turns the driver's answers into the `MouseEvent` the PS/2 packet decoder produces.

Everything that does not touch DPMI or real hardware is written in a driver crate against the `Bios` and `PortIo` traits and tested on the host. What does (the interrupt stub, the real-mode call, the BIOS data area) is in `rt` and is tested in DOSBox.

Sources, all read from memory of the published documents and not re-read in this spike, so details should be checked before they are relied on:

- The DPMI 0.9 specification: 0204h and 0205h (get and set the protected-mode interrupt vector), 0600h (lock linear region), 0300h (simulate a real-mode interrupt), 0303h (allocate a real-mode callback), and the rule that a hardware interrupt handler and everything it touches must be locked.
- The CWSDPMI documentation (`csdpmi7b.zip`) for the host's behaviour: a hardware interrupt reaches a handler set with 0205h on a stack of the host's own.
- Ralf Brown's Interrupt List for INT 33h (functions 0000h to 0004h, 0007h, 0008h, 000Bh, 0015h, 0021h, 0024h), the BIOS data area (0040:0017, 0040:0018, 0040:001A, 0040:001C, 0040:0097) and INT 9 behaviour.
- The CuteMouse documentation for the wheel API (function 0011h, the wheel counter in BH of function 0003h).
- The DOSBox Staging manual for `AUTOTYPE` and the `[mouse]` section.

## The keyboard: IRQ 1 under DPMI

**The stub.** `rt::irq` generalises the `rt::timer` hook, which never calls Rust, into one stub per IRQ line that does. The host calls the stub through the protected-mode vector with CS holding the program's code selector and a stack, and probably the segment registers, that are the host's own. The stub saves EAX, ECX, EDX, DS and ES; loads the flat data selector, which it reads through CS because DS cannot be trusted yet (CS and DS have the same base, which is what makes the same offsets name the same bytes); **switches to an 8 KiB interrupt stack in the program image** (SS, ESP saved to a static and restored with `lss`), because Rust code assumes a pointer to a local can be dereferenced through DS and the host's stack may not satisfy that; clears the direction flag; and calls `rt_irq_dispatch(line)`. The dispatcher looks the line up, runs the `IrqHandler`, and for a line Desk owns sends the end-of-interrupt with `desk_pic::Pic8259::eoi`; a spurious IRQ 7 or 15 runs no handler and gets `eoi_spurious`. The stub then returns with `iretd`, or for a chained line jumps to the previous handler with `jmp fword ptr`, which acknowledges the interrupt itself. A handler runs with interrupts off, so it is never re-entered and one stack serves every line.

**Locking.** `register` locks the whole image (code, data, bss and both stacks, from `__image_start` to `__image_end`) with DPMI 0600h before it changes the first vector. A handler's state must be in that image: a `Box::leak` would put it in a memory block from 0501h, which is not locked. `rt::irq::Slot` is a `static` that is filled once and hands out the `&'static mut` that `IrqHandler::from_static` takes, which keeps the handler's state in the image.

**Restoring.** The previous protected-mode vector (0204h) is saved at registration, put back by `unregister` with 0205h, and put back by a function on the runtime's clean-up list that runs at exit and in the panic handler before the report is printed. The program registers a report function first, so it runs last and says whether the vector read back with 0204h is the one it saved.

**The end-of-interrupt.** The HAL's contract (`Chain::None`: the platform sends it; `Chain::Previous`: the previous owner does) is followed as written: the handler (`KbdIrq`) never sends one, the dispatcher does. The mock test counts them: one per interrupt, none from the handler. The same dispatcher serves IRQ 0 with `Chain::Previous` in principle; only IRQ 1 was exercised.

**The controller.** Under DOS the BIOS has set the 8042 up: both ports enabled, interrupts on, and the translation bit set so the CPU reads scancode set 1. The program does not touch the controller at all, and the decoder's input is set 1 as it is natively. DOSBox's keyboard model delivered set 1 bytes in every run.

**The BIOS keyboard handler: replaced, not chained, by default.** The program installs with `Chain::None` (the HAL's choice for a line Desk owns outright) and the BIOS handler does not run while it is installed. The effects, and what was decided about each:

| What the BIOS handler does | With ours installed | Decision |
|---|---|---|
| Fills the type-ahead buffer (0040:001E to 003D) | Nothing is added, so keys typed in the program do not appear at the DOS prompt afterwards | The behaviour Desk wants. `INPUT.EXE chain` shows the other case: after the same keys the buffer holds 8 entries, and the program flushes it on exit (`bios_kbd::flush`, head set to tail) because a full buffer makes the BIOS beep |
| Tracks Shift, Ctrl, Alt, the locks and the toggles in 0040:0017 and 0040:0018 | Stale while we run | `bios_kbd::locks` seeds the decoder (`Decoder::with_locks`) from the lock bits at start, so the first Caps Lock press toggles from what the LED shows. At exit `bios_kbd::restore` puts the bytes back as found, with the momentary bits cleared, so a Shift whose release we saw is not left down at the prompt. With the BIOS handler chained, it kept the flags and the LEDs in step itself, so `INPUT.EXE chain` keeps the locks as they are at exit and clears only the held keys and the buffer |
| Drives the keyboard LEDs (the `0xED` command) | LEDs do not follow Caps Lock and Num Lock | Not done. Restoring the flags as found keeps them in agreement with the LEDs, which nothing changed (not so when chained: the BIOS then moves the LEDs, and the flags are left as the BIOS made them). Driving the LEDs needs the keyboard command sequence through the `desk-i8042` driver and the ACK bytes it produces, which the IRQ 1 handler would also receive and the decoder ignores (0xFA and 0xFE); left for later |
| Ctrl+Break: sets the break flag and calls INT 1Bh; Ctrl+C and DOS's check | No Ctrl+Break, and DOS's own break checking never sees a key | Desk handles Ctrl+C and Ctrl+Break as keys (the decoder reports `Break`); a Desk that hung could not be interrupted this way |
| Ctrl+Alt+Del: warm boot | **Lost.** A program that hangs with the handler installed cannot be rebooted from the keyboard | The decoder reports the keys, so the core can implement the SPEC's "asks first, defaults to no"; a hung core has no escape but the reset button. A last-resort chain for this one combination is possible and not built |
| Print Screen (INT 5), SysRq and the INT 15h AH=4Fh keyboard intercept | Lost | Reported as keys (`PrintScreen`, `SysRq`); no TSR hooking the intercept sees them |
| Alt+numeric keypad entry, Pause (stops the BIOS until a key) | Lost | The decoder reports them as keys |

`chain` mode (`Chain::Previous`) exists and works: the BIOS handler reads port 0x60 as well, the DOSBox model returns the same byte to the second read (a real 8042 keeps the byte in its output buffer until the next arrives, which is what the chaining technique relies on, but that is from general knowledge and not tested here), and the BIOS acknowledges the interrupt. It is the right mode for a program that wants Ctrl+Alt+Del, Ctrl+Break and the LEDs to keep working, at the price of the type-ahead buffer filling, a reflection of every interrupt to real mode, and the DOS keyboard state being the BIOS's, not the decoder's.

**The ring and overflow.** The ring is the HAL's `SpscRing<u8, 64>`: 32-bit loads and stores, no compare-and-swap (DOSBox's `cputype = 486` rejects `CMPXCHG`), the producer in the handler and the consumer in the core. A full ring drops the newest byte and counts it; the handler never waits. Dropping bytes can lose a break code, which would leave a key down, and a held Shift or Ctrl would then change every later key, so `DosInput` notices that the dropped count moved, waits until the ring is empty (the hole is then behind the decoder), and resets the decoder's held keys (the locks stay, as with `Decoder::reset`) and counts a recovery. A key that was legitimately held and went down after the lost bytes is forgotten too, and its release then reports an `up` event without a `down`; losing 64 bytes means the core was away for far longer than a held key matters. Shown in both ways: a host test sends Shift down, 40 pairs of 'a', and the lost Shift release, and checks Shift is not stuck; the DOS run types 40 'a' presses with the core stalled for six seconds, so 80 bytes arrive, 16 are dropped, 32 pairs decode, the decoder recovers once and the Escape that follows ends the run normally.

## The mouse: INT 33h

**Calls.** `desk-int33::Mouse` makes each call through `Bios::int(0x33, ...)`, which under DOS is DPMI 0300h (a real-mode software interrupt; the host switches to real mode, runs the driver's handler and returns). Every function the program needs takes registers only, so no transfer buffer is involved:

| Function | Call | Used for |
|---|---|---|
| 0000h | reset and detect: AX = FFFFh and BX = buttons when installed, AX = 0 when not | `Mouse::detect` and `reset`; BX = FFFFh is taken as two buttons (the Microsoft driver's convention), BX = 2 or 3 as is |
| 0001h, 0002h | show, hide the driver's cursor (a counter) | Wrapped; **never called by the program**: the driver's cursor stays hidden after the reset (the counter is at -1), because Desk draws its own |
| 0003h | BX = buttons (bit 0 left, 1 right, 2 middle), CX, DX = position; with the wheel API BH = the wheel counter since the last read, signed | `Mouse::position`, every poll |
| 0004h | set the position to CX, DX | Centring the driver's position at start; the range test |
| 0007h, 0008h | set the horizontal and vertical range | Matching the video mode (below) |
| 000Bh | motion since the last call in mickeys: CX, DX signed 16 bit; the counters are then zero | `Mouse::motion`, every poll: **the source of `dx` and `dy`** |
| 0011h | the wheel API: AX = 574Dh ("WM") when the driver implements it | `Mouse::has_wheel` |
| 0015h | the bytes the driver's state needs: BX | Wrapped, as a check that the extended calls answer |
| 0021h | software reset | Defined, not called |
| 0024h | driver version and type: BH.BL = version, CH = type (1 bus, 2 serial, 3 InPort, 4 PS/2, 5 HP), CL = IRQ | `Mouse::driver_type`; DOSBox's built-in driver answers as unsupported, so the program prints `None` |

The functions that need a buffer (0009h set the cursor shape, 000Ch install a user callback, 0016h and 0017h save and restore the driver's state) take it at ES:DX, and the `Bios` trait's buffer convention puts it at ES:DI, so they cannot be called through it as it stands (see [HAL feedback](#hal-feedback)). None is needed by the polling design.

**No driver.** `Mouse::detect` first reads the INT 33h vector from the real-mode table (0000:00CC, through `Bios::read_low`): zero, or pointing at an `IRET` (0xCF), is the classic "no driver", and the reset call is not made (an unhooked INT 33h would be a spurious call into whatever the table holds). Then function 0000h: an answer other than FFFFh is also no driver. The result is `Error::NoDriver`, the program prints `input: mouse: no mouse driver; keyboard only`, `DosInput::has_mouse` is false, `poll_mouse` returns nothing, and the keyboard works as it always does. DOSBox shows it with `builtin_dos_mouse_driver = off` (checked in the headless run and under Xvfb with the pointer moving).

**Units.** A mickey is the driver's name for one count of the mouse's motion sensor, nominally 1/200 inch. `dx` and `dy` of the `MouseEvent` are the mickeys from function 000Bh, unscaled: the same unit as the PS/2 packet's counts, which are what a PS/2 mouse reports at its default resolution, and what a driver at default sensitivity passes through. (Whether a given real driver's mickey is the device's count, or scaled by the driver's resolution and sensitivity settings, is the driver's business and was not measured on hardware; the Desk core owns acceleration and pointer speed, as it must for the native track, and should treat both as "counts".) In DOSBox with SDL's relative motion, one pixel of pointer motion was exactly one mickey: 40 moves 40 (see [Results](#results)).

**The driver's own position is not used.** The cursor is the clamped sum of the deltas, exactly what the core does with a PS/2 report, so both tracks behave the same at the edges and the driver's coordinates cannot differ from the core's. The driver's position runs at its own ratio: by default 8 mickeys to 8 pixels horizontally and 16 mickeys to 8 pixels vertically, so it moves two mickeys per row. The Xvfb run shows it: 50 mickeys down moved the cursor 50 rows and the driver's position 25. `set_range` and `set_position` are called anyway, with the video mode in mind, because some drivers draw nothing and clamp their reports to the range and because a driver left with a range from another mode misleads whatever runs next:

- VBE 640×480: 0 to 639 and 0 to 479.
- Mode 13h (320×200): 0 to 639 and 0 to 199. The driver and the BIOS treat mode 13h as 640 columns wide with every column doubled, so the horizontal range is the 640-wide virtual one. DOSBox keeps the horizontal position even in this mode (a set to 700 reads back as 638, not 639), and in text mode keeps positions at character cells (632 and 472, rounded down to multiples of eight).
- A negative value passed to function 0004h as `-5` was treated by DOSBox as a large unsigned number and clamped to the maximum, not the minimum; ranges in the specification are signed, but that was not relied on, and the program probes the clamp with a positive out-of-range value.

**Polling or a callback.** The adapter polls once per core iteration: one 000Bh and one 0003h, each a mode-switch pair under DPMI. The alternative, function 000Ch with a DPMI real-mode callback (0303h) that pushes events from the driver's interrupt, was not built: it gives every packet as it arrives and cannot lose a quick click, but it needs the callback's register-structure protocol, a locked handler reached from real mode and a second path to test, and the polling has one advantage that matters here: it is a function that is called, so the same `MouseInput` runs against the model on the host.

The consequences of polling are the difference from the PS/2 path:

- One event per poll with the summed motion, not one per packet. A core that integrates deltas cannot tell. The host test (`motion between polls is summed`) shows two moves reported as one.
- A button pressed and released between two polls is not seen. The Xvfb run found it at once: `xdotool click` (a release milliseconds after the press) produced no button event in DOSBox, a press held for 0.3 s produced one. At a frame rate of tens of hertz a human click lasts several frames, so this is a limit of the spike's tight loop test more than of the design, but the fix is known: the driver's press and release counters (functions 0005h and 0006h, per button) catch them, at three more calls per poll for each of the two, or the callback.

**Wheel.** With the wheel API (function 0011h answers 574Dh) the wheel counter is in BH of function 0003h, cleared by the read. DOSBox's `builtin_dos_mouse_driver_model = wheel` provides it; the sign is positive for a scroll down (X button 5), which is the convention of `MouseEvent::wheel` (positive towards the user). The sign convention of other drivers was not checked and the CuteMouse documentation should be consulted before relying on it. A two or three button model reports `wheel API no` and the program never reports a wheel.

## The event contract shared with the native kernel

The core sees `desk_hal::input::{KeyEvent, MouseEvent}` and the `InputSource` trait on both tracks ([`C-INPUT.md`](C-INPUT.md), "The event contracts"). The differences that remain are in the events' timing, not their shape:

| | Native (`Ps2Input`) | Hosted (`DosInput`) |
|---|---|---|
| Key bytes | IRQ 1 into an `SpscRing<u8, 64>` | the same, by the same `KbdIrq` |
| Decoder | `desk_kbd::Decoder`, starts with no locks | the same, started with the BIOS lock state (`with_locks`) |
| Lock key LEDs | not driven | not driven; the BIOS flags are restored as found |
| Mouse source | IRQ 12 into an `SpscRing<MouseEvent, 32>`, one event per packet | INT 33h polled once per core iteration, one event per poll |
| `dx`, `dy` | the packet's nine-bit counts, `dy` negated | the mickeys, right and down positive |
| `buttons` | bits 0 to 2 of the packet: left, right, middle | bits 0 to 2 of function 0003h: left, right, middle |
| `wheel` | the packet's fourth byte | the wheel API's counter, 0 without it |
| No mouse | the set-up reports it and skips | `has_mouse` is false |
| End-of-interrupt | the platform, after the handler | the platform, after the handler |
| A full ring | drops and counts | drops and counts; the decoder is reset after the drain |

## The program

`INPUT.EXE` (`spikes/b-dos-input/prog/src/main.rs`, about 400 lines):

1. Registers a report function first (it runs last), saves the BIOS keyboard flags and the lock state, finds the mouse driver (`Mouse::detect(DosBios)`) and prints what it found.
2. Sets VBE 640×480×8 with a linear frame buffer through `DosDisplay` (mode 13h with `13`, or when VBE is not offered; none with `text`), loads a seven-colour palette, and sets the driver's range and position.
3. Installs the IRQ 1 handler through `DosIrq` with the ring, the statistics and a `Slot`, replacing the BIOS handler (chaining with `chain`).
4. Loops: poll the keys, print each as `key A down mods=none`; poll the mouse, print each as `mouse dx=3 dy=-2 buttons=LR wheel=0`; integrate the deltas into a cursor position clamped to the screen; draw the three button boxes (amber when pressed), a square for each key event (green down, grey up) and the `SoftCursor`. It ends on the Escape press, or after a timeout (exit 4).
5. Ends through the clean-up list on every path: the mouse driver reset (function 0000h), the video mode back to text, the IRQ 1 vector, the BIOS keyboard state, then the report.

Other arguments: `nomouse`, `stall=N` (do not read for N seconds, to overflow the ring), `timeout=N`, `dump` (the screen and its palette, for `tools/raw2png.py`), `panic` and `leave`. Exit status: 0 Escape, 2 the video mode was not available, 3 a panic, 4 the timeout.

![The screen after the Xvfb run: the cursor at (375, 290), the left button lit, three key squares](images/b-dos-input/screen.png)

This is not a DOSBox rendering: it is the frame buffer read back by the program (`dump`), converted by `tools/raw2png.py`, after the run described under [Results](#results) with the left button held and the keys A and Escape typed.

## How it is tested

Three layers, cheapest first:

**The host mock (always on, in `cargo test` in `hal/`).** `desk-int33` has 16 tests. The wrapper against `Int33Model` (an empty or `IRET` vector, a driver that answers 0, the reset leaving the cursor hidden, the two-button conventions, range and position clamping, motion that a read clears); `MouseInput` (motion, buttons held and released, the wheel, summed motion, a driver that stops answering); and the whole input path: an `I8042Model` with a keyboard, `KbdIrq` registered on a `MockIrq` with `Chain::None`, the 64-byte ring, `DosInput` and the shared decoder, fed the scancode bytes of the keys `AUTOTYPE` sends, compared with `spikes/b-dos-input/expected/keys.txt`, and checked to send exactly one end-of-interrupt per interrupt. The same file is compared with the DOS run's output, so the host mock and the real IRQ 1 path agree on one transcript.

**Headless DOSBox with `AUTOTYPE` (always on, `spikes/b-dos-input/check.sh`, about a minute).** `tools/dosrun.sh` gained `DOS_AUTOTYPE`, which puts DOSBox's `AUTOTYPE` command before the program: `autotype -w 3 -p 0.2 a b capslock c capslock up delete f1 lshift d esc`. Each key is pressed and released through the emulated keyboard controller, so the program reads set 1 bytes from port 0x60 in a real IRQ 1 as a keyboard would send them. It needs no X server, so it runs in CI. It checks: the transcript (VBE, mode 13h and text mode; chained and replaced), the BIOS type-ahead count in each, the overflow run, no mouse driver, and the negative runs listed under [Results](#results).

**Xvfb and `xdotool` (opt-in, `INPUT_EXTRA=1`, `spikes/b-dos-input/check-xvfb.sh`, about twenty seconds).** DOSBox has no scripted pointer: there is no command, key or configuration setting that moves its mouse. `tools/docker/Dockerfile.dos-xvfb` (Ubuntu 24.04, `xvfb`, `xdotool`, the X client libraries) builds `desk-dos-xvfb:1`, cached by tag; the container mounts the repository (DOSBox itself is in `vendor/`), starts Xvfb, runs `tools/dosrun.sh` with `DOS_X11=1` (the x11 video driver, `DISPLAY` kept) and `mouse_capture = onstart` (the pointer is captured when DOSBox starts, so SDL reports relative motion), waits for the window, and plays `mouse.xdotool`: an absolute move, relative moves, each button held for 0.3 s, the wheel, and `a` and Escape through the X keyboard. The program's lines are reduced to totals and compared with the script. This is the one test of the mouse path against a real driver implementation, and it does not run where there is no Docker: the CI image has neither Docker nor X.

The js-dos route was not needed (spike s5's harness was not read for this).

## Results

The key transcript, from DOSBox, identical to what the host mock produces (the final `Escape` release is absent from the DOS run because the program ends on the press):

```text
key A down mods=none
key A up mods=none
key B down mods=none
key B up mods=none
key CapsLock down mods=CAPS
key CapsLock up mods=CAPS
key C down mods=CAPS
key C up mods=CAPS
key CapsLock down mods=none
key CapsLock up mods=none
key Up down mods=none
key Up up mods=none
key Delete down mods=none
key Delete up mods=none
key F1 down mods=none
key F1 up mods=none
key LeftShift down mods=SHIFT
key LeftShift up mods=none
key D down mods=none
key D up mods=none
key Escape down mods=none
```

The mouse transcript from the Xvfb run (`mouse.xdotool`, DOSBox Staging 0.83, the built-in driver as a wheel mouse):

```text
mouse dx=40 dy=0 buttons=- wheel=0
mouse dx=40 dy=0 buttons=- wheel=0
mouse dx=0 dy=30 buttons=- wheel=0
mouse dx=0 dy=30 buttons=- wheel=0
mouse dx=-25 dy=-10 buttons=- wheel=0
mouse dx=0 dy=0 buttons=L wheel=0
mouse dx=0 dy=0 buttons=- wheel=0
mouse dx=0 dy=0 buttons=R wheel=0
mouse dx=0 dy=0 buttons=- wheel=0
mouse dx=0 dy=0 buttons=M wheel=0
mouse dx=0 dy=0 buttons=- wheel=0
mouse dx=0 dy=0 buttons=- wheel=1
mouse dx=0 dy=0 buttons=- wheel=-1
mouse dx=0 dy=0 buttons=L wheel=0
key A down mods=none
key A up mods=none
key Escape down mods=none
```

Totals 55 and 50 (the script moved 40 + 40 − 25 right and 30 + 30 − 10 down), a cursor at (375, 290), a driver position of (375, 265). The check compares totals and the order of the button states, because the same motion may arrive as one event or several.

| Check | Result |
|---|---|
| Keys through the real IRQ 1 path, VBE 640×480×8 | the transcript above; 25 scancode bytes read, none dropped, no decoder recoveries |
| Mode 13h and text mode | the same transcript; the driver's range 0 to 639 by 0 to 199 and its clamp at (638, 199) |
| The IRQ 1 vector | changed while the handler is installed (so the comparison can fail), the original again after exit, panic and `leave` |
| `chain` | the same transcript; the BIOS type-ahead buffer ends with 8 keys, against none when replaced; the flags read back as found |
| Ring overflow | 80 bytes in, 16 dropped, 32 pairs decoded, 1 recovery, the Escape that followed ends the run (65 events) |
| No mouse driver | `input: mouse: no mouse driver; keyboard only`, status 0, the keys as before (headless and under Xvfb with the pointer moving) |
| A key missing (the `d` left out of the typed keys) | 19 events against 21: the comparison fails |
| A wrong expected transcript | rejected |
| No Escape | status 4 after the timeout, everything put back |
| A panic with everything taken over | status 3; the report line `IRQ 1 handler is the original one: true, ours installed: false, video mode 0x03` comes before `RT PANIC` |
| Mouse, Xvfb | motion exactly as scripted, one mickey to the pixel; buttons `-,L,-,R,-,M,-,L`; wheel +1 and −1; the keys through the X keyboard; five runs in a row the same |
| A quick click (`xdotool click`) | no event: press and release inside one poll |

## What DOSBox models and what real hardware may do differently

- **The keyboard controller.** DOSBox's model delivers set 1 bytes, raises IRQ 1 and lets the handler read port 0x60 and the status port. A real 8042 has the output buffer, the input buffer, the source bit, typematic repeat (the keyboard's own: held keys repeat as further make codes, which the decoder reports as further `pressed` events and `AUTOTYPE` does not exercise), the keyboard's own scancode set and the translation bit. The handler tests the source bit as on the native track.
- **The BIOS keyboard handler.** DOSBox implements INT 9 in its own code, not as a real BIOS routine. The replace case and the chain case behave as designed in it. Real BIOS handlers differ in what they do with the shift keys, Ctrl+Break, Pause, Print Screen, SysRq and the keyboard intercept, and in whether they tolerate a second read of port 0x60. **Not tested against any real BIOS.**
- **Mouse drivers.** The built-in DOSBox driver is not the Microsoft driver, CTMOUSE or a vendor driver. Drivers differ in: the button count in BX; whether 0024h is implemented; whether 0021h resets; the mickey definition and the default 8:8 and 8:16 ratios; the sign convention of the wheel; signed or unsigned arguments of 0004h; whether the driver draws its own cursor when the counter is not negative (ours stays hidden); and which functions work in which video modes.
- **PS/2 and USB.** A PS/2 mouse is read by the driver (CTMOUSE and the vendor drivers use the auxiliary port and IRQ 12). A USB mouse works through the BIOS's legacy emulation, which presents an 8042 and an auxiliary port that a driver then reads, or through a USB mouse driver that provides INT 33h itself; some BIOSes emulate the keyboard but not the mouse. Whatever it is, Desk sees INT 33h only, or finds none and runs keyboard-only. A USB keyboard behind legacy emulation reaches port 0x60 and IRQ 1 through system management mode, which makes the interrupt late and the port reads slow; no timing was measured.
- **Serial mice** work through CTMOUSE on a COM port with the same INT 33h; their resolution (a mickey, typically 200 dpi) differs from a PS/2 mouse's default (about 100 dpi at 4 counts per millimetre), so one mickey may be a different distance. The core's pointer speed setting handles it; the figures are from general knowledge.
- **Windows DOS boxes, EMM386 and other DPMI hosts.** Windows 9x provides the mouse driver and virtualises the keyboard; hooking IRQ 1 and reading port 0x60 in a DOS box under Windows 9x generally works for DJGPP programs but is the host's business, and the host delivers hardware interrupts through its virtual interrupt controller. Other hosts may call a protected-mode handler with interrupts enabled, on a different stack, or may not accept 0205h for hardware vectors. `rt::irq` assumes CWSDPMI's behaviour (interrupts off in the handler) and `DosBios` and the stub were tested under it only.
- **Timing.** `cpu_cycles = max` in the runner and an interpreter core make the program fast and the keys slow relative to it; on a 486 the polling loop has to share the CPU with a core that draws.
- **The pointer in DOSBox under Xvfb.** `mouse_capture = onstart` and SDL's relative mode gave exact counts. On a desktop with pointer acceleration the counts would differ (`mouse_raw_input = on` bypasses it).

## What the native platform must keep in step

- The same ring type, handler and decoder (already shared). The native kernel starts the decoder with no locks and drives no LEDs; if either changes (LEDs through `0xED`), the DOS platform should follow, with the BIOS flags written back as the LEDs are changed.
- `MouseEvent` per packet versus per poll: the core must not depend on a mouse event's timing or count, only the sum of the deltas and the button state at the end. A native platform that later polls (a USB controller) fits the same shape.
- `InputSource::has_mouse` has to be implemented wherever a mouse can be missing.
- The core's Ctrl+Alt+Del handling is the only way out of a hung core under DOS when the BIOS handler is replaced.

## HAL feedback

Changes made to the HAL in this branch, small and compatible:

- `InputSource::has_mouse` (default `true`): the core could not tell a machine with no mouse driver from a mouse that is not moving.
- `Decoder::with_locks` in `desk-kbd`: a platform whose firmware owns the lock state starts the decoder from it. Without it the first Caps Lock press on a machine with the LED already on would turn the decoder's lock on while the LED went off.
- `desk-int33`, `Int33Model`, `transcript`: the DOS mouse adapter the HAL document said spike B would add, in the shared crate (it needs only `Bios`), and one text form of events so the host test and the DOS run compare against one file.
- `SoftCursor` in `desk-vga`.

Recommendations, not changes:

- **`Bios::int`'s buffer is at ES:DI only.** INT 33h's buffer functions (0009h, 000Ch, 0016h, 0017h) and some INT 10h functions take it at ES:DX. The trait could document that the platform sets only ES and offset 0 and the caller sets the register that names the offset (VBE then sets DI itself), or take the register as a parameter. Not changed here: it would touch the VBE layer and `DosBios` from spike A, and no call needed it.
- **A critical section.** `InterruptController::mask` and `unmask` are read-modify-write and want interrupts off; the HAL has no way to say so, and a DPMI client cannot cheaply turn interrupts off. `DosIrq::register` reads the mask and unmasks only if the line is masked, which for IRQ 1 never writes; a platform `critical(|| ...)` would let a driver unmask safely.
- **A way to ask what is installed.** The restore check needed `DosIrq::current(irq)`, the vector as it reads now, to prove the original handler is back. `IrqRegistry` has no such query; it would be useful for the native kernel's tests too.
- **`StaticCell`.** Spike C asked for it; `rt::irq::Slot` is it, written for DPMI because it must be in the locked image. It has nothing DOS-specific and could move to `desk-hal`.
- **The cleanup list has eight slots.** The program uses five (the report, the mouse, the BIOS keyboard state, the IRQ vector and the display); a program that also hooks IRQ 0 and the PIT rate is at seven.
- **Where `HAL.md` said the DOS stub "switches to a locked interrupt stack", it is built and works, but it was not shown to be necessary.** The test would be a handler that dereferences a pointer to a local with the host's stack in use; not tried.
- **Press and release counters (0005h, 0006h) or the callback (000Ch)** if a click shorter than a poll matters. The `MouseEvent` has the state, not the changes, so a press and release inside one poll cannot be represented without synthesising two events.
- **`MouseEvent` from a poll sums deltas.** If the core ever wants per-packet events (gesture recognition, say), the DOS platform cannot give them without the callback.

## Commands

```bash
tools/setup.sh                                    # once
spikes/b-dos-input/build.sh                       # dist/b/INPUT.EXE
spikes/b-dos-input/check.sh                       # headless, AUTOTYPE: the keyboard path and the negative runs
INPUT_EXTRA=1 tools/check.sh                      # also the Xvfb mouse run (needs Docker)
spikes/b-dos-input/check-xvfb.sh                  # the Xvfb run alone; builds desk-dos-xvfb:1 the first time

# By hand
DOS_AUTOTYPE='-w 3 -p 0.2 a b esc' tools/dosrun.sh dist/b INPUT.EXE text
DOS_CONF=$'[mouse]\nbuiltin_dos_mouse_driver = off' tools/dosrun.sh dist/b INPUT.EXE text
python3 -I tools/raw2png.py dist/b/SCREEN.RAW dist/b/SCREEN.PAL 640 480 screen.png
```

## What was and was not verified

Verified, on the development machine:

- `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` in `hal/` (173 tests in the workspace, 16 of them new for `desk-int33`, 3 for the cursor and 1 for `with_locks`), the clippy build for the kernel's target and the `cargo check` for the DOS target; clippy and format for the spike workspaces.
- `tools/check.sh` end to end, including `spikes/b-dos-input/check.sh` and the existing d0, a-vga-vbe and native checks.
- `spikes/b-dos-input/check-xvfb.sh` in Docker (Ubuntu 24.04 with Xvfb and `xdotool`, DOSBox Staging 0.83 from `vendor/`).

Not verified:

- Real hardware of any kind: a real 8042, a real BIOS keyboard handler, a PS/2, serial or USB mouse, a real mouse driver (CTMOUSE, a vendor driver). 86Box, QEMU and Bochs were not used for this spike.
- DPMI hosts other than CWSDPMI; Windows DOS boxes; EMM386; whether the interrupt stack switch is needed.
- The real-mode callback (000Ch and 0303h), the functions that need a buffer (0009h, 0016h, 0017h), the press and release counters (0005h, 0006h), mouse sensitivity and resolution calls.
- A held key's typematic repeat (`AUTOTYPE` does not repeat), the LEDs, Ctrl+Break and Ctrl+Alt+Del with the BIOS handler (not exercised: replaced means lost, which was not tested on a BIOS that does anything with them).
- The wheel sign on any driver but DOSBox's built-in one.
- The run under the CI image: it has no Docker, so the Xvfb check is opt-in and ran only here.
- The sources listed under [Method](#method) were not re-read in this spike.
