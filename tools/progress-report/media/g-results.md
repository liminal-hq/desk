| Emulator | BIOS | CPU model | Result | Differences from the QEMU transcript |
|---|---|---|---|---|
| QEMU 11.1 `-cpu 486` | SeaBIOS (QEMU's) | QEMU 486 | pass, the reference | E820, 6 entries, 15,871 KiB usable |
| QEMU `-cpu pentium`, `-machine q35`, `-machine isapc` | SeaBIOS | Pentium, 486 | pass | `isapc` has 5 entries and 15,999 KiB; `q35` has 8 entries and 15,867 KiB |
| Bochs 2.7 as a 486 (`--enable-cpu-level=4`) | `BIOS-bochs-legacy` | 486DX (`486ni` only) | pass, the full test (loader variants, faults, hang) | E820, 5 entries, 15,996 KiB; A20 already on |
| Bochs 2.7 as a 486 | `BIOS-bochs-latest` | as above | pass, the full test | E820, 6 entries, the last one "ACPI reclaimable" (kept reserved by the kernel), 15,932 KiB |
| Bochs 2.7 as a 486 | SeaBIOS 1.13 (shipped with Bochs) | as above | pass, the full test | E820, 6 entries, 15,991 KiB |
| Bochs 2.7 from Ubuntu's package, Pentium model | `BIOS-bochs-legacy` | `pentium` | pass (by hand) | as the legacy row; with `BIOS-bochs-latest` the BIOS itself crashes before the boot sector (see [What failed](#what-failed-on-the-way)) |
| 86Box 6.0, `ami471` (SiS 471, 33 MHz 486DX, 16 MiB) | AMIBIOS 08/08/93 | 486DX | pass after a loader fix; `tools/native-test-86box.sh` runs the normal boot and a divide error | A20 closed at boot (the loader opens it through the fast gate); neither E820 nor E801, so the AH=88 fallback runs, 2 entries, 16,000 KiB; the BIOS enables the cache, so the CR0 dump is `0x00000033` as on QEMU |
