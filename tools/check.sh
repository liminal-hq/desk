#!/bin/bash
# Everything the CI runs: licence headers, markdown style and links, Rust format and lints, then the DOS build and a headless run.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check.sh        (run tools/setup.sh once first)
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

step "Licence headers"
tools/check-headers.sh

step "Markdown style and links"
python3 -I tools/check-docs.py

step "Rust format and lints"
(cd core && cargo fmt --check)
(cd core && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build DESK.EXE"
tools/build.sh

# The line the Rust core must produce, byte for byte. The numbers are 1^2 + ... + 10^2, that sum times 1000003, and the x87's
# square root of the sum.
EXPECTED='DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417'

step "Run DESK.EXE in headless DOSBox"
status=0
actual="$(tools/dosrun.sh dist DESK.EXE)" || status=$?
file="$(tr -d '\r' < dist/OUT.TXT 2> /dev/null || true)"
[ "$status" = 0 ] || fail "DESK.EXE exited with status $status; output: $actual"
[ "$actual" = "$EXPECTED" ] || fail "standard output is not the expected line
  expected: $EXPECTED
  actual:   $actual"
[ "$file" = "$EXPECTED" ] || fail "OUT.TXT is not the expected line
  expected: $EXPECTED
  actual:   $file"
echo "$actual"

step "A Rust panic reports through C and exits non-zero"
status=0
actual="$(tools/dosrun.sh dist DESK.EXE panic)" || status=$?
[ "$status" = 3 ] || fail "DESK.EXE panic exited with status $status, not 3; output: $actual"
case "$actual" in
  "DESK-SKELETON PANIC at line "*) ;;
  *) fail "no panic report in the output: $actual" ;;
esac
grep -q 'DESK-SKELETON PANIC' dist/OUT.TXT || fail "the panic report is missing from OUT.TXT"
echo "$actual (exit status $status)"

printf '\nAll checks passed.\n'
