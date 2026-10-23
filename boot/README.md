# boot

The native track's boot path, in NASM. `stage1.asm` is the 512-byte boot sector: it reads stage 2 and the kernel from the floppy with INT 13h. `stage2.asm` opens A20, reads the BIOS memory map, enters 32-bit protected mode with flat segments and starts the kernel at 1 MiB. Both are commented with why each step exists. The flow, the image layout and the debugging notes are in [`../docs/NATIVE-BOOT.md`](../docs/NATIVE-BOOT.md).

Build with [`../tools/native-build.sh`](../tools/native-build.sh) and assemble the image with [`../tools/native-image.sh`](../tools/native-image.sh).
