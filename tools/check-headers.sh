#!/bin/bash
# Checks that every source, script and workflow file starts with the licence and copyright header.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-headers.sh
#
# Covers .rs, .c, .h, .py, .sh, .ts, .js, .mjs, .asm (NASM, `;` comments) and workflow YAML. Skips design/ (reference material),
# vendor/, target/, dist/ and node_modules/. Markdown, JSON and other config-only files carry no header.
set -euo pipefail
cd "$(dirname "$0")/.."

bad=0
while IFS= read -r f; do
  if ! head -n 15 "$f" | grep -q 'SPDX-License-Identifier: Apache-2.0 OR MIT'; then
    echo "missing licence header: $f" >&2
    bad=1
  fi
done < <(git ls-files --cached --others --exclude-standard -- \
  '*.rs' '*.c' '*.h' '*.py' '*.sh' '*.ts' '*.js' '*.mjs' '*.asm' '.github/workflows/*.yml' \
  ':!design' ':!vendor' ':!target' ':!dist' ':!node_modules')

[ "$bad" = 0 ] && echo "licence headers: ok"
exit "$bad"
