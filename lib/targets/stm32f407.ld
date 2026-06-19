/* STM32F407VG linker script — minimal embedded layout */
MEMORY
{
    FLASH (rx)  : ORIGIN = 0x08000000, LENGTH = 0x00100000
    SRAM  (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00020000
    CCMRAM (rw) : ORIGIN = 0x10000000, LENGTH = 0x00010000
}

SECTIONS
{
    .vectors : ALIGN(4)
    {
        LONG(ORIGIN(SRAM) + LENGTH(SRAM));
        LONG(_reset + 1);
        FILL(0)
        . = 0x400;
    } > FLASH

    .text : ALIGN(4)
    {
        *(.text*)
        *(.rodata*)
    } > FLASH

    .data : ALIGN(4)
    {
        _sdata = .;
        *(.data*)
        _edata = .;
    } > SRAM AT > FLASH

    .bss : ALIGN(4)
    {
        _sbss = .;
        *(.bss*)
        _ebss = .;
    } > SRAM

    _stack_top = ORIGIN(SRAM) + LENGTH(SRAM);
}
