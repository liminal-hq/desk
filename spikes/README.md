# spikes

Small experiments that decide the architecture. Spikes s1 to s5 below belong to the hosted track (Desk on DOS). Native-track spikes will be listed here as they start; the first, **n0 boot** (a bootable Rust kernel in QEMU), is done as the native boot skeleton (`docs/NATIVE-BOOT.md`). Each spike lives in its own folder and ends with a written result in `docs/` (what worked, what did not, what it changes):

- **s1-dxe3**: can Rust code run as a DXE3 module that calls back into the core? **Done: yes, with limits** (`docs/S1-DXE3.md`; runs from `tools/check.sh`).
- **d0-rust-dos** (done, hosted track; `docs/D0-RUST-DOS.md`: yes, a pure Rust DOS program with no gcc, no libc and no crt0 runs under CWSDPMI): a Rust-only DOS program with its own `_start`, DPMI wrappers and INT 21h output, linked by `lld` and packed with CWSDPMI's stub, run headless in DOSBox; if it works, the hosted skeleton's C layer and `coff2go32.py` can be retired.
- **g-boot-emulators** (native track; `docs/NATIVE-EMULATORS.md`): does the native image boot outside QEMU and SeaBIOS? **Done: yes**, in Bochs as a 486 with three BIOS images and in 86Box on a 1993 AMIBIOS board, after a loader fix for BIOSes without E820 or E801. Opt-in checks: `tools/native-test-bochs.sh` (also `NATIVE_EXTRA=1 tools/check.sh`) and `tools/native-test-86box.sh`; both need Docker.
- **s2-vbe**: VBE 2.0 linear frame buffer and VGA mode 12h from Rust.
- **s3-mouse**: INT 33h from Rust.
- **s4-handoff**: `DESK.COM` hands the machine to a real-mode program and takes it back.
- **s5-jsdos**: does the browser build work with the linear frame buffer?
- **s6-ttf**: can an integer rasteriser draw a real TrueType face at 14 px and 24 px on a 486 within the frame budget, and how does it look beside the bitmap faces? Later than the others; see `docs/OPEN-QUESTIONS.md`, "Reading TrueType fonts".

The hosted walking skeleton that proves the build toolchain is done (`docs/TOOLCHAIN.md`), and so are the native boot skeleton (`docs/NATIVE-BOOT.md`) and `s1-dxe3`; the next hosted spike is `s2-vbe`.

Spike code is experimental and may be thrown away; it is labelled `spike` and `experimental` on GitHub.
