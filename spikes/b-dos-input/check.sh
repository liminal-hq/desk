#!/bin/bash
# The spike B checks: format and lints, the build, then INPUT.EXE in headless DOSBox with keys typed into it by AUTOTYPE, against the
# transcript the host tests also produce, and the negative runs (a missing key, no Escape, no mouse driver, a panic).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/b-dos-input/check.sh        (after tools/setup.sh)
#
# The key transcript is spikes/b-dos-input/expected/keys.txt, which `cargo test` in hal/ checks the mock keyboard against as well. The
# mouse needs a pointer that DOSBox's dummy video driver cannot give, so the motion and the buttons are checked by
# spikes/b-dos-input/check-xvfb.sh (opt-in, INPUT_EXTRA=1 in tools/check.sh); here the driver is found, its calls are exercised, and
# a still mouse produces no events.
set -euo pipefail
cd "$(dirname "$0")/../.."
DIR=spikes/b-dos-input
EXP=$DIR/expected
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

normalise() { sed -E 's/\.rs:[0-9]+:[0-9]+:/.rs:L:C:/'; }

# The keys sent to the program, and the AUTOTYPE timing: start after 3 s, a key every 0.2 s. Each key is pressed and released.
KEYS='a b capslock c capslock up delete f1 lshift d esc'
TYPE="-w 3 -p 0.2"

# run <autotype keys> <program> [args...]: sets $out (normalised standard output) and $status (the program's)
run() {
  local keys=$1 raw
  shift
  status=0
  raw="$(DOS_AUTOTYPE="$TYPE $keys" DOS_CONF="${CONF:-}" DOS_TIMEOUT=60 tools/dosrun.sh dist/b "$@" 2> /dev/null)" || status=$?
  out="$(printf '%s\n' "$raw" | normalise || true)"
}

# The event lines of $out, without the program's own lines.
events() { printf '%s\n' "$out" | grep -E '^(key|mouse) ' || true; }

has() { # <what> <fixed text that $out must contain>
  case "$out" in *"$2"*) ;; *) fail "$1: expected \"$2\" in: $out" ;; esac
}

hasnt() {
  case "$out" in *"$2"*) fail "$1: did not expect \"$2\" in: $out" ;; esac
}

# The program ends on Escape being pressed; the release goes to whoever runs next, so it is not in the transcript.
head -n -1 $EXP/keys.txt > "${TMPDIR:-/tmp}/binput-keys.$$"
trap 'rm -f "${TMPDIR:-/tmp}/binput-keys.$$" "${TMPDIR:-/tmp}/binput-wrong.$$"' EXIT
EXPECTED_KEYS="${TMPDIR:-/tmp}/binput-keys.$$"

step "Rust format and lints (spike workspace)"
(cd $DIR && cargo fmt --check)
(cd $DIR && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build INPUT.EXE: rust-lld, then tools/elf2go32.py"
$DIR/build.sh

step "Host tests: the same keys through the mock 8042, the IRQ 1 service, the ring and the shared decoder; INT 33h against the model"
(cd hal && cargo test --locked --quiet -p desk-int33 -p desk-kbd)

step "Keys through the real IRQ 1 path: VBE 640x480x8, the BIOS handler replaced"
run "$KEYS" INPUT.EXE
[ "$status" = 0 ] || fail "INPUT.EXE exited with status $status: $out"
if [ "$(events)" != "$(cat "$EXPECTED_KEYS")" ]; then
  printf 'FAIL: the key transcript differs from %s (without the final Escape release)\n' "$EXP/keys.txt" >&2
  diff <(events) "$EXPECTED_KEYS" >&2 || true
  exit 1
fi
has "VBE" "input: video: mode 0x101, 640 x 480, pitch 640"
has "handler" "IRQ 1 handler installed at vector 0x09 (the vector changed: true), replacing the BIOS handler"
has "no mouse events from a still mouse" "0 mouse events"
has "every byte read, none dropped" "25 read, 0 dropped, 0 decoder recoveries, BIOS type-ahead 0 keys"
has "restored" "IRQ 1 handler is the original one: true, ours installed: false, video mode 0x03, BIOS keyboard flags 0x00 (were 0x00), BIOS type-ahead 0 keys"
printf '%s\n' "$out" | grep -E '^input: (keyboard|video|after)' | cut -c1-150

step "The mouse driver: found, its range and position calls, and the position clamp"
has "driver" "input: mouse: driver found, 2 buttons, wheel API no"
has "centre and clamp" "centre Some((320, 240)), an out-of-range position clamps to Some((639, 479))"
printf '%s\n' "$out" | grep -E '^input: mouse' | cut -c1-150

step "Mode 13h: the driver's range is 640 virtual columns over 200 rows (DOSBox keeps them even, so the largest is 638)"
run "$KEYS" INPUT.EXE 13
[ "$status" = 0 ] || fail "INPUT.EXE 13 exited with status $status: $out"
has "13h" "input: video: mode 0x013, 320 x 200, pitch 320"
has "13h range" "range 0..=639 x 0..=199, centre Some((320, 100)), an out-of-range position clamps to Some((638, 199))"
[ "$(events)" = "$(cat "$EXPECTED_KEYS")" ] || fail "mode 13h: the key transcript differs"
printf '%s\n' "$out" | grep -E '^input: (video|mouse: range)' | cut -c1-150

step "Chaining to the BIOS handler fills the BIOS type-ahead buffer; replacing it does not"
run "$KEYS" INPUT.EXE text chain
[ "$status" = 0 ] || fail "INPUT.EXE chain exited with status $status: $out"
[ "$(events)" = "$(cat "$EXPECTED_KEYS")" ] || fail "chained: the key transcript differs"
has "chained" "chained to the BIOS handler"
has "chained buffer" "dropped, 0 decoder recoveries, BIOS type-ahead 8 keys"
has "chained restore" "(were 0x00), BIOS type-ahead 0 keys"
printf '%s\n' "$out" | grep -E '^input: (keyboard|[0-9]+ key)' | cut -c1-200

step "Overflow: 80 scancode bytes arrive while the core is not reading; the ring drops 16 and the decoder recovers"
many="$(printf 'a %.0s' $(seq 40)) $(printf ', %.0s' $(seq 200)) esc"
TYPE="-w 1 -p 0.05" run "$many" INPUT.EXE text stall=6
[ "$status" = 0 ] || fail "INPUT.EXE stall exited with status $status: $out"
has "dropped" "16 dropped by the full ring"
has "recovered" "16 dropped, 1 decoder recoveries"
[ "$(events | grep -c .)" = 65 ] || fail "expected 64 kept bytes to decode to 32 A presses and releases, plus the Escape press: $(events | grep -c .) events"
printf '%s\n' "$out" | grep -E '^input: (stalled|[0-9]+ key)' | cut -c1-170

step "No mouse driver: the program says so and runs keyboard-only"
CONF=$'[mouse]\nbuiltin_dos_mouse_driver = off' run "$KEYS" INPUT.EXE text
[ "$status" = 0 ] || fail "INPUT.EXE without a mouse driver exited with status $status: $out"
has "no driver" "input: mouse: no mouse driver; keyboard only"
[ "$(events)" = "$(cat "$EXPECTED_KEYS")" ] || fail "no driver: the key transcript differs"
printf '%s\n' "$out" | grep -E '^input: mouse'

step "A missing key fails the comparison (the 'd' is left out)"
run "${KEYS/ d / }" INPUT.EXE text
[ "$status" = 0 ] || fail "INPUT.EXE exited with status $status: $out"
if [ "$(events)" = "$(cat "$EXPECTED_KEYS")" ]; then fail "a transcript with a key missing was accepted"; fi
echo "rejected, as it should be ($(events | grep -c .) events, expected $(grep -c . "$EXPECTED_KEYS"))"

step "A wrong expected transcript fails the check"
wrong="${TMPDIR:-/tmp}/binput-wrong.$$"
sed 's/key B down/key X down/' "$EXPECTED_KEYS" > "$wrong"
run "$KEYS" INPUT.EXE text
if [ "$(events)" = "$(cat "$wrong")" ]; then fail "a wrong transcript was accepted"; fi
echo "rejected, as it should be"

step "No Escape: the program gives up after the timeout, exits 4 and puts everything back"
run "a b" INPUT.EXE timeout=3
[ "$status" = 4 ] || fail "INPUT.EXE timeout=3 exited with status $status, not 4: $out"
has "timeout" "input: timed out after 3 s without Escape"
has "timeout restore" "IRQ 1 handler is the original one: true, ours installed: false, video mode 0x03"
echo "status $status"

step "A panic with the keyboard and the mouse taken over: the vector, the video mode and the mouse are put back before the report"
run "$KEYS" INPUT.EXE panic
[ "$status" = 3 ] || fail "INPUT.EXE panic exited with status $status, not 3: $out"
has "panic" "input: now panicking"
has "panic restore" "IRQ 1 handler is the original one: true, ours installed: false, video mode 0x03"
case "$out" in *'original one: true'*'RT PANIC'*) ;; *) fail "the clean-up list did not run before the panic report: $out" ;; esac
echo "status $status"

step "An exit with everything taken over puts it back"
run "$KEYS" INPUT.EXE leave
[ "$status" = 0 ] || fail "INPUT.EXE leave exited with status $status: $out"
has "leave" "input: now exiting"
has "leave restore" "IRQ 1 handler is the original one: true, ours installed: false, video mode 0x03"
echo "status $status"

printf '\nAll spike B checks passed.\n'
