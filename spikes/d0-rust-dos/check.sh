#!/bin/bash
# The d0-rust-dos checks: format and lints, both builds, then every program run in headless DOSBox against its expected transcript.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/d0-rust-dos/check.sh        (after tools/setup.sh)
#
# Transcripts live in spikes/d0-rust-dos/expected/. Two things are normalised before comparing: source line and column numbers in
# panic reports (they move when the source does), and the registers `ctx` marks as scratch (leftovers of the stub's own work).
set -euo pipefail
cd "$(dirname "$0")/../.."
DIR=spikes/d0-rust-dos
EXP=$DIR/expected
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

normalise() { sed -E 's/\.rs:[0-9]+:[0-9]+:/.rs:L:C:/; s#panicked at .*/alloc\.rs:[0-9]+:[0-9]+:#panicked at alloc.rs:L:C:#' | grep -v '^scratch:'; }

# run <dir> <program> [args...]: sets $out (normalised standard output) and $status (the program's)
run() {
  local dir=$1 raw
  shift
  status=0
  raw="$(tools/dosrun.sh "$dir" "$@" 2> /dev/null)" || status=$?
  out="$(printf '%s\n' "$raw" | normalise || true)"
}

# same <name> <expected file>: $out must equal the transcript
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

step "Build: rust-lld, then tools/elf2go32.py (no gcc, no ld, no C library)"
$DIR/build.sh

step "Build, stage 1: DJGPP ld with -nostdlib (no gcc, no C library, no crt0)"
$DIR/build-stage1.sh

step "HELLO.EXE: output, arguments, heap, x87, DPMI, files"
run dist/d0 HELLO.EXE
[ "$status" = 0 ] || fail "HELLO.EXE exited with status $status"
same HELLO.EXE $EXP/hello.txt
echo "$out" | head -3

step "HELLO1.EXE (stage 1, linked by DJGPP ld) prints the same transcript"
run dist/d0 HELLO1.EXE
[ "$status" = 0 ] || fail "HELLO1.EXE exited with status $status"
same HELLO1.EXE $EXP/hello.txt
echo "identical to HELLO.EXE"

step "Exit status and arguments"
run dist/d0 HELLO.EXE exit 7
[ "$status" = 7 ] || fail "HELLO.EXE exit 7 exited with status $status"
case "$out" in *'args: 2 ["exit", "7"]'*) ;; *) fail "arguments were not passed: $out" ;; esac
echo "status $status"

step "CTX.EXE: what the stub hands over at entry"
run dist/d0 CTX.EXE one two
[ "$status" = 0 ] || fail "CTX.EXE exited with status $status"
same CTX.EXE $EXP/ctx.txt

step "HW.EXE: IRQ 0 hook and VGA mode 13h"
run dist/d0 HW.EXE
[ "$status" = 0 ] || fail "HW.EXE exited with status $status"
same HW.EXE $EXP/hw.txt
echo "$out" | sed -n 2,9p

step "A panic restores the hooked vector and exits with status 3"
run dist/d0 PANICKY.EXE
[ "$status" = 3 ] || fail "PANICKY.EXE exited with status $status, not 3"
same PANICKY.EXE $EXP/panicky.txt
echo "$out | status $status" | tail -2

step "Exit with the timer hooked and mode 13h set: the clean-up list runs on exit"
run dist/d0 HW.EXE leave-hooked
[ "$status" = 0 ] || fail "HW.EXE leave-hooked exited with status $status"
case "$out" in *'exiting with the timer hooked and mode 13h set') ;; *) fail "unexpected output: $out" ;; esac
echo "ok"

step "Allocation failure is a panic"
run dist/d0 HELLO.EXE oom
[ "$status" = 3 ] || fail "HELLO.EXE oom exited with status $status, not 3"
case "$out" in *'memory allocation of 943718400 bytes failed') ;; *) fail "unexpected output: $out" ;; esac
echo "status $status"

step "Self-contained executable (CWSDSTUB.EXE, host included) runs with no CWSDPMI.EXE beside it"
[ ! -e dist/d0/solo/CWSDPMI.EXE ] || fail "dist/d0/solo must not hold CWSDPMI.EXE"
run dist/d0/solo HELLO.EXE
[ "$status" = 0 ] || fail "solo HELLO.EXE exited with status $status"
same "solo HELLO.EXE" $EXP/hello.txt
echo "identical to HELLO.EXE"

step "Without CWSDPMI.EXE the stub says so and fails"
mkdir -p dist/d0/nohost
cp dist/d0/HELLO.EXE dist/d0/nohost/HELLO.EXE
rm -f dist/d0/nohost/CWSDPMI.EXE
run dist/d0/nohost HELLO.EXE
[ "$status" != 0 ] || fail "HELLO.EXE ran without a DPMI host"
case "$out" in *'no DPMI'*) ;; *) fail "the stub gave no message about the DPMI host: $out" ;; esac
echo "$out (status $status)"

step "A program that returns from _start does not exit cleanly"
run dist/d0 RETSTART.EXE
[ "$status" != 0 ] || fail "RETSTART.EXE exited with status 0"
echo "status $status"

step "A wrong expected transcript fails the check"
wrong="$(mktemp "${TMPDIR:-/tmp}/d0-wrong.XXXXXX")"
trap 'rm -f "$wrong"' EXIT
sed 's/squares=10/squares=11/' $EXP/hello.txt > "$wrong"
run dist/d0 HELLO.EXE
if (same HELLO.EXE "$wrong") > /dev/null 2>&1; then fail "a wrong transcript was accepted"; fi
echo "rejected, as it should be"

printf '\nAll d0-rust-dos checks passed.\n'
