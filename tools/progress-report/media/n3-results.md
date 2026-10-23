| Check | std | cirrus | vmware | QEMU 8.2.2 (std, cirrus, vmware) | Bochs 2.7, BIOS-bochs-legacy | BIOS-bochs-latest | SeaBIOS 1.13 under Bochs |
|---|---|---|---|---|---|---|---|
| VBE version, modes listed | 3.0, 93 | 3.0, 31 | 3.0, 93 | the same | 2.0, 24 | 2.0, 24 | none: 4F00h answers "not supported" and the call returns cleanly |
| VBE 4F02h set 640×480×8 LFB, picture checksum | `0x5bc65936`, equals spike A | same | same | same | same | same | not reached |
| 800×600×8 | set, matches memory | same | same | same | same | same | not reached |
| `INT 10h` AX=0003h, text screen restored | yes, screenshot is text | **yes** | yes | yes | yes (BIOS data area mode 3; no screenshot) | yes (same) | not reached |
| Set, draw, text, twice | yes | yes | yes | yes | yes | yes | not reached |
| `INT 13h` floppy read, equal to the bytes in memory | yes | yes | yes | yes | yes | yes | yes |
| `INT 13h` LBA (AH=42h) on IDE | yes | | | | not run | not run | not run |
| E820 after boot equals the loader's map | yes | yes | yes | yes | yes | yes | yes |
| `INT 1Ah` equals the CMOS | yes | yes | yes | yes | yes | yes | yes |
| 1,000-call stress, state identical | yes | yes | yes | yes | yes | yes | yes |
| 100 kernel ticks per RTC second | yes | yes | yes | yes | yes | yes | yes |
| Error paths, `InInterrupt`, `NoHandler`, `Busy` | yes | yes | yes | yes | yes | yes | yes |
