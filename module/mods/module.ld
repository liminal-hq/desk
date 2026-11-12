/* The linker script of a module and of an application: the layout docs/LOADER.md "Building a module" describes. `.desk.imp` is an
   output section of its own, in the writable segment ahead of `.data`, because the packer finds an import slot by the section its
   symbol is in: slots merged into `.data` are refused. The segments are R E, R and RW from the link base, which is the symbol
   `__desk_link_base` (`--defsym`, set by `link.rs` from `DESK_LINK_BASE`) or 0x400000.

   The entry point and the entry points to keep through `--gc-sections` are not named here but by `link.rs` (`-e`, `-u`), because a
   module keeps `app_start` and, if it has them, `app_event` and `app_stop`, and an application keeps `desk_main`. */
SECTIONS
{
    . = DEFINED(__desk_link_base) ? __desk_link_base : 0x400000;

    .text : { *(.text .text.*) }
    .rodata : ALIGN(4) { *(.rodata .rodata.*) }

    /* The writable part starts on the header's 16-byte alignment, not on a page: a page boundary here padded the image with about
       3.6 KiB of zeros (44% of HELLO.DMD). Page alignment of the writable part comes only once a loader can protect pages (R78),
       and then the packer, not the script, can place it. */
    . = ALIGN(16);
    .desk.imp : ALIGN(4) { KEEP(*(.desk.imp)) }
    .data : ALIGN(4) { *(.data .data.*) }
    .bss : ALIGN(4) { *(.bss .bss.*) *(COMMON) }

    /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note*) }
}
