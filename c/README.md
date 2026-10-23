# c

The hardware layer, in C99 for DJGPP: VBE 2.0 and VGA mode 12h, the mouse (INT 33h), the keyboard (IRQ 1), the timer, OPL and Sound Blaster sound, and files. One concern per file; anything an interrupt handler touches is `volatile`; no allocation in a handler; every hooked vector is restored on exit.

Today it is the walking skeleton's C side. The hardware pieces arrive with the spike or milestone that needs them.

| File | Concern |
|---|---|
| `main.c` | The entry point: calls `desk_main` in the Rust core and returns the exit status; `DESK.EXE panic` triggers a deliberate Rust panic for the checks |
| `out.c`, `out.h` | `desk_emit`: one result line to stdout and to `OUT.TXT` |
| `panic.c` | `desk_panic`: the Rust panic handler's exit; reports the line and exits with status 3. Where the screen and interrupt vectors will be restored |

`tools/build.sh` compiles them with `i586-pc-msdosdjgpp-gcc -std=c99 -Wall -Wextra -Werror -march=i486` and links them with the converted Rust archive.
