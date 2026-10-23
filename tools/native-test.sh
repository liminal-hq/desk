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
# a failure status and the expected dump; a hung kernel must end by the watchdog timeout. Skips (exit 0, with a message) only
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

printf 'native: all checks passed\n'
