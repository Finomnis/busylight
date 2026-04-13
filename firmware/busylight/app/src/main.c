#include <zephyr/device.h>
#include <zephyr/kernel.h>
#include <zephyr/linker/linker-defs.h>
#include <zephyr/logging/log.h>

#include <zephyr/drivers/led_strip.h>

#include <app_version.h>

LOG_MODULE_REGISTER(app, LOG_LEVEL_DBG);

static const struct device *led_strip = DEVICE_DT_GET(DT_ALIAS(rgb_led));

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

    // int success = led_strip_update_rgb(led_strip, &color, 1);

    // if (0 != success) {
    //     LOG_ERR("Failed to set color: %d", success);
    // }

    return 0;
}
