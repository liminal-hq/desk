# The handoff

How the Desk runs a DOS program with the whole machine and comes back.

## Why not just spawn it

DJGPP can spawn a real-mode program from protected mode, but CWSDPMI keeps the memory it has taken, the Desk's interrupt hooks stay in place, and the Desk's VESA mode and palette are left for the program to fight with. Real-mode games like Keen expect conventional memory and a clean machine. Stepping out entirely is simpler and works for everything.

## DESK.COM

A real-mode `.COM`, a few KB of C or assembly, built with NASM or a 16-bit C toolchain (to be confirmed in the handoff spike). Its whole job:

```
loop:
  run C:\DESK\DESKCORE.EXE [/RESUME]
  if C:\DESK\DESK.RUN does not exist: exit to DOS
  read DESK.RUN
  cd to its folder
  if its .DIF says ems=yes: run JEMMEX.EXE LOAD
  run the program with its arguments (INT 21h AH=4Bh)
  if JEMMEX was loaded: run JEMMEX.EXE UNLOAD
  if its pause flag is set: "Press a key to return to the Desk", wait
  goto loop with /RESUME
```

It shrinks its own memory block before each exec, so the program gets all but about 4 KB of conventional memory.

## What the core does before it exits

1. Saves `DESK.INI` and asks each app to save (`app_stop`).
2. Writes `DESK.RUN`.
3. Fades the palette to black through the DAC.
4. Sets text mode 3 and the default palette.
5. Unhooks IRQ 1, the timer and the mouse callback, and silences the OPL2 and the speaker.
6. Exits with code 0.

## What the core does on /RESUME

1. Reads `DESK.RUN`, restores the windows in its `[layout]` and reopens the apps that owned them.
2. Deletes `DESK.RUN`.
3. Sets the video mode and fades the palette in.
4. Shows the "Welcome back" toast (1c).

## Failure cases

| What happens | What the user sees |
|---|---|
| The program is missing | A toast in the Desk before the handoff: "FIZZ.EXE isn't in C:\FIZZ any more." No handoff. |
| The program does not start (exec fails) | DESK.COM restarts the Desk; a toast says why ("Not enough memory: it needs 8 MB.") |
| The program leaves the machine in a graphics mode | The core always sets its own mode on resume |
| The program hangs | Nothing can be done from DOS; on restart `DESK.RUN` is still there, so the Desk resumes normally |
| `DESKCORE.EXE` crashes | DESK.COM sees a non-zero exit and no `DESK.RUN`: it prints one line and returns to DOS |

## Started from Terminal

When Terminal runs a program it passes its window id with the handoff, and `DESK.RUN` keeps it in `[layout]` as `return=<id>`. On `/RESUME` the core sends that window an `open_file` event with `data = 2` (returned), so the tab can print that the program finished and carry on at the same prompt.

## Testing

`dosrun.sh` runs `DESK.COM` with a scripted input file that opens `TONETEST.EXE` from Files; the test passes when the core's second start writes `resumed` and the same layout checksum to `OUT.TXT`.
