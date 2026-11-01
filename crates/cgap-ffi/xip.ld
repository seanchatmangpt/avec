/* xip.ld — lane A19 (v26.11.1) libcgap XIP flash-residency linker script.
 *
 * Spec: docs/jira/v26.11.1/lanes/A19-libcgap-ffi-xip.md section 5 (the
 * PROPOSED sketch, reviewed against the ESP32-S3 memory map). Flash-resident
 * text + rodata; SRAM-resident data/bss/stack. Map is ESP32-S3-shaped; the
 * RISC-V variant (ESP32-C3 rv32imc) swaps the flash origin/len per SoC
 * datasheet.
 *
 * Build-level court (sovereign.rs::xip_courts): a link with no .oabi/.bn3
 * input sections MUST fail right here on the ASSERTs — a script whose
 * ASSERTs cannot fire is decoration, so the court runs the section-absent
 * mutant every session and refuses to pass if the link still succeeds.
 */
MEMORY
{
  FLASH (rx)  : ORIGIN = 0x42000020, LENGTH = 4M   /* app partition mmap */
  SRAM  (rwx) : ORIGIN = 0x3FC80000, LENGTH = 320K
}
ENTRY(cgap_xip_start)
SECTIONS
{
  .text : { KEEP(*(.text.cgap_*)) *(.text*) } > FLASH

  /* Sovereign rule tables + networks live in flash, page-aligned (the
   * ESP32-S3 mmap page is 64 KiB; misalignment is a link-time error via the
   * region overflow, never a runtime surprise). */
  .oabi : ALIGN(64K) { KEEP(*(.oabi)) } > FLASH
  .bn3  : ALIGN(64K) { KEEP(*(.bn3))  } > FLASH

  .rodata : { *(.rodata*) *(.srodata*) } > FLASH

  .data : { *(.data*) *(.sdata*) } > SRAM AT>FLASH
  .bss  : { *(.bss*) *(.sbss*) *(COMMON) } > SRAM
  _stack_top = ORIGIN(SRAM) + LENGTH(SRAM);
}
ASSERT(SIZEOF(.oabi) > 0, "cgap XIP: .oabi section absent")
ASSERT(SIZEOF(.bn3) > 0, "cgap XIP: .bn3 section absent")
