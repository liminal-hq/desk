| # | Mode | What happens |
|---|---|---|
| 1 | 32-bit | Checks: the guard (an `xchg`) is free; interrupts off; no 8259 in-service bit set; `CR0.PG` clear; the vector table entry for the interrupt is not `0000:0000` |
| 2 | 32-bit | Saves the PIC masks; reads the BIOS tick count and the timer's request bit; masks NMI (`out 0x70, 0x80`) |
| 3 | 32-bit | Puts both 8259s on the BIOS vectors `0x08` and `0x70` with the mask [below](#interrupts-the-pics-the-timer-and-the-keyboard); writes the vector, the flags image and the registers into the control block at `0x10000` |
| 4 | 32-bit | **Entry** (`0x10100`): `pushfd`, `pushad`, `cli`; saves ESP, GDTR, IDTR, CR0, CR3 and the five segment selectors in the blob |
| 5 | 32-bit | `lgdt` the blob's table; loads selector `0x20` into DS, ES, FS, GS and SS; sets ESP to `0x4000`; far jump to selector `0x18` |
| 6 | 16-bit protected | Clears `CR0.PG` and `CR0.PE`; far jump to `0x1000:rm_entry` |
| 7 | real | DS and SS = `0x1000`, SP = `0x4000`, ES, FS and GS = 0; `lidt` with limit `0x3FF`, base 0; reads the handler's far address from the vector table |
| 8 | real | Loads the caller's flags and pushes a copy; loads ES, FS, GS, EBP, ESI, EDI, EDX, ECX, EBX, EAX and last DS from the control block; `call far` to the handler, which returns with `iret` (or `retf 2`) as it would from an `INT` |
| 9 | real | Stores the registers and the flags the handler returned; `cli`, `cld`; `lgdt`; sets `CR0.PE`; far jump to selector `0x08` at `BASE + pm32_back` |
| 10 | 32-bit | Loads the flat data selector everywhere; ESP from the saved value; restores GDTR and IDTR; reloads the saved segment selectors (SS last); restores CR3 and then CR0 (so `CR0.PG` returns last, if it was set) |
| 11 | 32-bit | `popad`, `popfd`, `ret` to Rust |
| 12 | 32-bit | Puts the PICs back on `0x20` and `0x28` with the saved masks; restores NMI to the kernel's resting state (masked) |
| 13 | 32-bit | Adds the ticks the BIOS counted and any undelivered timer tick to the kernel's clock |
| 14 | 32-bit | Hands a keyboard or mouse byte left in the 8042 to the driver's handler by hand; counts the call |
