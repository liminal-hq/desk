#!/bin/bash
# Measures the size of the hello programs under the size flags, and the time of a cold -Zbuild-std. Not part of the checks.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/e-std-stubs/measure.sh        (after tools/setup.sh and build.sh)
#
# Sizes are the .EXE files (the 2 KiB DJGPP stub, the .text/.rodata image and the .data image, as tools/elf2go32.py lays them out).
# The flag sets are rustc's size levers on top of opt-level "s", LTO and panic=abort, which every build already has:
#
#   default   nothing more
#   imm       -Cpanic=immediate-abort: a panic aborts at once, with no message and no formatting code
#   imm+loc   imm, plus -Zlocation-detail=none (no file names in panic locations) and -Zfmt-debug=none (no field names in derived Debug)
set -euo pipefail
cd "$(dirname "$0")/../.."
E=spikes/e-std-stubs
export RUSTC_BOOTSTRAP=1
STD="$PWD/$E/target/desk-src"
LINK="-C link-arg=-Tlink.ld -C link-arg=-nostdlib -C link-arg=-static -C relocation-model=static"
declare -A SET=(
  [default]=""
  [imm]="-Zunstable-options -Cpanic=immediate-abort"
  [imm+loc]="-Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none -Zfmt-debug=none"
)
STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
tmp="$(mktemp -d "${TMPDIR:-/tmp}/e-measure.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
"$STUBIFY" -g "$tmp/stub.exe" > /dev/null

# exe <elf> -> the packed size
exe() { tools/elf2go32.py "$tmp/stub.exe" "$1" "$tmp/x.exe" && wc -c < "$tmp/x.exe"; }

printf '%-10s %10s %10s %10s %10s %10s\n' "flags" "no_std" "std stub" "std trusty" "std desk" "desk+map"
for set in default imm "imm+loc"; do
  flags="$LINK ${SET[$set]}"
  td="target/m-${set//+/-}"
  base="$(cd $E && RUSTFLAGS="$flags" cargo build --release --locked --target targets/dos-std.json -p base -Zbuild-std=core,alloc,compiler_builtins \
    --target-dir $td-base > /dev/null 2>&1 && echo ok || echo fail)"
  stub="$(cd $E && __CARGO_TESTS_ONLY_SRC_ROOT="$STD/library" RUSTFLAGS="$flags" cargo build --release --locked --target targets/stub-std.json \
    -p probe --target-dir $td-stub > /dev/null 2>&1 && echo ok || echo fail)"
  desk="$(cd $E && __CARGO_TESTS_ONLY_SRC_ROOT="$STD/library" RUSTFLAGS="$flags" cargo build --release --locked --target targets/desk-app.json \
    -p probe --features desk --target-dir $td-desk > /dev/null 2>&1 && echo ok || echo fail)"
  trusty="$(cd $E && RUSTFLAGS="$flags" cargo build --release --locked --target targets/dos-trusty.json -p probe --features trusty \
    --target-dir $td-trusty > /dev/null 2>&1 && echo ok || echo fail)"
  f() { # <status> <elf> -> size or the failure
    if [ "$1" = ok ]; then exe "$2"; else echo "build failed"; fi
  }
  printf '%-10s %10s %10s %10s %10s %10s\n' "$set" \
    "$(f "$base" $E/$td-base/dos-std/release/base)" \
    "$(f "$stub" $E/$td-stub/stub-std/release/hello_std)" \
    "$(f "$trusty" $E/$td-trusty/dos-trusty/release/hello_std)" \
    "$(f "$desk" $E/$td-desk/desk-app/release/hello_std)" \
    "$(f "$desk" $E/$td-desk/desk-app/release/hello_map)"
done

echo
echo "Cold -Zbuild-std (seconds, this machine), then the program build:"
cd $E
for what in "core,alloc,compiler_builtins:base:targets/dos-std.json" "std,panic_abort:probe:targets/desk-app.json"; do
  IFS=: read -r bs pkg tgt <<< "$what"
  rm -rf target/m-cold
  start=$(date +%s.%N)
  __CARGO_TESTS_ONLY_SRC_ROOT="$STD/library" cargo build --release --locked --target "$tgt" -p "$pkg" -Zbuild-std="$bs" --target-dir target/m-cold \
    $([ "$pkg" = probe ] && echo "--features desk") > /dev/null 2>&1
  end=$(date +%s.%N)
  printf '  -Zbuild-std=%-26s -p %-6s %6.1f s\n' "$bs" "$pkg" "$(python3 -c "print($end - $start)")"
done
