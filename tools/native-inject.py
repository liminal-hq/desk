#!/usr/bin/env python3
# Sends scripted input to a running QEMU through its human monitor socket, so the native kernel's keyboard and mouse can be tested.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/native-inject.py SOCKET SERIAL_FILE SCRIPT
#
# Script lines (blank lines and lines starting with # are ignored):
#   wait TEXT       wait until TEXT appears in the serial transcript (up to 30 s); the kernel prints "input: ready" when its
#                   handlers are live, and keys sent earlier could be lost
#   sleep SECONDS   pause, so a mouse burst and a key burst do not interleave in the kernel's queues
#   anything else   a monitor command (`{out}` in it is replaced by the directory in $DESK_SHOTS, for `screendump {out}/a.ppm`), for example "sendkey shift-a", "mouse_move 10 -5" or "mouse_button 1"
# Each monitor command is sent only after the previous one has been answered. Exit status 0 when the script ran to the end, 1 if
# the socket never appeared, the text never came or the monitor closed early.
import os
import re
import socket
import sys
import time

PROMPT = b"(qemu) "
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def read_prompt(sock):
    data = b""
    while not data.endswith(PROMPT):
        chunk = sock.recv(4096)
        if not chunk:
            raise ConnectionError("the monitor closed the connection")
        data += chunk
    return data


def connect(path, deadline):
    while time.monotonic() < deadline:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            sock.connect(path)
            return sock
        except OSError:
            sock.close()
            time.sleep(0.05)
    raise TimeoutError("the monitor socket never appeared: " + path)


def wait_for(serial_path, text, deadline):
    while time.monotonic() < deadline:
        try:
            with open(serial_path, encoding="utf-8", errors="replace") as f:
                if text in f.read():
                    return
        except OSError:
            pass
        time.sleep(0.05)
    raise TimeoutError("never saw in the serial output: " + text)


def main(socket_path, serial_path, script_path):
    sock = connect(socket_path, time.monotonic() + 30)
    read_prompt(sock)
    for raw in open(script_path, encoding="utf-8"):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("wait "):
            wait_for(serial_path, line[5:], time.monotonic() + 30)
        elif line.startswith("sleep "):
            time.sleep(float(line[6:]))
        else:
            line = line.replace("{out}", os.environ.get("DESK_SHOTS", "."))
            sock.sendall(line.encode() + b"\n")
            reply = read_prompt(sock)
            # The monitor echoes the command; anything else it prints is an error message worth showing.
            text = ANSI.sub("", reply.replace(PROMPT, b"").decode(errors="replace")).replace("\r", "")
            for e in text.strip().splitlines():
                # The monitor's line editor echoes the command, in pieces when it is long; anything else is worth showing.
                if e.strip() and not e.strip().endswith(line.split()[-1]):
                    print("monitor: " + e.strip(), file=sys.stderr)
    sock.close()


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    try:
        main(*sys.argv[1:])
    except (OSError, TimeoutError, ConnectionError) as err:
        print("native-inject: " + str(err), file=sys.stderr)
        sys.exit(1)
