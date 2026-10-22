# c

The hardware layer, in C99 for DJGPP: VBE 2.0 and VGA mode 12h, the mouse (INT 33h), the keyboard (IRQ 1), the timer, OPL and Sound Blaster sound, and files. One concern per file; anything an interrupt handler touches is `volatile`; no allocation in a handler; every hooked vector is restored on exit.

Nothing is here yet; each piece arrives with the spike or milestone that needs it.
