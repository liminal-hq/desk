#!/bin/bash
# A short check of the native image in 86Box (experimental, slow: about 15 seconds a boot): the normal boot against the expected
# transcript, adjusted for what the 1993 AMIBIOS of the emulated machine does, then a deliberate divide error. The screenshot of
# the normal boot is kept in dist/native-86box.png.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-test-86box.sh
#
# The BIOS leaves the A20 gate closed (the loader opens it through the fast gate) and has neither INT 15h AX=E820 nor AX=E801 (the
# loader falls back to AH=88), so those transcript lines differ from the QEMU ones. Needs Docker, NASM and Rust (to build the image).
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v docker > /dev/null; then
  echo "native-86box: SKIPPED — docker is not installed, so 86Box cannot run"
  exit 0
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() { echo "native-86box: FAILED — $*" >&2; exit 1; }

tools/native-build.sh > /dev/null
tools/native-image.sh > /dev/null
sed -e 's|^stage2: a20 already enabled$|stage2: a20 enabled by the fast gate|' \
  -e 's|memory map from E820, entries: {N:1..32}|memory map from INT 15h AH=88, entries: 2|' \
  -e 's|memory map from E820 ({N:1..32} entries)|memory map from INT 15h AH=88 (2 entries)|' tests/native/expected.txt > "$work/expected.txt"

tools/native-run-86box.sh --screenshot dist/native-86box.png > "$work/boot.txt" 2> "$work/boot.err" \
  || { cat "$work/boot.txt" "$work/boot.err" >&2; fail "the normal boot did not report success"; }
python3 -I tools/native-check.py "$work/expected.txt" "$work/boot.txt" || fail "the normal boot transcript differs"
echo "native-86box: ok — boot (screenshot in dist/native-86box.png)"

tools/native-image.sh --test-mode div0 --out dist/native-div0-86box.img > /dev/null
got=0
tools/native-run-86box.sh dist/native-div0-86box.img > "$work/div0.txt" 2> "$work/div0.err" || got=$?
rm -f dist/native-div0-86box.img
[ "$got" = 1 ] || { cat "$work/div0.txt" "$work/div0.err" >&2; fail "div0: exit status $got, expected 1"; }
python3 -I tools/native-check.py tests/native/expected-div0.txt "$work/div0.txt" || fail "div0: transcript differs"
echo "native-86box: ok — div0 (exit 1)"
echo "native-86box: all checks passed"
