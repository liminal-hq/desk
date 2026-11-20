# dos

The Rust-on-DOS foundation: what a Rust program needs to run as a 32-bit DOS program under a DPMI host, with no C compiler, no C library and no `crt0`. It is the layer above `hal/` and below everything that runs on DOS (`docs/LAYERS.md`), and it names nothing but `hal/`.

| Path | What it is |
|---|---|
| `rt/` | The runtime crate: the entry, the DPMI client, INT 21h, the heap, IRQ stubs, the timer, the panic path, x87 set-up, clean-up on exit, and the DOS implementation of the HAL (`rt/src/hal.rs`) |
| `dos-elf.json` | The target specification: a freestanding i486 ELF target, x87 hard float, SIMD off |
| `link.ld` | The linker script `rust-lld` uses with it |
| `.cargo/config.toml` | The unstable options Rust needs to build `core` and `alloc` for this target (`-Zbuild-std`), and the linker flags. Programs that use the runtime carry the same file next to them |
| `examples/` | The example programs (`hello`, `hw`, `panicky`, `ctx`, `retstart`) the runtime's transcripts are recorded from |
| `expected/` | The recorded transcripts, which `check.sh` compares each program's output with |
| `build.sh`, `check.sh` | Build the examples into `dist/d0/` and run each in headless DOSBox against its transcript |
| `tools/elf2go32.py` | The packer that turns the linked ELF into a go32 executable behind DJGPP's or CWSDPMI's stub |
| `tools/dosrun.sh` | The headless DOSBox harness: runs a DOS program under DOSBox and returns its status and output |
| `tools/transcript.sh` | The helpers `check.sh` uses to run a program, normalise what it prints and compare it with a transcript |

The `d0-rust-dos` spike (`spikes/d0-rust-dos/`) keeps only stage 1, which links one example through DJGPP's `ld`. The design and what was learned are in [`docs/D0-RUST-DOS.md`](../docs/D0-RUST-DOS.md) and [`docs/RUST-TARGETS.md`](../docs/RUST-TARGETS.md).

The workspace has its own lockfile and is not built for the host. `tools/check.sh` checks its format and lints in the `rust:hal` part; its examples are built and run, and stage 1 after them, in the `dos:d0` part, and the other DOS groups link it.
