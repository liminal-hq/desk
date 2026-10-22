#!/bin/bash
# Builds the native track: the two boot stages with NASM and the Rust kernel with cargo, into dist/native/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-build.sh [--nasm-flags "-DA20_TEST=2"]
#
# Outputs: dist/native/stage1.bin, stage2.bin, kernel.elf (symbols, for gdb) and kernel.bin (the flat image loaded at 1 MiB).
# The kernel uses -Zbuild-std on the pinned stable toolchain, which needs RUSTC_BOOTSTRAP=1 (see kernel/.cargo/config.toml).
set -euo pipefail
cd "$(dirname "$0")/.."

nasm_flags=""
while [ $# -gt 0 ]; do
  case "$1" in
    --nasm-flags) nasm_flags="$2"; shift 2 ;;
    *) echo "usage: tools/native-build.sh [--nasm-flags FLAGS]" >&2; exit 2 ;;
  esac
done

# NASM from the host, or from the QEMU image (tools/docker/Dockerfile.native-qemu) when only Docker is available.
if command -v nasm > /dev/null; then
  run_nasm() { nasm "$@"; }
elif command -v docker > /dev/null && docker image inspect desk-native-qemu:1 > /dev/null 2>&1; then
  run_nasm() { docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work desk-native-qemu:1 nasm "$@"; }
else
  echo "native-build: nasm is not installed (and the desk-native-qemu:1 Docker image has not been built)" >&2
  exit 3
fi

out=dist/native
mkdir -p "$out"

run_nasm -f bin -Werror boot/stage1.asm -o "$out/stage1.bin" -l "$out/stage1.lst"
# shellcheck disable=SC2086
run_nasm -f bin -Werror $nasm_flags boot/stage2.asm -o "$out/stage2.bin" -l "$out/stage2.lst"

(cd kernel && RUSTC_BOOTSTRAP=1 cargo build --release --quiet)
cp kernel/target/i486-desk-none/release/desk-kernel "$out/kernel.elf"
python3 -I tools/elf2bin.py "$out/kernel.elf" "$out/kernel.bin"
