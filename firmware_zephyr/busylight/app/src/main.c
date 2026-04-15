#include <zephyr/device.h>
#include <zephyr/kernel.h>
#include <zephyr/linker/linker-defs.h>
#include <zephyr/logging/log.h>

#include <zephyr/drivers/led_strip.h>
#include <zephyr/drivers/spi.h>

#include <app_version.h>

LOG_MODULE_REGISTER(app, LOG_LEVEL_DBG);

static const struct device *led_strip = DEVICE_DT_GET(DT_ALIAS(led_strip));
static const struct device *spi1      = DEVICE_DT_GET(DT_NODELABEL(spi1));

int main(void)
{
    LOG_INF("Busylight Application %s (git: %s)", APP_VERSION_STRING, STRINGIFY(APP_BUILD_VERSION));

    if (!device_is_ready(led_strip)) {
        LOG_ERR("LED strip driver is not ready");
        return 0;
    }

    size_t num_leds = led_strip_length(led_strip);

    LOG_INF("Number of leds: %d", (int)num_leds);

    struct led_rgb color = {
        .r = 100,
        .g = 100,
        .b = 100,
    };

    int success = led_strip_update_rgb(led_strip, &color, 1);

    if (0 != success) {
        LOG_ERR("Failed to set color: %d", success);
    }

    static const struct spi_config spi_cfg = {
        .frequency  = 6000000,
        .operation  = SPI_OP_MODE_MASTER | SPI_TRANSFER_MSB | SPI_WORD_SET(16) | SPI_HALF_DUPLEX,
        .slave      = 0,
        .word_delay = 1,
    };

    static uint8_t pattern[64];

    memset(pattern, 0xAA, sizeof(pattern));

    if (!device_is_ready(spi1)) {
        printk("spi1 not ready\n");
        return 0;
    }

    struct spi_buf tx_buf = {
        .buf = pattern,
        .len = sizeof(pattern),
    };

    struct spi_buf_set tx = {
        .buffers = &tx_buf,
        .count   = 1,
    };

    printk("before spi_write\n");
    int rc = spi_write(spi1, &spi_cfg, &tx);
    printk("after spi_write rc=%d\n", rc);

    return 0;
}
