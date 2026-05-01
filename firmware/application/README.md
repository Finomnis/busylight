# Busylight Firmware

## Datasheets:
- [STM32U073CCU6 Datasheet](https://www.st.com/resource/en/datasheet/stm32u073cc.pdf)
- [STM32U0 Reference Manual](https://www.st.com/resource/en/reference_manual/rm0503-stm32u0-series-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)

## Getting Started

Before getting started, you need Rust/Cargo installed.

See https://rustup.rs/.

## Compilation

```
cargo build --release
cargo flash --release
```

### Generate hex

```
cargo objcopy --release -- -O ihex app.hex
```
