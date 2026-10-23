#!/bin/bash
# Builds a patched copy of the std sources in the pinned toolchain's rust-src: the patch adds a "desk" and "dos" platform layer.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/e-std-stubs/patch/mkstd.sh <destination>
#
# The copy is a `rust` source root: `<destination>/library` is the toolchain's library workspace with desk-std.patch applied (the
# arms that select the platform layer) and overlay/ copied over it (the platform layer's own files). Cargo is pointed at it with the
# environment variable `__CARGO_TESTS_ONLY_SRC_ROOT=<destination>`, which build-std reads instead of the sysroot's rust-src.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="${1:?usage: mkstd.sh <destination>}"
SRC="$(rustc --print sysroot)/lib/rustlib/src/rust"
[ -d "$SRC/library" ] || { echo "mkstd: no rust-src in the toolchain ($SRC); run tools/setup.sh" >&2; exit 1; }
rm -rf "$DEST"
mkdir -p "$DEST"
cp -r "$SRC/library" "$DEST/library"
[ ! -d "$SRC/src" ] || cp -r "$SRC/src" "$DEST/src"
chmod -R u+w "$DEST"
patch -s -p1 -d "$DEST/library/.." < "$HERE/desk-std.patch" > /dev/null
cp -r "$HERE/overlay/." "$DEST/"
echo "mkstd: $DEST (patched std for $(rustc --version | cut -d' ' -f2))"
