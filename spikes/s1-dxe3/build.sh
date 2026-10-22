#!/bin/bash
# Builds the s1-dxe3 spike: the Rust modules as .DXE files (route a and route b) and the C host, in dist/s1/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/s1-dxe3/build.sh        (after tools/setup.sh)
#
#   modules/ (Rust, no_std)  --cargo -Zbuild-std, target core/dos.json-->  modules/target*/dos/release/lib<name>.a
#   lib<name>.a              --tools/coff2go32.py---------------------->  one go32 object
#   the object               --dxe3gen -E _app_ -U--------------------->  dist/s1/<NAME>.DXE
#   host/*.c + c/out.c       --i586-pc-msdosdjgpp-gcc------------------>  dist/s1/HOST.EXE
set -euo pipefail
cd "$(dirname "$0")/../.."
SPIKE=spikes/s1-dxe3
OUT=dist/s1

DJGPP="$PWD/vendor/djgpp/djgpp"
[ -x "$DJGPP/bin/i586-pc-msdosdjgpp-gcc" ] || { echo "s1-dxe3: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
# dxe3gen lives beside the cross tools in the target directory, and needs DJDIR to find its linker script (dxe.ld).
export PATH="$DJGPP/bin:$DJGPP/i586-pc-msdosdjgpp/bin:$PATH"
export DJDIR="$DJGPP/i586-pc-msdosdjgpp"

rm -rf "$OUT"
mkdir -p "$OUT/obj"

# 1. Rust: the shared crate and three modules, once per route. Route b is the same source with the `route-b` feature.
#    OPT can override the profile for the size comparison in docs/spikes/s1-dxe3.md (for example OPT=3).
# opt-level is a number or a string in TOML (0-3, but "s" and "z" are quoted).
case "${OPT:-}" in [0-3]) OPT_TOML="$OPT" ;; *) OPT_TOML="\"${OPT:-}\"" ;; esac
cargo_build() { # <target dir> [cargo args...]
  local target="$1"; shift
  (cd "$SPIKE/modules" && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$target" cargo build --release --locked \
    ${OPT:+--config "profile.release.opt-level=$OPT_TOML"} ${LTO:+--config "profile.release.lto=$LTO"} "$@")
}
cargo_build target
cargo_build target/route-b --features hello/route-b,counter/route-b,crash/route-b

# 2. Convert each static library, unpack its one object and make a DXE3 module from it.
#    `-E _app_` exports only the three entry points (the C symbol `app_start` is `_app_start` in COFF); everything else, including
#    the module's own copy of core and alloc, stays private to the module. `-U` lets the imports from the core stay unresolved
#    until dlopen. dxe3gen's linker step (`ld -r`) takes objects: given an archive it pulls members only to satisfy undefined
#    symbols and leaves an empty 96-byte module, and with `-u` or `--whole-archive` to force them in, the module came out with
#    extra unresolved imports (`___register_frame_info`) that dlopen refuses, so the archive is unpacked (LTO leaves one object).
#    libgcc is linked into each module by path (dxe3gen's `-L` only covers $DJDIR/lib) for the 64-bit division helper that
#    core's float formatter calls.
LIBGCC="$(i586-pc-msdosdjgpp-gcc -print-libgcc-file-name)"
make_dxe() { # <crate> <target dir> <DXE name>
  local work="$OUT/obj/$3"
  mkdir -p "$work"
  tools/coff2go32.py "$SPIKE/modules/$2/dos/release/lib$1.a" "$work/$1.a" i586-pc-msdosdjgpp-ar > /dev/null
  (cd "$work" && i586-pc-msdosdjgpp-ar x "$1.a" && dxe3gen -o "../../$3.DXE" -E _app_ -U ./*.o "$LIBGCC")
}
for crate in hello counter crash; do
  upper="$(echo "$crate" | tr a-z A-Z)"
  make_dxe "$crate" target "$upper"
  make_dxe "$crate" target/route-b "${upper}B"
done
# A second copy of HELLO under another name, to load two instances of one module.
cp "$OUT/HELLO.DXE" "$OUT/HELLO2.DXE"
cp "$OUT/HELLOB.DXE" "$OUT/HELLO2B.DXE"

# 3. The host, in C. c/out.c is the skeleton's result output.
i586-pc-msdosdjgpp-gcc -std=gnu99 -Wall -Wextra -Werror -O2 -march=i486 -I c -I "$SPIKE/host" \
  -o "$OUT/obj/host" "$SPIKE"/host/*.c c/out.c
cp "$OUT/obj/host.exe" "$OUT/HOST.EXE"
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"

echo "s1-dxe3: $OUT/HOST.EXE and $(ls "$OUT"/*.DXE | wc -l) modules"
ls -l "$OUT"/*.DXE | awk '{printf "  %-12s %6d bytes\n", $9, $5}' | sed "s|$OUT/||"
