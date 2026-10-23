#!/bin/bash
# Builds DESK.EXE from the Rust core and the C layer, ready to run beside CWSDPMI.EXE in dist/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/build.sh        (after tools/setup.sh)
#
#   core/ (Rust, no_std)   --cargo -Zbuild-std, target core/dos.json-->  core/target/dos/release/libdesk_core.a
#   libdesk_core.a         --tools/coff2go32.py------------------------>  dist/obj/desk_core_go32.a
#   c/*.c + the archive    --i586-pc-msdosdjgpp-gcc-------------------->  dist/DESK.EXE
#   vendor/cwsdpmi                                                  -->  dist/CWSDPMI.EXE
set -euo pipefail
cd "$(dirname "$0")/.."

DJGPP_BIN="$PWD/vendor/djgpp/djgpp/bin"
[ -x "$DJGPP_BIN/i586-pc-msdosdjgpp-gcc" ] || { echo "build: DJGPP not found in vendor/; run tools/setup.sh" >&2; exit 1; }
export PATH="$DJGPP_BIN:$PATH"

mkdir -p dist/obj

# 1. Rust. -Zbuild-std compiles core, alloc and compiler_builtins for dos.json (see core/.cargo/config.toml), and those options are
#    unstable. RUSTC_BOOTSTRAP=1 lets the pinned stable toolchain accept them, for this build only.
(cd core && RUSTC_BOOTSTRAP=1 cargo build --release --locked)

# 2. Convert the objects for DJGPP's linker (why: tools/coff2go32.py).
tools/coff2go32.py core/target/dos/release/libdesk_core.a dist/obj/desk_core_go32.a i586-pc-msdosdjgpp-ar

# 3. C layer and link. -march=i486 keeps our own code to what the lowest target CPU has. For `-o dist/obj/desk` DJGPP's gcc writes the
#    COFF image `desk` and `desk.exe`, the same image behind DJGPP's stub loader, which is what DOS runs. (With an upper-case
#    `-o DESK.EXE` it writes `DESK.EXE` and a second `DESK.exe`, hence the copy.)
i586-pc-msdosdjgpp-gcc -std=c99 -Wall -Wextra -Werror -O2 -march=i486 \
  -o dist/obj/desk c/main.c c/out.c c/panic.c dist/obj/desk_core_go32.a
cp dist/obj/desk.exe dist/DESK.EXE

# 4. The DPMI host DJGPP programs load when none is running. It must sit beside the program.
cp vendor/cwsdpmi/bin/CWSDPMI.EXE dist/CWSDPMI.EXE

echo "build: dist/DESK.EXE ($(wc -c < dist/DESK.EXE) bytes)"
