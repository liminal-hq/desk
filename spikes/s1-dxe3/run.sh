#!/bin/bash
# Runs every s1-dxe3 scenario in headless DOSBox and compares each transcript with spikes/s1-dxe3/expected/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/s1-dxe3/run.sh [--update]      (after spikes/s1-dxe3/build.sh)
#
# A scenario is one HOST.EXE run. Its output, without the "mem:" lines (which hold numbers that change from run to run), must
# equal the expected file byte for byte, and its exit status must be the one listed here. --update rewrites the expected files
# from the current output instead of comparing: review the diff before committing it.
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=dist/s1
EXPECTED=spikes/s1-dxe3/expected
update=0
[ "${1:-}" = "--update" ] && update=1
fail() { printf 'FAIL: %s\n' "$1" >&2; failed=1; }
failed=0

# An empty module is the trap: `ld -r` leaves an archive member out when nothing asks for it, and dlopen then succeeds on a
# 96-byte file with no entry points. Every module must be real.
for f in "$OUT"/*.DXE; do
  size="$(wc -c < "$f")"
  [ "$size" -gt 4000 ] || fail "$f is only $size bytes: the linker left the code out"
done

# name | HOST.EXE arguments | expected exit status
SCENARIOS='
life|life|0
life-b|life-b|0
two|two|0
two-global|two global|0
two-b|two-b|0
panic|panic|0
panic-b|panic-b|0
fault-div-catch|fault div catch|0
fault-div-raw|fault div raw|255
noreg|noreg|0
leak|leak 200|0
leak-crash|leak-crash 200|0
leak-control|leak-control 200|1
'
while IFS='|' read -r name args want; do
  [ -n "$name" ] || continue
  status=0
  raw="$(tools/dosrun.sh "$OUT" HOST.EXE $args 2> /dev/null)" || status=$?
  actual="$(grep -v '^mem: ' <<< "$raw" || true)"
  if [ "$update" = 1 ]; then
    printf '%s\n' "$actual" > "$EXPECTED/$name.txt"
    printf 'updated %s (exit %s)\n' "$name" "$status"
    continue
  fi
  [ "$status" = "$want" ] || fail "$name: exit status $status, expected $want"
  if [ "$actual" != "$(cat "$EXPECTED/$name.txt")" ]; then
    fail "$name: the transcript differs from $EXPECTED/$name.txt"
    diff <(printf '%s\n' "$actual") "$EXPECTED/$name.txt" | head -20 >&2 || true
  else
    printf 'ok   %-16s (exit %s, %s lines)\n' "$name" "$status" "$(printf '%s\n' "$actual" | wc -l)"
  fi
done <<< "$SCENARIOS"

# The recoveries must really have happened, not just matched by accident.
if [ "$update" = 0 ]; then
  grep -q 'crashed: panic: panic with heap' "$EXPECTED/panic.txt" || fail "panic.txt does not show the panic"
  grep -q 'reclaimed 6 blocks' "$EXPECTED/panic.txt" || fail "panic.txt does not show the heap being reclaimed"
  grep -q 'SIGFPE' "$EXPECTED/fault-div-catch.txt" || fail "fault-div-catch.txt does not show the fault"
  ! grep -q 'survived' "$EXPECTED/fault-div-raw.txt" || fail "fault-div-raw.txt claims to survive an unhandled fault"
  grep -q 'HIGHER (leak)' "$EXPECTED/leak-control.txt" || fail "leak-control.txt does not show the leak being noticed"
  grep -q 'no growth' "$EXPECTED/leak.txt" || fail "leak.txt does not show stable memory"
fi
exit "$failed"
