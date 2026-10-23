#!/bin/bash
# Re-records the shell's goldens: runs the host scenario test in record mode, then converts each frame to an indexed PNG.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/shell-goldens.sh
#
# Starts from clean (the old dumps and PNGs are removed), so a scenario or frame that no longer exists leaves no stale file. Look at
# the PNGs, and at `git status`, before committing. See tests/shell/README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

dumps=shell/target/frames
rm -rf "$dumps"
mkdir -p "$dumps"

(cd shell && DESK_FRAMES_RECORD=1 cargo test --locked --quiet -p desk-shell --test scenarios)

for scn in tests/shell/*.scn; do
  name="$(basename "$scn" .scn)"
  out="tests/shell/golden/$name"
  rm -rf "$out"
  mkdir -p "$out"
  # A scenario's frames are the lines of its .sums: NAME DEPTH WIDTHxHEIGHT INDEX COLOUR.
  while read -r frame depth size _; do
    case "$frame" in '' | '#'*) continue ;; esac
    python3 -I tools/frames.py png "$dumps/$name/$frame-$depth.RAW" "$dumps/$name/$frame-$depth.PAL" "${size%x*}" "${size#*x}" "$out/$frame-$depth.png"
  done < "tests/shell/$name.sums"
  python3 -I tools/frames.py goldens "tests/shell/$name.sums" "$out"
done
