# hal

The shared hardware abstraction layer and the drivers written once against it: one Cargo workspace of `no_std` library crates that build for the host (where the tests run), for the native kernel's target and for the pure-Rust DOS target. The design, and what is built versus only designed, is in [`../docs/HAL.md`](../docs/HAL.md). The native kernel uses it for serial, the PIC, the PIT and the keyboard and mouse.

| Crate | Directory | What it is |
|---|---|---|
| `desk-hal` | `hal/` | The traits and types: `PortIo`, `InterruptController`, `IrqRegistry`, `IrqHandler`, `IrqService` and `SharedIrqService`, `Clock`, `Delay`, `DebugOut`, `SpscRing`, `MmioRegion`, `HalError`, and `x86::RawPorts` (the real `in` and `out` instructions, x86 targets only) |
| `desk-serial` | `drivers/serial/` | The 16550 UART, equivalent to `kernel/src/serial.rs` at COM1 and 115,200 baud |
| `desk-pit` | `drivers/pit/` | The 8254 timer: channel 0 rate, the counter latch, and `PitClock` |
| `desk-pic` | `drivers/pic/` | The two 8259s: native initialisation, masking, end-of-interrupt, spurious checks |
| `desk-kbd` | `drivers/kbd/` | The scancode set 1 `Decoder` (the `E0` prefix, Pause, modifiers and lock keys) and the US key map |
| `desk-i8042` | `drivers/i8042/` | The 8042 controller, the keyboard and mouse set-up, the IRQ 1 and IRQ 12 services that fill the rings, and `Ps2Input` for the core side (`docs/C-INPUT.md`) |
| `desk-hal-mock` | `platform/mock/` | The host platform for tests: `MockPortBus` (records, replays scripts, attaches device models), models of the 16550, the 8254 and the 8042 with a keyboard and a mouse, `MockIrq`, `MockClock`, `MockDelay` |

```bash
cargo test                       # host tests against the mock
RUSTC_BOOTSTRAP=1 cargo check --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec
RUSTC_BOOTSTRAP=1 cargo check --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec
```

`tools/check.sh` runs all three, with `cargo fmt --check` and `clippy -D warnings`. `clippy.toml` bans compare-and-swap and the atomic operations that compile to a `CMPXCHG` loop.
