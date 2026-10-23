/* Links the kernel for a load address of 1 MiB. The boot loader copies the flat image there and jumps to the first byte, which
   is the `_start` stub in `.text.boot`. */
ENTRY(_start)

SECTIONS
{
    . = 0x100000;
    __kernel_start = .;

    .text : ALIGN(16) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(16) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(16) {
        *(.data .data.*)
    }

    /* Not in the flat image: the entry stub clears it, which is why the end of the image and the start of .bss differ. */
    .bss (NOLOAD) : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        __bss_end = .;
    }

    __kernel_end = ALIGN(4096);

    /DISCARD/ : {
        *(.eh_frame*)
        *(.comment)
        *(.note*)
    }
}
