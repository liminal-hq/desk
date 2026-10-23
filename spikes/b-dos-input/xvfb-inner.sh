#!/bin/bash
# Runs inside the desk-dos-xvfb container (spikes/b-dos-input/check-xvfb.sh starts it): a virtual X server, INPUT.EXE in DOSBox's SDL
# window on it, and xdotool moving and clicking the pointer over the window. Prints INPUT.EXE's output.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: xvfb-inner.sh <script>   where <script> is a file of xdotool commands, one line each, run after the window is up. The last
# line should press Escape, which ends INPUT.EXE; AUTOTYPE_ESCAPE=<seconds> types one as well, as a fallback.
set -uo pipefail
cd "$(dirname "$0")/../.."
script=$1

Xvfb :99 -screen 0 1024x768x24 -nolisten tcp > /tmp/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2> /dev/null' EXIT
for _ in $(seq 50); do xdpyinfo -display :99 > /dev/null 2>&1 && break; sleep 0.2; done
export DISPLAY=:99

# The SDL window captures the pointer when DOSBox starts (autolock), and the built-in driver reports three buttons and a wheel;
# MOUSE_DRIVER=off takes the driver away.
export DOS_X11=1 DOS_TIMEOUT=60
export DOS_CONF=$'[mouse]\nmouse_capture = onstart\nbuiltin_dos_mouse_driver_model = wheel\nbuiltin_dos_mouse_driver = '"${MOUSE_DRIVER:-on}"
[ -z "${AUTOTYPE_ESCAPE:-}" ] || export DOS_AUTOTYPE="-w $AUTOTYPE_ESCAPE esc"
tools/dosrun.sh dist/b INPUT.EXE ${INPUT_ARGS:-} > /tmp/input.out 2> /tmp/input.err &
dos=$!

# Wait for DOSBox's window, then for the program to have started (it prints nothing until it ends, so wait a fixed time).
wid=""
for _ in $(seq 100); do
  wid="$(xdotool search --name 'DOSBox' 2> /dev/null | head -n 1)"
  [ -n "$wid" ] && break
  sleep 0.2
done
echo "window: ${wid:-none}" >&2
sleep "${START_WAIT:-6}"
while IFS= read -r line; do
  [ -n "$line" ] || continue
  # shellcheck disable=SC2086
  xdotool $line
  sleep "${STEP_WAIT:-0.15}"
done < "$script"
wait $dos
status=$?
cat /tmp/input.out
cat /tmp/input.err >&2
exit $status
