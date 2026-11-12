MEMORY { FREE (rx) : ORIGIN = 0x17400, LENGTH = 0x2000 }
SECTIONS
{
  .hook24 0xED36 : { KEEP(*(.hook24)) }
  .hookw  0xFC54 : { KEEP(*(.hookw)) }
  .hookframe 0x1054E : { KEEP(*(.hookframe)) }
  .text : { *(.text.stubs) *(.text*) *(.rodata*) } > FREE
  .data : { *(.data*) *(.bss*) *(COMMON) } > FREE
  /DISCARD/ : { *(.ARM.*) *(.comment) }
}
ASSERT(SIZEOF(.data) == 0, "patch must not use RAM globals")
