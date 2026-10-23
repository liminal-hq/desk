#!/bin/bash
# The native end-to-end check (tools/native-test.sh) run under Bochs as a 486 instead of QEMU: the normal boot, the loader
# variants, the negative checks and the hang, each against the same expected transcripts. First it proves that this Bochs rejects
# instructions a 486 lacks, which QEMU's -cpu 486 lets through.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-test-bochs.sh
#
# Needs Docker (Bochs is built into an image the first time) and NASM. DESK_BOCHS_BIOS=legacy|latest|seabios picks the BIOS (see
# tools/native-run-bochs.sh). Opt-in: tools/check.sh runs it only with NATIVE_EXTRA=1.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v docker > /dev/null; then
  echo "native-bochs: SKIPPED — docker is not installed, so Bochs cannot run"
  exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "== Instructions a 486 lacks must raise #UD"
printf 'probe: #UD\nboot complete\n' > "$work/expected-ud.txt"
# RDTSC and RDMSR (Pentium), EMMS (MMX), a read of CR4 (Pentium) and UD2, which every CPU rejects.
for spec in "rdtsc:db 0x0f, 0x31" "rdmsr:db 0x0f, 0x32" "emms:db 0x0f, 0x77" "mov-from-cr4:db 0x0f, 0x20, 0xe0" "ud2:db 0x0f, 0x0b"; do
  name=${spec%%:*}
  nasm -f bin "-DOPC=${spec#*:}" tests/native/probe.asm -o "$work/probe.bin"
  # A 1.44 MB floppy: the probe in sector 0, zeros after it.
  { cat "$work/probe.bin"; head -c $((1474560 - 512)) /dev/zero; } > "$work/probe.img"
  tools/native-run-bochs.sh --timeout 10 "$work/probe.img" > "$work/probe.txt" 2> "$work/probe.err" \
    || { cat "$work/probe.txt" "$work/probe.err" >&2; echo "native-bochs: FAILED — probe $name did not run to the end" >&2; exit 1; }
  python3 -I tools/native-check.py "$work/expected-ud.txt" "$work/probe.txt" > /dev/null \
    || { cat "$work/probe.txt" >&2; echo "native-bochs: FAILED — $name was executed without #UD" >&2; exit 1; }
  echo "native-bochs: ok — $name raises #UD"
done

DESK_NATIVE_RUNNER=tools/native-run-bochs.sh exec tools/native-test.sh
