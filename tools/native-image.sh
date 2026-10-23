#!/bin/bash
# Assembles the 1.44 MB floppy image from the pieces tools/native-build.sh made.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-image.sh [--out FILE] [--test-mode NAME]
#
# Layout (512-byte sectors): LBA 0 stage 1, LBA 1-17 stage 2, LBA 18 onwards the flat kernel, zero padding to 2880 sectors
# (1,474,560 bytes). Two bytes are patched in: the kernel's length in sectors into stage 1 (offset 3) and the test mode into
# stage 2 (offset 3). Test modes make the kernel misbehave on purpose for the negative checks: none, div0, gp, ud, panic, hang,
# notick.
set -euo pipefail
cd "$(dirname "$0")/.."

out=dist/desk-native.img
mode=none
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --test-mode) mode="$2"; shift 2 ;;
    *) echo "usage: tools/native-image.sh [--out FILE] [--test-mode NAME]" >&2; exit 2 ;;
  esac
done
case "$mode" in
  none) code=0 ;; div0) code=1 ;; gp) code=2 ;; ud) code=3 ;; panic) code=4 ;; hang) code=5 ;; notick) code=6 ;;
  *) echo "unknown test mode: $mode" >&2; exit 2 ;;
esac

in=dist/native
size=$((2880 * 512))
kbytes=$(stat -c %s "$in/kernel.bin")
ksectors=$(((kbytes + 511) / 512))
# Stage 1 reads the kernel into 0x10000-0x9FFFF; keep a margin below the BIOS data at 0x9FC00.
if [ "$ksectors" -gt 1000 ]; then echo "kernel too large for the low load buffer: $ksectors sectors" >&2; exit 1; fi
[ "$(stat -c %s "$in/stage1.bin")" = 512 ] || { echo "stage 1 is not 512 bytes" >&2; exit 1; }
[ "$(stat -c %s "$in/stage2.bin")" = $((17 * 512)) ] || { echo "stage 2 is not 17 sectors" >&2; exit 1; }

mkdir -p "$(dirname "$out")"
head -c "$size" /dev/zero > "$out"
dd if="$in/stage1.bin" of="$out" bs=512 seek=0 conv=notrunc status=none
dd if="$in/stage2.bin" of="$out" bs=512 seek=1 conv=notrunc status=none
dd if="$in/kernel.bin" of="$out" bs=512 seek=18 conv=notrunc status=none
# Little-endian 16-bit kernel length at stage 1 offset 3, test mode byte at stage 2 offset 3.
printf "$(printf '\\x%02x\\x%02x' $((ksectors & 255)) $((ksectors >> 8)))" | dd of="$out" bs=1 seek=3 conv=notrunc status=none
printf "$(printf '\\x%02x' "$code")" | dd of="$out" bs=1 seek=$((512 + 3)) conv=notrunc status=none
echo "$out: $size bytes, kernel $kbytes bytes in $ksectors sectors, test mode $mode"
