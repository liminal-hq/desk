#!/bin/bash
# Boots a floppy image in 86Box (an AMIBIOS 486 machine, headless on a virtual X server in Docker) and prints the guest's serial
# output. Experimental: see docs/NATIVE-EMULATORS.md.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-run-86box.sh [--timeout SECONDS] [--serial FILE] [--screenshot FILE] [IMAGE]
#
# IMAGE defaults to dist/desk-native.img. Like tools/native-run-bochs.sh, it ends the run itself by watching the serial transcript
# and returns the same statuses (0 success, 1 failure, 2 the emulator died, 124 timeout, 3 no Docker or no image). The machine is
# an AMIBIOS 486 board (86Box's "ami471": SiS 471 chipset, a 486DX at 33 MHz, 16 MiB, VGA); the ROM images come from the
# 86Box/roms release and are downloaded into the Docker image, never committed. A boot takes about 20 seconds.
set -uo pipefail
cd "$(dirname "$0")/.."

timeout_s=60
image=dist/desk-native.img
serial=""
screenshot=""
while [ $# -gt 0 ]; do
  case "$1" in
    --timeout) timeout_s="$2"; shift 2 ;;
    --serial) serial="$2"; shift 2 ;;
    --screenshot) screenshot="$2"; shift 2 ;;
    *) image="$1"; shift ;;
  esac
done
[ -f "$image" ] || { echo "no image: $image (run tools/native-build.sh and tools/native-image.sh)" >&2; exit 3; }
command -v docker > /dev/null || { echo "docker is not available (86Box runs in a container)" >&2; exit 3; }

tag=desk-native-86box:1
if ! docker image inspect "$tag" > /dev/null 2>&1; then
  echo "building the $tag image (once; downloads about 170 MB)" >&2
  docker build -q -t "$tag" -f tools/docker/Dockerfile.native-86box tools/docker > /dev/null || exit 3
fi

work=$(mktemp -d)
name="desk-86box-$$-$RANDOM"
cleanup() { docker kill "$name" > /dev/null 2>&1; rm -rf "$work"; }
trap cleanup EXIT
cp "$image" "$work/disk.img"

docker run --rm --name "$name" --user "$(id -u):$(id -g)" -v "$PWD/tools/86box:/cfg:ro" -v "$work:/vm" "$tag" \
  /cfg/run-in-container.sh "$timeout_s" > /dev/null 2>&1 || true
status=$(cat "$work/status" 2> /dev/null || echo 2)

[ -n "$serial" ] && cp "$work/serial.txt" "$serial" 2> /dev/null
[ -n "$screenshot" ] && cp "$work/screen.png" "$screenshot" 2> /dev/null
cat "$work/serial.txt" 2> /dev/null
case "$status" in
  0) echo "native-run-86box: guest reported success" >&2 ;;
  1) echo "native-run-86box: guest reported failure" >&2 ;;
  124) echo "native-run-86box: timed out after ${timeout_s}s" >&2 ;;
  *) echo "native-run-86box: 86Box exited without a guest result; see its log:" >&2; tail -n 12 "$work/86box.log" >&2 ;;
esac
exit "$status"
