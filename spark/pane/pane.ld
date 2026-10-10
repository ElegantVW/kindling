ENTRY(_start)

PHDRS
{
    load PT_LOAD FLAGS(7);
}

SECTIONS
{
    . = 0x50000000;

    .text : {
        KEEP(*(.text.entry))
        *(.text .text.*)
    } :load

    .rodata : {
        *(.rodata .rodata.*)
    } :load

    .data : {
        *(.data .data.*)
        *(.got .got.plt)
        *(.data.rel.ro .data.rel.ro.*)
    } :load

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :load

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
        *(.comment)
        *(.gcc_except_table*)
    }
}
