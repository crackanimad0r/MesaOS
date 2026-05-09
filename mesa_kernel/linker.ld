/* mesa_kernel/linker.ld */

OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(kernel_start)

PHDRS
{
    requests PT_LOAD FLAGS(6);  /* rw- : requests de limine */
    text     PT_LOAD FLAGS(5);  /* r-x : código */
    rodata   PT_LOAD FLAGS(4);  /* r-- : solo lectura */
    data     PT_LOAD FLAGS(6);  /* rw- : datos */
}

SECTIONS
{
    . = 0xffffffff80000000;

    .requests : ALIGN(4K)
    {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    } :requests

    .text : ALIGN(4K)
    {
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)
    } :rodata

    .data : ALIGN(4K)
    {
        *(.data .data.*)
    } :data

    .bss : ALIGN(4K)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    } :data

    /DISCARD/ :
    {
        *(.eh_frame*)
        *(.note*)
        *(.comment*)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.interp)
    }
}