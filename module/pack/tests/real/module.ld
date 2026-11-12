/* The linker script for a module: the layout docs/LOADER.md "Building a module" describes. `.desk.imp` is an output section of its
   own, in the writable segment ahead of `.data`, because the packer finds an import slot by the section its symbol is in: slots
   merged into `.data` are refused. The `EXTERN` lines keep the optional entry points through `--gc-sections`; the test removes them
   for the module that has none. */
ENTRY(app_start)
EXTERN(app_event)
EXTERN(app_stop)

SECTIONS
{
    . = 0x400000;

    .text : { *(.text .text.*) }
    .rodata : ALIGN(4) { *(.rodata .rodata.*) }

    . = ALIGN(0x1000);
    .desk.imp : ALIGN(4) { KEEP(*(.desk.imp)) }
    .data : ALIGN(4) { *(.data .data.*) }
    .bss : ALIGN(4) { *(.bss .bss.*) *(COMMON) }

    /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note*) }
}
