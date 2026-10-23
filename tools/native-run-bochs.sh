#!/bin/bash
# Boots a floppy image in headless Bochs (built as a 486, in Docker) and prints the guest's serial output.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/native-run-bochs.sh [--timeout SECONDS] [--serial FILE] [--bios legacy|latest|seabios] [IMAGE]
#
# IMAGE defaults to dist/desk-native.img. Bochs has no equivalent of QEMU's isa-debug-exit device, so this script watches the
# serial transcript and ends the run itself: "boot complete" is success; a kernel panic, an exception dump or a loader error line
# is failure, once the transcript has stopped growing for a second. Its exit statuses match tools/native-run.sh:
#   0    the transcript ended with "boot complete"
#   1    the transcript shows a panic, an exception or a loader error
#   2    Bochs exited by itself without either (a triple fault with reset_on_triple_fault=0 does this, for example)
#   124  the watchdog timeout ended the run (a hung boot)
#   3    Docker is not available or the image could not be built
# --bios picks the BIOS: legacy (BIOS-bochs-legacy, the default), latest (BIOS-bochs-latest) or seabios (SeaBIOS 1.13, the BIOS
# QEMU uses, here under Bochs). DESK_BOCHS_BIOS sets the default, for tools/native-test-bochs.sh.
set -uo pipefail
cd "$(dirname "$0")/.."

timeout_s=30
image=dist/desk-native.img
serial=""
bios=${DESK_BOCHS_BIOS:-legacy}
while [ $# -gt 0 ]; do
  case "$1" in
    --timeout) timeout_s="$2"; shift 2 ;;
    --serial) serial="$2"; shift 2 ;;
    --bios) bios="$2"; shift 2 ;;
    *) image="$1"; shift ;;
  esac
done
case "$bios" in
  legacy) bios_file=BIOS-bochs-legacy ;;
  latest) bios_file=BIOS-bochs-latest ;;
  seabios) bios_file=bios.bin-1.13.0 ;;
  *) echo "unknown --bios $bios (legacy, latest or seabios)" >&2; exit 3 ;;
esac
[ -f "$image" ] || { echo "no image: $image (run tools/native-build.sh and tools/native-image.sh)" >&2; exit 3; }
command -v docker > /dev/null || { echo "docker is not available (Bochs runs in a container)" >&2; exit 3; }

tag=desk-native-bochs:1
if ! docker image inspect "$tag" > /dev/null 2>&1; then
  echo "building the $tag image (once; compiles Bochs, a few minutes)" >&2
  docker build -q -t "$tag" -f tools/docker/Dockerfile.native-bochs tools/docker > /dev/null || exit 3
fi

work=$(mktemp -d)
name="desk-bochs-$$-$RANDOM"
cleanup() { docker kill "$name" > /dev/null 2>&1; rm -rf "$work"; }
trap cleanup EXIT
cp "$image" "$work/disk.img"
: > "$work/serial.txt"

# A 486 with 16 MiB, the image in the floppy drive and nothing else to boot from. COM1 is a file. reset_on_triple_fault=0 turns a
# triple fault into a Bochs exit instead of a reboot loop (the counterpart of QEMU's -no-reboot). The panic action is fatal for the
# same reason; the display is the nogui build's.
cat > "$work/bochsrc" << CONFIG
cpu: model=bx_generic, count=1, ips=4000000, reset_on_triple_fault=0
memory: guest=16, host=32
romimage: file=/opt/bochs/share/bochs/$bios_file
vgaromimage: file=/opt/bochs/share/bochs/VGABIOS-lgpl-latest
floppya: 1_44=/work/disk.img, status=inserted
boot: floppy
com1: enabled=1, mode=file, dev=/work/serial.txt
port_e9_hack: enabled=1
log: /work/bochs.log
logprefix: %t%e%d
panic: action=fatal
error: action=report
info: action=ignore
debug: action=ignore
display_library: nogui
mouse: enabled=0
CONFIG

docker run -d --rm --name "$name" --user "$(id -u):$(id -g)" -v "$work:/work" -w /work "$tag" \
  bochs -q -f /work/bochsrc > /dev/null || exit 3

# Watchdog: poll the transcript every 0.2 s.
status=""
last_size=-1
stable=0
deadline=$((SECONDS + timeout_s))
while [ "$SECONDS" -lt "$deadline" ]; do
  sleep 0.2
  if grep -q '^boot complete' "$work/serial.txt"; then status=0; break; fi
  if grep -q -E '^(KERNEL PANIC|EXCEPTION 0x|Disk read error|stage2: cannot enable a20|stage2: no memory map)' "$work/serial.txt"; then
    size=$(stat -c %s "$work/serial.txt")
    if [ "$size" = "$last_size" ]; then stable=$((stable + 1)); else stable=0; last_size=$size; fi
    if [ "$stable" -ge 5 ]; then status=1; break; fi
  fi
  if [ -z "$(docker ps -q -f "name=^$name\$")" ]; then status=2; break; fi
done
[ -n "$status" ] || status=124

if [ -n "$serial" ]; then cp "$work/serial.txt" "$serial"; fi
cat "$work/serial.txt"
case "$status" in
  0) echo "native-run-bochs: guest reported success" >&2 ;;
  1) echo "native-run-bochs: guest reported failure" >&2 ;;
  124) echo "native-run-bochs: timed out after ${timeout_s}s" >&2 ;;
  *) echo "native-run-bochs: Bochs exited without a guest result; see its log:" >&2; tail -n 12 "$work/bochs.log" >&2 ;;
esac
exit "$status"
