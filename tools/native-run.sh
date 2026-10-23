#!/bin/bash
# Boots a floppy image in headless QEMU as a 486 and prints the guest's serial output.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-run.sh [--timeout SECONDS] [--serial FILE] [--debug-log FILE] [IMAGE]
#
# IMAGE defaults to dist/desk-native.img. The guest ends the run by writing to the isa-debug-exit device, which makes QEMU exit
# with status (value << 1) | 1: 33 for success and 35 for failure. This script turns that into its own exit status:
#   0    the guest reported success
#   1    the guest reported failure (panic, exception dump, failed assertion)
#   2    QEMU exited some other way (a reset or triple fault with -no-reboot, for example)
#   124  the watchdog timeout ended the run (a hung boot)
#   3    neither QEMU nor Docker is available
# QEMU comes from the host if installed, otherwise from the Docker image built from tools/docker/Dockerfile.native-qemu. Set
# DESK_NATIVE_BACKEND=docker to use Docker even when QEMU is installed.
set -uo pipefail
cd "$(dirname "$0")/.."

timeout_s=20
image=dist/desk-native.img
serial=""
debug_log=""
while [ $# -gt 0 ]; do
  case "$1" in
    --timeout) timeout_s="$2"; shift 2 ;;
    --serial) serial="$2"; shift 2 ;;
    --debug-log) debug_log="$2"; shift 2 ;;
    *) image="$1"; shift ;;
  esac
done
[ -f "$image" ] || { echo "no image: $image (run tools/native-build.sh and tools/native-image.sh)" >&2; exit 3; }

mkdir -p dist
if [ -z "$serial" ]; then
  serial=$(mktemp dist/serial.XXXXXX)
  trap 'rm -f "$serial"' EXIT
fi
: > "$serial"

# The same QEMU arguments everywhere. -cpu 486 keeps the guest to 486 instructions: no CPUID, no TSC, no MMX or SSE.
qemu_args=(-cpu 486 -m 16 -drive "if=floppy,format=raw,file=$image" -display none -serial "file:$serial"
  -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot -monitor none)
[ -n "$debug_log" ] && qemu_args+=(-d int,cpu_reset -D "$debug_log")

backend=host
if [ "${DESK_NATIVE_BACKEND:-}" = docker ] || ! command -v qemu-system-i386 > /dev/null; then
  backend=docker
fi

if [ "$backend" = host ]; then
  timeout --foreground "$timeout_s" qemu-system-i386 "${qemu_args[@]}"
  status=$?
else
  command -v docker > /dev/null || { echo "neither qemu-system-i386 nor docker is available" >&2; exit 3; }
  tag=desk-native-qemu:1
  if ! docker image inspect "$tag" > /dev/null 2>&1; then
    echo "building the $tag image (once)" >&2
    docker build -q -t "$tag" -f tools/docker/Dockerfile.native-qemu tools/docker > /dev/null || exit 3
  fi
  # The repository is mounted at the same path inside, so the relative paths above stay valid; the timeout runs in the
  # container so a hung guest cannot outlive it.
  docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work" -w /work "$tag" \
    timeout --foreground "$timeout_s" qemu-system-i386 "${qemu_args[@]}"
  status=$?
fi

cat "$serial"
case "$status" in
  33) echo "native-run: guest reported success" >&2; exit 0 ;;
  35) echo "native-run: guest reported failure" >&2; exit 1 ;;
  124) echo "native-run: timed out after ${timeout_s}s" >&2; exit 124 ;;
  *) echo "native-run: QEMU exited with status $status without a guest exit code" >&2; exit 2 ;;
esac
