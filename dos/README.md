# dos

The Rust-on-DOS foundation: what a Rust program needs to run as a 32-bit DOS program under a DPMI host, with no C compiler, no C library and no `crt0`. It is the layer above `hal/` and below everything that runs on DOS (`docs/LAYERS.md`), and it names nothing but `hal/`.

| Path | What it is |
|---|---|
| `rt/` | The runtime crate: the entry, the DPMI client, INT 21h, the heap, IRQ stubs, the timer, the panic path, x87 set-up, clean-up on exit, and the DOS implementation of the HAL (`rt/src/hal.rs`) |
| `dos-elf.json` | The target specification: a freestanding i486 ELF target, x87 hard float, SIMD off |
| `link.ld` | The linker script `rust-lld` uses with it |
| `.cargo/config.toml` | The unstable options Rust needs to build `core` and `alloc` for this target (`-Zbuild-std`), and the linker flags. Programs that use the runtime carry the same file next to them |

The packer that turns the linked ELF into a go32 executable (`tools/elf2go32.py`) and the headless DOSBox harness (`tools/dosrun.sh`) are still in `tools/`; the programs that use the runtime, with their expected transcripts, are the `d0-rust-dos` spike (`spikes/d0-rust-dos/`). The design and what was learned are in [`docs/D0-RUST-DOS.md`](../docs/D0-RUST-DOS.md) and [`docs/RUST-TARGETS.md`](../docs/RUST-TARGETS.md).

The workspace has its own lockfile and is not built for the host. `tools/check.sh` checks its format and lints in the `rust:hal` part; the programs that link it are built and run in the `dos:d0` part and the other DOS groups.
