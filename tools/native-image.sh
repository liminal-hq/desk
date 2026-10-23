#!/bin/bash
# Assembles the 1.44 MB floppy image from the pieces tools/native-build.sh made.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-image.sh [--out FILE] [--test-mode NAME] [--video text|vbe]
#
# Layout (512-byte sectors): LBA 0 stage 1, LBA 1-17 stage 2, LBA 18 onwards the flat kernel, zero padding to 2880 sectors
# (1,474,560 bytes). Bytes patched in: the kernel's length in sectors into stage 1 (offset 3), the test mode into stage 2 (offset 3)
# and the video request into stage 2 (offset 4: `text` keeps the BIOS text screen, `vbe` makes the loader try a VBE linear mode).
# Test modes make the kernel misbehave on purpose for the negative checks: none, div0, gp, ud, panic, hang, notick, input, input-masked, input-idle (the last three wait for keyboard and mouse events that tools/native-run.sh --inject sends),
# gfx (the graphics tour: modes, palette, a window and a cursor; docs/D-NATIVE-VGA.md), bios (BIOS calls from the kernel through the
# real-mode trampoline: VBE, disk, memory, time and the stress run; docs/N3-BIOS-PATH.md), bios-solo (the same without the steps that
# need typed keys, for an emulator that cannot inject them), shell (the first window's scripted scenario, a frame at a time:
# docs/FIRST-WINDOW.md), shell-corrupt (the same, with one pixel of the screen changed after the `moved` frame) and shell-live (the first
# window on the PS/2 keyboard and mouse until Escape, with F12 printing a mark: docs/LIVE-INPUT.md).
set -euo pipefail
cd "$(dirname "$0")/.."

out=dist/desk-native.img
mode=none
video=text
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --test-mode) mode="$2"; shift 2 ;;
    --video) video="$2"; shift 2 ;;
    *) echo "usage: tools/native-image.sh [--out FILE] [--test-mode NAME] [--video text|vbe]" >&2; exit 2 ;;
  esac
done
case "$mode" in
  none) code=0 ;; div0) code=1 ;; gp) code=2 ;; ud) code=3 ;; panic) code=4 ;; hang) code=5 ;; notick) code=6 ;;
  input) code=7 ;; input-masked) code=8 ;; input-idle) code=9 ;; gfx) code=10 ;; bios) code=11 ;; bios-solo) code=12 ;;
  shell) code=13 ;; shell-corrupt) code=14 ;; shell-live) code=15 ;;
  *) echo "unknown test mode: $mode" >&2; exit 2 ;;
esac

case "$video" in text) vcode=0 ;; vbe) vcode=1 ;; *) echo "unknown video request: $video" >&2; exit 2 ;; esac

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
printf "$(printf '\\x%02x' "$vcode")" | dd of="$out" bs=1 seek=$((512 + 4)) conv=notrunc status=none
echo "$out: $size bytes, kernel $kbytes bytes in $ksectors sectors, test mode $mode, video $video"
