#!/bin/bash
# Runs a DOS command in headless DOSBox Staging and passes back its output and exit status.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/dosrun.sh <dir> <command> [args...]
#
#   <dir>      mounted as C: and made the current directory; holds the program and CWSDPMI.EXE
#   <command>  run there, e.g. `DESK.EXE panic`
#
# The program's standard output goes to this script's standard output (with DOS line ends removed) and its exit status becomes
# this script's, so `tools/dosrun.sh dist DESK.EXE` behaves like running the program. Two statuses are the script's own: 124 when
# the command outlived DOS_TIMEOUT seconds (default 60), and 125 when DOSBox itself failed, in which case its log is printed to
# standard error. Set DOS_VERBOSE=1 to always see DOSBox's log.
#
# DOSBox needs no display: SDL_VIDEODRIVER=dummy gives it a video driver that draws nowhere, and SDL_AUDIODRIVER=dummy the same
# for audio. The environment's DISPLAY and WAYLAND_DISPLAY are removed so a developer's desktop is never used by accident.
#
# DOS has no way to hand an exit status to its host, and DOSBox Staging does not expand %ERRORLEVEL%. The generated batch file
# recovers it with a ladder of `IF ERRORLEVEL n` tests, which hold for every n up to the status, so the last line that matches
# is the status.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"

[ $# -ge 2 ] || { sed -n '/^# usage/,/^$/p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }
DIR="$(cd "$1" && pwd)"
shift

DOSBOX=("$HERE"/vendor/dosbox/*/dosbox)
[ -x "${DOSBOX[0]}" ] || { echo "dosrun: no DOSBox in vendor/; run tools/setup.sh" >&2; exit 125; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/desk-dosrun.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# The scratch directory is mounted as D: so the program's own directory only gets what the program writes.
{
  echo '@echo off'
  echo "$* > D:\\STDOUT.TXT"
  echo 'set RC=0'
  for n in $(seq 1 255); do echo "if errorlevel $n set RC=$n"; done
  echo 'echo %RC%> D:\EXITCODE.TXT'
} | sed 's/$/\r/' > "$WORK/RUN.BAT"

cat > "$WORK/dosbox.conf" <<CONF
[sdl]
fullscreen = false
# 'texture' draws through SDL's software renderer. The default asks for OpenGL, which the dummy video driver cannot provide.
output = texture

[dosbox]
machine = svga_s3
memsize = 16

[cpu]
# The interpreter is the slowest core and the most predictable. cputype 486 makes DOSBox fault on any instruction a 486 lacks
# (CPUID, CMOV and so on), so a build that needs a Pentium fails here instead of on the target.
core = normal
cputype = 486
cpu_cycles = max
cpu_cycles_protected = auto

[mixer]
nosound = true

[midi]
mididevice = none

[sblaster]
sbtype = none

[speaker]
pcspeaker = off

[autoexec]
mount c "$DIR"
mount d "$WORK"
c:
call d:\\run.bat
exit
CONF

status=0
env -u DISPLAY -u WAYLAND_DISPLAY SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy \
  timeout "${DOS_TIMEOUT:-60}" "${DOSBOX[0]}" --noprimaryconf --nolocalconf --conf "$WORK/dosbox.conf" \
  > "$WORK/dosbox.log" 2>&1 || status=$?

if [ "$status" = 124 ]; then
  echo "dosrun: timed out after ${DOS_TIMEOUT:-60}s: $*" >&2
  [ -f "$WORK/STDOUT.TXT" ] && tr -d '\r' < "$WORK/STDOUT.TXT"
  exit 124
fi
if [ "$status" != 0 ] || [ ! -f "$WORK/EXITCODE.TXT" ]; then
  echo "dosrun: DOSBox failed (status $status); its log follows" >&2
  cat "$WORK/dosbox.log" >&2
  exit 125
fi
[ -z "${DOS_VERBOSE:-}" ] || cat "$WORK/dosbox.log" >&2

[ ! -f "$WORK/STDOUT.TXT" ] || tr -d '\r' < "$WORK/STDOUT.TXT"
exit "$(tr -d '\r\n ' < "$WORK/EXITCODE.TXT")"
