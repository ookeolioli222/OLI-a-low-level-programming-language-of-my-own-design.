/* The linker script of tests/c/sect_side.oli: each section it names by
 * its own name, where the script says. */
ENTRY(_start)
SECTIONS
{
    . = 0x800000;
    .boot : { *(.boot) }
    .text : { *(.text) *(.text.*) }
    .rodata : { *(.rodata) }
    . = 0x900000;
    .lowdata : { *(.lowdata) }
    .data : { *(.data) }
    .bss : { *(.bss) }
}
