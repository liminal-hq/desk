#!/bin/bash
# Fetches js-dos from the npm registry, checks the tarball against a pinned SHA-256 and unpacks the parts the page needs into
# vendor/js-dos/, with its licence and provenance beside it. Nothing it writes is committed (vendor/ is ignored).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/s5-jsdos/fetch-jsdos.sh        (needs curl, sha256sum, tar and a GPL-2.0 text from tools/setup.sh's vendor/dosbox)
#
# js-dos is GPL-2.0 (https://github.com/caiiiycuk/js-dos). It is a separate program: fetched at build time and served beside the
# page and the DOS bundles, never linked into or merged with Desk's own code, which stays Apache-2.0 OR MIT.
set -euo pipefail
cd "$(dirname "$0")/../.."

VERSION=8.5.3 # published 2026-10-07
SHA256=711f7741486daef927c7f2b6ac6c1037b830dffc75ebf7d7106173f18fbe8756
URL="https://registry.npmjs.org/js-dos/-/js-dos-$VERSION.tgz"
OUT=vendor/js-dos

if [ -f "$OUT/PROVENANCE.txt" ] && grep -q "sha256 $SHA256" "$OUT/PROVENANCE.txt"; then
  echo "js-dos $VERSION: already in $OUT"
  exit 0
fi

GPL="$(ls vendor/dosbox/*/doc/licenses/GPL-2.0.txt 2> /dev/null | head -1 || true)"
[ -n "$GPL" ] || { echo "fetch-jsdos: no GPL-2.0 text in vendor/dosbox; run tools/setup.sh" >&2; exit 1; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/jsdos.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
curl -fsSL -o "$WORK/js-dos.tgz" "$URL"
echo "$SHA256  $WORK/js-dos.tgz" | sha256sum -c --quiet - || { echo "fetch-jsdos: checksum mismatch for $URL" >&2; exit 1; }
mkdir "$WORK/pkg"
tar -xzf "$WORK/js-dos.tgz" -C "$WORK/pkg"
SRC="$WORK/pkg/package/dist"

rm -rf "$OUT"
mkdir -p "$OUT/emulators"
cp "$SRC/js-dos.js" "$SRC/js-dos.css" "$OUT/"
# The DOSBox backend and the zip reader are what the page loads; the DOSBox-X backend is kept for the comparison runs.
for f in emulators.js wdosbox.js wdosbox.wasm wlibzip.js wlibzip.wasm wdosbox-x.js wdosbox-x.wasm; do
  cp "$SRC/emulators/$f" "$OUT/emulators/$f"
done
cp "$GPL" "$OUT/LICENSE-GPL-2.0.txt"
cat > "$OUT/PROVENANCE.txt" << TXT
js-dos $VERSION (GPL-2.0), https://js-dos.com, source: https://github.com/caiiiycuk/js-dos

Unpacked unmodified from the npm package js-dos@$VERSION ($URL, sha256 $SHA256):
  dist/js-dos.js, dist/js-dos.css, and from dist/emulators: emulators.js, wdosbox.js, wdosbox.wasm, wlibzip.js, wlibzip.wasm,
  wdosbox-x.js, wdosbox-x.wasm.

The licence is the GNU General Public License, version 2 (LICENSE-GPL-2.0.txt). The corresponding source is available from the
project's repository above.
TXT
du -sh "$OUT"
