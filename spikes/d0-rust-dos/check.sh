#!/bin/bash
# The d0-rust-dos spike's own checks: the runtime's example program linked a second way, by DJGPP's ld, and run against the same transcript.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/d0-rust-dos/check.sh        (after tools/setup.sh, and after dos/check.sh, which builds dist/d0/ and the DPMI host in it)
#
# The runtime, its example programs and their transcripts are in dos/ (docs/LAYERS.md); only stage 1, which links one of them through
# DJGPP's ld and needs the C-side tools, is left here.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck source=dos/tools/transcript.sh
. dos/tools/transcript.sh

step "Rust format and lints (spike workspace)"
(cd spikes/d0-rust-dos && cargo fmt --check)
(cd spikes/d0-rust-dos && RUSTC_BOOTSTRAP=1 cargo clippy --release --locked -- -D warnings)

step "Build, stage 1: DJGPP ld with -nostdlib (no gcc, no C library, no crt0)"
spikes/d0-rust-dos/build-stage1.sh

step "HELLO1.EXE (stage 1, linked by DJGPP ld) prints the same transcript"
run dist/d0 HELLO1.EXE
[ "$status" = 0 ] || fail "HELLO1.EXE exited with status $status"
same HELLO1.EXE dos/expected/hello.txt
echo "identical to HELLO.EXE"

printf '\nAll d0-rust-dos checks passed.\n'
