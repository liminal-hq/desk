#!/bin/bash
# Everything the CI runs: licence headers, the markdown style and link check, and the font build check (skipped
# where bun is not set up). The DOS build and tests join this script as the code that needs them arrives.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."
step() { printf '\n== %s\n' "$1"; }

step "Licence headers"
tools/check-headers.sh

step "Markdown style and links"
python3 -I tools/check-docs.py

step "Fonts"
if command -v bun >/dev/null && [ -d fonts/node_modules ]; then
  (cd fonts && bun build.ts --check && bun test)
else
  echo "skipped: needs bun and \`bun install\` in fonts/"
fi

printf '\nAll checks passed.\n'
