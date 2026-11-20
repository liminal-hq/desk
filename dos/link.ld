/* Linker script for 32-bit DOS programs behind DJGPP's stub: two loadable segments (text, data) and .bss, packed by tools/elf2go32.py.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 *
 * The stub copies .text and .data from the file to the image's block and clears .bss; it places the image at offset 0 of the
 * block, so the addresses here are offsets from the block base (not linear addresses). Offset 0 up to 0x1000 is left empty, which
 * gives a null pointer a little room before real code. Each section starts on a 512-byte boundary, like DJGPP's, because the
 * stub reads the file in sectors.
 */
ENTRY(_start)

PHDRS {
  text PT_LOAD FLAGS(5);
  data PT_LOAD FLAGS(6);
}

SECTIONS {
  . = 0x1000;
  __image_start = .;
  .text : {
    *(.text.start)
    *(.text .text.*)
    *(.rodata .rodata.*)
    . = ALIGN(512);
  } :text
  .data : ALIGN(512) {
    *(.data .data.*)
    *(.got .got.plt)
    . = ALIGN(512);
  } :data
  .bss : ALIGN(512) {
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(512);
  } :data
  __image_end = .;
  /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note*) *(.gcc_except_table*) }
}
