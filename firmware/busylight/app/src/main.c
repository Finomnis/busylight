#include <zephyr/kernel.h>
#include <zephyr/linker/linker-defs.h>

int main(void)
{
    printk("Address of sample %p\n", (void *)__rom_region_start);
    printk("Hello sysbuild with mcuboot! %s\n", CONFIG_BOARD);
    return 0;
}
