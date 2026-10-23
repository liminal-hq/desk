| Program | Check | js-dos 8.5.3 (DOSBox backend) | Native Staging |
|---|---|---|---|
| `HELLO.EXE` (text, d0) | Standard output equals `spikes/d0-rust-dos/expected/hello.txt` | identical, 22 lines, exit status 0 | identical |
| `HELLO.EXE` as `solo/HELLO.EXE` (`CWSDSTUB.EXE`, no `CWSDPMI.EXE`) | same | identical | identical |
| `HELLO.EXE` with `core = normal`, `cputype = 486` | same | identical | identical |
| `HELLO.EXE exit 7` | Exit status read back from the emulated disk | 7 | 7 |
| `DESK.EXE` (C layer plus Rust core) | The skeleton line on standard output and in `OUT.TXT` | identical, exit status 0 | identical |
| `HW.EXE` (IRQ 0 hook, mode 13h, d0) | Standard output equals `expected/hw.txt` | identical (ticks in range, vector restored, frame buffer checksum `0x49fd8cc5` matches) | identical |
| `VGA13.EXE` (mode 13h held on screen) | Screenshot against the computed reference | 320×200, 256 colours, 0 of 64 000 pixels differ; checksum matches | checksum `0x49fd8cc5` matches (no screenshot) |
| `VBE.EXE` (640×480×8, **linear frame buffer**) | Screenshot against the computed reference; read-back checksum | 640×480, 256 colours, 0 of 307 200 pixels differ; checksum `0x3d8bcade` matches | checksum matches |
| `VBE.EXE banked` (same mode, bank window) | same | 0 of 307 200 pixels differ; 5 bank switches, none failed | matches |
| `VBE.EXE` under `machine = vesa_nolfb` | The LFB is withheld | no mode has the LFB attribute and the physical base is 0; the probe reports it and exits with status 3 | not run |
| `HOST.EXE life`, `panic`, `fault div catch` (s1 modules) | Transcript equals `spikes/s1-dxe3/expected/` without the `mem:` lines | identical (17, 26, 16 lines); the DPMI exception path (the caught `SIGFPE`) works | identical |
| `TONE.EXE` | Pitches from the emulator's sample stream; the hardware's own answers | OPL2 440 Hz, Sound Blaster DMA 400 Hz, PC speaker 440.1 Hz; AdLib detected, DSP 2.1, DMA terminal count reached, IRQ 7 pending | not run (the native harness has sound off) |
