; A one-sector boot program that executes a single instruction a 486 does not have, to show whether an emulator's CPU model rejects it.
;
; (c) Copyright 2026 Liminal HQ, Scott Morris
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; Build: nasm -f bin -DOPC='db 0x0f, 0x31' tests/native/probe.asm -o probe.bin   (OPC is the instruction, here RDTSC)
; It points the invalid-opcode vector at a handler, runs OPC, and prints "probe: #UD" if the CPU raised the exception or
; "probe: executed" if not, then "boot complete" so tools/native-run-bochs.sh ends the run. The serial port is set to 8N1 first:
; Bochs honours the word length (the reset value is 5 bits), QEMU does not.
bits 16
org 0x7c00
        cli
        xor ax, ax
        mov ds, ax
        mov es, ax
        mov ss, ax
        mov sp, 0x7000
        mov word [6 * 4], ud
        mov word [6 * 4 + 2], 0
        mov dx, 0x3fb                   ; line control: 8 data bits, no parity, 1 stop bit
        mov al, 0x03
        out dx, al
        OPC
        mov si, msg_ok
        jmp done
ud:     mov si, msg_ud
done:   cld
        mov sp, 0x7000
.next:  lodsb
        test al, al
        jz .halt
        mov bl, al
        mov dx, 0x3fd
.wait:  in al, dx
        test al, 0x20                   ; transmitter holding register empty
        jz .wait
        mov dx, 0x3f8
        mov al, bl
        out dx, al
        jmp .next
.halt:  hlt
        jmp .halt
msg_ok: db 'probe: executed', 10, 'boot complete', 10, 0
msg_ud: db 'probe: #UD', 10, 'boot complete', 10, 0
times 510 - ($ - $$) db 0
dw 0xaa55
