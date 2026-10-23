; Stage 1: the 512-byte boot sector. Loads stage 2 and the kernel from the floppy with BIOS INT 13h, then jumps to stage 2.
;
; (c) Copyright 2026 Liminal HQ, Scott Morris
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; The BIOS reads this sector to 0000:7C00 and jumps here with DL = boot drive. We are in 16-bit real mode and nothing else is
; set up, so the first jobs are the segment registers and a stack.
;
; Floppy layout (LBA, 512-byte sectors; a 1.44 MB disk is 80 cylinders x 2 heads x 18 sectors per track):
;     0          this sector
;     1..17      stage 2 (the rest of the first track; the image tool pads it to 17 sectors)
;     18..       the flat kernel image (starts on the second track so that every read below is a whole track)
;
; The image tool patches the kernel's length in sectors into KERNEL_SECTORS (offset 3), so this file never changes when the
; kernel grows.

bits 16
org 0x7C00

STAGE2_SEG      equ 0x07E0              ; stage 2 is loaded at 0000:7E00, straight after this sector
STAGE2_LBA      equ 1
STAGE2_SECTORS  equ 17
KERNEL_LBA      equ 18
KERNEL_SEG      equ 0x1000              ; the kernel is read to physical 0x10000 (below the 1 MiB mark that real mode can reach)
SECTORS_PER_TRACK equ 18
HEADS           equ 2
RETRIES         equ 5

start:
        jmp short entry
        nop
KERNEL_SECTORS: dw 0                    ; offset 3: patched by tools/native-image.sh

entry:
        ; Make CS, DS, ES and SS agree on segment 0 (some BIOSes enter as 07C0:0000), and put the stack just below this sector.
        cli
        xor ax, ax
        mov ds, ax
        mov es, ax
        mov ss, ax
        mov sp, 0x7C00
        sti
        cld
        jmp 0x0000:.flushed             ; far jump to normalise CS
.flushed:
        mov [boot_drive], dl            ; the BIOS tells us which drive we came from; INT 13h needs it back
        mov si, msg_boot
        call print

        ; Stage 2 first, so that its checks and messages are available as early as possible.
        mov ax, STAGE2_SEG
        mov es, ax
        mov ax, STAGE2_LBA
        mov cx, STAGE2_SECTORS
        call read_sectors

        ; Then the kernel into the low buffer; stage 2 moves it above 1 MiB once the CPU is in protected mode.
        mov ax, KERNEL_SEG
        mov es, ax
        mov ax, KERNEL_LBA
        mov cx, [KERNEL_SECTORS]
        test cx, cx
        jz fail_image                   ; an unpatched sector means the image tool did not run
        call read_sectors

        mov dl, [boot_drive]            ; stage 2 receives the drive in DL
        jmp 0x0000:0x7E00

; read_sectors: reads CX sectors starting at LBA AX into ES:0, advancing ES as it goes.
; Reads go up to the end of the current track in one call (a floppy then streams the whole track in one rotation) but never
; past a 64 KiB physical boundary, which the floppy DMA controller cannot cross.
read_sectors:
.next:
        test cx, cx
        jz .done
        push ax
        push cx
        ; CHS from LBA: sector = LBA mod 18 + 1, head = (LBA / 18) mod 2, cylinder = LBA / 36.
        xor dx, dx
        mov bx, SECTORS_PER_TRACK
        div bx                          ; AX = LBA / 18 (track number), DX = LBA mod 18
        inc dx
        mov [chs_sector], dl
        xor dx, dx
        mov bx, HEADS
        div bx                          ; AX = cylinder, DX = head
        mov [chs_cyl], al
        mov [chs_head], dl
        ; Sectors left on this track.
        mov al, SECTORS_PER_TRACK + 1
        sub al, [chs_sector]
        xor ah, ah
        ; Sectors that fit before the next 64 KiB physical boundary: (0x1000 - (ES & 0xFFF)) / 32.
        mov bx, es
        and bx, 0x0FFF
        mov dx, 0x1000
        sub dx, bx
        shr dx, 5
        cmp ax, dx
        jbe .a
        mov ax, dx
.a:     pop cx
        cmp ax, cx
        jbe .b
        mov ax, cx
.b:     mov [chunk], al
        push cx                         ; (the caller's count again, for after the read)
        call read_chunk
        pop cx
        pop ax
        movzx dx, byte [chunk]
        add ax, dx                      ; next LBA
        sub cx, dx
        shl dx, 5                       ; 32 paragraphs per sector
        mov bx, es
        add bx, dx
        mov es, bx
        push ax
        mov al, '.'                     ; one dot per chunk as a progress indicator (putc clobbers AX, which holds the LBA)
        call putc
        pop ax
        jmp .next
.done:  ret

; read_chunk: INT 13h AH=02 for [chunk] sectors at the stored CHS into ES:0, with retries and a controller reset between tries.
read_chunk:
        mov di, RETRIES
.try:
        mov ah, 0x02
        mov al, [chunk]
        mov ch, [chs_cyl]
        mov cl, [chs_sector]            ; cylinders above 255 would need the high bits in CL; a 1.44 MB disk has 80
        mov dh, [chs_head]
        mov dl, [boot_drive]
        xor bx, bx
        int 0x13
        jnc .ok
        xor ax, ax                      ; AH=00: reset the disk system, then try again
        mov dl, [boot_drive]
        int 0x13
        dec di
        jnz .try
        mov si, msg_read
        jmp die
.ok:    ret

fail_image:
        mov si, msg_image
die:
        call print
        mov al, 0x11                    ; ends a QEMU run with a failure status (isa-debug-exit); no effect elsewhere
        out 0xF4, al
.hang:  cli
        hlt
        jmp .hang

; print: writes the zero-terminated string at DS:SI with the BIOS teletype service.
print:
        lodsb
        test al, al
        jz .end
        call putc
        jmp print
.end:   ret

putc:
        mov ah, 0x0E
        xor bx, bx
        int 0x10
        ret

boot_drive:     db 0
chs_sector:     db 0
chs_head:       db 0
chs_cyl:        db 0
chunk:          db 0
msg_boot:       db 'Desk boot', 0
msg_read:       db 13, 10, 'Disk read error', 13, 10, 0
msg_image:      db 13, 10, 'Image not prepared', 13, 10, 0

        times 510 - ($ - $$) db 0
        dw 0xAA55
