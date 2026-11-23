ENTRY(entry)
SECTIONS {
  . = 0;
  .text : { KEEP(*(.text.entry)) *(.text .text.*) *(.rodata .rodata.*) }
  /DISCARD/ : { *(.eh_frame*) *(.comment) *(.note*) }
}
