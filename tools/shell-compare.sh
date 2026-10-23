#!/bin/bash
# The two tracks, side by side: the first window's frames as DOSBox saw them (SHELL.EXE's read-back of the frame buffer and the DAC) and as
# QEMU drew them (screendumps of the kernel's display), compared with `frames.py same`, then the comparisons that must fail.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/shell-compare.sh [--run]
#
# Without arguments it reads what shell/dos/check.sh and tools/native-test.sh left behind: dist/shell/dumps-{8,4,vga}/ (RAW and PAL) and
# dist/shell-native-{8,4}/ (PNG, or PPM). With --run it runs both first, which is the slow part. The comparison is of six-bit colour per
# pixel, not of palette indices: the native PNGs renumber the palette. Depth 8 is compared with depth 8, depth 4 with depth 4, and the
# DOS run on a machine with no VBE (which falls back to depth 4) with the native depth 4 run. The result is in docs/FIRST-WINDOW.md.
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
frames() { python3 -I tools/frames.py "$@"; }
sums=tests/shell/first-window.sums

if [ "${1:-}" = "--run" ]; then
  shell/dos/check.sh
  tools/native-test.sh
elif [ $# -gt 0 ]; then
  echo "usage: tools/shell-compare.sh [--run]" >&2
  exit 2
fi

for dir in dist/shell/dumps-8 dist/shell/dumps-4 dist/shell/dumps-vga dist/shell-native-8 dist/shell-native-4; do
  [ -d "$dir" ] || fail "$dir is missing; run shell/dos/check.sh and tools/native-test.sh first, or this script with --run"
done

work="$(mktemp -d "${TMPDIR:-/tmp}/shell-compare.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# must_fail WHAT COMMAND...: the command has to exit non-zero
must_fail() {
  local what=$1
  shift
  if "$@" > "$work/out.txt" 2>&1; then fail "$what was accepted"; fi
  echo "ok — $what is rejected: $(head -n 1 "$work/out.txt")"
}

# digests DIR NAME DEPTH: the colour digest of one frame in a capture set (a RAW needs its PAL beside it)
digest() {
  local dir=$1 name=$2 f
  for f in "$dir/$name".png "$dir/$name".ppm "$dir/$(echo "$name" | tr a-z A-Z)".RAW; do
    [ -f "$f" ] || continue
    case "$f" in
      *.RAW) frames digest "$f" "${f%.RAW}.PAL" 640 480 | cut -d' ' -f3 ;;
      *) frames digest "$f" | cut -d' ' -f3 ;;
    esac
    return
  done
  echo "missing"
}

for depth in 8 4; do
  echo "== depth $depth: DOS read-back against native screendumps"
  printf '%-8s %-10s %-10s %-10s\n' frame sums dos native
  while read -r name d _ _ want; do
    [ "$d" = "$depth" ] || continue
    printf '%-8s %-10s %-10s %-10s\n' "$name" "$want" "$(digest dist/shell/dumps-$depth "$name")" "$(digest dist/shell-native-$depth "$name")"
  done < <(grep -v '^#' "$sums" | grep -v '^$')
  frames same dist/shell/dumps-$depth dist/shell-native-$depth || fail "the DOS and native captures differ at depth $depth"
  [ "$(frames same dist/shell/dumps-$depth dist/shell-native-$depth | sed -n 's/^ok: \([0-9]*\) frames.*/\1/p')" = "$(awk -v d=$depth '$2 == d' "$sums" | wc -l)" ] || fail "depth $depth: not every frame of the sums was compared"
done

echo "== the DOS run with no VBE (depth 4 by fallback) against the native depth 4 screendumps"
frames same dist/shell/dumps-vga dist/shell-native-4 || fail "the DOS fallback captures differ from the native depth 4 captures"

echo "== the comparison has to be able to fail"
must_fail "DOS depth 8 against native depth 4" frames same dist/shell/dumps-8 dist/shell-native-4
must_fail "DOS depth 4 against native depth 8" frames same dist/shell/dumps-4 dist/shell-native-8

# One pixel changed in a copy of the DOS depth 8 dumps (a palette index, so a different colour on screen).
cp -r dist/shell/dumps-8 "$work/altered"
printf '\x7f' | dd of="$work/altered/TYPED.RAW" bs=1 seek=100000 count=1 conv=notrunc 2> /dev/null
cmp -s "$work/altered/TYPED.RAW" dist/shell/dumps-8/TYPED.RAW && fail "the altered dump equals the original"
must_fail "one pixel changed in a DOS dump" frames same "$work/altered" dist/shell-native-8

# A frame missing from one side.
cp -r dist/shell/dumps-8 "$work/short"
rm -f "$work/short/CLOSED.RAW" "$work/short/CLOSED.PAL"
must_fail "a DOS set missing a frame" frames same "$work/short" dist/shell-native-8

# A whole frame wrong in a copy of the native set, whatever form it is in (PNG or PPM): the TYPED frame replaced by the FOCUS one.
mkdir "$work/swapped"
cp dist/shell-native-8/* "$work/swapped/"
for ext in png ppm; do
  if [ -f "$work/swapped/focus.$ext" ]; then cp "$work/swapped/focus.$ext" "$work/swapped/typed.$ext"; fi
done
must_fail "the native TYPED frame replaced by the FOCUS frame" frames same dist/shell/dumps-8 "$work/swapped"

printf '\nThe two tracks drew the same pictures.\n'
