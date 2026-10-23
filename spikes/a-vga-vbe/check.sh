#!/bin/bash
# The spike A checks: format and lints, the build, then VGAVBE.EXE in headless DOSBox against its expected transcript, and the negative
# runs (a wrong checksum, no VBE, a panic and an exit in graphics mode).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/a-vga-vbe/check.sh        (after tools/setup.sh)
#
# The transcript is spikes/a-vga-vbe/expected/all.txt. Source line and column numbers in panic reports are normalised away.
set -euo pipefail
cd "$(dirname "$0")/../.."
DIR=spikes/a-vga-vbe
EXP=$DIR/expected
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

normalise() { sed -E 's/\.rs:[0-9]+:[0-9]+:/.rs:L:C:/'; }

# run <dir> <program> [args...]: sets $out (normalised standard output) and $status (the program's)
run() {
  local dir=$1 raw
  shift
  status=0
  raw="$(tools/dosrun.sh "$dir" "$@" 2> /dev/null)" || status=$?
  out="$(printf '%s\n' "$raw" | normalise || true)"
}

same() {
  if [ "$out" != "$(cat "$2")" ]; then
    printf 'FAIL: %s: output differs from %s\n' "$1" "$2" >&2
    diff <(printf '%s\n' "$out") "$2" >&2 || true
    exit 1
  fi
}

step "Rust format and lints (spike workspace)"
(cd $DIR && cargo fmt --check)
(cd $DIR && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build VGAVBE.EXE: rust-lld, then tools/elf2go32.py"
$DIR/build.sh

step "Every stage: DAC through the ports, mode 13h and 12h by registers and by INT 10h, VBE 640x480x8 linear frame buffer"
run dist/a VGAVBE.EXE dump
[ "$status" = 0 ] || fail "VGAVBE.EXE exited with status $status"
same VGAVBE.EXE $EXP/all.txt
echo "$out" | grep -E 'checksums match|read back equals|VBE|vbe: using' | head -12

step "The dumps become PNGs (tools/raw2png.py)"
png="$(mktemp -d "${TMPDIR:-/tmp}/avga-png.XXXXXX")"
trap 'rm -rf "$png"' EXIT
python3 -I tools/raw2png.py dist/a/M13.RAW dist/a/M13.PAL 320 200 "$png/m13.png" 2
python3 -I tools/raw2png.py dist/a/VBE.RAW dist/a/VBE.PAL 640 480 "$png/vbe.png"
python3 -I tools/raw2png.py dist/a/M12.RAW dist/a/M12.PAL 640 480 "$png/m12.png"
for f in m13 vbe m12; do
  [ "$(head -c 8 "$png/$f.png" | od -An -c | tr -d ' ')" = '211PNG\r\n032\n' ] || fail "$f.png is not a PNG"
done
echo "three PNGs written"

step "A wrong checksum fails the run (one pixel flipped after drawing)"
run dist/a VGAVBE.EXE corrupt
[ "$status" = 1 ] || fail "VGAVBE.EXE corrupt exited with status $status, not 1"
case "$out" in *'checksums match: false'*'avga: FAILED'*) ;; *) fail "no mismatch reported: $out" ;; esac
case "$out" in *'the video mode is 0x03') ;; *) fail "text mode was not restored: $out" ;; esac
echo "status $status, mismatch reported, text mode back"

step "No VBE: the run reports it, restores text mode and exits 2"
status=0
raw="$(DOS_MACHINE=vgaonly tools/dosrun.sh dist/a VGAVBE.EXE vbe 2> /dev/null)" || status=$?
[ "$status" = 2 ] || fail "VGAVBE.EXE vbe on a VGA-only machine exited with status $status, not 2"
case "$raw" in *'display offers 2 modes, VBE absent'*'not available'*'the video mode is 0x03'*) ;; *) fail "unexpected output: $raw" ;; esac
echo "status $status"

step "A panic in graphics mode returns to text mode (the clean-up list runs before the report)"
run dist/a VGAVBE.EXE panic
[ "$status" = 3 ] || fail "VGAVBE.EXE panic exited with status $status, not 3"
same "VGAVBE.EXE panic" $EXP/panic.txt
echo "status $status"

step "An exit in graphics mode returns to text mode"
run dist/a VGAVBE.EXE leave
[ "$status" = 0 ] || fail "VGAVBE.EXE leave exited with status $status"
same "VGAVBE.EXE leave" $EXP/leave.txt
echo "status $status"

step "A wrong expected transcript fails the check"
wrong="$(mktemp "${TMPDIR:-/tmp}/avga-wrong.XXXXXX")"
trap 'rm -rf "$png" "$wrong"' EXIT
sed 's/checksum 0x5bc65936/checksum 0x5bc65937/' $EXP/all.txt > "$wrong"
run dist/a VGAVBE.EXE dump
if (same VGAVBE.EXE "$wrong") > /dev/null 2>&1; then fail "a wrong transcript was accepted"; fi
echo "rejected, as it should be"

printf '\nAll spike A checks passed.\n'
