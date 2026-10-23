; Stage 2: enables A20, reads the memory map, enters 32-bit protected mode and starts the kernel.
;
; (c) Copyright 2026 Liminal HQ, Scott Morris
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; Stage 1 loads this to 0000:7E00 (17 sectors reserved) and the kernel to physical 0x10000, then jumps here in real mode with
; DL = boot drive. The steps, in order, and why:
;
;   1. Serial port and screen output, so every later step can report what it did.
;   2. A20. The 8086 wrapped addresses past 1 MiB; the PC keeps that behaviour until the A20 gate is opened. Without it, every
;      other megabyte of memory would alias the one below it.
;   3. The memory map (E820, else E801). Only the BIOS knows what RAM exists, and it is only reachable in real mode.
;   4. The boot information block at 0x500, which is how the kernel finds the map.
;   5. A GDT and the switch to protected mode with flat 4 GiB segments, so the kernel sees plain 32-bit addresses.
;   6. Copy the kernel from its low buffer to 1 MiB (real mode cannot address that high, so this has to wait), set a stack and
;      jump to the kernel with EAX = the boot information address.
;
; Build flags for testing the less common paths (see tools/native-build.sh): -DA20_TEST=1 closes the A20 gate first so the
; enabling code really runs; -DA20_TEST=2 also skips the fast gate so the keyboard controller method runs; -DNO_E820 pretends
; the BIOS lacks E820 so the E801 fallback runs.

bits 16
org 0x7E00

%ifndef A20_TEST
%define A20_TEST 0
%endif

BOOTINFO        equ 0x0500
MAP             equ 0x0520              ; the map follows the 32-byte header
MAP_MAX         equ 32
KERNEL_LOW      equ 0x10000
KERNEL_ENTRY    equ 0x100000
STACK32         equ 0x7000
STAGE1_KSECTORS equ 0x7C03              ; stage 1 is still in memory, and holds the kernel's length in sectors
COM1            equ 0x3F8

start:
        jmp short entry
        nop
test_mode:      db 0                    ; offset 3: patched by tools/native-image.sh, passed on to the kernel

entry:
        xor ax, ax
        mov ds, ax
        mov es, ax
        mov [boot_drive], dl
        cld
        call serial_init
        mov si, msg_hello
        call print

        ; ---- A20 ----
%if A20_TEST != 0
        in al, 0x92                     ; close the gate so the code below has something to do
        and al, 0xFD
        out 0x92, al
%endif
        call a20_enabled
        jnz .a20_ok_already
%if A20_TEST != 2
        in al, 0x92                     ; fast A20: bit 1 of system control port A opens the gate; bit 0 would reset the CPU
        or al, 2
        and al, 0xFE
        out 0x92, al
        call a20_wait
        jnz .a20_fast
%endif
        call a20_keyboard               ; the fallback for chipsets without port 0x92
        call a20_wait
        jnz .a20_kbc
        mov si, msg_a20
        jmp die
.a20_ok_already:
        mov si, msg_a20_already
        jmp .a20_report
.a20_fast:
        mov si, msg_a20_fast
        jmp .a20_report
.a20_kbc:
        mov si, msg_a20_kbc
.a20_report:
        call print

        ; ---- Memory map ----
        mov dword [BOOTINFO + 0], 0x4B534544   ; "DESK"
        movzx eax, byte [boot_drive]
        mov [BOOTINFO + 4], eax
        movzx eax, byte [test_mode]
        mov [BOOTINFO + 8], eax
        call map_e820
        jnc .map_ok
        call map_e801
        jnc .map_ok
        mov si, msg_map
        jmp die
.map_ok:
        mov si, msg_map_src
        call print
        mov eax, [BOOTINFO + 12]
        cmp eax, 1
        mov si, name_e820
        je .named
        mov si, name_e801
.named: call print
        mov si, msg_map_count
        call print
        mov ax, [BOOTINFO + 16]
        call print_dec
        mov si, msg_crlf
        call print

        ; ---- Protected mode ----
        mov si, msg_pm
        call print
        cli                             ; no interrupt handlers exist for protected mode yet
        lgdt [gdt_ptr]
        mov eax, cr0
        or eax, 1                       ; CR0.PE
        mov cr0, eax
        jmp 0x08:pm_entry               ; far jump: loads CS with the 32-bit code segment and flushes the prefetch queue

; ---------------------------------------------------------------------------------------------------------------------------
; Real-mode helpers

; a20_enabled: ZF clear when the A20 gate is open. Writes different bytes at 0000:0500 and FFFF:0510; these are the same
; physical byte exactly when the address wraps at 1 MiB (A20 closed). The original bytes are put back.
a20_enabled:
        push ds
        push es
        xor ax, ax
        mov ds, ax
        not ax
        mov es, ax
        mov si, 0x0500
        mov di, 0x0510
        mov al, [ds:si]
        mov ah, [es:di]
        push ax
        mov byte [ds:si], 0x00
        mov byte [es:di], 0xFF
        cmp byte [ds:si], 0xFF          ; reads back 0xFF only if both addresses are the same byte
        pop ax
        mov [ds:si], al
        mov [es:di], ah
        pop es
        pop ds
        jne .open
        xor ax, ax                      ; ZF set: closed
        ret
.open:  or al, 1                        ; ZF clear: open
        ret

; a20_wait: re-checks the gate for a short while, since the hardware may take a moment. ZF clear when open.
a20_wait:
        mov cx, 0x4000
.again: call a20_enabled
        jnz .done
        loop .again
        xor ax, ax                      ; ZF set
.done:  ret

; a20_keyboard: opens the gate through the keyboard controller's output port (the PC/AT way, bit 1 of the output port).
a20_keyboard:
        call kbc_wait_input
        mov al, 0xAD                    ; disable the keyboard while we talk to the controller
        out 0x64, al
        call kbc_wait_input
        mov al, 0xD0                    ; read the output port
        out 0x64, al
        call kbc_wait_output
        in al, 0x60
        push ax
        call kbc_wait_input
        mov al, 0xD1                    ; write the output port
        out 0x64, al
        call kbc_wait_input
        pop ax
        or al, 2                        ; A20 gate
        out 0x60, al
        call kbc_wait_input
        mov al, 0xAE                    ; re-enable the keyboard
        out 0x64, al
        call kbc_wait_input
        ret

kbc_wait_input:                         ; until the controller can accept a byte (status bit 1 clear), bounded
        mov cx, 0xFFFF
.w:     in al, 0x64
        test al, 2
        loopnz .w
        ret

kbc_wait_output:                        ; until the controller has a byte for us (status bit 0 set), bounded
        mov cx, 0xFFFF
.w:     in al, 0x64
        test al, 1
        loopz .w
        ret

; map_e820: INT 15h AX=E820 walks the BIOS memory map one 24-byte entry at a time, with EBX as the continuation. The ACPI
; "extended attributes" dword is preset to 1 so BIOSes that return only 20 bytes still look valid. CF set on failure.
map_e820:
%ifdef NO_E820
        stc
        ret
%endif
        xor ebx, ebx
        xor bp, bp                      ; entries stored
        mov di, MAP
.next:
        mov eax, 0xE820
        mov edx, 0x534D4150             ; 'SMAP'
        mov ecx, 24
        mov dword [es:di + 20], 1
        int 0x15
        jc .end                         ; carry on a later call means "no more entries"
        cmp eax, 0x534D4150
        jne .fail
        jcxz .skip                      ; empty reply
        cmp cx, 20
        jbe .keep
        test byte [es:di + 20], 1       ; ACPI 3.0: bit 0 clear means "ignore this entry"
        jz .skip
.keep:
        mov eax, [es:di + 8]            ; ignore zero-length entries
        or eax, [es:di + 12]
        jz .skip
        inc bp
        add di, 24
        cmp bp, MAP_MAX
        jae .end
.skip:
        test ebx, ebx
        jnz .next
.end:
        test bp, bp
        jz .fail
        mov dword [BOOTINFO + 12], 1
        movzx eax, bp
        mov [BOOTINFO + 16], eax
        clc
        ret
.fail:  stc
        ret

; map_e801: the fallback for BIOSes from before E820. INT 15h AX=E801 reports the KiB between 1 and 16 MiB and the 64 KiB
; blocks above 16 MiB. The answer is rewritten as E820-style entries so the kernel parses only one format: conventional
; memory from INT 12h, then the two extended ranges. CF set on failure.
map_e801:
        mov ax, 0xE801
        xor cx, cx
        xor dx, dx
        int 0x15
        jc .fail
        test cx, cx                     ; some BIOSes answer in CX and DX instead of AX and BX
        jz .have
        mov ax, cx
        mov bx, dx
.have:
        test ax, ax
        jz .fail
        mov di, MAP
        push ax
        push bx
        int 0x12                        ; AX = conventional memory in KiB
        movzx eax, ax
        shl eax, 10
        mov dword [di + 0], 0           ; base
        mov dword [di + 4], 0
        mov [di + 8], eax               ; length
        mov dword [di + 12], 0
        mov dword [di + 16], 1          ; usable
        mov dword [di + 20], 1
        add di, 24
        pop bx
        pop ax
        movzx eax, ax
        shl eax, 10                     ; KiB to bytes
        mov dword [di + 0], 0x100000
        mov dword [di + 4], 0
        mov [di + 8], eax
        mov dword [di + 12], 0
        mov dword [di + 16], 1
        mov dword [di + 20], 1
        add di, 24
        mov cx, 2
        test bx, bx
        jz .store
        movzx eax, bx
        shl eax, 16                     ; blocks of 64 KiB to bytes
        mov dword [di + 0], 0x1000000
        mov dword [di + 4], 0
        mov [di + 8], eax
        mov dword [di + 12], 0
        mov dword [di + 16], 1
        mov dword [di + 20], 1
        inc cx
.store: mov dword [BOOTINFO + 12], 2
        movzx eax, cx
        mov [BOOTINFO + 16], eax
        clc
        ret
.fail:  stc
        ret

; Output goes to both the BIOS screen and COM1, so a headless run records the loader too.
serial_init:
        mov dx, COM1 + 1
        xor al, al
        out dx, al                      ; no UART interrupts
        mov dx, COM1 + 3
        mov al, 0x80                    ; DLAB on to set the divisor
        out dx, al
        mov dx, COM1
        mov al, 1                       ; divisor 1 = 115200 baud
        out dx, al
        inc dx
        xor al, al
        out dx, al
        mov dx, COM1 + 3
        mov al, 3                       ; 8N1
        out dx, al
        mov dx, COM1 + 2
        mov al, 0xC7                    ; FIFOs on
        out dx, al
        mov dx, COM1 + 4
        mov al, 0x0B
        out dx, al
        ret

print:                                  ; DS:SI zero-terminated
        lodsb
        test al, al
        jz .end
        call putc
        jmp print
.end:   ret

putc:                                   ; AL
        pusha
        mov ah, 0x0E
        xor bx, bx
        int 0x10
        popa
        pusha
        mov bl, al
        mov cx, 0x4000
        mov dx, COM1 + 5
.wait:  in al, dx
        test al, 0x20
        loopz .wait
        mov dx, COM1
        mov al, bl
        out dx, al
        popa
        ret

print_dec:                              ; AX unsigned
        pusha
        xor cx, cx
        mov bx, 10
.div:   xor dx, dx
        div bx
        push dx
        inc cx
        test ax, ax
        jnz .div
.out:   pop ax
        add al, '0'
        call putc
        loop .out
        popa
        ret

die:
        call print
        mov al, 0x11                    ; ends a QEMU run with a failure status (isa-debug-exit); no effect elsewhere
        out 0xF4, al
.hang:  cli
        hlt
        jmp .hang

boot_drive:     db 0
msg_hello:      db 13, 10, 'stage2: loader running', 13, 10, 0
msg_a20_already: db 'stage2: a20 already enabled', 13, 10, 0
msg_a20_fast:   db 'stage2: a20 enabled by the fast gate', 13, 10, 0
msg_a20_kbc:    db 'stage2: a20 enabled by the keyboard controller', 13, 10, 0
msg_a20:        db 'stage2: cannot enable a20', 13, 10, 0
msg_map:        db 'stage2: no memory map from the BIOS', 13, 10, 0
msg_map_src:    db 'stage2: memory map from ', 0
msg_map_count:  db ', entries: ', 0
msg_pm:         db 'stage2: entering protected mode', 13, 10, 0
msg_crlf:       db 13, 10, 0
name_e820:      db 'E820', 0
name_e801:      db 'E801', 0

; ---------------------------------------------------------------------------------------------------------------------------
; Protected mode

        align 8
gdt:
        dq 0                            ; the null descriptor every GDT starts with
        dq 0x00CF9A000000FFFF           ; 0x08: code, base 0, limit 4 GiB (4 KiB granularity), 32-bit, ring 0, execute/read
        dq 0x00CF92000000FFFF           ; 0x10: data, same extent, read/write
gdt_end:
gdt_ptr:
        dw gdt_end - gdt - 1
        dd gdt

bits 32
pm_entry:
        mov ax, 0x10
        mov ds, ax
        mov es, ax
        mov fs, ax
        mov gs, ax
        mov ss, ax
        mov esp, STACK32
        ; Move the kernel from the low buffer to 1 MiB: sectors * 512 bytes, as dwords.
        movzx ecx, word [STAGE1_KSECTORS]
        shl ecx, 7
        mov esi, KERNEL_LOW
        mov edi, KERNEL_ENTRY
        cld
        rep movsd
        mov eax, BOOTINFO
        mov ebx, KERNEL_ENTRY
        jmp ebx

        times 17 * 512 - ($ - $$) db 0  ; pad to the reserved size; an overflow here is a build error
