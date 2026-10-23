# spikes

Small experiments that decide the architecture. Spikes s1 to s5 below belong to the hosted track (Desk on DOS). Native-track spikes will be listed here as they start; the first, **n0 boot** (a bootable Rust kernel in QEMU), is done as the native boot skeleton (`docs/NATIVE-BOOT.md`). Each spike lives in its own folder and ends with a written result in `docs/` (what worked, what did not, what it changes):

- **s1-dxe3**: can Rust code run as a DXE3 module that calls back into the core? **Done: yes, with limits** (`docs/S1-DXE3.md`; runs from `tools/check.sh`).
- **d0-rust-dos** (done, hosted track; `docs/D0-RUST-DOS.md`: yes, a pure Rust DOS program with no gcc, no libc and no crt0 runs under CWSDPMI): a Rust-only DOS program with its own `_start`, DPMI wrappers and INT 21h output, linked by `lld` and packed with CWSDPMI's stub, run headless in DOSBox; if it works, the hosted skeleton's C layer and `coff2go32.py` can be retired.
- **H: HAL design** (done, both tracks; `docs/HAL.md`): the hardware abstraction layer and the shared drivers. The code is not a spike folder but the `hal/` workspace (traits, the 16550, 8254 and 8259 drivers, and a mock port bus they are tested against on the host); its contracts direct the next spikes: G (86Box and Bochs), E (`std` stubs), A (VGA and VBE on the DOS runtime), C (keyboard and PS/2 mouse in the kernel), then B, D and s5.
- **C (n1): keyboard and PS/2 mouse in the kernel** (done, native track; `docs/C-INPUT.md`): the shared `desk-i8042` and `desk-kbd` drivers, the kernel's interrupt registry and the QEMU input checks that send keys and mouse motion through the monitor. Verified in QEMU and on the host only; nothing on real hardware.
- **A (s2-vbe)** (`spikes/a-vga-vbe`, hosted and shared; `docs/A-VGA-VBE.md`): VGA mode 13h and 12h by register programming, the DAC and fades, and VBE 2.0 with a linear frame buffer from Rust, through the shared driver `desk-vga`. **Done in DOSBox Staging**: every mode draws and reads back its checksum, VBE 640×480×8 works through DPMI 0800h; not yet run on real hardware, 86Box or QEMU.
- **s2-vbe**: VBE 2.0 linear frame buffer and VGA mode 12h from Rust.
- **s3-mouse**: INT 33h from Rust.
- **s4-handoff**: `DESK.COM` hands the machine to a real-mode program and takes it back.
- **s5-jsdos**: does the browser build work with the linear frame buffer?
- **s6-ttf**: can an integer rasteriser draw a real TrueType face at 14 px and 24 px on a 486 within the frame budget, and how does it look beside the bitmap faces? Later than the others; see `docs/OPEN-QUESTIONS.md`, "Reading TrueType fonts".

The hosted walking skeleton that proves the build toolchain is done (`docs/TOOLCHAIN.md`), and so are the native boot skeleton (`docs/NATIVE-BOOT.md`) and `s1-dxe3`; the next hosted spikes are s3 to s5, and spike A (`s2-vbe`) is done in DOSBox.

Spike code is experimental and may be thrown away; it is labelled `spike` and `experimental` on GitHub.
