---
id: input
n: 11
title: Typing in the dark
date: 2026-10-23
summary: The native kernel learns to read a keyboard and a mouse by talking to the 8042 controller itself, decoding what it hears and handing it to the main loop through a ring that no lock guards.
prs: [10]
---
A kernel with no BIOS cannot call `INT 16h` and ask for a key. It has to do what the BIOS did: talk to the keyboard controller, a small Intel 8042 on two ports, `0x60` for data and `0x64` for status and commands. Pull request #10 (branch `spike/c-kernel-input`, stacked on the HAL of chapter 10) writes that as shared drivers, `desk-i8042` and `desk-kbd`, plus a model of the controller for host tests, and then puts them in the kernel and checks the result in QEMU by typing at it.

**The handshake.** The controller will not take a byte while its input buffer is full, and it will not give one until its output buffer says it has one, so every exchange is a bounded wait on a status bit. The set-up sequence disables both ports, flushes stray bytes, reads and rewrites the configuration byte (interrupts off, translation on), runs the controller's self-test and expects `0x55`, tests each port, and enables them. Interrupts are switched on last, after the handlers exist. Every wait has a bound (100,000 status reads for an ordinary reply, a million for a keyboard's self-test), and a floating bus that reads all ones makes the first command time out instead of hanging. The keyboard is then stopped, reset, expected to answer `FA` then `AA`, and started; the mouse goes through the same dance behind a `D4` prefix.

**Scancode set 1.** A keyboard speaks scancode set 2 after a reset, and the controller translates it to set 1 when one bit of its configuration byte is set, so the CPU reads set 1, which is what the decoder understands. A make code is the key's number, a break code the same with bit 7 set, `E0` prefixes the extra keys of the 101-key layout, and Pause sends a six-byte sequence on press and nothing on release. The fake shifts that a keyboard wraps around Print Screen and the cursor block are swallowed so they cannot disturb the real shift state. The decoder is a pure function, `Decoder::feed(byte) -> Option<KeyEvent>`, which is why it can be tested on a laptop.

**The IntelliMouse.** A mouse sends three bytes per movement: buttons and sign bits, then the low eight bits of the horizontal and vertical counts, which are nine-bit two's-complement numbers with their signs in the first byte. The driver asks for the wheel by setting the sample rate to 200, 100 and 80 in turn and then reading the device ID; an answer of 3 means a wheel and a fourth byte. The driver negates the vertical count because the device counts upward as positive and the screen counts downward. A packet with an overflow bit is dropped and counted, and a byte that cannot be the first of a packet is dropped until one can, which recovers from a lost byte within a few packets (a host test, since QEMU never loses one).

**The ring.** The interrupt handlers do the least possible. IRQ 1 reads port `60h` and pushes the raw byte into a ring of 64 bytes; IRQ 12 assembles a packet and pushes one `MouseEvent` into a ring of 32. Both are `SpscRing`: a power-of-two buffer where only the producer ever writes `head` and only the consumer ever writes `tail`, with indices that count up forever and are masked to find the slot, so there is no compare-and-swap, no lock and nothing for an interrupt to deadlock on. A full ring drops the new item and counts it. `split` hands out the two ends once, so a second producer does not compile. The handler tests the status register's source bit so that IRQ 1 never takes a mouse byte and the reverse, which the design's contract had missed.

Then the check that matters. The harness `tools/native-inject.py` waits for the kernel to print `input: ready`, and plays commands into QEMU's monitor, `mouse_move`, `mouse_button` and `sendkey`, from `tests/native/input.inject`. The kernel decodes what arrives and prints a line for each event, and CI compares the transcript with `tests/native/expected-input.txt`. The machine below replays that recorded run, one injected command at a time.

<!-- explainer input -->

Two things in the harness are worth a line. Without short pauses after each mouse command QEMU folded a press and release that arrived before the guest had read the previous packet, giving six to eight events across runs; the kernel's drop and resync counters stayed at zero, so the folding was on QEMU's side. With pauses, eight runs in a row gave the same eight events. And the negative checks matter as much as the positive one: the same kernel with the `ctrl-alt-delete` line removed still exits cleanly, but its transcript is rejected by the comparison; with IRQ 1 left masked, the mouse events arrive, no key does, and the kernel panics after five seconds with the expected message.

What QEMU is not is a real keyboard. The pull request lists what a real machine may do differently: USB legacy emulation that is incomplete, controllers whose translation bit does not stick, mice that omit an acknowledgement, a keyboard self-test that takes up to a second, an edge-triggered line that loses an edge while masked, and laptop touchpads that want vendor sequences. None of it was tried.

> **From the stacks — The keyboard controller that opened the gate.** The Intel 8042 came to the PC with the IBM PC AT, where it replaced the original PC's 8255 peripheral chip at ports `60h` to `63h`. It also controlled the A20 line gate for the AT's 80286, which software could command to reset the processor. As the story is usually told, a spare pin on the keyboard controller was the cheapest place to put the gate that lets a PC address memory above one mebibyte. That is why the Desk's driver promises, and a host test asserts, never to touch the 8042's output port.
> Source: [Intel 8042, Wikipedia](https://en.wikipedia.org/wiki/Intel_8042)

## The shelves

### Catalogued
- A headless QEMU run (`-cpu 486`) that injects `a`, `Shift+A`, `1`, `Shift+1`, Enter, keypad Enter, Up, Ctrl+Alt+Delete and Escape and eight mouse movements and clicks prints 26 key events and 8 mouse events that equal `tests/native/expected-input.txt`, with nothing dropped. CI is green on `{{pr10.head}}` of the open pull request. Not merged yet.
- The 46 host tests of the controller driver and the 22 of the decoder cover the wire sequences, every timeout path, packet decoding, the whole interrupt-to-ring path, and a masked IRQ 1 delivering nothing; the three negative runs (a removed key, a masked line, an idle input) behave as described.
- The replay above refuses to build unless a model of the decoder turns the derived bytes back into exactly the recorded lines.

### Uncatalogued
- A real 8042, USB legacy emulation, a laptop touchpad, a wheel mouse, a real keyboard's timing, key repeat and the lock-key lights: none was tried.
- Mouse wheel motion: QEMU's monitor cannot send it, so the wheel path is tested on the host only, though the wheel negotiation runs in QEMU with every packet.
- Resynchronisation after a real lost byte: only the host tests exercise it.
- The scancode and packet bytes in the replay are derived from the decoder's tables and the packet format, because the run does not log them; the ring indices assume the loop drains after each command.
- 86Box and Bochs were not used.

### Wish list
- Clamping rather than dropping a mouse packet that overflows, if a fast flick turns out to need it.
- A way for the core's loop to ask the input source whether anything is waiting, so it can sleep with `hlt` safely.
- A `StaticCell`-style helper so a kernel can take input before it has a heap.
- Driving the keyboard's lock lights.

```figures
id: input-replay
kind: explainer
track: native
title: The injected input run, replayed
alt: A step-through of the recorded keyboard and mouse run on the native kernel: each injected key or mouse command, the bytes in the ring, and the decoded event lines.
caption: Keys and mouse events side by side with the state of the ring.
at: explainer input
thumb: steps
```
