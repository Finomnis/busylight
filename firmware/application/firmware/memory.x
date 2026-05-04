MEMORY
{
  BOOTLOADER                        : ORIGIN = 0x08000000, LENGTH = 48K
  BOOTLOADER_STATE                  : ORIGIN = 0x0800C000, LENGTH = 2K
  FLASH                             : ORIGIN = 0x0800C800, LENGTH = 102K
  DFU                               : ORIGIN = 0x08026000, LENGTH = 104K
  RAM                         (rwx) : ORIGIN = 0x20000000, LENGTH = 40K
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE) - ORIGIN(BOOTLOADER);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE) - ORIGIN(BOOTLOADER);

__bootloader_dfu_start = ORIGIN(DFU) - ORIGIN(BOOTLOADER);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU) - ORIGIN(BOOTLOADER);
