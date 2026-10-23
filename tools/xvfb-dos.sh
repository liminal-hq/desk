#!/bin/bash
# Runs a DOS program in DOSBox's SDL window on a virtual X server, with xdotool moving, clicking and typing over the window, in Docker.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/xvfb-dos.sh [--mouse on|off] [--escape SECONDS] [--start-wait SECONDS] [--step-wait SECONDS] DIR SCRIPT PROGRAM [ARGS...]
#
#   DIR        the program's directory (mounted as C: by tools/dosrun.sh, so the files the program writes land there)
#   SCRIPT     a file of xdotool commands, one line each, run after the window is up (blank lines and lines starting with # are skipped);
#              `tools/frames.py live SCENARIO --xdotool` writes one. The last line should press Escape if the program ends on it
#   PROGRAM    run in DIR with ARGS; its standard output is this script's, its exit status becomes this script's
#   --mouse    `off` takes DOSBox's built-in mouse driver away (the default is `on`, with three buttons and the wheel)
#   --escape   also types Escape through AUTOTYPE after this many seconds, as a fallback for a program that waits for it
#   --start-wait   seconds between DOSBox's window appearing and the first line of SCRIPT (default 6: the program prints nothing until it ends)
#   --step-wait    seconds after each line of SCRIPT (default 0.15)
#
# DOSBox has no scripted pointer, so the run is in a container (tools/docker/Dockerfile.dos-xvfb, built once as desk-dos-xvfb:1) that holds
# Xvfb and xdotool. DOSBox captures the pointer at start (`mouse_capture = onstart`) and SDL reports relative motion, which xdotool
# produces by known amounts. The DOS tools of vendor/ are mounted at the path vendor/ links to, so they resolve inside. Skipped (exit 3,
# with a message) when Docker is not installed.
set -uo pipefail
cd "$(dirname "$0")/.."

if [ "${XVFB_DOS_INSIDE:-}" != 1 ]; then
  # ---- Outside: the arguments, the image and the container ----
  mouse=on escape="" start_wait="" step_wait=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --mouse) mouse=$2; shift 2 ;;
      --escape) escape=$2; shift 2 ;;
      --start-wait) start_wait=$2; shift 2 ;;
      --step-wait) step_wait=$2; shift 2 ;;
      --) shift; break ;;
      -*) echo "xvfb-dos: unknown option $1" >&2; exit 2 ;;
      *) break ;;
    esac
  done
  [ $# -ge 3 ] || { sed -n '/^# usage/,/^set -/p' "$0" | sed '/^set -/d; s/^# \{0,1\}//' >&2; exit 2; }
  case "$mouse" in on | off) ;; *) echo "xvfb-dos: --mouse is on or off" >&2; exit 2 ;; esac
  dir=$1 script=$2
  [ -d "$dir" ] || { echo "xvfb-dos: no directory $dir" >&2; exit 2; }
  [ -f "$script" ] || { echo "xvfb-dos: no script $script" >&2; exit 2; }
  command -v docker > /dev/null || { echo "xvfb-dos: skipped, needs Docker" >&2; exit 3; }
  if ! docker image inspect desk-dos-xvfb:1 > /dev/null 2>&1; then
    docker build -q -t desk-dos-xvfb:1 -f tools/docker/Dockerfile.dos-xvfb tools/docker > /dev/null || exit 3
  fi
  # vendor/ links to the DOS tools, which may live outside the repository (the CI image keeps them in /opt/dos-toolchain, a host may keep
  # them anywhere); the container sees them at the same path so the links resolve there too.
  tools="$(dirname "$(readlink -f vendor/dosbox)")"
  exec docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -e XVFB_DOS_INSIDE=1 -e MOUSE_DRIVER="$mouse" \
    ${escape:+-e AUTOTYPE_ESCAPE="$escape"} ${start_wait:+-e START_WAIT="$start_wait"} ${step_wait:+-e STEP_WAIT="$step_wait"} \
    -v "$PWD":/work -v "$tools":"$tools":ro desk-dos-xvfb:1 /work/tools/xvfb-dos.sh "$@"
fi

# ---- Inside the container: a virtual X server, the program in DOSBox's window on it, and xdotool over the window ----
dir=$1 script=$2
shift 2

Xvfb :99 -screen 0 1024x768x24 -nolisten tcp > /tmp/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2> /dev/null' EXIT
for _ in $(seq 50); do xdpyinfo -display :99 > /dev/null 2>&1 && break; sleep 0.2; done
export DISPLAY=:99

# The SDL window captures the pointer when DOSBox starts (autolock), and the built-in driver reports three buttons and a wheel;
# MOUSE_DRIVER=off takes the driver away.
export DOS_X11=1 DOS_TIMEOUT=${DOS_TIMEOUT:-60}
export DOS_CONF=$'[mouse]\nmouse_capture = onstart\nbuiltin_dos_mouse_driver_model = wheel\nbuiltin_dos_mouse_driver = '"${MOUSE_DRIVER:-on}"
[ -z "${AUTOTYPE_ESCAPE:-}" ] || export DOS_AUTOTYPE="-w $AUTOTYPE_ESCAPE esc"
tools/dosrun.sh "$dir" "$@" > /tmp/program.out 2> /tmp/program.err &
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
  case "$line" in '' | '#'*) continue ;; esac
  # shellcheck disable=SC2086
  xdotool $line
  sleep "${STEP_WAIT:-0.15}"
done < "$script"
wait $dos
status=$?
cat /tmp/program.out
cat /tmp/program.err >&2
exit $status
