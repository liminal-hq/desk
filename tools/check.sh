#!/bin/bash
# Everything the CI runs: licence headers, the markdown style and link check, and the native boot test. The DOS build and tests
# join this script as the code that needs them arrives.
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

step "Native boot: NASM loader, Rust kernel, QEMU as a 486"
tools/native-test.sh

printf '\nAll checks passed.\n'
