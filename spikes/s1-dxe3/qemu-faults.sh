#!/bin/bash
# Runs the CPU-fault scenarios that headless DOSBox cannot run, under QEMU with FreeDOS and CWSDPMI (not part of tools/check.sh).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: spikes/s1-dxe3/qemu-faults.sh        (after spikes/s1-dxe3/build.sh; needs Docker and network access)
#
# DOSBox Staging 0.83 aborts itself ("Pagefault didn't correct page") when a DOS program takes a page fault that CWSDPMI reflects
# to the program, for example a write to address 0, so those faults cannot be tested there. QEMU emulates the whole machine, so
# the page fault reaches CWSDPMI and DJGPP's exception code as it would on hardware. This script boots the FreeDOS 1.4 floppy
# edition (downloaded into a scratch directory, never committed) with HOST.EXE on a second drive and prints what each run wrote
# to OUT.TXT and its exit status. Each scenario is a fresh boot of about 25 seconds.
set -euo pipefail
cd "$(dirname "$0")/../.."
[ -f dist/s1/HOST.EXE ] || { echo "qemu-faults: build first (spikes/s1-dxe3/build.sh)" >&2; exit 1; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/desk-qemu.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

echo "qemu-faults: fetching the FreeDOS 1.4 floppy edition"
mkdir -p "$WORK/dl" "$WORK/fd"
curl -fsSL -o "$WORK/dl/fd.zip" https://download.freedos.org/1.4/FD14-FloppyEdition.zip
python3 -I -c "import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$WORK/dl/fd.zip" "$WORK/fd"
mkdir -p "$WORK/dist"
cp dist/s1/*.DXE dist/s1/HOST.EXE dist/s1/CWSDPMI.EXE "$WORK/dist/"

# Runs inside an Alpine container that has QEMU and mtools: build a boot floppy whose FDAUTO.BAT runs the scenario, boot it with
# the build output as a FAT drive, power off when the batch file ends (or give up after 40 seconds), and print the result.
cat > "$WORK/inner.sh" <<'INNER'
#!/bin/sh
set -e
apk add --no-cache mtools qemu-system-i386 > /dev/null 2>&1
cd /w
for scenario in "fault null catch" "fault null raw" "fault stack catch" "fault stack raw"; do
  rm -rf c; mkdir c; cp dist/* c/
  cp fd/144m/x86BOOT.img boot.img
  printf 'LASTDRIVE=Z\r\nFILES=40\r\nBUFFERS=20\r\nSHELL=\\FREEDOS\\BIN\\COMMAND.COM \\FREEDOS\\BIN /E:2048 /P=\\FDAUTO.BAT\r\n' > fdconfig.sys
  printf '@echo off\r\nSET PATH=A:\\FREEDOS\\BIN\r\nc:\r\nhost.exe %s\r\necho %%errorlevel%% > c:\\rc.txt\r\nfdapm poweroff\r\n' "$scenario" > fdauto.bat
  mdel -i boot.img ::SETUP.BAT ::FDCONFIG.SYS ::FDAUTO.BAT
  mcopy -i boot.img fdconfig.sys ::FDCONFIG.SYS
  mcopy -i boot.img fdauto.bat ::FDAUTO.BAT
  timeout 40 qemu-system-i386 -display none -m 32 -fda boot.img -drive file=fat:rw:c,format=raw,if=ide -boot a -no-reboot > /dev/null 2>&1 || true
  echo "=== HOST.EXE $scenario: exit status $(tr -d '\r\n ' < c/rc.txt 2> /dev/null || echo none)"
  tr -d '\r' < c/out.txt 2> /dev/null | grep -v -E '^  (window|log)>' || echo "(no OUT.TXT)"
done
chown -R "$OWNER" /w # the container runs as root; hand the scratch directory back so it can be removed
INNER
chmod +x "$WORK/inner.sh"
docker run --rm -e OWNER="$(id -u):$(id -g)" -v "$WORK:/w" -w /w alpine:3.20 ./inner.sh
