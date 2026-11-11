---
id: m-hal
n: M9
part: IV
title: The HAL
summary: The contract crate desk-hal read from the code: port I/O and where the unsafe sits, memory-mapped regions, interrupts and the ring, time, video and input types, errors, no_std and features, the alternatives rejected, the mock platform, and the migration steps done and not done.
date: 2026-10-23
reviewed: 44f0265
watches: [hal/hal/, hal/platform/mock/, hal/README.md, hal/clippy.toml, docs/HAL.md, kernel/src/hal.rs, spikes/d0-rust-dos/rt/src/hal.rs, spikes/d0-rust-dos/rt/src/irq.rs]
log: [hal]
anatomy: [desk:hal, dos:rthal]
---
## What it is

The [HAL](term:hal) (hardware abstraction layer) is the one small crate, `desk-hal`, that every driver is written against and every [platform](term:platform) implements. Nothing in it touches a chip. It holds traits (`PortIo`, `InterruptController`, `IrqRegistry`, `Clock`, `Delay`, `Display`, `InputSource`, `Bios`, `DebugOut`), a few plain types (`Irq`, `Ticks`, `Rgb6`, `KeyEvent`, `MouseEvent`, `HalError`), and three small tools that make interrupt-safe code possible without locks: a bounds-checked view of device memory (`MmioRegion`), a lock-free queue (`SpscRing`) and a counter (`TickCounter`). The rule it exists to enforce is the one the [log chapter](chapter:hal) states: a line of device logic exists once, in a driver, and a platform contains only what genuinely differs.

On the map of the system ([M1](chapter:m-map)) it is the `hal` box of the native track's anatomy page and, on the hosted track, the DOS platform's side of it, the `rthal` box (`DosBios` and `DosDisplay`). The rest of the layer is taught elsewhere: the drivers that use the traits are in [M10](chapter:m-sysdrivers) (serial port, interrupt controllers, timer), [M11](chapter:m-input) (keyboard and mouse) and [M12](chapter:m-graphics) (VGA and VBE); the hardware they drive is in [M3](chapter:m-devices); the native kernel's half of the interrupt contract is in [M6](chapter:m-interrupts) and its BIOS half in [M8](chapter:m-bioscalls). This chapter is the contract between them.

After it you should be able to read any file in `hal/hal/src/` and say what it promises and to whom, say why port methods are safe to call while making the bus is not, explain why a handler is a function pointer and not a closure, write a host test for a driver against the mock bus, and tell which parts of the design in `docs/HAL.md` are built, which are used by a real platform, and which are still only a sketch. Where the document and the code disagree, this chapter follows the code and says so in the Revisions.

## How it works

### Four kinds of code and one rule

`docs/HAL.md` sorts every piece of code into one of four kinds, and the sort is what the crate layout enforces.

| Kind | What it is | May call | Must not call | Lives in |
|---|---|---|---|---|
| HAL | The traits and small shared types | `core` | Anything platform-specific | `hal/hal/` (`desk-hal`) |
| Driver | Device logic written once: register sequences, timing rules, decoding, interrupt-side work | `desk-hal`, `core` | DOS, BIOS, DPMI, kernel internals, `alloc` in interrupt code | `hal/drivers/*` |
| Platform | The HAL implemented for one environment | Its environment, drivers, `desk-hal` | The Desk core | `kernel/src/hal.rs`, `spikes/d0-rust-dos/rt/src/hal.rs` and `irq.rs`, `hal/platform/mock/` |
| Desk core | The OS-independent shell | The HAL's display and input surfaces, `alloc` | Ports, DOS, DJGPP, kernel internals, a driver's registers | `shell/` |

<!-- figure hal-layers -->

The crate has two edges. The lower edge is what drivers use: ports, interrupts, memory-mapped regions, a tick source. The upper edge is what the core uses: a `Display`, an `InputSource`, a `Clock`. They share a crate because they share types such as `Irq` and `HalError`. `hal/hal/src/lib.rs` lists one module for each surface (`port`, `input`, `irq`, `time`, `ring`, `mmio`, `error`, `video`, `bios`, and `x86` on x86 targets only) and re-exports the names callers need at the crate root.

### Port I/O, and where the unsafe sits

A [port](term:port-io) access is an `in` or `out` instruction that the compiler must not remove, merge or reorder. [`PortIo`](sym:hal/hal/src/port.rs::PortIo) is six methods, `inb`, `outb`, `inw`, `outw`, `inl` and `outl`, plus a provided one, `io_wait`, which writes zero to the unused POST diagnostic port `0x80` (`POST_PORT`) and so costs about one microsecond on the [ISA bus](term:isa-bus). Two properties of the trait are choices worth seeing.

First, the methods are safe. A value that implements `PortIo`, a [port bus](term:port-bus), is the capability to touch hardware, and the dangerous step is making one. The only real implementation, [`RawPorts`](sym:hal/hal/src/x86.rs::RawPorts), is a zero-sized `Copy` struct with a private field, so it can be made only by `RawPorts::new`, which is an `unsafe fn` whose doc comment says what the caller promises: that it runs where `in` and `out` are permitted (ring 0, or a DPMI client with I/O permission) and that it takes responsibility for which drivers get the bus. Every `unsafe` block in `x86.rs` carries a one-line `SAFETY:` comment that points back to that contract. The alternative, an `unsafe fn outb`, would put an `unsafe` block with no local justification on every register write of every driver; the document rejects it for exactly that reason (`docs/HAL.md`, "Decisions" 3 and "Alternatives considered"). The result is visible in a search: the serial, PIC and PIT drivers contain the word `unsafe` zero times (see the commands at the end).

Second, the methods take `&self`, not `&mut self`. Several drivers can then share one bus, and `RawPorts` is `Copy`, so a driver can own its own copy. There is also a blanket `impl PortIo for &P`, so a driver generic over `P: PortIo` can be handed `&bus` while another driver holds the same bus. The kernel's `PORTS` is one `static RawPorts` made once, with a `SAFETY:` line saying the kernel runs in ring 0, and the PIC and the UART are `static`s built from copies of it (the `PitClock` needs no bus: the timer is programmed through `Pit::new(PORTS)` when it is needed).

The trait's header comment also states the ordering rule. Each access is performed exactly once, in program order; the `asm!` blocks in `x86.rs` omit `nomem` on purpose, so the compiler treats each as possibly reading and writing memory and does not move ordinary loads and stores across it. A driver that fills a [DMA](term:dma) buffer and then starts the transfer with a port write therefore needs no explicit fence on this platform, and a platform that cannot promise that must say so. `x86` is compiled only where `target_arch = "x86"`, so a host build has no port instructions and a host test cannot touch hardware by accident.

### Memory-mapped regions

Some devices are addresses, not ports: the VGA window at `0xA0000` and the linear frame buffer. [`MmioRegion`](sym:hal/hal/src/mmio.rs::MmioRegion) is a raw pointer and a length. Its constructor `MmioRegion::new` is the `unsafe` step (the region must stay mapped and no Rust reference may cover it), and everything after is safe and checked. `read8`, `write8`, `read32` and `write32` fail with `HalError::InvalidArgument` if the access falls outside the length or, for 32-bit accesses, is not aligned to 4 (`check` adds the offset and the size with `checked_add`, so an offset of `usize::MAX` is refused rather than wrapping, and tests the alignment of the address, base plus offset, not of the offset alone). All stores are volatile. `write_bytes` copies one volatile byte at a time and is the reference a faster path must match; `fill8` writes the unaligned head and tail bytewise and the aligned middle a 32-bit word at a time, which cuts the bus writes to a quarter and was added by the native VGA spike. The platform makes the region (`kernel/src/gfx.rs` builds one over `0xA0000`, 64 KiB; `rt::hal::DosDisplay` builds one over the same window through DPMI, or over a mapped [linear frame buffer](term:lfb)).

### Interrupts and the ring

An interrupt line is an [`Irq`](sym:hal/hal/src/irq.rs::Irq), a newtype over 0 to 15 with named constants (`Irq::TIMER` is 0, `KEYBOARD` 1, `CASCADE` 2, `COM1` 4, `MOUSE` 12 and so on) and a constructor, `Irq::new`, that returns `InvalidArgument` for 16 and above. `is_slave` is true for 8 to 15. A handler is not a closure. [`IrqHandler`](sym:hal/hal/src/irq.rs::IrqHandler) is two words, an `unsafe extern "C" fn(*mut ())` and a context pointer, because an assembly stub must call it from an interrupt with nothing but what it stored at registration, and under a DPMI host everything the handler reaches must sit in locked memory. Two safe constructors cover driver code:

- `IrqHandler::from_static` takes a `&'static mut T` where `T: IrqService + Send`. The mutable reference moves to the handler for good, so nothing else can alias the state; whatever the handler shares with the core is inside it behind a shared reference (an atomic, a ring's producer).
- `IrqHandler::from_shared` takes a `&'static T` where `T: SharedIrqService`, state that is all atomics, such as a tick counter. It can be a plain `static`.

`from_raw` is the unsafe escape for a platform's own handlers, and `call` is unsafe with a one-caller-at-a-time contract. Registration is the trait [`IrqRegistry`](sym:hal/hal/src/irq.rs::IrqRegistry), with `register(irq, handler, chain)` and `unregister`; `Chain::None` means the platform sends the end-of-interrupt and `Chain::Previous` means the previous owner of the vector runs next and sends it. The controller side is [`InterruptController`](sym:hal/hal/src/irq.rs::InterruptController): `mask`, `unmask`, `is_masked`, `eoi`, and two provided methods for the [spurious](term:spurious-interrupt) case (`is_spurious` defaults to false, `eoi_spurious` to nothing). Two rules in its doc comments carry the design: drivers never send the [EOI](term:eoi), the platform does after the handler returns, because under DOS a chained BIOS handler sends it and a second one would acknowledge a different interrupt; and `mask` and `unmask` are a read-modify-write of a register every line shares, so callers outside interrupt context hold the platform's critical section, which a driver cannot enter by design.

The header of `irq.rs` lists the rules for what a handler may do: no allocation, no lock the core might hold, bounded work, no DOS or BIOS call, no floating point, and anything shared with the core goes through an atomic or an `SpscRing`. The platform guarantees it is never re-entered, because it runs with interrupts off.

The hand-off is [`SpscRing<T, N>`](sym:hal/hal/src/ring.rs::SpscRing), a single-producer, single-consumer queue. The producer alone writes `head`, the consumer alone writes `tail`, and the indices run freely and wrap (`N` must be a power of two, checked by an inline `const` assertion in `new`, so a bad `N` fails the build wherever the ring is made; the slot is `index & (N - 1)` and the length `head - tail` with wrapping subtraction). There is no lock and no compare-and-swap. `split` returns the two ends once and `None` afterwards, using `swap`, so a second producer is a type error instead of a race. A full ring does not block: `push` returns the item as an `Err`, and a `dropped` counter, written only by the producer with a plain load and store, counts the loss. The kernel's `input.rs` instantiates two, `SpscRing<u8, 64>` for keyboard bytes and `SpscRing<MouseEvent, 32>` for mouse packets ([M11](chapter:m-input)).

The reason there is no compare-and-swap anywhere is in `hal/clippy.toml`. DOSBox with `cputype = 486` rejects `CMPXCHG`, so a CI that runs the hosted track would fail on code a real 486 would run; and a 486 has no 64-bit atomics. The file lists ten banned methods of `core::sync::atomic::Atomic` (`compare_exchange`, `compare_exchange_weak`, `fetch_update`, `fetch_and`, `fetch_or`, `fetch_xor`, `fetch_nand`, `fetch_not`, `fetch_max`, `fetch_min`) and `clippy -D warnings` refuses them. What is allowed is `load`, `store`, `swap` (`xchg`) and `fetch_add` (`lock xadd`), which is exactly what `ring.rs` and `time.rs` use.

### Time

A 486 has no time-stamp counter, so the clock is an interrupt counted in a 32-bit word. [`TickCounter`](sym:hal/hal/src/time.rs::TickCounter) holds an `AtomicU32`; `tick` is a `fetch_add` and `now` an acquire load. It implements `SharedIrqService`, so `IrqHandler::from_shared(&COUNTER)` is the whole timer handler. [`Ticks`](sym:hal/hal/src/time.rs::Ticks) is a `u32` newtype whose `since` uses wrapping subtraction, so an interval is right across one wrap of the counter. The trait [`Clock`](sym:hal/hal/src/time.rs::Clock) has `now` and `millihertz`, the rate in thousandths of a hertz so that the BIOS's 18.2 Hz (18,207) and the PIT's real "100 Hz" (99,998) are both exact enough; `millis_since` is provided and uses 64-bit arithmetic, which is plain code on a 486 (only 64-bit atomics are missing), rounding down and saturating at `u32::MAX`. [`Delay`](sym:hal/hal/src/time.rs::Delay) is the calibrated busy wait for device timing, and no platform implements it yet. The header of `time.rs` works out the wrap: about 497 days at 100 Hz and 49 days at 1 kHz.

### Video, input, the BIOS and errors

The video types are plain data. `Rgb6` is a [DAC](term:dac) colour, six bits a channel; `Rgb6::new` masks to six bits, `from_rgb8` is `(v * 63 + 127) / 255` on each channel and `to_rgb8` repeats the top two bits into the bottom two (63 gives 255). `PixelLayout` is `Indexed8`, `Planar4`, `Rgb565` or `Xrgb8888`; `ModeInfo` carries an id, a size, a layout and whether the buffer is linear; `FrameBuffer` pairs a mode with its `MmioRegion`, a pitch and a physical address. The `Display` trait is `modes`, `set_mode` (which returns the frame buffer), `restore_text_mode`, `set_palette` and `palette` (both fallible in the code), and `wait_vretrace`. The core sees these and never INT 10h, a VBE information block or a DPMI selector ([M12](chapter:m-graphics)).

The input types are `Scancode` (a `u8`), `Modifiers` (a bit set of shift, control, alt, GUI and the three locks), `KeyCode` (a Desk enum that does not depend on a [scancode](term:scancode) set), `KeyEvent { code, pressed, modifiers }` and `MouseEvent { dx, dy, buttons, wheel }` where positive `dy` is down the screen. The `InputSource` trait the core drains is `poll_key`, `poll_mouse` and a provided `has_mouse` that is true unless a hosted platform finds no mouse driver.

`Bios` is the one trait that exists for firmware: `int` runs a real-mode software interrupt with a `BiosRegs` and an optional buffer of at most `BIOS_BUFFER_MAX` (512) bytes that the platform copies below 1 MiB and back, and `read_low` reads the first megabyte. Under DOS it is the DPMI call; the native kernel implements it with the real-mode trampoline of [M8](chapter:m-bioscalls) (`impl Bios for NativeBios` in `kernel/src/bios.rs`).

Every fallible operation returns [`HalError`](sym:hal/hal/src/error.rs::HalError), a `Copy` enum of seven cases: `Unsupported`, `NoDevice`, `Timeout`, `InvalidArgument`, `Busy`, `OutOfMemory` and `Platform(u16)` for a passed-through DPMI code. It has `Display`, and `std::error::Error` only on a host build. Device classes that need more would add their own error type; none has yet.

<!-- explainer m-crates -->

### no_std and features

`desk-hal` has no dependencies and one feature. Its `lib.rs` begins `#![cfg_attr(not(any(test, feature = "std")), no_std)]`, so a kernel or DOS build is `no_std` and a test build has `std`. The `std` feature only adds `impl std::error::Error for HalError`, "never on for a DOS or kernel build" in the `Cargo.toml` comment. No driver uses `alloc`. The mock platform does (`Vec`, `VecDeque`, `Rc`), which is why it too builds for both targets. There are no features to choose a platform: a binary picks its platform by depending on it.

### The mock platform

The [mock platform](term:mock-platform) in `hal/platform/mock/` is the third platform, and the reason a driver can be tested without an emulator. Its centre is [`MockPortBus`](sym:hal/platform/mock/src/bus.rs::MockPortBus), a `PortIo` that records every access, and answers reads in a fixed order of precedence: first a strict script, if one is set; then values queued for that port; then the device model attached to that port range; and last a floating bus, all ones, as an empty ISA slot reads. Writes to the POST port are not logged unless `record_post_delays(true)` is called, since they are timing and not behaviour. A script is a list of [`Access`](sym:hal/platform/mock/src/bus.rs::Access) values (`Access::outb(0x43, 0x34)`, `Access::inb(0x40, 0x9C)`); the bus panics with the position and both accesses when one differs, or when the driver makes an access past the end, and `script_done` says whether all were consumed. The tests of the bus itself include two `#[should_panic]` cases for the wrong access and an extra access.

Beside the bus are models of chips, just enough to see what a driver programs: `Uart16550` (divisor latch, line and modem control, loopback, a transmitter that can be held busy), `Pit8254` (mode, access and reload per channel; its counter does not run, a test sets `count`) and `I8042Model` with a keyboard and a mouse in `ps2.rs`, `VgaModel`, `MockBios`, `Int33Model`; and `MockIrq`, a registry and controller whose `raise` delivers an interrupt as a platform's dispatch does (nothing if the line is masked or empty, otherwise the handler runs and then the EOI is recorded or the interrupt is passed to a previous owner). `MockClock` is a clock a test advances by hand and `MockDelay` adds up the time asked for instead of waiting. The mock is single-threaded, with `RefCell` behind the `&self` methods the traits take.

The models are the author's reading of the data sheets. None has been compared with a real 16550, 8254 or 8259, and `Pit8254` models one detail the author thought a driver could get wrong (the low and high bytes of a latched count are read in order, and only a control word resets the sequence), not the chip as a whole. A test that passes against the mock shows the driver does what the author expected of the chip; it is the emulator run, and eventually a real machine, that shows the chip does it.

## Why it is this way

- **Two edges in one crate.** `docs/HAL.md` ("Decisions" 1) first called the lower edge "the port-I/O and IRQ abstraction" and the upper edge "the HAL" and kept them together so that the types they share have one home. The code agrees: `Irq`, `HalError`, `MmioRegion` and `SpscRing` are used by drivers and by `Display` alike.
- **Generic drivers, not trait objects.** A driver is `Serial16550<P: PortIo>`. The document rejects `&dyn PortIo` because the mock and the real bus would then cost an indirect call per port access on a 486, and because a binary has one platform anyway ("Decisions" 2; "Alternatives considered and rejected"). The mock is simply another `P`.
- The capability is the value, and the `unsafe` sits on holding the bus. "Decisions" 3, and the rejected alternative "Unsafe port methods". The same pattern guards memory (`MmioRegion::new`) and interrupts (`IrqHandler::from_raw`). "The unsafe policy" adds that drivers contain none, and the core only where it receives a raw frame buffer.
- **Handlers are a function pointer and a context pointer.** "Decisions" 4. "Closures as interrupt handlers" is rejected outright: a closure's environment has no fixed address or size, cannot be named from an assembly stub, and may allocate.
- **No compare-and-swap.** "Decisions" 5, from the DOSBox finding recorded in `docs/D0-RUST-DOS.md`. The document reports that a throwaway crate instantiating the ring, the clock and the three drivers for the kernel target emitted no `cmpxchg`; that check was not rerun for this chapter.
- No shared drivers, global functions, `embedded-hal`. The three other alternatives rejected in "The four kinds of code": one HAL per track (every driver written twice and tested only in an emulator), free functions over a global bus as the kernel had (untestable without link-time substitution), and `embedded-hal` (its traits model GPIO, SPI and I²C, not ISA port I/O, IRQ lines and DMA).
- **Platforms live with their environment.** "Decisions" 6 and "Why the real platforms are not crates in `hal/`": a DOS platform crate would only re-export `rt` and would not build for the host, which would split the workspace's `cargo test`. That is why `kernel/src/hal.rs` and `rt/src/hal.rs` are where they are.
- **A known document-versus-code gap.** `docs/HAL.md` begins with a status that says nothing runs in the kernel or under DOS and that "the kernel and `rt` are unchanged". The code is later than that paragraph (the kernel boots on the crates; see the next section). The document's own "Crate layout" says so, and the Revisions list the other places where it is out of date.

### The migration, as the code shows it now

`docs/HAL.md` ("Migration plan") lists steps 0 to 5. The state below is read from the repository, not from the plan.

| Step | What the plan says | What the code shows |
|---|---|---|
| 0 | The workspace, traits, ring, region, three drivers, mock, tests and cross builds | Done. `hal/` is a workspace of nine members; `tools/check.sh` runs `fmt`, `clippy -D warnings` and `cargo test` there, and the two cross builds |
| 1 | The native kernel uses the shared drivers | Done. `kernel/Cargo.toml` depends on `desk-hal`, `desk-serial`, `desk-pit`, `desk-pic` and the rest; `kernel/src/port.rs` no longer exists; `PORTS`, `PIC`, `SERIAL`, `CLOCK` and `IRQS` are in `kernel/src/hal.rs`. Not done within it: `vga.rs` has not moved to a `desk-vgatext` over an `MmioRegion` (that crate does not exist; the plan called it optional) |
| 2 | The DOS runtime becomes the hosted platform | Partly. `rt::hal` has `DosBios` and `DosDisplay`, and `rt::irq` has `DosIrq` (a stub per line, `Chain::Previous`, the clean-up list, the stack switch). Not done: `rt` is still `spikes/d0-rust-dos/rt`, not a top-level `rt/`; `rt::timer` is still its own hook and nothing under DOS uses `desk-pit` or `PitClock`; `desk-serial` is not used by `rt`; no `PhysMem`, `WallClock` or `DebugOut` over standard output |
| 3 | The hosted skeleton moves to the HAL | Not done. `core/` is unchanged, and the shell lives in `shell/` |
| 4 | New drivers in `hal/drivers/` | Partly. `desk-vga`, `desk-kbd`, `desk-i8042` and `desk-int33` exist; `desk-dma8237`, `desk-opl`, `desk-sb`, `desk-ata`, `desk-fdc` and `desk-rtc` do not |
| 5 | The core runs on both platforms through `Platform` | Not done. There is no `Platform` trait in the code, and no `PhysMem` or `BlockDevice` |

The pieces the document marks as sketch and the code lacks are the physical-memory surface (`PhysMem`, `DmaBuffer`), the wall clock (`WallClock`, `DateTime`), sound (`Opl`, `PcmOut`), storage (`BlockDevice`) and the `Platform` bundle. Every one is still only text in `docs/HAL.md`.

## The code

| Where | What it holds |
|---|---|
| [`hal/hal/src/lib.rs`](repo:hal/hal/src/lib.rs) | The module list, the `no_std` attribute, the re-exports and [`DebugOut`](sym:hal/hal/src/lib.rs::DebugOut) |
| [`hal/hal/src/port.rs`](repo:hal/hal/src/port.rs) | [`PortIo`](sym:hal/hal/src/port.rs::PortIo), [`POST_PORT`](sym:hal/hal/src/port.rs::POST_PORT) and the blanket impl for `&P` |
| [`hal/hal/src/x86.rs`](repo:hal/hal/src/x86.rs) | [`RawPorts`](sym:hal/hal/src/x86.rs::RawPorts) and its `unsafe` constructor: the one audited boundary for port I/O |
| [`hal/hal/src/mmio.rs`](repo:hal/hal/src/mmio.rs) | [`MmioRegion`](sym:hal/hal/src/mmio.rs::MmioRegion): bounds- and alignment-checked volatile access, `write_bytes`, `fill8` |
| [`hal/hal/src/irq.rs`](repo:hal/hal/src/irq.rs) | [`Irq`](sym:hal/hal/src/irq.rs::Irq), [`InterruptController`](sym:hal/hal/src/irq.rs::InterruptController), [`IrqService`](sym:hal/hal/src/irq.rs::IrqService), [`SharedIrqService`](sym:hal/hal/src/irq.rs::SharedIrqService), [`IrqHandler`](sym:hal/hal/src/irq.rs::IrqHandler), [`Chain`](sym:hal/hal/src/irq.rs::Chain), [`IrqRegistry`](sym:hal/hal/src/irq.rs::IrqRegistry) |
| [`hal/hal/src/ring.rs`](repo:hal/hal/src/ring.rs) | [`SpscRing`](sym:hal/hal/src/ring.rs::SpscRing), its `Producer` and `Consumer`, and the three ring tests |
| [`hal/hal/src/time.rs`](repo:hal/hal/src/time.rs) | [`Ticks`](sym:hal/hal/src/time.rs::Ticks), [`Clock`](sym:hal/hal/src/time.rs::Clock), [`Delay`](sym:hal/hal/src/time.rs::Delay), [`TickCounter`](sym:hal/hal/src/time.rs::TickCounter) |
| [`hal/hal/src/video.rs`](repo:hal/hal/src/video.rs) | [`Rgb6`](sym:hal/hal/src/video.rs::Rgb6), [`PixelLayout`](sym:hal/hal/src/video.rs::PixelLayout), [`ModeInfo`](sym:hal/hal/src/video.rs::ModeInfo), [`FrameBuffer`](sym:hal/hal/src/video.rs::FrameBuffer), [`Display`](sym:hal/hal/src/video.rs::Display) |
| [`hal/hal/src/input.rs`](repo:hal/hal/src/input.rs) | [`Modifiers`](sym:hal/hal/src/input.rs::Modifiers), [`KeyCode`](sym:hal/hal/src/input.rs::KeyCode), [`KeyEvent`](sym:hal/hal/src/input.rs::KeyEvent), [`MouseEvent`](sym:hal/hal/src/input.rs::MouseEvent), [`InputSource`](sym:hal/hal/src/input.rs::InputSource) |
| [`hal/hal/src/bios.rs`](repo:hal/hal/src/bios.rs) | [`Bios`](sym:hal/hal/src/bios.rs::Bios), [`BiosRegs`](sym:hal/hal/src/bios.rs::BiosRegs), `BIOS_BUFFER_MAX` |
| [`hal/hal/src/error.rs`](repo:hal/hal/src/error.rs) | [`HalError`](sym:hal/hal/src/error.rs::HalError) |
| [`hal/clippy.toml`](repo:hal/clippy.toml) | The ten banned atomic methods |
| [`hal/platform/mock/src/bus.rs`](repo:hal/platform/mock/src/bus.rs) | [`MockPortBus`](sym:hal/platform/mock/src/bus.rs::MockPortBus), [`Access`](sym:hal/platform/mock/src/bus.rs::Access), [`Device`](sym:hal/platform/mock/src/bus.rs::Device) |
| [`hal/platform/mock/src/devices.rs`](repo:hal/platform/mock/src/devices.rs) | [`Uart16550`](sym:hal/platform/mock/src/devices.rs::Uart16550) and [`Pit8254`](sym:hal/platform/mock/src/devices.rs::Pit8254) |
| [`hal/platform/mock/src/irq.rs`](repo:hal/platform/mock/src/irq.rs) | [`MockIrq`](sym:hal/platform/mock/src/irq.rs::MockIrq) |
| [`hal/platform/mock/src/time.rs`](repo:hal/platform/mock/src/time.rs) | [`MockClock`](sym:hal/platform/mock/src/time.rs::MockClock) and [`MockDelay`](sym:hal/platform/mock/src/time.rs::MockDelay) |
| [`kernel/src/hal.rs`](repo:kernel/src/hal.rs) | The native platform: [`PORTS`](sym:kernel/src/hal.rs::PORTS), [`IrqTable`](sym:kernel/src/hal.rs::IrqTable) as the `IrqRegistry`, [`irq_dispatch`](sym:kernel/src/hal.rs::irq_dispatch) |
| [`spikes/d0-rust-dos/rt/src/hal.rs`](repo:spikes/d0-rust-dos/rt/src/hal.rs) | The DOS platform's [`DosBios`](sym:spikes/d0-rust-dos/rt/src/hal.rs::DosBios) and [`DosDisplay`](sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay) |
| [`spikes/d0-rust-dos/rt/src/irq.rs`](repo:spikes/d0-rust-dos/rt/src/irq.rs) | The DOS platform's [`DosIrq`](sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq): a stub per line and the registry |

For depth: `docs/HAL.md` ([decisions](doc:docs/HAL.md#decisions), [the four kinds of code](doc:docs/HAL.md#the-four-kinds-of-code), [interrupts](doc:docs/HAL.md#interrupts-built), [concurrency](doc:docs/HAL.md#concurrency), [the unsafe policy](doc:docs/HAL.md#the-unsafe-policy), [migration plan](doc:docs/HAL.md#migration-plan)) and `hal/README.md`.

## Worked example

**One bounds check, one script, one handler.** The three small tests below are real and can be read in the files named; the numbers are theirs.

*A region refuses what it should.* `accesses_are_bounded_and_aligned` in `hal/hal/src/mmio.rs` builds a region over a 16-byte array. It writes `0xA0B0_C0D0` at offset 12 and reads it back, then expects `InvalidArgument` for `read32(16)` (the word would start at the end), for `read32(2)` (not a multiple of 4), for `write_bytes(14, &[0; 3])` (three bytes from offset 14 end at 17, past the length of 16) and for `read8(usize::MAX)` (the offset plus one overflows, which `checked_add` catches).

*A script is a specification.* In `hal/drivers/pit` the test `programming_100_hz_writes_what_the_kernel_writes` hands the driver a bus whose script is three accesses, `outb(0x43, 0x34)`, `outb(0x40, 0x9C)`, `outb(0x40, 0x2E)`, and calls `program_periodic(100)`. The driver must answer `Ok(11_932)` (the divisor, `0x2E9C`) and make exactly those writes in that order; a fourth write or a different byte panics with the position. The script was written out by hand from the kernel's own code, which is the kind of equivalence the document claims: by hand-written values, not by running the kernel against the mock.

*A handler runs from a static, and the platform sends the EOI.* In the same crate, `ticks_arrive_through_a_registered_handler` makes a `static CLOCK: PitClock`, programs it for 100 Hz (the clock records 99,998 millihertz), registers `IrqHandler::from_shared(CLOCK.counter())` on `Irq::TIMER` with `Chain::None`, and calls `MockIrq::raise` 25 times. The tick count has advanced by 25, `millis_since` says 250, and `irq.eois()` is twenty-five copies of `Irq::TIMER`: the end-of-interrupts were sent by the platform (the mock), not by the driver. A second `register` on the same line is `Err(HalError::Busy)`, and after `unregister` a `raise` does nothing and returns false. That is the whole interrupt contract exercised with no interrupt hardware.

The arithmetic checks out against `Clock::millis_since`: 25 ticks times 1,000,000 divided by 99,998 millihertz is 250.005, rounded down to 250.

## State of this layer

```status
id: hal.contract
label: The traits and types of desk-hal pass their host tests (ring, region, time, interrupts, video, input)
state: built-verified
evidence: test:hal/hal/src/ring.rs::a_producer_thread_and_a_consumer_thread_agree, test:hal/hal/src/mmio.rs::accesses_are_bounded_and_aligned, test:hal/hal/src/time.rs::intervals_survive_the_wrap, test:hal/hal/src/irq.rs::a_handler_reaches_its_context, sym:hal/hal/src/lib.rs::DebugOut
note: Run on the host: 13 tests in desk-hal, and 179 in the whole hal/ workspace, in the run recorded below.

id: hal.rawports
label: RawPorts executes in and out in ring 0 on the native kernel
state: built-verified
evidence: sym:hal/hal/src/x86.rs::RawPorts, test:tests/native/expected.txt::pic: remapped to 0x20 and 0x28, test:tests/native/expected.txt::pit: channel 0 at 100 Hz
note: Verified in QEMU as a 486 (the boot was rerun for this chapter and its transcript matched). It has not executed on a real machine, and not under DOS in a run of this chapter.

id: hal.ring
label: SpscRing carries keyboard bytes and mouse packets from interrupt to core in the kernel
state: built-verified
evidence: sym:kernel/src/input.rs::KEY_BYTES, test:tests/native/expected-input.txt::input: IRQ 1 and IRQ 12 handlers registered
note: QEMU only, with keys sent through its monitor; the committed transcript, not rerun for this chapter. A full ring drops and counts; the drop path is host-tested, not exercised in QEMU.

id: hal.irq-native
label: The native IrqRegistry: gates, a handler table, EOI after the handler
state: built-verified
evidence: sym:kernel/src/hal.rs::IrqTable, test:tests/native/expected.txt::timer: waited for 10 ticks with hlt

id: hal.mmio-native
label: MmioRegion over the VGA window in the kernel's graphics tour
state: built-verified
evidence: sym:kernel/src/gfx.rs::vga_window, test:tests/native/expected-gfx-vbe.txt::gfx: vbe picture: frame buffer checksum 0x5bc65936
note: QEMU only; the committed transcript, not rerun for this chapter. The unaligned head and tail paths of fill8 are host-tested.

id: hal.mock
label: The mock platform: scripted bus, device models, MockIrq, MockClock and MockDelay
state: built-verified
evidence: test:hal/platform/mock/src/bus.rs::a_script_rejects_the_wrong_access, test:hal/platform/mock/src/devices.rs::the_8254_read_sequence_survives_a_latch, sym:hal/platform/mock/src/irq.rs::MockIrq
note: Host tests only. The models are the author's reading of the data sheets and have not been compared with real chips; MockClock and MockDelay have no test of their own.

id: hal.nocas
label: No compare-and-swap in the shared crates (clippy bans ten methods)
state: built-verified
evidence: repo:hal/clippy.toml, test:tools/check.sh::cargo clippy --locked --all-targets
note: `cargo clippy --locked --all-targets -- -D warnings` passes in hal/ (run for this chapter). That clippy refuses a deliberate violation was checked by the design's author on the day of pull request #8; it was not repeated here.

id: hal.dos
label: DosBios, DosDisplay and DosIrq under DOS
state: built-unverified
evidence: sym:spikes/d0-rust-dos/rt/src/hal.rs::DosDisplay, sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq, test:spikes/a-vga-vbe/expected/all.txt::avga: read back equals written: true
note: Written and covered by committed headless-DOSBox transcripts, but DOSBox was not run for this chapter, and the offline data has no CI result for the pull requests that proved them.

id: hal.native-bios
label: NativeBios implements the Bios trait through the real-mode trampoline
state: built-verified
evidence: sym:kernel/src/bios.rs::NativeBios, test:tests/native/expected-bios.txt::bios: real-mode trampoline at 0x10000
note: The committed transcript, not rerun for this chapter. The whole round trip is the subject of M8.

id: hal.step1
label: Migration step 1: the kernel runs on the shared drivers
state: built-verified
evidence: sym:kernel/src/hal.rs::PORTS, test:tests/native/expected.txt::boot complete
note: The optional move of vga.rs to a desk-vgatext crate was not done.

id: hal.step2
label: Migration step 2: the DOS runtime as the hosted platform
state: built-unverified
evidence: sym:spikes/d0-rust-dos/rt/src/irq.rs::DosIrq, doc:docs/HAL.md#migration-plan
note: Partly done: rt::hal and rt::irq exist; rt is not promoted, rt::timer is separate, and desk-pit and desk-serial are not used under DOS.

id: hal.surfaces
label: PhysMem, WallClock, Opl, PcmOut, BlockDevice and the Platform bundle
state: planned
evidence: doc:docs/HAL.md#the-surfaces

id: hal.steps35
label: Migration steps 3 and 5: the hosted skeleton and the core on both platforms through Platform
state: planned
evidence: doc:docs/HAL.md#migration-plan

id: hal.hardware
label: Any of this on a real machine
state: planned
evidence: doc:docs/HAL.md#what-was-and-was-not-verified
```

The contract and the native platform are {{state.hal.rawports}} in QEMU as a 486 and {{state.hal.contract}} on the host. The DOS platform is {{state.hal.dos}} here, and a real machine is {{state.hal.hardware}}: nothing in this chapter was run on real hardware.

> **From the stacks — Before the abstraction.** Wikipedia's article on hardware abstraction says that early software was developed without a hardware abstraction, so developers had to understand several devices to make a program compatible with them. It adds that CP/M, DOS, Solaris, Linux, BSD and macOS also have a HAL, even if it is not explicitly designated as such. The article gives no dates for those early systems.
> Source: [Hardware abstraction, Wikipedia](https://en.wikipedia.org/wiki/Hardware_abstraction)

## Try it yourself

The first command runs the whole workspace's host tests and keeps the summary lines of the crates that have tests; the others look at the properties this chapter claims. Everything runs on the host with a Rust toolchain; no emulator is needed.

```try
needs: host: cargo, grep
ran: 2026-10-23 on the host at 44f0265, Rust stable
$ cd hal && cargo test 2>&1 | grep -E "^test result: ok. [1-9]"
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cd hal && cargo test -p desk-hal 2>&1 | grep -E "^test "
test input::tests::modifier_sets_combine_and_subtract ... ok
test irq::tests::a_handler_reaches_its_context ... ok
test irq::tests::a_shared_handler_needs_only_a_static ... ok
test irq::tests::lines_above_fifteen_do_not_exist ... ok
test mmio::tests::accesses_are_bounded_and_aligned ... ok
test ring::tests::items_come_out_in_order_and_a_full_ring_drops ... ok
test ring::tests::the_indices_wrap ... ok
test time::tests::intervals_survive_the_wrap ... ok
test time::tests::the_counter_as_a_service_ticks ... ok
test video::tests::rgb8_converts_both_ways ... ok
test video::tests::scaling_rounds_down_and_clamps ... ok
test mmio::tests::fill_covers_unaligned_heads_and_tails_and_stays_inside ... ok
test ring::tests::a_producer_thread_and_a_consumer_thread_agree ... ok
$ cd hal && cargo clippy --locked --all-targets -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s
$ grep -c unsafe hal/drivers/serial/src/lib.rs hal/drivers/pic/src/lib.rs hal/drivers/pit/src/lib.rs hal/hal/src/x86.rs
hal/drivers/serial/src/lib.rs:0
hal/drivers/pic/src/lib.rs:0
hal/drivers/pit/src/lib.rs:0
hal/hal/src/x86.rs:8
$ grep -c "DebugOut" kernel/src/main.rs kernel/src/console.rs kernel/src/serial.rs
kernel/src/main.rs:0
kernel/src/console.rs:0
kernel/src/serial.rs:0
```

The test summary is 179 tests in all; the document that first described the workspace counts 35. The last command shows a gap between that document and the kernel: `DebugOut` is implemented by the serial driver, and nothing uses it.

**Exercises.** (1) Without running it, predict what `SpscRing::<u8, 3>::new()` does, then try it in a scratch test and read the error. (2) Write a host test in a scratch branch that attaches a `Uart16550` model at `COM2` and checks that after `init(9_600)` its `divisor` is 12; then find the existing test that already does it (`the_divisor_follows_the_rate`). (3) Add a `compare_exchange` call to any file in `hal/hal/src/` and run `cargo clippy` in `hal/`; say which entry of `clippy.toml` fires. (4) In `MockIrq`, `raise` on a masked line returns false: predict what `register` then `mask` then `raise` returns, and check.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: HAL
def: The hardware abstraction layer: the small crate desk-hal of traits and shared types that drivers are written against and platforms implement, so that a line of device logic exists once.

term: Platform
def: The HAL implemented for one environment: the native kernel, the hosted DOS runtime over DPMI, or the host mock that tests run against. A platform hooks vectors, finds physical memory and makes the port bus; it never holds device logic.

term: Port bus
def: A value that implements PortIo, which is the capability to touch the machine's ports. RawPorts is the real one and its constructor is unsafe; the mock platform's MockPortBus records the accesses instead.

term: Mock platform
def: The host platform in hal/platform/mock/: a scripted port bus, models of a few chips, a fake interrupt registry and a hand-driven clock, so that driver tests run on the development machine with no emulator.
```

```figures
id: m9-layers
reuse: hal-layers
at: figure hal-layers

id: m9-crates
reuse: m1-crates
at: explainer m-crates
```

## Revisions

- 2026-10-23: first written, against `44f0265`. Every surface was read in `hal/hal/src/`, the mock in `hal/platform/mock/src/`, the two platforms' use of the contract in `kernel/src/hal.rs` and `spikes/d0-rust-dos/rt/src/`, and `docs/HAL.md` in full; `cargo test` in `hal/` was run on the host (179 tests, all passing). Where `docs/HAL.md` and the code disagree, the code is followed. Found while writing: (1) the document's opening status and its "What was and was not verified" say no binary uses the crates and that the kernel and `rt` are unchanged, but the kernel depends on the crates (migration step 1 is done, as the document's own crate-layout paragraph says); (2) it counts 35 host tests, and the workspace now has 179; (3) its platforms table marks `kernel::hal` and `rt::hal` as planned, and both exist; (4) its `Display` sketch has infallible `set_palette` and `palette`, and the code returns `Result` (the document notes this in its video section); (5) it says the panic handlers of both platforms use `DebugOut`, but nothing outside `desk-serial` and `desk-hal` names the trait, and the kernel's panic handler prints with `println!`; (6) it says `Serial16550` is a `DebugOut` for both platforms, but the DOS runtime does not use `desk-serial` or `desk-pit`; (7) its "Today `hal/` holds" list omits `desk-vga` and `desk-int33`, which the workspace has. The `Display`, `Bios` and input surfaces are lightly covered here and taught in M11 and M12.
