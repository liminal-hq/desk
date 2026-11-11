# HAL: the hardware abstraction layer and the shared drivers

Spike H: one design that lets the same driver code (the 16550 serial port, the PIT and PIC, the keyboard and PS/2 mouse, VGA and VBE, OPL and the Sound Blaster, and later storage) run in three places: the hosted DOS runtime, the native kernel, and host unit tests against a mock port bus. The reference implementation is in [`../hal/`](../hal/).

> **Status.** *Built and verified on the host:* the `hal/` workspace with the port I/O, interrupt, time, ring-buffer and memory-mapped-region surfaces, a mock platform, and three drivers (16550, 8254, 8259) tested against the mock; the crates build for the kernel's target and the pure-Rust DOS target. *Designed only:* everything else in this document, including the video, input, sound, storage and physical-memory surfaces, both real platforms, and the migration. Nothing here has run in the kernel, under DOS, in an emulator or on real hardware: the kernel and `rt` are unchanged.

## Contents

- [Decisions](#decisions)
- [The four kinds of code](#the-four-kinds-of-code)
- [Crate layout](#crate-layout)
- [The surfaces](#the-surfaces)
- [Concurrency](#concurrency)
- [The unsafe policy](#the-unsafe-policy)
- [Errors](#errors)
- [no_std, alloc and features](#no_std-alloc-and-features)
- [How it fits the targets](#how-it-fits-the-targets)
- [The platforms](#the-platforms)
- [Apps and modules](#apps-and-modules)
- [Migration plan](#migration-plan)
- [Follow-up spikes](#follow-up-spikes)
- [Open questions](#open-questions)
- [What was and was not verified](#what-was-and-was-not-verified)

## Decisions

1. **Two edges, one crate.** `desk-hal` has a *lower edge* that drivers use (port I/O, interrupts, memory-mapped regions and DMA memory, a tick source) and an *upper edge* that the Desk core uses (display, input queues, clock, sound, storage, debug output). `DIRECTION.md` called the lower edge "the port-I/O and IRQ abstraction" and the upper edge "the HAL"; they are kept in one small crate so the types they share (`Irq`, `HalError`, `MmioRegion`, `SpscRing`) have one home.
2. **Drivers are generic over traits, not trait objects.** A driver is `Serial16550<P: PortIo>`; a binary has exactly one platform, so monomorphisation costs no code size and leaves no vtable on the interrupt path. The mock is just another `P`.
3. **The capability is the value.** `PortIo` methods are safe; making a real port bus (`x86::RawPorts::new`) is `unsafe`, once per platform. Drivers get a bus, a base port and nothing else.
4. **Interrupt handlers are a function pointer and a context pointer**, built safely from a `&'static mut T` whose ownership moves to the handler, or from a `&'static T` whose state is all atomics. They talk to the core only through atomics and lock-free single-producer, single-consumer rings. The platform sends the end-of-interrupt, never the driver.
5. **No compare-and-swap anywhere.** Loads, stores, `swap` (`xchg`) and `fetch_add` (`lock xadd`) only. A 486 has `CMPXCHG`, but DOSBox's `cputype = 486` rejects it ([`D0-RUST-DOS.md`](D0-RUST-DOS.md)), so CI would not run code that uses it. `hal/clippy.toml` enforces this.
6. **Platforms live with their environment.** The DOS platform becomes a module of `rt`, the native platform a module of the kernel, and only the portable crates (traits, drivers, mock) live in `hal/`. Everything in `hal/` builds for the host, so `cargo test` runs there.

## The four kinds of code

| Kind | What it is | May call | Must not call | Lives in |
|---|---|---|---|---|
| **HAL** | The traits and small shared types: the contract | `core` | Anything platform-specific | `hal/hal/` (`desk-hal`) |
| **Driver** | Device logic written once against the HAL's lower edge: register sequences, timing rules, decoding, interrupt-side work | `desk-hal`, `core` | DOS, BIOS, DPMI, kernel internals, `alloc` in interrupt code | `hal/drivers/*` |
| **Platform** | The HAL implemented for one environment: hosted DOS over `rt` and DPMI, the native kernel, the host mock | Its environment, drivers, `desk-hal` | The Desk core | `rt` (DOS), `kernel/` (native), `hal/platform/mock/` |
| **Desk core** | The OS-independent shell: compositor, windows, widgets, fonts, palette, event loop, app host | The HAL's upper edge, `alloc` | Ports, DOS, DJGPP, kernel internals, drivers' register-level API | `core/` |

The rule that keeps the tracks from drifting: **a line of device logic exists once**, in a driver; a platform contains only what genuinely differs (how a vector is hooked, how physical memory is found, how a mode is set through the BIOS).

Alternatives considered and rejected:

- **One HAL per track, no shared drivers** (the status quo: the kernel's `serial.rs` and `pit.rs`, `rt`'s `timer.rs`). Every driver would be written twice and tested only in an emulator. Rejected: the point of the native track is that it grows into the hosted track's drivers and the reverse.
- **Free functions over a global port bus** (`hal::outb(port, v)` as the kernel has today). Simplest, but untestable without link-time substitution, and it hides which code touches hardware. Rejected for drivers; a platform may still keep free functions internally.
- **Trait objects (`&dyn PortIo`)** for drivers. Works, and was the fallback; rejected because the mock and the real bus would then cost an indirect call per port access on a 486, and because it buys nothing when a binary has one platform.
- **`embedded-hal`**. Its traits model GPIO, SPI and I²C on microcontrollers, not ISA port I/O, IRQ lines and DMA; adopting it would mean wrapping everything anyway.
- **Unsafe port methods (`unsafe fn outb`)**. Every driver line would need an `unsafe` block with no local justification to give. The danger is in holding the bus, so that is where the `unsafe` goes.
- **Closures as interrupt handlers.** A closure's environment has no fixed address or size, cannot be named from an assembly stub, and may allocate. Rejected outright.

## Crate layout

```
hal/                          one Cargo workspace; no default target, so `cargo test` is a host build
├── Cargo.toml, Cargo.lock, clippy.toml, rustfmt.toml
├── hal/                      desk-hal            traits and types                         built
├── drivers/
│   ├── serial/               desk-serial         16550 UART                               built
│   ├── pit/                  desk-pit            8254 timer, PitClock                     built
│   ├── pic/                  desk-pic            two 8259s                                built
│   ├── i8042/                desk-i8042          keyboard controller, PS/2 mouse          built (spike C)
│   ├── kbd/                  desk-kbd            scancode set 1 decoder, key map          built (spike C)
│   ├── vga/                  desk-vga            DAC, retrace, mode 13h/12h registers      spike A, then D
│   ├── vgatext/              desk-vgatext        80x25 text console (from kernel/vga.rs)   migration step 1
│   ├── dma8237/              desk-dma8237        ISA DMA controller                        sound milestone
│   ├── opl/                  desk-opl            OPL2/OPL3 register writes                 sound milestone
│   ├── sb/                   desk-sb             Sound Blaster DSP                         sound milestone
│   └── ata/, fdc/            desk-ata, desk-fdc  IDE and floppy block devices              storage milestone
└── platform/
    └── mock/                 desk-hal-mock       MockPortBus, device models, MockIrq      built
rt/  (from spikes/d0-rust-dos/rt)   rt::hal         the hosted DOS platform                migration step 2
kernel/src/hal.rs                   kernel::hal     the native platform                    migration step 1
```

**Why the real platforms are not crates in `hal/`.** The DOS platform is DPMI calls, the interrupt stub and the clean-up list, all of which are `rt`; the native platform is the IDT, the frame allocator and the boot information, all of which are the kernel. A separate `platform/dos` crate would only re-export `rt`, and would not build for the host, which would split the workspace's `cargo test`. If Lieutenant Fizz Episode 2 or the `liminal-hq/dos-rs` idea in `DIRECTION.md` wants `rt` and its platform without Desk, they move together.

Today `hal/` holds `desk-hal`, `desk-serial`, `desk-pit`, `desk-pic`, `desk-kbd`, `desk-i8042` and `desk-hal-mock`. The native kernel depends on them (migration step 1, done with spike C); nothing else does yet.

## The surfaces

The code blocks are the interfaces. Those marked *built* are in `hal/hal/src/` as shown (doc comments trimmed); those marked *sketch* are the contract for the spike that builds them and may change with what it learns.

### Port I/O (built)

```rust
pub trait PortIo {
    fn inb(&self, port: u16) -> u8;
    fn outb(&self, port: u16, value: u8);
    fn inw(&self, port: u16) -> u16;
    fn outw(&self, port: u16, value: u16);
    fn inl(&self, port: u16) -> u32;
    fn outl(&self, port: u16, value: u32);
    fn io_wait(&self) { self.outb(0x80, 0) }   // about 1 µs on the ISA bus
}
impl<P: PortIo + ?Sized> PortIo for &P { /* forwards */ }
```

- **`&self`, not `&mut self`**, so several drivers can share one bus. `x86::RawPorts` is a zero-sized `Copy` type, so a driver can also own a copy; the mock uses `RefCell` inside.
- **Ordering.** Each access happens exactly once and in program order: each is an `asm!` block, which the compiler may not remove, merge or reorder with another, and x86 port I/O is strongly ordered. The blocks omit `nomem`, so the compiler also does not move ordinary memory accesses across them: a DMA buffer is filled before the port write that starts the transfer, without an explicit fence. A platform whose implementation cannot promise that must say so, and drivers that hand memory to a device put a `compiler_fence(SeqCst)` before the starting write anyway, as documentation.
- **The I/O delay** is a write to the POST port `0x80`, which is what the kernel's `io_wait` does and what the 8259 needs between initialisation words on old machines. It is a bus-cycle delay, not a time: anything with a datasheet time (the OPL's 3.3 µs and 23 µs) uses `Delay`.
- **16- and 32-bit accesses** are there for IDE data (`inw` in a loop, later `rep insw` in a platform fast path) and PCI configuration (`inl`/`outl` at `0xCF8`/`0xCFC`), which a 486 board may have.

### Memory-mapped regions and physical memory (region built, physical memory sketch)

```rust
pub struct MmioRegion { base: *mut u8, len: usize }        // built
impl MmioRegion {
    pub const unsafe fn new(base: *mut u8, len: usize) -> MmioRegion;
    pub fn read8(&self, offset: usize) -> Result<u8, HalError>;
    pub fn write8(&mut self, offset: usize, value: u8) -> Result<(), HalError>;
    pub fn read32(&self, offset: usize) -> Result<u32, HalError>;      // address must be a multiple of 4
    pub fn write32(&mut self, offset: usize, value: u32) -> Result<(), HalError>;
    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError>;
}

// sketch
pub struct DmaConstraints {
    pub limit: u32,      // exclusive: 16 MiB for ISA DMA (24 address lines)
    pub boundary: u32,   // 64 KiB for 8-bit channels 0-3, 128 KiB for 16-bit channels 5-7
}
pub const ISA_DMA_8: DmaConstraints = DmaConstraints { limit: 16 << 20, boundary: 64 << 10 };
pub const ISA_DMA_16: DmaConstraints = DmaConstraints { limit: 16 << 20, boundary: 128 << 10 };

pub struct DmaBuffer { pub region: MmioRegion, pub phys: u32 }

pub trait PhysMem {
    /// Contiguous memory a device can reach by physical address, inside `c.limit` and not crossing `c.boundary`.
    fn alloc_dma(&self, len: u32, c: DmaConstraints) -> Result<DmaBuffer, HalError>;
    fn free_dma(&self, buf: DmaBuffer);
    /// Device memory at a physical address (the VBE linear frame buffer); `free_mapping` undoes it.
    unsafe fn map_physical(&self, phys: u32, len: u32) -> Result<MmioRegion, HalError>;
    fn free_mapping(&self, region: MmioRegion);
}
```

- **The VGA window at `A0000h`** (64 KiB) and the text buffer at `B8000h` need no `PhysMem` call: on the native kernel they are identity mapped, and under CWSDPMI the first megabyte is mapped at linear 0 and reached through the 4 GiB data segment (`rt::dpmi::linear_to_ptr`, as `rt::vga` does). The platform hands the driver an `MmioRegion` over them.
- **The VBE linear frame buffer** is above 16 MiB on most cards. Natively it is identity mapped while there is no paging (the kernel has none yet); with paging, the kernel maps it uncached. Under DPMI it is function 0800h (physical address mapping) and 0801h on clean-up.
- **DMA buffers** for the Sound Blaster (8-bit channel 1, 16-bit channel 5) and the floppy (channel 2) must be physically contiguous, below 16 MiB, and must not cross a 64 KiB (or 128 KiB) boundary, because the 8237 counts within a page and the page register does not carry. The platform allocates twice the length and uses the half that does not cross, or aligns to the boundary. Natively that is `memory::alloc_contiguous` restricted to frames below 16 MiB. Under DOS it is DPMI 0100h (conventional memory, below 1 MiB), where linear equals physical **only without a V86 memory manager**: under EMM386 or QEMM the platform must use Virtual DMA Services (INT 4Bh, flagged by bit 5 of `0040:007B`) to lock the buffer and get its physical address. Not handled yet; see [open questions](#open-questions).
- **Performance.** `write_bytes` is one volatile byte store at a time, which is correct for any device memory and slow for a 64,000-byte blit on a 486. The compositor's blit will be a platform fast path (`rep movsd` into the frame buffer) behind the same bounds check; until it is measured, the volatile loop is the reference.

### Interrupts (built)

```rust
pub struct Irq(u8);                    // 0..=15; Irq::new(n) refuses 16 and above
impl Irq { pub const TIMER: Irq; pub const KEYBOARD: Irq; pub const CASCADE: Irq; pub const MOUSE: Irq; /* ... */ }

pub trait InterruptController {
    fn mask(&self, irq: Irq);
    fn unmask(&self, irq: Irq);
    fn is_masked(&self, irq: Irq) -> bool;
    fn eoi(&self, irq: Irq);                        // called by the platform only
    fn is_spurious(&self, irq: Irq) -> bool { false }
    fn eoi_spurious(&self, irq: Irq) {}             // what a spurious line needs instead of eoi
}

pub trait IrqService { fn on_irq(&mut self); }      // a driver's interrupt-side half, state only it touches
pub trait SharedIrqService: Sync { fn on_irq(&self); }  // interrupt work over atomics shared with the core

pub struct IrqHandler { func: unsafe extern "C" fn(*mut ()), ctx: *mut () }
impl IrqHandler {
    pub fn from_static<T: IrqService + Send + 'static>(svc: &'static mut T) -> IrqHandler;
    pub fn from_shared<T: SharedIrqService + 'static>(svc: &'static T) -> IrqHandler;
    pub const unsafe fn from_raw(func: unsafe extern "C" fn(*mut ()), ctx: *mut ()) -> IrqHandler;
    pub unsafe fn call(&self);                      // the platform's dispatch only
}

pub enum Chain { None, Previous }

pub trait IrqRegistry {
    fn register(&self, irq: Irq, handler: IrqHandler, chain: Chain) -> Result<(), HalError>;
    fn unregister(&self, irq: Irq) -> Result<(), HalError>;
}
```

- **Registration is a function pointer plus a context pointer**, two words the platform's assembly stub can keep in a locked table. Two safe constructors cover driver code. `from_static` takes a `&'static mut T` (typically `Box::leak` at start-up) for state only the handler touches, such as a ring's producer and a packet being assembled: the `&'static mut` moves to the handler, so nothing else can touch it, and anything the core shares with it sits behind a separate `&'static` (an atomic counter, an `SpscRing`). `from_shared` takes a `&'static T` whose state is all atomics (`TickCounter`), which can be a plain `static` with no allocation. A platform with no allocator and mutable handler state uses `from_raw` over a static it owns, inside its own audited `unsafe`. The trampolines are monomorphised, so there is no `dyn` and no closure, and they are `extern "C"`, so an assembly stub may call the function directly with the context as its one cdecl argument.
- **Who sends the end-of-interrupt.** The platform, after the handler returns, according to `Chain`: `Chain::None` means the platform sends it (natively always; under DOS for lines Desk owns outright, the keyboard and the PS/2 mouse); `Chain::Previous` means the previous owner runs next and sends it (under DOS, the BIOS timer handler, which also keeps the DOS clock; the d0 IRQ 0 hook already works this way). A driver never sends one: the kernel's `pit::tick` did, and under DOS chaining that would acknowledge a second, unrelated interrupt.
- **Spurious interrupts.** For IRQ 7 and 15 the platform asks `is_spurious` first; `desk-pic` reads the in-service register. If it is, the platform runs no handler and calls `eoi_spurious` instead of `eoi`: a spurious IRQ 7 gets no end-of-interrupt; a spurious IRQ 15 gets one on the master only, because the master did see IRQ 2 (otherwise IRQ 3 to 7 stay blocked).
- **Masking is a read-modify-write** of a register every line shares, so the core calls `mask` and `unmask` inside the platform's "interrupts off" critical section; otherwise a handler (ours, the BIOS's, a TSR's) that changes the mask in between has its change lost. Drivers cannot turn interrupts off, so this is a rule for the platform and the core.
- **The 8259 under DOS.** The BIOS programmed the controllers (vectors 08h and 70h) and owns them; the hosted platform never calls `Pic8259::init`, only `mask`, `unmask`, `eoi` and `is_spurious`, which DJGPP programs also do directly. The native kernel calls `init(REMAPPED_BASES, mask)`.
- **Rules for handlers**, which a reviewer checks and the types help with:
  - No allocation and no `alloc` types (the allocator is not re-entrant and may hold its lock: the kernel's is "interrupts off", DJGPP's `malloc` is not re-entrant).
  - No lock the core might hold. There are no spin locks on a uniprocessor that an interrupt can wait on; the only critical section is "interrupts off" in the core, which an interrupt by definition does not run inside.
  - Bounded work: read the device, push to a ring, count. Decoding (scancodes to keys, packets to pointer moves) is done by the core when it drains the ring.
  - No DOS, BIOS or DPMI call, and no floating point (the x87 state belongs to the interrupted code).
  - Shared data is atomics or an `SpscRing`. `volatile` is the device's business (`MmioRegion`, the port instructions); Rust code shares memory through atomics, which on x86 compile to the same plain `mov`s.
  - **Locked memory under DPMI.** The handler, its context, the ring, every `static` it touches and the stack it runs on must not be paged out: CWSDPMI can page, and a page fault inside an interrupt handler is fatal. `rt` locks the whole image (DPMI 0600h on `__image_start..__image_end`) before hooking, which covers code, `.data`, `.bss` and its own stack; a `Box::leak`ed context is in the heap and is **not** covered, so the DOS platform's `register` locks the context's pages too (or contexts live in `static`s).
  - **Re-entry.** The platform guarantees a handler is never re-entered: natively the IDT gates are interrupt gates (IF cleared on entry) and the EOI is sent after the handler; under DPMI the host delivers hardware interrupts with interrupts disabled and the stub does not enable them.

### Time (built; the wall clock is a sketch)

```rust
pub struct Ticks(pub u32);
impl Ticks { pub const fn since(self, earlier: Ticks) -> u32 /* wrapping */ }

pub trait Clock {
    fn now(&self) -> Ticks;
    fn millihertz(&self) -> u32;                         // 18,207 for the BIOS rate, 99,998 for "100 Hz"
    fn millis_since(&self, earlier: Ticks) -> u32 { /* u64 maths, saturating */ }
}
pub trait Delay { fn delay_us(&self, us: u32); }
pub struct TickCounter { /* AtomicU32 */ }                // tick() is fetch_add (lock xadd)

// sketch: arrives with its first implementation (CMOS natively, INT 21h under DOS)
pub struct DateTime { pub year: u16, pub month: u8, pub day: u8, pub hour: u8, pub minute: u8, pub second: u8 }
pub trait WallClock { fn now(&self) -> Result<DateTime, HalError>; }
```

- **Monotonic clock.** A 486 has no TSC (`rdtsc` faults under QEMU's `-cpu 486` and DOSBox's `cputype = 486`), so the clock is the PIT's channel 0 interrupt counted in a 32-bit word, the widest atomic the target has. It wraps after 497 days at 100 Hz, 49 days at 1 kHz; every comparison uses `Ticks::since` (wrapping subtraction). The rate is in millihertz because the hosted track keeps the BIOS rate of 18.2 Hz.
- **Sub-tick time** comes from latching and reading channel 0's counter (`Pit::read_counter`, 838 ns resolution), with interrupts off.
- **The calibrated delay** counts loop iterations across one tick at start-up (or, better, against the PIT counter, which needs no full tick), stores iterations per microsecond, and spins. For device timing only; the core waits for events.
- **Hosted timer rate.** Desk may keep the BIOS's 18.2 Hz (chain every tick, as d0 does), or reprogram the PIT faster and chain to the BIOS handler only every Nth tick so DOS's clock stays right, restoring the BIOS rate on exit (`Pit::restore_bios_rate`, registered with `rt`'s clean-up list). The first is the default until the shell needs finer time.
- **Wall clock.** Natively the CMOS real-time clock (ports `0x70`/`0x71`, a small `desk-rtc` driver); under DOS, INT 21h AH=2Ah and 2Ch through `rt`. Local time, no zone. Not monotonic: the user can set it.

### Console and debug output (built: `DebugOut` and the 16550)

```rust
pub trait DebugOut { fn write_bytes(&self, bytes: &[u8]); }   // never fails, never allocates
```

- Implemented by `Serial16550` (both platforms), by a VGA text console (`desk-vgatext`, the kernel's `vga.rs` moved over an `MmioRegion` at `B8000h`), and under DOS by standard output through `rt::dos::write`. The kernel's `println!` writes to serial and VGA; the panic handlers of both platforms use `DebugOut` because it cannot fail or allocate.
- Serial under DOS: COM1 may belong to a mouse driver or a terminal program. The hosted platform uses serial only when asked (a `DESK.INI` setting or a command-line flag); DOSBox's headless runs capture standard output instead.

### Video (sketch; spike A built it)

Spike A built these types, the `desk-vga` driver and the DOS `Display`; what changed from this sketch (a `Bios` trait, fallible palette calls) is in [`A-VGA-VBE.md`](A-VGA-VBE.md#hal-feedback). Spike D ran the same driver in the native kernel; its additions (the text restore, `MmioRegion::fill8`) and its recommendations (fades that do not trust the retrace bit, a native `Bios`, one VBE parser) are in [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md#hal-feedback). Spike n3 built the native `Bios` (a real-mode trampoline, `kernel/src/bios.rs`); what it says about the trait (the 512-byte buffer limit, the missing statement on interrupt context, `Pic8259::in_service_mask` and `pending_mask`) is in [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#hal-feedback).

```rust
pub struct Rgb6 { pub r: u8, pub g: u8, pub b: u8 }           // 0..=63, the VGA DAC's range

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelLayout { Indexed8, Planar4, Rgb565, Xrgb8888 }

pub struct ModeInfo { pub id: u16, pub width: u16, pub height: u16, pub layout: PixelLayout, pub linear: bool }

pub struct FrameBuffer { pub info: ModeInfo, pub pitch: u32, pub region: MmioRegion, pub phys: u32 }

pub trait Display {
    fn modes(&self) -> &[ModeInfo];
    /// Sets the mode and returns its frame buffer. The platform registers the return to text mode with its clean-up list.
    fn set_mode(&mut self, id: u16) -> Result<FrameBuffer, HalError>;
    fn restore_text_mode(&mut self);
    fn set_palette(&mut self, first: u8, colours: &[Rgb6]);
    fn palette(&self, first: u8, out: &mut [Rgb6]);
    /// Waits, boundedly, for the start of vertical retrace.
    fn wait_vretrace(&self) -> Result<(), HalError>;
}
```

- **Mode setting is the platform's job** (INT 10h and VBE function 4F02h through DPMI 0300h under DOS; register programming, or a BIOS call made before entering protected mode, natively). **Everything after the mode set is the driver's**: the DAC (`0x3C7`/`0x3C8`/`0x3C9`), retrace (`0x3DA` bit 3), the sequencer map mask and graphics controller for planar mode 12h (`0x3C4`/`0x3C5`, `0x3CE`/`0x3CF`). That split is what lets spike D reuse spike A's driver.
- Fades and dims go through the DAC, never the frame buffer (`AGENTS.md`), so `set_palette` is in the trait from the start.
- The core sees `Display` and a `FrameBuffer`; it never sees INT 10h, a VBE info block or a DPMI selector.

### Input (built by spike C for the kernel and spike B for DOS)

Spike C built this and changed it a little; the result and the feedback are in [`C-INPUT.md`](C-INPUT.md). The sketch below is the original; the built types are in `hal/hal/src/input.rs`: `KeyEvent` is `{ code, pressed, modifiers }`, `MouseEvent` has a `wheel: i8`, and the handlers test the controller's source bit before reading a byte.

```rust
pub type Scancode = u8;                      // raw scancode set 1 byte, as port 0x60 delivers it with translation on

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent { pub dx: i16, pub dy: i16, pub buttons: u8 }   // dy positive is down the screen; bit 0 left, 1 right, 2 middle

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent { pub key: KeyCode, pub pressed: bool }          // KeyCode: a Desk enum, independent of scancode sets

pub trait InputSource {
    /// Drains what the interrupt side queued since the last call. Called by the core's event loop, never from an interrupt.
    fn poll_key(&mut self) -> Option<KeyEvent>;
    fn poll_mouse(&mut self) -> Option<MouseEvent>;
}
```

- **Event queues filled by interrupt handlers, drained by the core**: the keyboard handler pushes raw `Scancode`s into an `SpscRing<u8, 64>`, the PS/2 mouse handler assembles three-byte packets and pushes `MouseEvent`s into an `SpscRing<MouseEvent, 32>`. Decoding (the `E0` prefix, `E1` for Pause, make and break codes, the key map) is done by `desk-kbd`'s `Decoder` on the core side, which is pure and host-tested.
- **Hosted:** IRQ 1 is the same `desk-i8042` keyboard handler with `Chain::None` (Desk owns the keyboard while it runs, as `SPEC.md` says: "a Fizz-style IRQ 1 handler"), so DOS and the native kernel share the scancode path entirely. The mouse is INT 33h, a different device interface, so the hosted platform has an INT 33h adapter that produces the same `MouseEvent`s into the same ring type (polling the motion counters with AX=0Bh and the buttons with AX=03h once per frame, or a real-mode callback through DPMI 0303h). The core cannot tell the difference. Spike B built it ([`B-DOS-INPUT.md`](B-DOS-INPUT.md)): `desk-int33`'s `MouseInput` polls 000Bh and 0003h once per core iteration (one event per poll with the summed mickeys; the callback was not built), and reads the left button's press and release counters (0005h and 0006h) so a click shorter than a poll arrives as a press event and a release event, `DosInput` is the hosted `InputSource` (with `has_mouse`, false when there is no driver), and the keyboard half is exactly the kernel's, with `Decoder::with_locks` seeding the lock state from the BIOS.

### Sound (sketch; the sound milestone builds it)

```rust
pub trait Opl {
    /// One register write: `reg` 0x000-0x0FF on the first bank, 0x100-0x1FF on an OPL3's second. The driver waits the chip's
    /// address and data delays through `Delay`.
    fn write(&mut self, reg: u16, value: u8);
}

pub enum PcmFormat { U8Mono(u16 /* Hz */), S16Stereo(u16) }
pub trait PcmOut {
    /// Starts auto-initialised DMA playback from `buffer`; the platform calls `half_done` from the DSP's interrupt each time a
    /// half has played, so the core refills it.
    fn start(&mut self, format: PcmFormat, buffer: DmaBuffer, half_done: IrqHandler) -> Result<(), HalError>;
    fn stop(&mut self);
}
```

- `desk-opl` is port writes at `0x388`/`0x389` (or the Sound Blaster's base) with the datasheet delays; `desk-sb` is the DSP (reset at base + 6, read at + 0Ah, write at + 0Ch, status at + 0Eh), and `desk-dma8237` programs the DMA controller (mask, flip-flop clear, mode, address, count, page register, unmask), all over `PortIo`. The only platform parts are the DMA buffer (`PhysMem`), the IRQ (`IrqRegistry`) and the configuration: `BLASTER` under DOS, probing natively.
- Audio cannot be checked headless (DOSBox's sound is off in the checks); the driver tests check the register sequence against the mock.

### Storage (sketch; the native storage milestone builds it)

```rust
pub enum BlockError { NoMedia, Timeout, Io, ReadOnly, OutOfRange }
pub trait BlockDevice {
    fn block_size(&self) -> u32;                 // 512 for floppy and IDE
    fn blocks(&self) -> u32;                     // u32 LBA: 2 TiB at 512 bytes, plenty for a 486
    fn read(&mut self, lba: u32, buf: &mut [u8]) -> Result<(), BlockError>;
    fn write(&mut self, lba: u32, buf: &[u8]) -> Result<(), BlockError>;
}
```

- Native only. The hosted Desk does not use block devices: it reads files through DOS (`rt::dos::File`), so its upper edge for storage is a filesystem service, not `BlockDevice`. The native FAT driver sits on `BlockDevice` and offers the same filesystem service.
- Blocking at first (the core is cooperative; the drive's interrupt wakes a `hlt`), with an asynchronous request queue as a later change if the core needs to keep drawing during a long read.

### A platform as one value (sketch)

```rust
pub trait Platform {
    type Ports: PortIo;
    type Irq: IrqRegistry + InterruptController;
    type Clock: Clock;
    type Display: Display;
    type Input: InputSource;
    fn ports(&self) -> &Self::Ports;
    fn irq(&self) -> &Self::Irq;
    fn clock(&self) -> &Self::Clock;
    fn display(&mut self) -> &mut Self::Display;
    fn input(&mut self) -> &mut Self::Input;
    fn debug(&self) -> &dyn DebugOut;
}
```

The core's entry is `fn run<P: Platform>(p: &mut P) -> !`. Associated types keep everything static; the mock platform makes the whole core testable on the host with scripted input and a frame buffer in memory, which is how `SPEC.md`'s frame checksums can be computed in a host test as well as in DOSBox.

## Concurrency

- **The core is single-threaded and cooperative.** Interrupt handlers are the only concurrency, on both platforms, until the native track decides on preemption (`DIRECTION.md`, "Scheduling"). This is the model the hosted shell already assumes.
- **Hand-off from interrupt to core** is a lock-free single-producer, single-consumer ring (`SpscRing<T, N>`, built): the producer alone writes `head`, the consumer alone writes `tail`, the indices run freely and wrap, the capacity is a power of two, and a full ring drops and counts (`dropped`) rather than blocking an interrupt. `split` hands out the two ends once (`swap` on a flag), so a second producer is a type error, not a race. Counters are a `TickCounter` (`fetch_add`).
- **Atomics allowed on the targets:**

| Operation | Instruction on x86 | 486 | DOSBox `cputype = 486` | Allowed |
|---|---|---|---|---|
| `load`, `store` (any ordering, up to 32 bits) | `mov` (`xchg` for a `SeqCst` store) | yes | yes | yes |
| `swap` | `xchg` | yes | yes | yes |
| `fetch_add`, `fetch_sub` | `lock xadd` | yes (from the 486) | yes ([`D0-RUST-DOS.md`](D0-RUST-DOS.md)) | yes |
| `fetch_and`, `fetch_or`, `fetch_xor`, `fetch_not` | `lock and`/`or`/`xor` if the result is unused, a `cmpxchg` loop if used | yes | loop rejected | no |
| `compare_exchange`, `fetch_update`, `fetch_max`, `fetch_min`, `fetch_nand` | `lock cmpxchg` | yes (from the B step) | **rejected** | no |
| anything 64-bit | `cmpxchg8b` | **no** (Pentium) | no | no (`max-atomic-width: 32`) |

  `hal/clippy.toml` lists the banned methods (`core::sync::atomic::Atomic::*`, which covers every atomic type), so `clippy -D warnings` fails on them. A throwaway crate that instantiates the ring, the clock and all three drivers for the kernel target emitted no `cmpxchg` in its assembly.
- **Critical sections in the core** are "interrupts off" (`cli`, restore IF), a platform function, for the rare multi-word state an interrupt also reads (for example changing a driver's configuration). Under DPMI, `cli` in a ring-3 client is virtualised by the host (or use DPMI 0900h/0901h); the kernel's `sync::without_interrupts` is the native version.

## The unsafe policy

- **One audited boundary per platform**: `RawPorts::new` (port I/O), `MmioRegion::new` and `PhysMem::map_physical` (memory), `IrqHandler::from_raw` and the interrupt stub (interrupts), and the DPMI and `asm!` code in `rt` and the kernel. Each `unsafe` has a one-line `SAFETY:` comment saying what makes it sound (`AGENTS.md`).
- **Drivers contain no `unsafe`.** The three built drivers have none. If a driver needs some (a `rep insw` fast path), it goes behind a HAL method instead.
- **The core contains no `unsafe`** except where it receives a raw frame buffer for the blit fast path, which is a platform-provided safe method in the end.
- `unsafe_op_in_unsafe_fn` style: `unsafe fn` bodies still use `unsafe {}` blocks with their own justification (as `IrqHandler::call` does).

## Errors

- One small `Copy` error type, `HalError` (built): `Unsupported`, `NoDevice`, `Timeout`, `InvalidArgument`, `Busy`, `OutOfMemory`, and `Platform(u16)` for a passed-through DPMI error code. Small enough to return from a panic path, with `Display`, and `std::error::Error` with the `std` feature.
- Device classes add their own where the caller acts on the difference (`BlockError::NoMedia` versus `Io`). A driver error converts to `HalError` with `From` where the core does not care.
- **Bounded waits, never hangs.** Every poll of a status bit has a bound (`WRITE_POLLS` in `desk-serial`, the kernel's same bound). Where the kernel's behaviour on timeout is to carry on (the UART writes the byte anyway), the shared driver keeps it; where carrying on would be wrong (an 8042 command), it returns `Timeout`.
- **Programming errors** (a ring capacity that is not a power of two) fail at compile time where possible (`SpscRing::new` checks its capacity in an inline `const` block, an error wherever the ring is made; `Serial16550::new` asserts its base range, an error in a `const` or `static`) and panic otherwise; out-of-range device arguments are `InvalidArgument` and write nothing (tested for the PIT and the UART).

## no_std, alloc and features

- Every crate in `hal/` is `#![no_std]` outside tests (`#![cfg_attr(not(any(test, feature = "std")), no_std)]`). The drivers and `desk-hal` do not use `alloc`; `desk-hal-mock` does (`Vec`, `VecDeque`, `Rc`), which is why it builds for the targets too.
- **Feature `std`**: only for host builds (tests, a host bench, a host tool). It adds `std::error::Error` for `HalError` and nothing else; the cross builds never enable it. Dev-dependencies turn it on for tests.
- **No other features yet.** Platform choice is not a feature: each binary picks its platform by depending on it. Optional hardware (an OPL3's second bank, a 16550's FIFO) is detected at run time, not configured at compile time.
- **No external crates.** `hal/Cargo.lock` lists only the workspace's own crates.

## How it fits the targets

| Target | Spec | How `hal/` builds for it | Checked by |
|---|---|---|---|
| Host (`x86_64-unknown-linux-gnu` in CI) | built in | `cargo test` | `tools/check.sh`, every run |
| Native kernel | [`kernel/i486-desk-none.json`](../kernel/i486-desk-none.json) | `cargo clippy --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec` with `RUSTC_BOOTSTRAP=1` | `tools/check.sh`, about 8 s cold |
| Pure-Rust DOS | [`spikes/d0-rust-dos/dos-elf.json`](../spikes/d0-rust-dos/dos-elf.json) | `cargo check`, same flags | `tools/check.sh`, about 8 s cold |
| Hosted skeleton's COFF target | [`core/dos.json`](../core/dos.json) | not built | Retired with the C layer, per `D0-RUST-DOS.md`; the hosted Desk adopts the HAL on the ELF target |

- Both real targets are `i486`, 32-bit, `max-atomic-width: 32`, x87, no SIMD. `desk-hal::x86` is compiled only where `target_arch = "x86"`, so the host build has no port instructions and the host tests cannot touch hardware by accident.
- The cross builds take `--target` and `-Zbuild-std` on the command line rather than a `.cargo/config.toml`, so the workspace's default target stays the host's and `cargo test` needs no flags. The kernel and `rt` keep their own configs; when they depend on these crates by path, their `-Zbuild-std` compiles them with everything else.

## The platforms

What each platform supplies, by surface:

| Surface | Hosted DOS (`rt::hal`, planned) | Native (`kernel::hal`, planned) | Mock (`desk-hal-mock`, built) |
|---|---|---|---|
| `PortIo` | `RawPorts` (ring 3 with I/O permission under CWSDPMI; not yet verified, spike A does) | `RawPorts` | `MockPortBus` |
| `InterruptController` | `Pic8259` without `init` | `Pic8259` with `init(REMAPPED_BASES, …)` | `MockIrq` |
| `IrqRegistry` | DPMI 0204h/0205h, a stub per line (built by spike B: `rt::irq`), `Chain::Previous` for IRQ 0, clean-up list | IDT gates 20h–2Fh, a stub per line, EOI after the handler | `MockIrq::raise` |
| `Clock` | `PitClock` ticked by the chained IRQ 0 hook at 18,207 mHz | `PitClock` at the programmed rate | `MockClock` |
| `Delay` | Calibrated loop | Calibrated loop | `MockDelay` |
| `WallClock` | INT 21h 2Ah/2Ch | CMOS RTC | (not yet) |
| `DebugOut` | Standard output (INT 21h), serial on request | Serial and VGA text | Test buffer or `MockPortBus` with a UART model |
| `PhysMem` | DPMI 0100h, 0800h; VDS under EMM386 | Frame allocator, identity map | `Vec` with fake physical addresses |
| `Display` mode set | INT 10h, VBE through DPMI 0300h | Registers (spike D), VBE strategy open | In-memory frame buffer |
| Input | `desk-i8042` keyboard on IRQ 1; INT 33h adapter for the mouse | `desk-i8042` keyboard and mouse | Scripted rings |
| Storage | Files through DOS, no `BlockDevice` | `desk-ata`, `desk-fdc` | In-memory disk |

**The DOS interrupt stub, generalised from `rt::timer`.** One small stub per hooked line: save registers, load `DS` and `ES` with the flat data selector (read through `CS`, as `rt_irq0` does), **switch to a locked interrupt stack in our own data segment**, call `rt_irq_dispatch(line)` (which looks up the `IrqHandler` and calls it), switch back, restore, then either `jmp fword` to the previous handler (`Chain::Previous`) or send the EOI and `iretd`. The stack switch is new and necessary: the host delivers the interrupt on its own stack, whose selector may have a different base from `DS`, and Rust code assumes a pointer to a local can be dereferenced through `DS`. `rt_irq0` gets away without it because it never calls Rust.

## Apps and modules

Apps, whether DXE3 modules on the hosted track ([`S1-DXE3.md`](S1-DXE3.md)) or the native track's future module format, use the **SDK** ([`SDK.md`](SDK.md)), not the HAL: `desk_window_new`, `desk_fill`, `DeskEvent` and the rest, integers, pointers and lengths across a C ABI. An app never holds a `PortIo`, never registers an interrupt handler and never sees a `FrameBuffer`; it draws through the core, which owns the screen and the palette. This keeps apps portable between the tracks without recompiling against a platform, keeps a crashing app from leaving the hardware in a bad state, and keeps the HAL free to change. An app that needs a device (a sound app, a terminal on a serial port) gets an SDK call that the core implements over the HAL. The HAL is internal to the Desk; the SDK is its public face.

## Migration plan

From what exists (the kernel's `port.rs`, `serial.rs`, `pic.rs`, `pit.rs`, `vga.rs`; `rt`'s `timer.rs` and `vga.rs`; the hosted skeleton's `core/` and `c/`), one step at a time, each a pull request that leaves `tools/check.sh` green and its transcript unchanged.

**Step 0 (this spike).** The `hal/` workspace: traits, `SpscRing`, `MmioRegion`, the 16550, 8254 and 8259 drivers, the mock, host tests, cross builds in `tools/check.sh`. Nothing depends on it yet.

**Step 1. The native kernel uses the shared drivers.** Exactly what changes:

- `kernel/Cargo.toml`: `[dependencies] desk-hal = { path = "../hal/hal" }`, and the same for `desk-serial`, `desk-pit`, `desk-pic`. The kernel stays its own workspace; `-Zbuild-std` already in `kernel/.cargo/config.toml` builds the path dependencies for its target. `kernel/Cargo.lock` gains the four crates.
- `kernel/src/port.rs`: deleted. A `pub static PORTS: RawPorts = unsafe { RawPorts::new() };` (or a `const`) in a new `kernel/src/hal.rs`, with the `SAFETY:` line "the kernel runs in ring 0".
- `kernel/src/serial.rs`: becomes `Serial16550::new(PORTS, COM1)`; `serial::init()` is `SERIAL.init(115_200).unwrap()`; `console.rs` writes through it. Same port writes (tested: `init_matches_the_kernel_byte_for_byte`).
- `kernel/src/pic.rs`: becomes `Pic8259::new(PORTS)`; `init()` is `PIC.init(REMAPPED_BASES, 0xFFFE)` (same writes and delays, tested); `mask_timer()` is `PIC.mask(Irq::TIMER)`; `eoi_master()` is `PIC.eoi(Irq::TIMER)`, called by the dispatch, not by the timer code.
- `kernel/src/pit.rs`: becomes `static CLOCK: PitClock`, `Pit::new(PORTS).program_clock(&CLOCK, 100)` (the same writes, tested, and the clock's rate recorded in the same step) and `IrqHandler::from_shared(CLOCK.counter())` registered on IRQ 0; `pit::ticks()` is `CLOCK.now().0`. The `pit: channel 0 at 100 Hz` line keeps its text.
- `kernel/src/idt.rs`: `irq0_stub` calls a `dispatch(0)` that calls the registered `IrqHandler` and then `PIC.eoi`, instead of `timer_irq`; `kernel::hal` implements `IrqRegistry` over a 16-entry table. Gates for IRQ 1–15 come with spike C.
- `kernel/src/vga.rs`: moves to `desk-vgatext` over an `MmioRegion` at `B8000h` (optional in this step).
- Acceptance: `tests/native/expected.txt` unchanged, every negative mode unchanged (`notick` now masks through `Pic8259`).
- Risks: code size (the generics inline; measure `kernel.bin` before and after; LTO and `opt-level = "s"` should keep it flat); the `asm!` blocks without `nomem` stop a few optimisations around port I/O, which is the point but should be measured; the panic path uses the serial driver, which must not take a lock (it takes none).

**Step 2. The DOS runtime becomes the hosted platform.** Promote `spikes/d0-rust-dos/rt` to a top-level `rt/` (the d0 recommendation) and add `rt::hal`:

- `RawPorts` for port I/O, verified first by spike A (DAC writes from ring 3 under CWSDPMI).
- `rt::timer` generalised into the per-line stub and `IrqRegistry` described [above](#the-platforms), with the stack switch; IRQ 0 keeps `Chain::Previous`, so the `hw` transcript (18 to 20 ticks a second, the BIOS counter in step) is unchanged, and the tick count moves to a `PitClock` at 18,207 mHz.
- `rt::vga::framebuffer()` returns an `MmioRegion` over `A0000h`; the mode set stays in `rt` (INT 10h); the checksum code moves to a test.
- `PhysMem` over DPMI 0100h and 0800h/0801h, `WallClock` over INT 21h, `DebugOut` over standard output.
- Acceptance: every d0 transcript unchanged, plus a new `irq` test program that registers a handler through `IrqRegistry` and counts.
- Risks: the interrupt stack (size, locking, the selector), heap-allocated handler contexts that are not locked, and `cli` from ring 3 under hosts other than CWSDPMI.

**Step 3. The hosted skeleton.** `core/` does not touch hardware today and keeps not touching it. When the shell starts, it is written against `Platform` and moves to the ELF target with `rt` (the C layer and `coff2go32.py` retire, per `D0-RUST-DOS.md`); `c/` gets no new hardware code in the meantime. Risk: none until the shell exists; the decision to retire the C layer is the owner's.

**Step 4. New drivers land in `hal/drivers/`** from spikes A, B, C and D, each with host tests against the mock and a device model added to `desk-hal-mock` where the driver needs one.

**Step 5. The core runs on both platforms** through `Platform`, and on the mock in host tests.

## Follow-up spikes

The owner's next batch, in order, and how each uses the HAL so the tracks converge:

- **G: boot the kernel in 86Box and Bochs.** No new HAL surface. Do migration step 1 first (or as part of G), so the shared serial, PIT and PIC drivers are what 86Box and Bochs exercise; then the transcript in each emulator tests the shared code, and a difference between emulators is a driver fix in one place. Record emulator-specific timing (the `io_wait` port, the PIT rate under Bochs's `ips`) in `NATIVE-BOOT.md`.
- **E: the `std` stubs experiment.** Must not make any `hal/` crate depend on `std`; the HAL stays `no_std` whatever E finds. If `std` works, it is for apps over the SDK (`RUST-TARGETS.md`, space (d)), and the `std` feature here stays a host-only convenience.
- **A: VGA and VBE from Rust on the DOS runtime.** Builds `desk-vga` and the `Display` surface; the contract is [below](#contract-for-spike-a-vga-and-vbe-on-the-dos-runtime).
- **C: keyboard and PS/2 mouse in the kernel.** Builds `desk-i8042`, `desk-kbd` and the native `IrqRegistry` for IRQ 1 and 12; the contract is [below](#contract-for-spike-c-keyboard-and-ps2-mouse-in-the-kernel).
- **B: DOS keyboard and mouse.** Reuses C's keyboard handler and decoder unchanged on IRQ 1 with `Chain::None` under `rt::hal`; adds only the INT 33h adapter that fills the same `SpscRing<MouseEvent, 32>`. Success is the core-side test passing on the same `KeyEvent` and `MouseEvent` streams on both tracks.
- **D: VGA in the kernel.** Reuses A's `desk-vga` unchanged; adds the native mode set (register tables for mode 13h and 12h, written through `PortIo`, which can live in `desk-vga` as `set_mode_registers` because the register values are the same everywhere) and decides the VBE strategy (`DIRECTION.md`, "Graphics driver strategy").
- **s5: js-dos.** Hosted only. Tests that the hosted `Display` falls back from the linear frame buffer to banked VBE or mode 13h when the browser build lacks it; no new HAL surface, but the fallback must be in the DOS platform, not the core.

### Contract for spike A: VGA and VBE on the DOS runtime

Spike A implements, in this order, and its check proves each:

1. **Port I/O from the DOS runtime.** `rt` uses `desk_hal::x86::RawPorts`; a test program writes all 256 DAC entries through `0x3C8`/`0x3C9` and reads them back through `0x3C7`/`0x3C9`, equal, in headless DOSBox. This is the first evidence that ring-3 port I/O works under CWSDPMI.
2. **`hal/drivers/vga` (`desk-vga`)**, no `unsafe`, no INT 10h, over `PortIo` and `MmioRegion` only: `set_palette(&P, first: u8, &[Rgb6])`, `read_palette(&P, first: u8, &mut [Rgb6])`, `wait_vretrace(&P) -> Result<(), HalError>` (bounded on `0x3DA` bit 3), mode 13h pixel access on an `MmioRegion`, and the mode 12h plane write path (map mask and graphics controller). Host tests against a DAC and sequencer model in `desk-hal-mock`.
3. **The types** `Rgb6`, `PixelLayout`, `ModeInfo`, `FrameBuffer` and the `Display` trait, added to `desk-hal` as sketched [above](#video-sketch-spike-a-builds-it); A may change them, and records why in its result.
4. **`rt::hal::DosDisplay: Display`**: modes from VBE 4F00h/4F01h (through DPMI 0300h and the transfer buffer), `set_mode` by INT 10h or VBE 4F02h with the linear-frame-buffer bit, the frame buffer mapped with DPMI 0800h as an `MmioRegion`, the return to text mode and the 0801h unmapping on `rt`'s clean-up list.
5. **Checks** in `spikes/` or `tests/dos/`: the mode 13h checksum (as `hw` does, now through `desk-vga`), a VBE 640×480×8 linear-frame-buffer checksum if DOSBox Staging offers the mode, the palette round trip, and a panic in graphics mode that returns to text mode.

Out of scope for A: the compositor, fonts, any native code.

### Contract for spike C: keyboard and PS/2 mouse in the kernel

Spike C implements:

1. **The native `IrqRegistry`** in `kernel::hal` for IRQ 0–15: gates 20h–2Fh to one stub per line, a `dispatch(line)` that checks `is_spurious` for 7 and 15, calls the registered `IrqHandler`, then sends the EOI through `desk-pic` (master only for a spurious 15). Migration step 1 for IRQ 0 is a prerequisite or part of C.
2. **`hal/drivers/i8042` (`desk-i8042`)**, over `PortIo`: controller set-up (disable both ports, flush the output buffer, read and write the configuration byte, self-test `0xAA` → `0x55`, enable the auxiliary port), mouse set-up (reset `0xFF` → `0xFA 0xAA 0x00`, defaults `0xF6`, reporting on `0xF4`), every wait on status bits 0 and 1 of `0x64` bounded and returning `Timeout`. Two interrupt services:
   - `KbdIrq { ports, tx: Producer<'static, u8, 64> }`: on IRQ 1, if status bit 0 is set, read one byte from `0x60` and push it. No decoding.
   - `MouseIrq { ports, tx: Producer<'static, MouseEvent, 32>, packet: [u8; 3], index: u8 }`: on IRQ 12, read one byte; resynchronise when byte 0 lacks bit 3; on the third byte build a `MouseEvent` (nine-bit signed deltas, `dy` negated so positive is down, buttons from bits 0–2) and push it; drop packets with the overflow bits set.
3. **`hal/drivers/kbd` (`desk-kbd`)**: `Decoder::feed(&mut self, Scancode) -> Option<KeyEvent>` for scancode set 1 with the `E0` prefix and the `E1` Pause sequence, `KeyCode` as a Desk enum, and lock-key state. Pure; host-tested with recorded byte sequences.
4. **Mock models**: an 8042 with a keyboard and a mouse behind it in `desk-hal-mock`, so `desk-i8042`'s set-up and both services are host-tested, including the timeout paths and packet resynchronisation.
5. **Checks** in `tools/native-test.sh`: QEMU's monitor (a socket instead of `-monitor none`) sends keys with `sendkey` and moves the mouse with `mouse_move`; the kernel prints the decoded `KeyEvent`s and summed `MouseEvent`s to serial; the transcript is compared as today. A negative check: a masked IRQ 1 yields no keys.

Out of scope for C: key repeat and layouts beyond US, the mouse cursor, DOS.

## Open questions

- **Physical addresses under a V86 memory manager.** DMA from DOS under EMM386 or QEMM needs Virtual DMA Services; whether to support it, refuse sound, or require a plain `HIMEM.SYS` setup.
- **Port I/O permission under DPMI hosts other than CWSDPMI** (Windows 9x's DOS box traps and emulates, HDPMI may differ). Spike A tests CWSDPMI only.
- **`cli` in a ring-3 DPMI client**: CWSDPMI's IOPL decides whether it traps; the virtual-interrupt calls (0900h/0901h) are the portable route. To be measured when the DOS platform needs a critical section.
- **The DOS interrupt stack**: one shared locked stack, or one per line; its size; whether nested hardware interrupts (a higher-priority line during a handler) can happen under the host.
- **VBE natively**: real-mode BIOS calls before entering protected mode (no mode change later), a V86 monitor, or register-level drivers per card; this decides how much of `Display` the native platform can offer (`DIRECTION.md`). *Spike n3 answered it for now: a real-mode trampoline called from the kernel behind the same `Bios` trait, a V86 monitor later; see [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md#hal-feedback).*
- **Whether the module loader belongs behind the HAL** (`DIRECTION.md`): this design says no; the loader is part of the core on each track, and only its memory comes from the platform.
- **Preemption.** If the native track becomes preemptive, the single-consumer side of the rings and the "interrupts off" critical section need revisiting; the rings' single-producer side does not.
- **Where `rt` and these crates live long-term**: this repository, or `liminal-hq/dos-rs` for the DOS-facing parts.

## What was and was not verified

Verified on the development machine (Arch Linux, Rust 1.98.0 as pinned):

- `cargo test` in `hal/`: 35 tests. They cover: the 16550's `init` writing exactly the kernel's seven values in order, the divisor table and refused rates, text reaching a UART model, the bounded wait on a stuck transmitter (100,000 status reads, then the byte is written anyway, as in the kernel), the loopback probe on a UART and on an empty slot, with stale received bytes drained and an echo that arrives late, non-blocking reads; the PIT divisor rounding (11,932 for 100 Hz, as in the kernel) and range (19 Hz to 795,454 Hz), the actual rate in millihertz, the kernel's three writes for 100 Hz, a chip model seeing mode 2, the BIOS's mode 3 and rate (written as 0) restored, refused clock divisors, nothing written for a refused rate, the counter latch (and an 8254 model whose read sequence only a control word resets, as on the chip), the clock's rate recorded with the programming, and ticks delivered through a handler registered from a plain `static` with the platform sending the end-of-interrupt; the 8259's native `init` reproducing the kernel's sequence with its eight I/O delays, read-modify-write masking that unmasks the cascade for a slave line, the end-of-interrupt order, spurious IRQ 7 and 15, and the master-only acknowledgement of a spurious IRQ 15; the ring's order, drops, wrap and a producer thread against a consumer thread; `MmioRegion` bounds and alignment; the mock bus's precedence, scripts and their failures.
- `cargo fmt --check` and `cargo clippy --all-targets -D warnings` on the host; `cargo clippy -D warnings` for `kernel/i486-desk-none.json` and `cargo check` for `spikes/d0-rust-dos/dos-elf.json`, both with `-Zbuild-std=core,alloc`.
- The `clippy.toml` ban fires on `compare_exchange` and `fetch_or` (checked with a temporary function, then removed). A throwaway crate instantiating the ring, `PitClock` with its shared handler, and the three drivers over `RawPorts` (the probe and the spurious acknowledgement included) for the kernel target emitted no `cmpxchg`, its port instructions inline, and `xchg` for `split`; rerun after the review fixes.
- `tools/check.sh` passes in full from a clean clone inside `ghcr.io/liminal-hq/ci-dos` (the digest in `.github/workflows/ci.yml`, with QEMU and NASM added as the workflow does); the HAL step alone takes about 18 s there from cold, cargo downloads included.

Not verified:

- Anything in the kernel, under DOS, in an emulator or on hardware: no binary uses these crates yet, and `RawPorts` has never executed an instruction.
- That the kernel and the d0 transcripts stay unchanged after migration (steps 1 and 2 are designed, not done), and the code-size cost.
- Port I/O from ring 3 under CWSDPMI, the DOS interrupt stub with a stack switch, locked heap contexts, VDS, VBE, the 8042, sound and storage: all designed only.
- Equivalence with the kernel is shown for the port sequences of `init`, `write_byte`, the PIT programming and the PIC initialisation, by tests against the kernel's values written out by hand, not by running the kernel against the mock.
