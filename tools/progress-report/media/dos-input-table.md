| | Native (`Ps2Input`) | Hosted (`DosInput`) |
|---|---|---|
| Key bytes | IRQ 1 into an `SpscRing<u8, 64>` | the same, by the same `KbdIrq` |
| Decoder | `desk_kbd::Decoder`, starts with no locks | the same, started with the BIOS lock state (`with_locks`) |
| Lock key LEDs | not driven | not driven; the BIOS flags are restored as found |
| Mouse source | IRQ 12 into an `SpscRing<MouseEvent, 32>`, one event per packet | INT 33h polled once per core iteration, one event per poll |
| `dx`, `dy` | the packet's nine-bit counts, `dy` negated | the mickeys, right and down positive |
| `buttons` | bits 0 to 2 of the packet: left, right, middle | bits 0 to 2 of function 0003h: left, right, middle |
| `wheel` | the packet's fourth byte | the wheel API's counter, 0 without it |
| No mouse | the set-up reports it and skips | `has_mouse` is false |
| End-of-interrupt | the platform, after the handler | the platform, after the handler |
| A full ring | drops and counts | drops and counts; the decoder is reset after the drain |
