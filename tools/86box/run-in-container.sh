#!/bin/bash
# Runs inside the desk-native-86box image: starts a virtual X server and 86Box with the floppy image, captures COM1 through the
# pseudo-terminal 86Box attaches to it, and ends the run when the transcript says the boot finished.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage (from tools/native-run-86box.sh): run-in-container.sh TIMEOUT_SECONDS
# Expects /cfg (this directory) and /vm (a writable work directory holding disk.img). Writes /vm/serial.txt, /vm/screen.png and
# /vm/status (0 success, 1 failure, 2 the emulator died, 124 timeout).
set -u
timeout_s=$1
export HOME=/tmp XDG_RUNTIME_DIR=/tmp/xdg QT_QPA_PLATFORM=xcb DISPLAY=:99
mkdir -p "$XDG_RUNTIME_DIR" /vm/nvr && chmod 700 "$XDG_RUNTIME_DIR"
cp /cfg/86box.cfg /vm/86box.cfg
# The BIOS setup data of the machine (CMOS RAM): AMIBIOS defaults with the drive types set, so the BIOS does not stop at its setup
# prompt. See docs/NATIVE-EMULATORS.md for how it was made.
python3 -I -c 'import sys; open("/vm/nvr/ami471.nvr","wb").write(bytes.fromhex(open("/cfg/ami471.nvr.hex").read().strip()))'

Xvfb :99 -screen 0 800x600x24 > /dev/null 2>&1 &
sleep 1
/opt/86box/AppRun -N -P /vm -R /opt/roms -L /vm/86box.log /vm/86box.cfg > /vm/86box.out 2>&1 &
emu=$!

# 86Box opens a pseudo-terminal for COM1 and shows its path in a message box that pauses the machine; open it, then dismiss the box.
pts=""
for _ in $(seq 1 100); do
  pts=$(ls /dev/pts 2> /dev/null | grep -m1 '^[0-9]') && [ -n "$pts" ] && break
  sleep 0.2
done
if [ -z "$pts" ]; then echo 2 > /vm/status; exit 0; fi
stty -F "/dev/pts/$pts" raw -echo
: > /vm/serial.txt
cat "/dev/pts/$pts" >> /vm/serial.txt &
sleep 1
xdotool key Return

status=124
last_size=-1
stable=0
for _ in $(seq 1 $((timeout_s * 5))); do
  sleep 0.2
  if grep -q '^boot complete' /vm/serial.txt; then status=0; break; fi
  if grep -q -E '^(KERNEL PANIC|EXCEPTION 0x|Disk read error|stage2: cannot enable a20|stage2: no memory map)' /vm/serial.txt; then
    size=$(stat -c %s /vm/serial.txt)
    if [ "$size" = "$last_size" ]; then stable=$((stable + 1)); else stable=0; last_size=$size; fi
    if [ "$stable" -ge 5 ]; then status=1; break; fi
  fi
  kill -0 "$emu" 2> /dev/null || { status=2; break; }
done
import -window root /vm/screen.png 2> /dev/null
echo "$status" > /vm/status
