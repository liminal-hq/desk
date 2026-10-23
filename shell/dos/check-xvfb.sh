#!/bin/bash
# The mouse path of SHELL.EXE (opt-in): tests/shell/live.scn played through DOSBox's SDL window under a virtual X server, and held to its goldens.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: shell/dos/check-xvfb.sh        (after tools/setup.sh; needs Docker)
#
# `frames.py live --xdotool` turns the scenario's steps after its first frame into xdotool lines (relative pointer motion, button
# presses, Alt+Tab, typing, Alt+F4, and F12 at each frame for a mark); tools/xvfb-dos.sh runs `SHELL.EXE depth8 marks` in DOSBox's window
# with them. Each mark must carry the scenario's digests in order (frames.py marks) and each dump must equal the golden (frames.py check).
# The run is repeated with --quick, where each adjacent press and release is one 5 ms click, and a run with the pointer motion taken out
# must fail. Where Docker is missing the check says it was skipped and succeeds.
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=dist/shell
SUMS=tests/shell/live.sums
SCN=tests/shell/live.scn
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
frames() { python3 -I tools/frames.py "$@"; }

command -v docker > /dev/null || { echo "skipped: needs Docker (INPUT_EXTRA=1)"; exit 0; }
[ -f $OUT/SHELL.EXE ] || shell/dos/build.sh
mkdir -p $OUT/xvfb

# play <name> <script>: SHELL.EXE depth8 marks in DOSBox's window with the xdotool script; its transcript is $OUT/xvfb/<name>.txt and its
# dumps are kept in $OUT/xvfb/<name>/; sets $status.
play() {
  local name=$1 script=$2
  rm -f "$OUT"/*.RAW "$OUT"/*.PAL
  status=0
  DOS_TIMEOUT=130 tools/xvfb-dos.sh --mouse on $OUT "$script" SHELL.EXE depth8 marks timeout=120 > "$OUT/xvfb/$name.txt" 2> "$OUT/xvfb/$name.err" || status=$?
  rm -rf "${OUT:?}/xvfb/$name"
  mkdir -p "$OUT/xvfb/$name"
  mv "$OUT"/M*.RAW "$OUT"/M*.PAL "$OUT/xvfb/$name/" 2> /dev/null || true
}

# verify <name>: the marks carry the digests and the dumps are the goldens
verify() {
  frames marks $SUMS $SCN 8 "$OUT/xvfb/$1.txt" --dumps "$OUT/xvfb/$1"
  frames check $SUMS 8 "$OUT/xvfb/$1"
}

frames live $SCN --xdotool > $OUT/xvfb/live.xdotool
frames live $SCN --xdotool --quick > $OUT/xvfb/quick.xdotool
grep -q '^mousedown 1 sleep 0.005 mouseup 1$' $OUT/xvfb/quick.xdotool || fail "--quick made no quick click"

step "live.scn through DOSBox's window: pointer motion, a click, a drag, Alt+Tab, typing, Alt+F4, a close box"
play live $OUT/xvfb/live.xdotool
[ "$status" = 0 ] || { tail -n 20 "$OUT/xvfb/live.txt" "$OUT/xvfb/live.err" >&2; fail "the live run exited with status $status"; }
case "$(cat "$OUT/xvfb/live.txt")" in *'shell: live ended by Escape: '*' keys, '*' pointer reports, '*' frames, '*' pixels presented, 8 marks'*) ;; *) fail "no summary of eight marks: $(cat "$OUT/xvfb/live.txt")" ;; esac
verify live

step "The same with quick clicks (a 5 ms press and release)"
play quick $OUT/xvfb/quick.xdotool
[ "$status" = 0 ] || { tail -n 20 "$OUT/xvfb/quick.txt" "$OUT/xvfb/quick.err" >&2; fail "the quick run exited with status $status"; }
verify quick

step "Negative: the pointer motion taken out of the script must fail the marks"
grep -v '^mousemove_relative ' $OUT/xvfb/live.xdotool > $OUT/xvfb/nomotion.xdotool
cmp -s $OUT/xvfb/live.xdotool $OUT/xvfb/nomotion.xdotool && fail "no motion line was removed"
play nomotion $OUT/xvfb/nomotion.xdotool
[ "$status" = 0 ] || fail "the run without motion exited with status $status"
if frames marks $SUMS $SCN 8 "$OUT/xvfb/nomotion.txt" > /dev/null; then fail "a run without pointer motion passed the marks"; fi
echo "refused, as it should be"

printf '\nAll shell on DOS mouse checks passed.\n'
