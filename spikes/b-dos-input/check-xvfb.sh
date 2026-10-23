#!/bin/bash
# The mouse checks (opt-in): INPUT.EXE in DOSBox's SDL window on a virtual X server, with xdotool moving and clicking the pointer.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/b-dos-input/check-xvfb.sh        (after tools/setup.sh and spikes/b-dos-input/build.sh; needs Docker)
#
# DOSBox has no scripted pointer, so the run is in a container (tools/docker/Dockerfile.dos-xvfb, built once as desk-dos-xvfb:1) that
# holds Xvfb and xdotool. DOSBox captures the pointer at start (`mouse_capture = onstart`) and SDL reports relative motion; xdotool
# moves it by known amounts, holds each button for 0.3 s (a press and release inside one poll would be one event), turns the wheel and
# types Escape through the X keyboard. The program's event lines are reduced to totals (the motion arrives in one or several events
# depending on timing) and compared with what the script did.
set -euo pipefail
cd "$(dirname "$0")/../.."
DIR=spikes/b-dos-input
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

command -v docker > /dev/null || { echo "skipped: needs Docker (INPUT_EXTRA=1)"; exit 0; }
[ -f dist/b/INPUT.EXE ] || $DIR/build.sh

step "The container image (cached by tag)"
if ! docker image inspect desk-dos-xvfb:1 > /dev/null 2>&1; then
  docker build -q -t desk-dos-xvfb:1 -f tools/docker/Dockerfile.dos-xvfb tools/docker > /dev/null
fi
echo "desk-dos-xvfb:1"

# vendor/ links to the DOS tools, which may live outside the repository (the CI image keeps them in /opt/dos-toolchain, a host may keep
# them anywhere); the container sees them at the same path so the links resolve there too.
tools="$(dirname "$(readlink -f vendor/dosbox)")"

# drive <MOUSE_DRIVER> <script>: runs the script against INPUT.EXE in the container and sets $out and $status.
drive() {
  status=0
  # A second argument asks for the screen dump, which the program writes next to itself in dist/b (the repository is mounted writable).
  out="$(docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -e MOUSE_DRIVER="$1" -e INPUT_ARGS="${2:+dump}${4:+ $4}" -v "$PWD":/work -v "$tools":"$tools":ro \
    desk-dos-xvfb:1 /work/$DIR/xvfb-inner.sh "/work/$3" 2> /dev/null)" || status=$?
}

# The motion and buttons the script produced: "dx dy wheel-sum states" with the button states in order, repeats collapsed.
reduce() {
  printf '%s\n' "$out" | awk '
    /^mouse / {
      for (i = 2; i <= NF; i++) { split($i, kv, "="); v[kv[1]] = kv[2] }
      dx += v["dx"]; dy += v["dy"]; wheel += v["wheel"]
      if (v["buttons"] != last) { states = states (states == "" ? "" : ",") v["buttons"]; last = v["buttons"] }
    }
    END { printf "dx=%d dy=%d wheel=%d states=%s\n", dx, dy, wheel, states }'
}

step "Pointer motion, three buttons, the wheel and a key through the real mouse driver and IRQ 1"
drive on dump $DIR/mouse.xdotool
[ "$status" = 0 ] || fail "the run exited with status $status: $out"
printf '%s\n' "$out" | grep -E '^(mouse|key) ' | cut -c1-100
got="$(reduce)"
want='dx=55 dy=50 wheel=0 states=-,L,-,R,-,M,-,L'
[ "$got" = "$want" ] || fail "the mouse transcript reduces to \"$got\", not \"$want\""
echo "reduced: $got"
case "$out" in *'wheel=1'*'wheel=-1'*) ;; *) fail "the wheel events (+1 then -1) are missing" ;; esac
case "$out" in *'key A down mods=none'*'key A up mods=none'*'key Escape down'*) ;; *) fail "the keys typed through X are missing" ;; esac
# The cursor is the sum of the deltas from the centre; the driver's own position moves two mickeys per row (its default ratio).
case "$out" in *'cursor at (375, 290)'*) ;; *) fail "the cursor is not at (375, 290)" ;; esac
case "$out" in *"the driver's own position is (375, 265)"*) ;; *) fail "the driver's position is not (375, 265)" ;; esac
case "$out" in *'IRQ 1 handler is the original one: true'*) ;; *) fail "the vector was not restored" ;; esac

step "The screen the program drew (the cursor at (375, 290), the left button lit, the key log) as a PNG"
python3 -I tools/raw2png.py dist/b/SCREEN.RAW dist/b/SCREEN.PAL 640 480 dist/b/screen.png
[ "$(head -c 8 dist/b/screen.png | od -An -c | tr -d ' ')" = '211PNG\r\n032\n' ] || fail "screen.png is not a PNG"
echo "dist/b/screen.png"

step "Clicks over in a few milliseconds, with the program polling every 165 ms: only the press and release counters see them"
# Each click is a press and a release five milliseconds apart (xdotool mousedown, sleep, mouseup), far shorter than a poll, so the
# button's state is the same before and after and only functions 0005h and 0006h show it. (A press and release with no gap at all,
# as `xdotool click` sends them, never reach DOSBox's driver, whatever the program does: X delivers both with one timestamp and SDL
# or DOSBox drops the pair, so no counter holds it.)
drive on "" $DIR/quick.xdotool slowpoll
[ "$status" = 0 ] || fail "the quick-click run exited with status $status: $out"
got="$(reduce)"
want_quick='dx=0 dy=0 wheel=0 states=L,-,L,-,L,-,L,-,L,-,L,-'
[ "$got" = "$want_quick" ] || fail "six quick clicks reduce to \"$got\", not \"$want_quick\""
echo "reduced: $got"

step "A transcript with the middle button missing is rejected"
saved_out=$out
out="$(printf '%s\n' "$out" | grep -v 'buttons=M' || true)"
[ "$(reduce)" != "$want" ] || fail "a transcript with the middle button missing matched"
out=$saved_out
echo "rejected, as it should be"

step "No mouse driver, pointer moving anyway: keyboard-only, no mouse events, the keys still arrive"
drive off "" $DIR/mouse.xdotool
[ "$status" = 0 ] || fail "the run without a driver exited with status $status: $out"
case "$out" in *'input: mouse: no mouse driver; keyboard only'*) ;; *) fail "no keyboard-only message: $out" ;; esac
[ -z "$(printf '%s\n' "$out" | grep '^mouse ' || true)" ] || fail "mouse events without a driver"
case "$out" in *'key A down mods=none'*'key Escape down'*) ;; *) fail "the keys did not arrive: $out" ;; esac
echo "keyboard only, keys arrived"

printf '\nAll spike B mouse checks passed.\n'
