# hal

The shared hardware abstraction layer and the drivers written once against it: one Cargo workspace of `no_std` library crates that build for the host (where the tests run), for the native kernel's target and for the pure-Rust DOS target. The design, and what is built versus only designed, is in [`../docs/HAL.md`](../docs/HAL.md). Nothing outside this workspace uses it yet.

| Crate | Directory | What it is |
|---|---|---|
| `desk-hal` | `hal/` | The traits and types: `PortIo`, `InterruptController`, `IrqRegistry` and `IrqHandler`, `Clock`, `Delay`, `WallClock`, `DebugOut`, `SpscRing`, `MmioRegion`, `HalError`, and `x86::RawPorts` (the real `in` and `out` instructions, x86 targets only) |
| `desk-serial` | `drivers/serial/` | The 16550 UART, equivalent to `kernel/src/serial.rs` at COM1 and 115,200 baud |
| `desk-pit` | `drivers/pit/` | The 8254 timer: channel 0 rate, the counter latch, and `PitClock` |
| `desk-pic` | `drivers/pic/` | The two 8259s: native initialisation, masking, end-of-interrupt, spurious checks |
| `desk-hal-mock` | `platform/mock/` | The host platform for tests: `MockPortBus` (records, replays scripts, attaches device models), models of the 16550 and 8254, `MockIrq`, `MockClock`, `MockDelay` |

```bash
cargo test                       # host tests against the mock
RUSTC_BOOTSTRAP=1 cargo check --target ../kernel/i486-desk-none.json -Zbuild-std=core,alloc -Zjson-target-spec
RUSTC_BOOTSTRAP=1 cargo check --target ../spikes/d0-rust-dos/dos-elf.json -Zbuild-std=core,alloc -Zjson-target-spec
```

`tools/check.sh` runs all three, with `cargo fmt --check` and `clippy -D warnings`. `clippy.toml` bans compare-and-swap and the atomic operations that compile to a `CMPXCHG` loop.
