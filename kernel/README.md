# kernel

The native track's `no_std` Rust kernel for 486-class PCs. It brings up the serial port and VGA text screen, a GDT and IDT, the PIC and PIT (through the shared drivers in `../hal/`), the keyboard and PS/2 mouse, the memory map, a frame allocator and heap, and the x87, then prints a transcript and ends the run through QEMU's debug-exit device. `i486-desk-none.json` is its target, `link.ld` its linker script, and `.cargo/config.toml` sets `-Zbuild-std`. The design, the target fields and how it is checked are in [`../docs/NATIVE-BOOT.md`](../docs/NATIVE-BOOT.md).

Build with [`../tools/native-build.sh`](../tools/native-build.sh), which sets `RUSTC_BOOTSTRAP=1` for the pinned stable toolchain; to build by hand, run `RUSTC_BOOTSTRAP=1 cargo build --release` in this directory.
