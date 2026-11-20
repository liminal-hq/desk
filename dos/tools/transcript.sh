#!/bin/bash
# The helpers the runtime's check and the d0 spike's stage-1 check share: run a program in headless DOSBox, normalise what it prints and
# compare it with a recorded transcript. Sourced, not run; the caller has set -euo pipefail and is in the repository root.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Two things are normalised before comparing: source line and column numbers in panic reports (they move when the source does), and the
# registers `ctx` marks as scratch (leftovers of the stub's own work).

step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }

normalise() { sed -E 's/\.rs:[0-9]+:[0-9]+:/.rs:L:C:/; s#panicked at .*/alloc\.rs:[0-9]+:[0-9]+:#panicked at alloc.rs:L:C:#' | grep -v '^scratch:'; }

# run <dir> <program> [args...]: sets $out (normalised standard output) and $status (the program's)
run() {
  local dir=$1 raw
  shift
  status=0
  raw="$(dos/tools/dosrun.sh "$dir" "$@" 2> /dev/null)" || status=$?
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
