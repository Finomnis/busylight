# Busylight Firmware

## Datasheets:
- [STM32U073CCU6 Datasheet](https://www.st.com/resource/en/datasheet/stm32u073cc.pdf)
- [STM32U0 Reference Manual](https://www.st.com/resource/en/reference_manual/rm0503-stm32u0-series-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)

## Getting Started

Before getting started, make sure you have a proper Zephyr development
environment. Follow the official
[Zephyr Getting Started Guide](https://docs.zephyrproject.org/latest/getting_started/index.html).

### Workspace setup

```shell
# Enter workspace directory
cd <repository-root>/firmware

# Create virtual environment
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
pip install west

# Initialize zephyr workspace
west init --local busylight

# Fetch dependencies
west update

# Install python dependencies
west packages pip --install

# Fetch required SDK
(cd zephyr; west sdk install -t arm-zephyr-eabi x86_64-zephyr-elf aarch64-zephyr-elf)

# Open the app workspace in VSCode
code ./busylight/busylight.code-workspace
```

### Running on target

- Build the code with the VSCode CMake plugin
- Then, run:
  ```bash
  source ../.venv/bin/activate
  west flash
  ```
- Connect to UART to see log/shell.
