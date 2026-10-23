#!/bin/bash
# The shell on DOS: format and lints, the build, then SHELL.EXE in headless DOSBox at both depths against its expected transcripts and the
# goldens, and the negative runs (a corrupted pixel, the wrong depth's sums, an altered pixel, altered sums, an altered transcript).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: shell/dos/check.sh        (after tools/setup.sh)
#
# The transcripts are tests/shell/expected-dos-8.txt and expected-dos-4.txt. Each run's dumps are kept in dist/shell/dumps-DEPTH/ for the
# comparison with the native track's (frames.py same). The interactive mode runs with typed keys only; the mouse is tried by hand (docs/FIRST-WINDOW.md).
set -euo pipefail
cd "$(dirname "$0")/../.."
DIR=shell/dos
SUMS=tests/shell/first-window.sums
OUT=dist/shell
step() { printf '\n== %s\n' "$1"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
frames() { python3 -I tools/frames.py "$@"; }

# run <args...>: SHELL.EXE in DOSBox; sets $out (its standard output, with DOS line ends removed) and $status
run() {
  status=0
  out="$(tools/dosrun.sh $OUT SHELL.EXE "$@" 2> /dev/null)" || status=$?
}

# keep <name>: moves the run's dumps to $OUT/<name>/ (emptied first)
keep() {
  rm -rf "${OUT:?}/$1"
  mkdir -p "$OUT/$1"
  mv "$OUT"/*.RAW "$OUT"/*.PAL "$OUT/$1/"
}

# same <label> <file>: $out equals the file
same() {
  if [ "$out" != "$(cat "$2")" ]; then
    printf 'FAIL: %s: output differs from %s\n' "$1" "$2" >&2
    diff <(printf '%s\n' "$out") "$2" >&2 || true
    exit 1
  fi
}

# the digests the program printed for each frame equal the digests it read back from the frame buffer and the DAC
readbacks_equal() {
  local frames readbacks
  frames="$(printf '%s\n' "$out" | sed -n 's/^shell: frame //p')"
  readbacks="$(printf '%s\n' "$out" | sed -n 's/^shell: readback \(.*\) equal$/\1/p')"
  [ -n "$frames" ] && [ "$frames" = "$readbacks" ] || fail "the read-back digests are not the frames' digests"
}

step "Rust format and lints (shell/dos workspace)"
(cd $DIR && cargo fmt --check)
(cd $DIR && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build SHELL.EXE: rust-lld, then tools/elf2go32.py"
$DIR/build.sh
rm -f "$OUT"/*.RAW "$OUT"/*.PAL

for depth in 8 4; do
  step "SHELL.EXE depth$depth scenario dump: transcript, read-back and goldens"
  run depth$depth scenario dump
  [ "$status" = 0 ] || fail "SHELL.EXE depth$depth exited with status $status; output: $out"
  same "depth$depth" tests/shell/expected-dos-$depth.txt
  readbacks_equal
  printf '%s\n' "$out" > "$OUT/transcript-$depth.txt"
  frames transcript $SUMS $depth "$OUT/transcript-$depth.txt"
  keep dumps-$depth
  frames check $SUMS $depth "$OUT/dumps-$depth"
done

step "fontfile: the faces read from UI.DFN and TITLE.DFN give the same transcript"
run depth8 scenario dump fontfile
[ "$status" = 0 ] || fail "SHELL.EXE fontfile exited with status $status; output: $out"
printf '%s\n' "$out" | sed 's/^shell: faces from .*/shell: faces from the program/' > "$OUT/transcript-font.txt"
out="$(cat "$OUT/transcript-font.txt")"
same "fontfile" tests/shell/expected-dos-8.txt
keep dumps-font
frames same "$OUT/dumps-8" "$OUT/dumps-font"

step "No VBE: depth8 reports the fallback and matches the depth 4 sums"
status=0
out="$(DOS_MACHINE=vgaonly tools/dosrun.sh $OUT SHELL.EXE depth8 scenario dump 2> /dev/null)" || status=$?
[ "$status" = 0 ] || fail "SHELL.EXE on a VGA-only machine exited with status $status; output: $out"
case "$out" in *'VBE absent'*'falling back to depth 4'*) ;; *) fail "no fallback reported: $out" ;; esac
readbacks_equal
printf '%s\n' "$out" > "$OUT/transcript-vga.txt"
frames transcript $SUMS 4 "$OUT/transcript-vga.txt"
keep dumps-vga
frames check $SUMS 4 "$OUT/dumps-vga"
frames same "$OUT/dumps-4" "$OUT/dumps-vga"

step "Interactive: the keys typed through IRQ 1 reach the focused field, and Escape ends the run"
rm -f "$OUT"/SCREEN.*
status=0
out="$(DOS_AUTOTYPE='-w 3 -p 0.3 d e s k esc' tools/dosrun.sh $OUT SHELL.EXE depth8 dump timeout=30 2> /dev/null)" || status=$?
[ "$status" = 0 ] || fail "interactive SHELL.EXE exited with status $status; output: $out"
case "$out" in *'shell: wrote SCREEN.RAW'*'the video mode is 0x03') ;; *) fail "no screen written, or text mode not restored: $out" ;; esac
[ "$(wc -c < "$OUT/SCREEN.RAW")" = 307200 ] || fail "SCREEN.RAW is not 640 x 480 bytes"
python3 -I tools/frames.py png "$OUT/SCREEN.RAW" "$OUT/SCREEN.PAL" 640 480 "$OUT/screen.png"
echo "status $status, $OUT/screen.png written"

step "Negative: one pixel flipped in the frame buffer fails the read-back and the goldens"
run depth8 scenario dump corrupt
[ "$status" = 1 ] || fail "SHELL.EXE corrupt exited with status $status, not 1"
case "$out" in *'shell: readback moved '*DIFFERENT*'shell: FAILED'*) ;; *) fail "no mismatch reported: $out" ;; esac
case "$out" in *'the video mode is 0x03') ;; *) fail "text mode was not restored: $out" ;; esac
keep dumps-corrupt
if frames check $SUMS 8 "$OUT/dumps-corrupt" > /dev/null; then fail "the corrupted dumps matched the goldens"; fi
echo "status $status, the read-back reports DIFFERENT, the dumps are refused"

step "Negative: the depth 8 dumps against the depth 4 sums"
if frames check $SUMS 4 "$OUT/dumps-8" > /dev/null 2>&1; then fail "depth 8 dumps matched the depth 4 sums"; fi
echo "refused, as it should be"

step "Negative: one pixel altered in a dump"
rm -rf "$OUT/dumps-altered"
cp -r "$OUT/dumps-8" "$OUT/dumps-altered"
printf '\x7f' | dd of="$OUT/dumps-altered/TYPED.RAW" bs=1 seek=100000 count=1 conv=notrunc 2> /dev/null
if frames check $SUMS 8 "$OUT/dumps-altered" > /dev/null; then fail "an altered pixel was accepted"; fi
echo "refused, as it should be"

step "Negative: altered sums, and a transcript with one digest changed"
altered="$(mktemp "${TMPDIR:-/tmp}/shell-sums.XXXXXX")"
wrong="$(mktemp "${TMPDIR:-/tmp}/shell-wrong.XXXXXX")"
trap 'rm -rf "$altered" "$wrong"' EXIT
sed '1s/0$/1/; 1t; 1s/[1-9a-f]$/0/' $SUMS > "$altered"
cmp -s "$altered" $SUMS && fail "the altered sums equal the sums"
if frames check "$altered" 8 "$OUT/dumps-8" > /dev/null; then fail "altered sums were accepted"; fi
sed 's/frame open 8 640x480 920263aa/frame open 8 640x480 920263ab/' tests/shell/expected-dos-8.txt > "$wrong"
cmp -s "$wrong" tests/shell/expected-dos-8.txt && fail "the wrong transcript equals the expected one"
if frames transcript $SUMS 8 "$wrong" > /dev/null; then fail "a transcript with one digest changed was accepted"; fi
out="$(cat "$OUT/transcript-8.txt")"
if (same depth8 "$wrong") > /dev/null 2>&1; then fail "a wrong expected transcript was accepted"; fi
echo "refused, as it should be"

printf '\nAll shell on DOS checks passed.\n'
