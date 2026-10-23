; The real-mode trampoline: runs one BIOS software interrupt for the protected-mode kernel, then returns to it.
;
; (c) Copyright 2026 Liminal HQ, Scott Morris
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; Assembled flat (`nasm -f bin`) by tools/native-build.sh, embedded in the kernel image and copied to physical 0x10000 at boot
; (`kernel/src/bios.rs`), because the kernel is linked at 1 MiB, which real mode cannot execute from. Everything in the blob is
; addressed relative to the blob's own start, so it is position independent apart from the one constant BASE below, which the
; kernel's `LOW_BASE` must equal. The low-memory map and the reasons are in docs/N3-BIOS-PATH.md:
;
;   0x10000  this blob: the control block (0x000-0x0FF), the code and its tables (0x100-0xFFF)
;   0x11000  the bounce buffer, 8 KiB, for buffers the BIOS reads and writes (VBE blocks, disk sectors)
;   0x13000  the real-mode stack, 4 KiB; SS = 0x1000, SP starts at 0x4000
;
; What this file does and does not do. It is only the CPU-mode part of a call: save the kernel's CPU state, go back to real mode,
; load the registers, run the BIOS handler, store the registers, return to protected mode, restore the CPU state. The interrupt
; controllers (the PICs back to the BIOS vectors and masks and then back again), NMI and the tick bookkeeping are done by the Rust
; code around the call, in the same critical section, with the shared 8259 driver, so there is one copy of that logic. The caller
; has interrupts off and has checked that paging is off.

BASE            equ 0x10000             ; physical address of the blob
BSEG            equ BASE >> 4           ; the same address as a real-mode segment
STACK_TOP       equ 0x4000              ; SP in segment BSEG: the stack is 0x13000-0x13FFF
SEL_CODE32      equ 0x08                ; flat 4 GiB code, the same selector the kernel uses
SEL_DATA32      equ 0x10                ; flat 4 GiB data, the same selector the kernel uses
SEL_CODE16      equ 0x18                ; 16-bit code, base BASE, limit 0xFFFF
SEL_DATA16      equ 0x20                ; 16-bit data, base BASE, limit 0xFFFF

; The control block at the start of the blob, shared with kernel/src/bios.rs (`RealModeRegs` is the same layout).
CTL_MAGIC       equ 0x00                ; dword "DKB1", checked by the kernel after the copy
CTL_VECTOR      equ 0x04                ; byte: the interrupt number
CTL_FLAGS_IN    equ 0x06                ; word: the flags to enter the handler with (CF, PF, AF, ZF, SF, DF, OF)
CTL_REGS        equ 0x08
R_EAX           equ CTL_REGS + 0
R_EBX           equ CTL_REGS + 4
R_ECX           equ CTL_REGS + 8
R_EDX           equ CTL_REGS + 12
R_ESI           equ CTL_REGS + 16
R_EDI           equ CTL_REGS + 20
R_EBP           equ CTL_REGS + 24
R_ES            equ CTL_REGS + 28
R_DS            equ CTL_REGS + 30
R_FS            equ CTL_REGS + 32
R_GS            equ CTL_REGS + 34
R_FLAGS         equ CTL_REGS + 36       ; word: the flags the handler returned
; CTL_REGS + 40 = 0x30 is the end of the shared part; the entry point is fixed at 0x100.

org 0
bits 32

control:
        dd 0x31424B44                   ; "DKB1"
        times 0x100 - ($ - $$) db 0

; ---------------------------------------------------------------------------------------------------------------------------
; Protected mode, 32-bit: the entry (called with the C calling convention from the kernel, interrupts off) and the way back.
; Run in place at BASE: the kernel's flat segments make BASE + offset the linear address, and paging, if it ever exists, must map
; this megabyte one to one (docs/N3-BIOS-PATH.md, "Paging").

entry:                                  ; offset 0x100
        pushfd
        pushad
        cli
        mov [BASE + saved_esp], esp
        sgdt [BASE + saved_gdtr]
        sidt [BASE + saved_idtr]
        mov eax, cr0
        mov [BASE + saved_cr0], eax
        mov eax, cr3
        mov [BASE + saved_cr3], eax
        mov [BASE + saved_ds], ds
        mov [BASE + saved_es], es
        mov [BASE + saved_fs], fs
        mov [BASE + saved_gs], gs
        mov [BASE + saved_ss], ss
        lgdt [BASE + tramp_gdtr]        ; our table has the kernel's two flat selectors at the same numbers, then the 16-bit pair
        mov ax, SEL_DATA16              ; limit 0xFFFF, byte granularity, 16-bit, writable: what real mode needs the segment caches
        mov ds, ax                      ; to hold after PE is cleared
        mov es, ax
        mov fs, ax
        mov gs, ax
        mov ss, ax
        mov esp, STACK_TOP
        jmp SEL_CODE16:pm16             ; CS gets the 16-bit code descriptor (limit 0xFFFF) too

; The way back, entered from real mode with PE just set and CS still the 16-bit code segment's cache until the far jump lands here.
pm32_back:
        mov ax, SEL_DATA32
        mov ds, ax
        mov es, ax
        mov fs, ax
        mov gs, ax
        mov ss, ax
        mov esp, [BASE + saved_esp]
        lgdt [BASE + saved_gdtr]
        lidt [BASE + saved_idtr]
        movzx eax, word [BASE + saved_fs]
        movzx ecx, word [BASE + saved_gs]
        movzx edx, word [BASE + saved_es]
        movzx ebx, word [BASE + saved_ds]
        movzx esi, word [BASE + saved_ss]
        mov fs, ax
        mov gs, cx
        mov es, dx
        mov ds, bx
        mov ss, si                      ; last, so the stack switch is the final segment change
        mov eax, [BASE + saved_cr3]
        mov cr3, eax                    ; harmless with paging off; with paging on it must be loaded before PG is set
        mov eax, [BASE + saved_cr0]
        mov cr0, eax                    ; PE is in the saved value; PG, if it was set, comes back here, in the identity-mapped blob
        popad
        popfd
        ret

; ---------------------------------------------------------------------------------------------------------------------------
; 16-bit protected mode, then real mode.

bits 16
pm16:                                   ; CS = SEL_CODE16 (base BASE), data segments = SEL_DATA16
        mov eax, cr0
        and eax, 0x7FFFFFFE             ; clear PG (if the kernel ever sets it) and PE
        mov cr0, eax
        jmp BSEG:rm_entry               ; reload CS with a real-mode segment

rm_entry:
        mov ax, BSEG
        mov ds, ax
        mov ss, ax
        mov sp, STACK_TOP
        xor ax, ax
        mov es, ax
        mov fs, ax
        mov gs, ax
        lidt [rm_idtr]                  ; the real-mode vector table: base 0, 256 four-byte vectors
        ; The handler address is read from the vector table and called with a flags word on the stack, which is what an INT does
        ; except that the INT also clears IF and TF; interrupts are already off here.
        movzx bx, byte [CTL_VECTOR]
        shl bx, 2
        mov eax, [es:bx]
        mov [target], eax
        ; Load the registers. The flags are set first (the moves below do not change them) and a copy goes on the stack for the
        ; handler's IRET, so the handler starts with the caller's carry, direction flag and the rest, as it would after an INT. DS goes
        ; last, because the block is addressed through DS until then.
        push word [CTL_FLAGS_IN]
        popf
        pushf
        mov es, [R_ES]
        mov fs, [R_FS]
        mov gs, [R_GS]
        mov ebp, [R_EBP]
        mov esi, [R_ESI]
        mov edi, [R_EDI]
        mov edx, [R_EDX]
        mov ecx, [R_ECX]
        mov ebx, [R_EBX]
        mov eax, [R_EAX]
        mov ds, [cs:R_DS]
        call far [cs:target]            ; the BIOS handler returns with IRET (or RETF 2) and the flags it chose
        mov [cs:R_EAX], eax
        mov [cs:R_EBX], ebx
        mov [cs:R_ECX], ecx
        mov [cs:R_EDX], edx
        mov [cs:R_ESI], esi
        mov [cs:R_EDI], edi
        mov [cs:R_EBP], ebp
        mov [cs:R_ES], es
        mov [cs:R_DS], ds
        mov [cs:R_FS], fs
        mov [cs:R_GS], gs
        pushf
        pop word [cs:R_FLAGS]
        cli
        cld
        mov ax, BSEG
        mov ds, ax
        mov es, ax
        mov ss, ax
        mov sp, STACK_TOP
        lgdt [tramp_gdtr]
        mov eax, cr0
        or al, 1                        ; PE; the full CR0 (with PG) is restored in 32-bit mode
        mov cr0, eax
        jmp dword SEL_CODE32:BASE + pm32_back

; ---------------------------------------------------------------------------------------------------------------------------
; Data

        align 4
saved_esp:      dd 0
saved_cr0:      dd 0
saved_cr3:      dd 0
saved_gdtr:     dw 0
                dd 0
saved_idtr:     dw 0
                dd 0
saved_ds:       dw 0
saved_es:       dw 0
saved_fs:       dw 0
saved_gs:       dw 0
saved_ss:       dw 0
target:         dd 0                    ; offset:segment of the handler, as the vector table holds it
rm_idtr:        dw 0x3FF
                dd 0

        align 8
tramp_gdt:
        dq 0
        dq 0x00CF9A000000FFFF           ; 0x08 code, base 0, 4 GiB, 32-bit
        dq 0x00CF92000000FFFF           ; 0x10 data, base 0, 4 GiB
        dw 0xFFFF, BASE & 0xFFFF        ; 0x18 code, base BASE, limit 0xFFFF, byte granularity, 16-bit
        db (BASE >> 16) & 0xFF, 0x9A, 0x00, BASE >> 24
        dw 0xFFFF, BASE & 0xFFFF        ; 0x20 data, same extent
        db (BASE >> 16) & 0xFF, 0x92, 0x00, BASE >> 24
tramp_gdt_end:
tramp_gdtr:
        dw tramp_gdt_end - tramp_gdt - 1
        dd BASE + tramp_gdt

end:
        times 0x1000 - ($ - $$) db 0    ; one page; an overflow is a build error
