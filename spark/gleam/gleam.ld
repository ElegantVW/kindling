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
    } :load

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :load

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
        *(.comment)
        *(.got .got.plt)
        *(.gcc_except_table*)
    }
}
