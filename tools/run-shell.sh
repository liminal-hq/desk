#!/bin/bash
# Opens the first window in an emulator window on your desktop, so you can move the pointer, click, drag, type and press Alt+Tab.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/run-shell.sh native [--depth 8|4] [--display gtk|sdl] [--demo]
#        tools/run-shell.sh dos    [--depth 8|4] [--nomouse] [--demo]
#
#   native   the Rust kernel's `shell-live` mode in QEMU as a 486, on its PS/2 keyboard and mouse (needs qemu-system-i386 on the host;
#            the Docker route of tools/native-run.sh is headless)
#   dos      SHELL.EXE in DOSBox Staging from vendor/, on IRQ 1 and the INT 33h mouse (tools/setup.sh puts DOSBox in vendor/)
#   --depth  8 for 256 colours (the default) or 4 for 16 colours
#   --display  native only: QEMU's display, gtk (the default, if QEMU has it) or sdl
#   --nomouse  dos only: no mouse driver, keyboard only
#   --demo   plays tests/shell/live.scn (native: the pointer and the keys) or live-keys.scn (dos: the keys) into the window, then leaves it to you
#
# A click in the window gives the emulator the pointer and the keyboard; the keys that give them back are printed before it starts.
# Escape ends the session (the terminal shows a summary line). Closing the window quits the emulator.
set -euo pipefail
cd "$(dirname "$0")/.."

usage() { sed -n '/^# usage/,/^set -/p' "$0" | sed '/^set -/d; s/^# \{0,1\}//' >&2; exit 2; }
stop() { echo "run-shell: $*" >&2; exit 1; }

[ $# -ge 1 ] || usage
track=$1
shift
depth=8 display="" nomouse="" demo=""
while [ $# -gt 0 ]; do
  case "$1" in
    --depth) [ $# -ge 2 ] || usage; depth=$2; shift 2 ;;
    --display) [ $# -ge 2 ] || usage; display=$2; shift 2 ;;
    --nomouse) nomouse=1; shift ;;
    --demo) demo=1; shift ;;
    *) usage ;;
  esac
done
case "$track" in native | dos) ;; *) usage ;; esac
case "$depth" in 8 | 4) ;; *) stop "--depth is 8 or 4, not $depth" ;; esac
[ "$track" = native ] || [ -z "$display" ] || stop "--display is for the native track"
[ "$track" = dos ] || [ -z "$nomouse" ] || stop "--nomouse is for the dos track"
case "$display" in "" | gtk | sdl) ;; *) stop "--display is gtk or sdl, not $display" ;; esac

# A window needs a desktop to open on.
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || stop "no desktop session (neither DISPLAY nor WAYLAND_DISPLAY is set), so there is nowhere to open a window"

frames() { python3 -I tools/frames.py "$@"; }
work="$(mktemp -d "${TMPDIR:-/tmp}/desk-run-shell.XXXXXX")"
trap 'rm -rf "$work"' EXIT

run_native() {
  command -v qemu-system-i386 > /dev/null || stop "qemu-system-i386 is not installed on this machine; a window needs the host's QEMU (the Docker route stays headless)"
  local modules
  modules="$(qemu-system-i386 -display help 2> /dev/null)"
  if [ -z "$display" ]; then
    case "$modules" in
      *gtk*) display=gtk ;;
      *sdl*) display=sdl ;;
      *) stop "this QEMU has neither the gtk nor the sdl display (some distributions package them separately, as qemu-ui-gtk and qemu-ui-sdl); install one" ;;
    esac
  else
    case "$modules" in *"$display"*) ;; *) stop "this QEMU has no $display display (see: qemu-system-i386 -display help)" ;; esac
  fi
  local video card
  if [ "$depth" = 8 ]; then video=vbe card=std; else video=text card=cirrus; fi
  echo "run-shell: building the kernel"
  tools/native-build.sh > /dev/null
  local image="dist/native-shell-live-$depth.img"
  tools/native-image.sh --test-mode shell-live --video "$video" --out "$image" > /dev/null
  local toolkit=GTK
  [ "$display" = gtk ] || toolkit=SDL
  cat <<HOTKEYS
QEMU ($toolkit): click in the window to give it the pointer and the keyboard. Press Ctrl+Alt+G to give them back.
            Alt+Tab, Alt+F4 and Tab reach the first window while the keyboard is grabbed.
HOTKEYS
  echo "Escape ends the session. Closing the window quits QEMU. The kernel's transcript scrolls below."
  local args=(--window "$display" --timeout 0 --vga "$card")
  if [ -n "$demo" ]; then
    # The live scenario's pointer and key steps, without its screendumps and without the closing Escape, so the session stays yours.
    frames live tests/shell/live.scn --qemu | grep -v '^screendump ' | sed '$d' > "$work/demo.inject"
    args+=(--inject "$work/demo.inject")
  fi
  tools/native-run.sh "${args[@]}" "$image"
}

run_dos() {
  ls vendor/dosbox/*/dosbox > /dev/null 2>&1 || stop "no DOSBox Staging in vendor/; run tools/setup.sh"
  echo "run-shell: building SHELL.EXE"
  shell/dos/build.sh > /dev/null
  cat <<'HOTKEYS'
DOSBox Staging: click in the window to give it the pointer (Ctrl+F10 or the middle button gives it back).
                Alt+Tab and Alt+F4 go to your desktop unless your DOSBox captures system shortcuts; click a window with the mouse instead.
Escape ends the session and closes DOSBox; the terminal then shows the summary line. Ctrl+F9 kills DOSBox.
HOTKEYS
  local conf=""
  [ -z "$nomouse" ] || conf=$'[mouse]\nbuiltin_dos_mouse_driver = off'
  local keys=""
  if [ -n "$demo" ]; then
    # The keyboard-only live scenario typed in, without its closing Escape.
    keys="$(frames live tests/shell/live-keys.scn --autotype | sed 's/ esc$//')"
  fi
  DOS_WINDOW=1 DOS_TIMEOUT=0 DOS_CONF="$conf" DOS_AUTOTYPE="$keys" tools/dosrun.sh dist/shell SHELL.EXE "depth$depth"
}

if [ "$track" = native ]; then run_native; else run_dos; fi
