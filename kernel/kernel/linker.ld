OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
  text PT_LOAD FLAGS(5);
  data PT_LOAD FLAGS(6);
}

SECTIONS
{
  . = 0x04000000;
  __kernel_start = .;

  .text : ALIGN(4K)
  {
    KEEP(*(.text._start))
    *(.text*)
  } :text

  .rodata : ALIGN(4K)
  {
    *(.rodata*)
  } :text

  .data : ALIGN(4K)
  {
    *(.data*)
  } :data

  .bss : ALIGN(4K)
  {
    *(.bss*)
    *(COMMON)
  } :data

  . = ALIGN(4K);
  __kernel_end = .;

  /DISCARD/ :
  {
    *(.eh_frame*)
    *(.comment*)
    *(.interp*)
    *(.note*)
  }
}
