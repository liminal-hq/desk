#!/bin/bash
# The native track's end-to-end check: builds the floppy image, boots it headless in QEMU as a 486 and compares the serial
# transcript with tests/native/expected.txt, then proves that the check itself can fail.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-test.sh
#
# Positive: the normal boot, plus three loader variants (A20 through the fast gate, A20 through the keyboard controller, and the
# E801 memory map fallback) that exercise code a normal QEMU boot would skip. Negative: a wrong expected transcript must be
# rejected; a divide error, a general protection fault, an invalid opcode, a panic and a masked timer must each end the run with
# a failure status and the expected dump; a hung kernel must end by the watchdog timeout. Keyboard and mouse: input injected through
# the QEMU monitor must appear decoded in the transcript, and the negative variants (a missing event, a masked IRQ 1, a kernel
# waiting for input that never comes) must fail. Graphics: the kernel's tour (VBE linear frame buffer, register-programmed modes 13h
# and 12h, a window with a software cursor, text mode restored) is booted with real pixels, and screenshots taken through the QEMU
# monitor are checked against reference checksums; a wrong reference, a cursor that did not move, a movement outside the cursor's
# rectangle and a loader that succeeded where it was told to fail must each be rejected. Skips (exit 0, with a message) only
# when neither QEMU nor Docker is available.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v qemu-system-i386 > /dev/null && ! command -v docker > /dev/null; then
  echo "native: SKIPPED — neither qemu-system-i386 nor docker is installed, so the image cannot be booted"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "native: FAILED — $*" >&2; exit 1; }

# run NAME IMAGE EXPECTED_EXIT EXPECTED_TRANSCRIPT [TIMEOUT]: boots, checks the exit status and compares the transcript.
run() {
  local name=$1 image=$2 want=$3 expected=$4 timeout=${5:-20} got=0
  tools/native-run.sh --timeout "$timeout" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = "$want" ] || { cat "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected $want"; }
  if [ -n "$expected" ]; then
    python3 -I tools/native-check.py "$expected" "$work/$name.txt" || fail "$name: transcript differs"
  fi
  echo "native: ok — $name (exit $got)"
}

echo "== Loader variants"
variant() {
  local name=$1 flags=$2 from=$3 to=$4
  tools/native-build.sh --nasm-flags "$flags"
  tools/native-image.sh --out "dist/native-$name.img" > /dev/null
  sed "s|^$from\$|$to|" tests/native/expected.txt > "$work/expected-$name.txt"
  cmp -s tests/native/expected.txt "$work/expected-$name.txt" && fail "$name: the expected transcript was not adjusted"
  run "$name" "dist/native-$name.img" 0 "$work/expected-$name.txt"
}
variant a20-fast "-DA20_TEST=1" "stage2: a20 already enabled" "stage2: a20 enabled by the fast gate"
variant a20-keyboard "-DA20_TEST=2" "stage2: a20 already enabled" "stage2: a20 enabled by the keyboard controller"
tools/native-build.sh --nasm-flags "-DNO_E820"
tools/native-image.sh --out dist/native-e801.img > /dev/null
sed -e 's|memory map from E820, entries: {N:1..32}|memory map from E801, entries: {N:2..3}|' \
  -e 's|memory map from E820 ({N:1..32} entries)|memory map from E801 ({N:2..3} entries)|' tests/native/expected.txt > "$work/expected-e801.txt"
run e801 dist/native-e801.img 0 "$work/expected-e801.txt"
rm -f dist/native-a20-fast.img dist/native-a20-keyboard.img dist/native-e801.img

echo "== Normal boot"
tools/native-build.sh
tools/native-image.sh
run boot dist/desk-native.img 0 tests/native/expected.txt

echo "== Negative checks"
if python3 -I tools/native-check.py tests/native/wrong-expected.txt "$work/boot.txt" > /dev/null 2>&1; then
  fail "a wrong expected transcript was accepted"
fi
echo "native: ok — a wrong expected transcript is rejected"
for mode in div0 gp ud panic notick; do
  tools/native-image.sh --test-mode "$mode" --out "dist/native-$mode.img" > /dev/null
  run "$mode" "dist/native-$mode.img" 1 "tests/native/expected-$mode.txt" 20
  rm -f "dist/native-$mode.img"
done
tools/native-image.sh --test-mode hang --out dist/native-hang.img > /dev/null
run hang dist/native-hang.img 124 "" 5
rm -f dist/native-hang.img

echo "== Keyboard and mouse"
# Input played through the QEMU monitor (tests/native/input.inject): the transcript has to show every injected key and mouse
# event, decoded. Negative: an event missing from the injected input fails the comparison; a masked IRQ 1 delivers no keys (the
# kernel gives up and reports it); a kernel that waits for input that never comes is ended by the watchdog.
run_input() {
  local name=$1 image=$2 want=$3 expected=$4 script=$5 timeout=${6:-30} got=0
  tools/native-run.sh --timeout "$timeout" --inject "$script" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = "$want" ] || { cat "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected $want"; }
  if [ -n "$expected" ]; then
    python3 -I tools/native-check.py "$expected" "$work/$name.txt" > /dev/null || fail "$name: transcript differs"
  fi
  echo "native: ok — $name (exit $got)"
}
tools/native-image.sh --test-mode input --out dist/native-input.img > /dev/null
run_input input dist/native-input.img 0 tests/native/expected-input.txt tests/native/input.inject
grep -v ctrl-alt-delete tests/native/input.inject > "$work/input-short.inject"
run_input input-missing-event dist/native-input.img 0 "" "$work/input-short.inject"
if python3 -I tools/native-check.py tests/native/expected-input.txt "$work/input-missing-event.txt" > /dev/null 2>&1; then
  fail "a transcript with a missing key event was accepted"
fi
echo "native: ok — a transcript with a missing event is rejected"
rm -f dist/native-input.img
tools/native-image.sh --test-mode input-masked --out dist/native-input-masked.img > /dev/null
run_input input-masked dist/native-input-masked.img 1 tests/native/expected-input-masked.txt tests/native/input.inject
rm -f dist/native-input-masked.img
tools/native-image.sh --test-mode input-idle --out dist/native-input-idle.img > /dev/null
run_input input-idle dist/native-input-idle.img 124 "" tests/native/input-idle.inject 5
rm -f dist/native-input-idle.img

echo "== Graphics: screenshots through the QEMU monitor"
# The tour (kernel/src/gfx.rs) prints "gfx: ready NAME" and waits for a key at each stable picture; the script
# (tests/native/gfx-*.inject) takes a screendump first and sends the key after. Each run checks its transcript, then every
# screenshot against a reference checksum (tests/native/gfx-*.sums). The PNGs of a run stay in dist/gfx-NAME/.
shot() { python3 -I tools/native-shot.py "$@"; }
run_gfx() {
  local name=$1 image=$2 card=$3 script=$4 expected=$5 sums=$6 got=0 dir="dist/gfx-$1"
  rm -rf "$dir"
  tools/native-run.sh --timeout 60 --vga "$card" --inject "$script" --shots "$dir" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = 0 ] || { tail -n 30 "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected 0"; }
  python3 -I tools/native-check.py "$expected" "$work/$name.txt" > /dev/null || fail "$name: transcript differs"
  shot check "$sums" "$dir" > "$work/$name.shots" || { cat "$work/$name.shots" >&2; fail "$name: a screenshot differs from its reference"; }
  shot has-text "$dir/text.ppm" > /dev/null || fail "$name: the last screenshot is not a text-mode screen"
  echo "native: ok — $name ($card card, $(grep -c matches "$work/$name.shots") screenshots match their references)"
}
# Converts the screenshots of a finished run to PNG (a few kilobytes each) and removes the PPMs.
finish_gfx() {
  local f
  for f in "dist/gfx-$1"/*.ppm; do shot png "$f" "${f%.ppm}.png" && rm -f "$f"; done
}
must_fail() {
  local what=$1
  shift
  if "$@" > /dev/null 2>&1; then fail "$what was accepted"; fi
  echo "native: ok — $what is rejected"
}

tools/native-build.sh
tools/native-image.sh --test-mode gfx --video vbe --out dist/native-gfx-vbe.img > /dev/null
tools/native-image.sh --test-mode gfx --video text --out dist/native-gfx-text.img > /dev/null
run_gfx gfx-vbe dist/native-gfx-vbe.img std tests/native/gfx-vbe.inject tests/native/expected-gfx-vbe.txt tests/native/gfx-vbe.sums
# The cursor moved from (320, 240) to (420, 300): the two screenshots may differ only inside the two 12 x 19 sprites' bounding box.
shot moved dist/gfx-gfx-vbe/vbe-window-a.ppm dist/gfx-gfx-vbe/vbe-window-b.ppm 320 240 432 319 > /dev/null || fail "the cursor did not move as expected"
echo "native: ok — the cursor moved, and only inside the old and new sprite rectangles"
# Typing changes only the window's echo line.
shot moved dist/gfx-gfx-vbe/vbe-window-b.ppm dist/gfx-gfx-vbe/vbe-window-c.ppm 97 150 543 174 > /dev/null || fail "typing changed more than the echo line"
echo "native: ok — typing changed only the echo line"
# Negative: each of these must fail.
sed 's/^vbe-picture \(.*\) .*$/vbe-picture \1 00000000/' tests/native/gfx-vbe.sums > "$work/wrong.sums"
must_fail "a wrong screenshot reference" python3 -I tools/native-shot.py check "$work/wrong.sums" dist/gfx-gfx-vbe
must_fail "a screenshot checked against another run's references" python3 -I tools/native-shot.py check tests/native/gfx-fallback.sums dist/gfx-gfx-vbe
must_fail "two identical screenshots as a cursor movement" python3 -I tools/native-shot.py moved dist/gfx-gfx-vbe/vbe-window-a.ppm dist/gfx-gfx-vbe/vbe-window-a.ppm 0 0 640 480
must_fail "a cursor movement outside its allowed rectangle" python3 -I tools/native-shot.py moved dist/gfx-gfx-vbe/vbe-window-a.ppm dist/gfx-gfx-vbe/vbe-window-b.ppm 320 240 400 319
must_fail "a graphics screenshot as a text-mode screen" python3 -I tools/native-shot.py has-text dist/gfx-gfx-vbe/vbe-window-a.ppm
must_fail "the VBE transcript as the failed-VBE transcript" python3 -I tools/native-check.py tests/native/expected-gfx-vbefail.txt "$work/gfx-vbe.txt"
finish_gfx gfx-vbe

# VBE not requested: modes 13h and 12h by registers (no BIOS), on a second card (Cirrus) to show the register path is not std-specific.
run_gfx gfx-text dist/native-gfx-text.img cirrus tests/native/gfx-fallback.inject tests/native/expected-gfx-fallback.txt tests/native/gfx-fallback.sums
shot moved dist/gfx-gfx-text/window-a.ppm dist/gfx-gfx-text/window-b.ppm 320 240 432 319 > /dev/null || fail "the cursor did not move in mode 12h"
echo "native: ok — the cursor moved in mode 12h"
finish_gfx gfx-text

# The loader runs the whole VBE search but the final mode-set call is made to fail (-DVBE_FAIL=1): it must say so, the kernel must say
# it fell back, and the screens must be the register-mode ones.
tools/native-build.sh --nasm-flags "-DVBE_FAIL=1"
tools/native-image.sh --test-mode gfx --video vbe --out dist/native-gfx-fail.img > /dev/null
run_gfx gfx-vbefail dist/native-gfx-fail.img std tests/native/gfx-fallback.inject tests/native/expected-gfx-vbefail.txt tests/native/gfx-fallback.sums
must_fail "the failed-VBE transcript as the normal one" python3 -I tools/native-check.py tests/native/expected-gfx-vbe.txt "$work/gfx-vbefail.txt"
finish_gfx gfx-vbefail
rm -f dist/native-gfx-vbe.img dist/native-gfx-text.img dist/native-gfx-fail.img
tools/native-build.sh > /dev/null

echo "== BIOS calls from the kernel (docs/N3-BIOS-PATH.md)"
# The kernel returns to real mode for each call (kernel/bios16.asm): memory, clock, disk, errors, a key sent during a call, 1,000
# calls, and VBE set after boot with the way back to text mode, on each of QEMU's three cards. The screenshots must match their
# references and the last one must be a text screen. A second run has an IDE disk for the LBA extensions.
run_bios() {
  local name=$1 card=$2 expected=$3 script=$4 hd=${5:-} got=0 dir="dist/gfx-$1"
  rm -rf "$dir"
  tools/native-run.sh --timeout 90 --vga "$card" ${hd:+--hd "$hd"} --inject "$script" --shots "$dir" dist/native-bios.img > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = 0 ] || { tail -n 30 "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected 0"; }
  python3 -I tools/native-check.py "$expected" "$work/$name.txt" > /dev/null || fail "$name: transcript differs"
  shot check tests/native/bios.sums "$dir" > "$work/$name.shots" || { cat "$work/$name.shots" >&2; fail "$name: a screenshot differs from its reference"; }
  shot has-text "$dir/text.ppm" > /dev/null || fail "$name: the last screenshot is not a text-mode screen"
  echo "native: ok — $name ($card card, $(grep -c matches "$work/$name.shots") screenshots match their references)"
}
tools/native-image.sh --test-mode bios --out dist/native-bios.img > /dev/null
for card in std cirrus vmware; do
  run_bios "bios-$card" "$card" tests/native/expected-bios.txt tests/native/bios.inject
done
# A disk with known text at the start of sectors 0 and 1, on IDE, so INT 13h AH=41h and AH=42h (LBA) have something to answer.
python3 -I -c "
d = bytearray(8 * 1024 * 1024)
d[0:16] = b'DESK-HD-SECTOR-0'
d[512:528] = b'DESK-HD-SECTOR-1'
open('dist/native-hd.img', 'wb').write(d)"
cat > "$work/hd-lines.txt" <<'LINES'
bios: INT 13h drive 0x80: present, CHS {N:1..1024} cylinders, {N:1..255} heads, {N:1..63} sectors per track
bios: INT 13h AH=41h on drive 0x80: extensions available, interface bits {HEX}
bios: INT 13h AH=42h read two sectors by LBA and they hold the test disk's known text: true
bios: INT 13h AH=42h reports 2 sectors transferred
bios: INT 13h AH=42h beyond the end of the disk fails: true
LINES
sed -e "/^bios: INT 13h drive 0x80: not present\$/{r $work/hd-lines.txt" -e "d}" tests/native/expected-bios.txt > "$work/expected-bios-hd.txt"
cmp -s tests/native/expected-bios.txt "$work/expected-bios-hd.txt" && fail "the hard-disk transcript was not adjusted"
run_bios bios-hd std "$work/expected-bios-hd.txt" tests/native/bios.inject dist/native-hd.img
must_fail "the floppy-only transcript as the hard-disk run's" python3 -I tools/native-check.py tests/native/expected-bios.txt "$work/bios-hd.txt"
rm -f dist/native-hd.img
# Negative: a wrong line in the expected transcript, a wrong screenshot reference, and a key that never arrives during the call (the
# kernel must notice that the byte was lost, and the run must fail).
sed 's/^bios: stress: the tick count kept one hundred ticks to the RTC second: true$/bios: stress: the tick count kept one hundred ticks to the RTC second: false/' tests/native/expected-bios.txt > "$work/wrong-bios.txt"
must_fail "a transcript with a wrong line" python3 -I tools/native-check.py "$work/wrong-bios.txt" "$work/bios-std.txt"
sed 's/^vbe-800 \(.*\) .*$/vbe-800 \1 00000000/' tests/native/bios.sums > "$work/wrong-bios.sums"
must_fail "a wrong BIOS screenshot reference" python3 -I tools/native-shot.py check "$work/wrong-bios.sums" dist/gfx-bios-std
must_fail "a VBE screenshot as the restored text screen" python3 -I tools/native-shot.py has-text dist/gfx-bios-std/vbe-640.ppm
grep -v 'sendkey a' tests/native/bios.inject > "$work/bios-nokey.inject"
got=0
tools/native-run.sh --timeout 40 --vga std --inject "$work/bios-nokey.inject" dist/native-bios.img > "$work/bios-nokey.txt" 2> "$work/bios-nokey.err" || got=$?
[ "$got" = 1 ] || { tail -n 20 "$work/bios-nokey.txt" >&2; fail "bios-nokey: exit status $got, expected 1"; }
grep -q 'the key sent during the call was lost' "$work/bios-nokey.txt" || fail "bios-nokey: the lost key was not reported"
echo "native: ok — a run in which no key arrives during the call fails and says the key was lost"
for run in bios-std bios-cirrus bios-vmware bios-hd; do finish_gfx "$run"; done
rm -f dist/native-bios.img

printf 'native: all checks passed\n'
