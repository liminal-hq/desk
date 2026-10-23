#!/bin/bash
# The native track's end-to-end check: builds the floppy image, boots it headless in QEMU as a 486 and compares the serial
# transcript with tests/native/expected.txt, then proves that the check itself can fail.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-test.sh
#
# Positive: the normal boot, plus four loader variants (A20 through the fast gate, A20 through the keyboard controller, and the
# E801 and AH=88 memory map fallbacks) that exercise code a normal QEMU boot would skip. Negative: a wrong expected transcript must be
# rejected; a divide error, a general protection fault, an invalid opcode, a panic and a masked timer must each end the run with
# a failure status and the expected dump; a hung kernel must end by the watchdog timeout. Keyboard and mouse: input injected through
# the QEMU monitor must appear decoded in the transcript, and the negative variants (a missing event, a masked IRQ 1, a kernel
# waiting for input that never comes) must fail. Graphics: the kernel's tour (VBE linear frame buffer, register-programmed modes 13h
# and 12h, a window with a software cursor, text mode restored) is booted with real pixels, and screenshots taken through the QEMU
# monitor are checked against reference checksums; a wrong reference, a cursor that did not move, a movement outside the cursor's
# rectangle and a loader that succeeded where it was told to fail must each be rejected. Shell: the first window's scripted scenario
# (tests/shell/first-window.scn) is run by the kernel at depth 8 (VBE) and depth 4 (mode 12h), and QEMU's screendumps at every frame must
# equal the goldens' six-bit colours (tools/frames.py check); a screen with one wrong pixel, altered sums and the other depth's sums must be
# rejected. Shell live: the same scenario's live form (tests/shell/live.scn and live-keys.scn) is played into the kernel's `shell-live` mode
# through the QEMU monitor (mouse_move, mouse_button, sendkey), and the marks it prints and QEMU's screendumps at each mark must equal the
# goldens; the same input with the mouse, a key or Alt+Tab left out, altered sums and the wrong depth must be rejected. Skips (exit 0, with a message) only
# when neither QEMU nor Docker is available.
#
# DESK_NATIVE_RUNNER=tools/native-run-bochs.sh runs the same checks under Bochs (see tools/native-test-bochs.sh); the runner must
# take the same arguments and return the same exit statuses as tools/native-run.sh.
set -euo pipefail
cd "$(dirname "$0")/.."

runner=${DESK_NATIVE_RUNNER:-tools/native-run.sh}
if [ "$runner" = tools/native-run.sh ]; then
  if ! command -v qemu-system-i386 > /dev/null && ! command -v docker > /dev/null; then
    echo "native: SKIPPED — neither qemu-system-i386 nor docker is installed, so the image cannot be booted"
    exit 0
  fi
elif ! command -v docker > /dev/null; then
  echo "native: SKIPPED — docker is not installed, so $runner cannot run its emulator"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "native: FAILED — $*" >&2; exit 1; }

# run NAME IMAGE EXPECTED_EXIT EXPECTED_TRANSCRIPT [TIMEOUT]: boots, checks the exit status and compares the transcript.
run() {
  local name=$1 image=$2 want=$3 expected=$4 timeout=${5:-20} got=0
  "$runner" --timeout "$timeout" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
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
tools/native-build.sh --nasm-flags "-DNO_E820 -DNO_E801"
tools/native-image.sh --out dist/native-e88.img > /dev/null
sed -e 's|memory map from E820, entries: {N:1..32}|memory map from INT 15h AH=88, entries: 2|' \
  -e 's|memory map from E820 ({N:1..32} entries)|memory map from INT 15h AH=88 (2 entries)|' tests/native/expected.txt > "$work/expected-e88.txt"
run e88 dist/native-e88.img 0 "$work/expected-e88.txt"
rm -f dist/native-a20-fast.img dist/native-a20-keyboard.img dist/native-e801.img dist/native-e88.img

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

# VBE on Cirrus: the card has no Bochs registers, so the tour leaves VBE through the BIOS (INT 10h AX=0003h by the real-mode
# trampoline) and every later screenshot, the text screen included, must be the same as on the std card. Spike D left a garbled screen here.
sed 's|^gfx: leaving VBE: Bochs VBE registers (id {HEX}) switched off$|gfx: leaving VBE: no Bochs VBE registers (read {HEX}), asking the BIOS: INT 10h AX=0003h\ngfx: leaving VBE: the BIOS set text mode 3|' tests/native/expected-gfx-vbe.txt > "$work/expected-gfx-vbe-cirrus.txt"
cmp -s tests/native/expected-gfx-vbe.txt "$work/expected-gfx-vbe-cirrus.txt" && fail "the Cirrus transcript was not adjusted"
run_gfx gfx-vbe-cirrus dist/native-gfx-vbe.img cirrus tests/native/gfx-vbe.inject "$work/expected-gfx-vbe-cirrus.txt" tests/native/gfx-vbe.sums
must_fail "the std transcript for a Cirrus run" python3 -I tools/native-check.py tests/native/expected-gfx-vbe.txt "$work/gfx-vbe-cirrus.txt"
finish_gfx gfx-vbe-cirrus

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

echo "== Shell: first window (docs/FIRST-WINDOW.md)"
# The kernel's `shell` test mode runs tests/shell/first-window.scn: at each `frame` step it prints the frame's digests and
# `shell: ready NAME`, and waits for a key. The monitor script (tools/frames.py inject) takes a screendump at each marker and sends
# the key, so the screendumps are QEMU's own picture of the display, held to the same goldens as the host and DOS runs. Depth 8 runs
# on the std card over the loader's VBE linear frame buffer, depth 4 on the Cirrus card in mode 12h by registers. Each run checks its
# transcript (against the expected file, and against the .sums with `frames.py transcript`) and its screendumps (`frames.py check`,
# which compares six-bit colours, so QEMU's expansion to eight bits does not matter). The dumps of a run stay in
# dist/shell-native-DEPTH/ as PNGs.
shell_sums=tests/shell/first-window.sums
python3 -I tools/frames.py inject tests/shell/first-window.scn > "$work/shell.inject"
frames() { python3 -I tools/frames.py "$@"; }
# run_shell NAME IMAGE CARD DEPTH DIR: the run must succeed and its transcript must be the expected one; the dumps are in DIR.
run_shell() {
  local name=$1 image=$2 card=$3 depth=$4 dir=$5 got=0
  rm -rf "$dir"
  tools/native-run.sh --timeout 60 --vga "$card" --inject "$work/shell.inject" --shots "$dir" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = 0 ] || { tail -n 30 "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected 0"; }
  # The corrupting runs add one line saying which pixel they changed; everything else is the normal transcript.
  grep -v '^shell: corrupted pixel ' "$work/$name.txt" > "$work/$name.plain" || true
  python3 -I tools/native-check.py "tests/native/expected-shell-$depth.txt" "$work/$name.plain" > /dev/null || fail "$name: transcript differs"
  frames transcript "$shell_sums" "$depth" "$work/$name.plain" > /dev/null || fail "$name: the transcript's digests are not the sums'"
}
# Converts a run's PPM screendumps to PNG (a few kilobytes each) and removes the PPMs and the diff pictures of the negative checks.
# The PNGs renumber the palette, so they are for looking at and for the colour comparison of `frames.py same`, not for the index digests.
finish_shell() {
  local f
  rm -f "$1"/*.diff.png
  for f in "$1"/*.ppm; do shot png "$f" "${f%.ppm}.png" && rm -f "$f"; done
}
tools/native-image.sh --test-mode shell --video vbe --out dist/native-shell-8.img > /dev/null
tools/native-image.sh --test-mode shell --video text --out dist/native-shell-4.img > /dev/null
tools/native-image.sh --test-mode shell-corrupt --video vbe --out dist/native-shell-corrupt-8.img > /dev/null
tools/native-image.sh --test-mode shell-corrupt --video text --out dist/native-shell-corrupt-4.img > /dev/null
run_shell shell-8 dist/native-shell-8.img std 8 dist/shell-native-8
frames check "$shell_sums" 8 dist/shell-native-8 > "$work/shell-8.check" || { cat "$work/shell-8.check" >&2; fail "shell-8: a screendump differs from the goldens"; }
echo "native: ok — shell-8 (std card, VBE linear frame buffer): $(cat "$work/shell-8.check")"
run_shell shell-4 dist/native-shell-4.img cirrus 4 dist/shell-native-4
frames check "$shell_sums" 4 dist/shell-native-4 > "$work/shell-4.check" || { cat "$work/shell-4.check" >&2; fail "shell-4: a screendump differs from the goldens"; }
echo "native: ok — shell-4 (Cirrus card, mode 12h by registers): $(cat "$work/shell-4.check")"
# Negative: each of these must fail. The corrupted runs print the true digests (the shell's own picture is fine) and change one pixel
# of the screen, so only the screendump check can notice; and the depth 8 dumps are not the depth 4 sums, nor the reverse.
for depth in 8 4; do
  card=$([ "$depth" = 8 ] && echo std || echo cirrus)
  run_shell "shell-corrupt-$depth" "dist/native-shell-corrupt-$depth.img" "$card" "$depth" "dist/shell-native-corrupt-$depth"
  grep -q '^shell: corrupted pixel ' "$work/shell-corrupt-$depth.txt" || fail "the depth $depth corrupt run did not corrupt a pixel"
  if frames check "$shell_sums" "$depth" "dist/shell-native-corrupt-$depth" > "$work/corrupt-$depth.check" 2>&1; then fail "a screen with one wrong pixel was accepted at depth $depth"; fi
  grep -q 'moved' "$work/corrupt-$depth.check" || fail "the depth $depth corrupt run was not rejected at the moved frame"
  echo "native: ok — a screen with one wrong pixel is rejected at depth $depth: $(grep moved "$work/corrupt-$depth.check" | head -n 1)"
  rm -rf "dist/shell-native-corrupt-$depth"
done
# One digest changed, whatever the goldens currently hold: the last hex digit of the depth 8 `moved` line's colour digest.
awk '$1 == "moved" && $2 == 8 { d = substr($5, 8, 1); $5 = substr($5, 1, 7) (d == "0" ? "1" : "0") } { print }' "$shell_sums" > "$work/wrong-shell.sums"
cmp -s "$shell_sums" "$work/wrong-shell.sums" && fail "the altered sums were not altered"
must_fail "screendumps checked against sums with one digest changed" frames check "$work/wrong-shell.sums" 8 dist/shell-native-8
must_fail "the depth 8 screendumps checked against the depth 4 sums" frames check "$shell_sums" 4 dist/shell-native-8
must_fail "the depth 4 screendumps checked against the depth 8 sums" frames check "$shell_sums" 8 dist/shell-native-4
must_fail "the depth 4 transcript checked against the depth 8 sums" frames transcript "$shell_sums" 8 "$work/shell-4.txt"
must_fail "the depth 8 transcript as the depth 4 one" python3 -I tools/native-check.py tests/native/expected-shell-4.txt "$work/shell-8.txt"
finish_shell dist/shell-native-8
finish_shell dist/shell-native-4
rm -f dist/native-shell-8.img dist/native-shell-4.img dist/native-shell-corrupt-8.img dist/native-shell-corrupt-4.img

echo "== Shell: live input (docs/LIVE-INPUT.md)"
# The kernel's `shell-live` test mode runs the first window on the PS/2 keyboard and mouse until Escape. The run script is the live
# scenario's steps turned into QEMU monitor commands (tools/frames.py live --qemu): mouse_move and mouse_button for the pointer, sendkey
# for the keys, and at each frame an F12, which the kernel answers with a `shell: mark NN` line once it has handled everything sent
# before it. The script waits for that line, then takes the screendump. Each run's marks must carry the scenario's digests in order
# (frames.py marks), its screendumps must equal the goldens (frames.py check), and its transcript must be the expected one (the event
# counts are ranges, since they depend on how QEMU delivers the input; the digests are exact). live.scn runs at both depths and
# live-keys.scn at depth 8, so both live scenarios run on the native path. Kept as PNGs in dist/shell-live-native-DEPTH/.
# play_live NAME IMAGE CARD INJECT DIR: boots the image with the input script; the run must succeed. The transcript is $work/NAME.txt.
play_live() {
  local name=$1 image=$2 card=$3 inject=$4 dir=$5 got=0
  rm -rf "$dir"
  tools/native-run.sh --timeout 60 --vga "$card" --inject "$inject" --shots "$dir" "$image" > "$work/$name.txt" 2> "$work/$name.err" || got=$?
  [ "$got" = 0 ] || { tail -n 30 "$work/$name.txt" "$work/$name.err" >&2; fail "$name: exit status $got, expected 0"; }
}
# run_live NAME IMAGE CARD DEPTH SCENARIO SUMS EXPECTED DIR: play_live, then every positive check.
run_live() {
  local name=$1 image=$2 card=$3 depth=$4 scn=$5 sums=$6 expected=$7 dir=$8
  frames live "$scn" --qemu > "$work/$name.inject" || fail "$name: $scn does not translate for QEMU"
  play_live "$name" "$image" "$card" "$work/$name.inject" "$dir"
  python3 -I tools/native-check.py "$expected" "$work/$name.txt" > /dev/null || fail "$name: transcript differs"
  frames marks "$sums" "$scn" "$depth" "$work/$name.txt" > "$work/$name.marks" || { cat "$work/$name.marks" >&2; fail "$name: a mark differs from the sums"; }
  frames check "$sums" "$depth" "$dir" > "$work/$name.check" || { cat "$work/$name.check" >&2; fail "$name: a screendump differs from the goldens"; }
  echo "native: ok — $name: $(cat "$work/$name.marks"); $(cat "$work/$name.check")"
}
live_sums=tests/shell/live.sums
tools/native-image.sh --test-mode shell-live --video vbe --out dist/native-shell-live-8.img > /dev/null
tools/native-image.sh --test-mode shell-live --video text --out dist/native-shell-live-4.img > /dev/null
run_live shell-live-8 dist/native-shell-live-8.img std 8 tests/shell/live.scn "$live_sums" tests/native/expected-shell-live-8.txt dist/shell-live-native-8
run_live shell-live-4 dist/native-shell-live-4.img cirrus 4 tests/shell/live.scn "$live_sums" tests/native/expected-shell-live-4.txt dist/shell-live-native-4
run_live shell-live-keys-8 dist/native-shell-live-8.img std 8 tests/shell/live-keys.scn tests/shell/live-keys.sums tests/native/expected-shell-live-keys-8.txt dist/shell-live-keys-native-8
# Negative: each of these must fail, and the check says so when one does not. A guest that is sent the wrong input still runs to the
# end (the script finishes with Escape), so the marks and the screendumps are what must notice.
broken() { # NAME FILTER...: the live script with the lines matching FILTER removed, played at depth 8, as NAME
  local name=$1
  shift
  grep -v "$@" "$work/shell-live-8.inject" > "$work/$name.inject"
  cmp -s "$work/shell-live-8.inject" "$work/$name.inject" && fail "$name: nothing was removed from the script"
  play_live "$name" dist/native-shell-live-8.img std "$work/$name.inject" "dist/shell-live-native-$name"
}
broken live-nomouse -e '^mouse_move '
must_fail "a live run whose mouse_move lines were removed (the marks, from the dragged frame)" frames marks "$live_sums" tests/shell/live.scn 8 "$work/live-nomouse.txt"
must_fail "the same run's screendumps" frames check "$live_sums" 8 dist/shell-live-native-live-nomouse
broken live-notype -e '^sendkey v$'
must_fail "a live run with one key missing from typing \"live\" (the marks)" frames marks "$live_sums" tests/shell/live.scn 8 "$work/live-notype.txt"
must_fail "the same run's screendumps" frames check "$live_sums" 8 dist/shell-live-native-live-notype
broken live-noalttab -e '^sendkey alt-tab$'
must_fail "a live run with the alt-tab line removed (the marks, from the switched frame)" frames marks "$live_sums" tests/shell/live.scn 8 "$work/live-noalttab.txt"
must_fail "the depth 8 live screendumps checked against the depth 4 sums" frames check "$live_sums" 4 dist/shell-live-native-8
awk '$1 == "dragged" && $2 == 8 { d = substr($5, 8, 1); $5 = substr($5, 1, 7) (d == "0" ? "1" : "0") } { print }' "$live_sums" > "$work/wrong-live.sums"
cmp -s "$live_sums" "$work/wrong-live.sums" && fail "the altered live sums were not altered"
must_fail "live marks checked against sums with one digest changed" frames marks "$work/wrong-live.sums" tests/shell/live.scn 8 "$work/shell-live-8.txt"
grep -v '^shell: mark 03 ' "$work/shell-live-8.txt" > "$work/live-onemark.txt"
must_fail "a live transcript with one mark line removed" frames marks "$live_sums" tests/shell/live.scn 8 "$work/live-onemark.txt"
must_fail "the live transcript as the scripted one" python3 -I tools/native-check.py tests/native/expected-shell-8.txt "$work/shell-live-8.txt"
for d in dist/shell-live-native-8 dist/shell-live-native-4 dist/shell-live-keys-native-8; do finish_shell "$d"; done
rm -rf dist/shell-live-native-live-nomouse dist/shell-live-native-live-notype dist/shell-live-native-live-noalttab
rm -f dist/native-shell-live-8.img dist/native-shell-live-4.img

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
