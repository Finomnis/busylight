# Busylight

<p>
<img src=".media/busylight_green.webp" alt="Green" width="270">
<img src=".media/busylight_yellow.webp" alt="Green" width="270">
<img src=".media/busylight_red.webp" alt="Green" width="270">
</p>

An indicator that shows coworkers if you are busy.

Intended to be mounted to your monitor and updated whenever your current state changes.

### Color Meanings
- 🟢 **Green** - Can be talked to casually.
- 🟡 **Yellow** - Concentrated, only talk to about the current project.
- 🔴 **Red** - In the zone, do not talk to unless absolutely necessary.

### Controls

- Button:
    - Short press: Switch between colors
    - Long press: Turn on/off
- USB:
    - USB-HID device that can be used to set the color and switch it on/off
    - Turns off when the USB data connection stops (e.g. the PC is shut down).
        - Intended for monitors that power the device constantly, even if the PC is off

## Parts

- Table Tennis Ball as light diffusor
    - For best experience, choose seamless balls
- [Custom PCB](https://oshwlab.com/finomnis/busylight)
    - Orderable on JLCPCB for ~60 Euros / 10 PCBs
- 3D Printed Case
    - coming soon
- LEDs: BTF-LIGHTING 144LEDs/m WS2812B
    - German Amazon: https://amzn.eu/d/06CgZDho
    - 3 LEDs needed per device

## Firmware

The newest version can be found in the releases page.

The bootloader has to be installed once from the `.hex` file using `JFlash` or

```shell
probe-rs download --binary-format hex busylight.bootloader.hex
```

Afterwards, the application can be flashed and updated fia the `.dfu` file:

```shell
dfu-util --download busylight.dfu
```

If DFU flashing fails, enter bootloader recovery mod by keeping the button pressed while plugging in the device. Then it should react to `dfu-util` again.

**NOTE:** It is normal that `dfu-util` ends with an error message like so:
```
Download        [=========================] 100%        90672 bytes
Download done.
dfu-util: unable to read DFU status after completion (LIBUSB_ERROR_IO)
```

This is a technical limitation that might get fixed in the future, but it's purely cosmetic.

## Build Instructions

Soon.

## Host software

The device can be controlled from the PC using the following programs:

- `busylight` - A tray icon with a menu
- `busylight-cli` - A command line tool

For Windows, precompiled executables can be found in the releases page.

For Linux / MacOS, the `software` directory contains the source code
that can be compiled as usual:

```shell
cd software
cargo build --release
```

MacOS is currently untested.
