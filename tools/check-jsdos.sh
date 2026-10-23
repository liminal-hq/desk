#!/bin/bash
# Opt-in check of the browser build: runs the DOS programs in js-dos 8 inside headless Chromium and compares what they print, draw
# and play with what they should. Needs the network (js-dos is fetched from the npm registry once), Node 22 or later and a Chromium.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-jsdos.sh [scenario...]        or: JSDOS=1 tools/check.sh
#
#   CHROMIUM=/path/to/chromium   the browser (default: chromium, chromium-browser, google-chrome or google-chrome-stable on PATH)
#   S5_BACKEND=dosboxX           run the scenarios on js-dos's DOSBox-X backend instead of its DOSBox
#
# Not part of the default check or the CI: the ci-dos image carries no browser and the fetch needs the network. `docs/S5-JSDOS.md`
# says what the CI would need. Screenshots and the results go to dist/s5/out/.
set -euo pipefail
cd "$(dirname "$0")/.."
fail() { printf 'check-jsdos: %s\n' "$1" >&2; exit 1; }

command -v node > /dev/null || fail "needs Node 22 or later (a global WebSocket and fetch)"
[ "$(node -p 'process.versions.node.split(".")[0]')" -ge 22 ] || fail "needs Node 22 or later, found $(node --version)"
if [ -z "${CHROMIUM:-}" ]; then
  for c in chromium chromium-browser google-chrome google-chrome-stable; do
    if command -v "$c" > /dev/null; then CHROMIUM="$(command -v "$c")"; break; fi
  done
fi
[ -n "${CHROMIUM:-}" ] && [ -x "$CHROMIUM" ] || fail "no Chromium found; set CHROMIUM=/path/to/chromium"
export CHROMIUM
echo "browser: $("$CHROMIUM" --version)"

spikes/s5-jsdos/fetch-jsdos.sh
spikes/s5-jsdos/build.sh > /dev/null
node spikes/s5-jsdos/drive.mjs "$@"
