#![no_std]
#![no_main]

use async_button::{Button, ButtonConfig, ButtonEvent};
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::Pull;
use embassy_stm32::rcc::mux::Clk48sel;
use embassy_stm32::spi::Spi;
use embassy_stm32::time::Hertz;
use embassy_stm32::{Config, bind_interrupts, dma, interrupt, peripherals, spi};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    DMA1_CH4_7_DMA2_CH1_5_DMAMUX_OVR => dma::InterruptHandler<peripherals::DMA1_CH5>;
    EXTI4_15 => exti::InterruptHandler<interrupt::typelevel::EXTI4_15>;
});

async fn set_led(mut spi: impl embedded_hal_async::spi::SpiBus<u8>, color: (u8, u8, u8)) {
    info!("Color: {:?}", color);
    let mut data = [0u8; 14];
    for (color_pos, &color_val) in [color.1, color.0, color.2].iter().enumerate() {
        let color_encoded = &mut data[(color_pos * 4 + 1)..(color_pos * 4 + 5)];

        fn bit(val: u8, pos: u8) -> u8 {
            return (val >> pos) & 1;
        }

        color_encoded[0] = 0x88 + 0x60 * bit(color_val, 7) + 0x06 * bit(color_val, 6);
        color_encoded[1] = 0x88 + 0x60 * bit(color_val, 5) + 0x06 * bit(color_val, 4);
        color_encoded[2] = 0x88 + 0x60 * bit(color_val, 3) + 0x06 * bit(color_val, 2);
        color_encoded[3] = 0x88 + 0x60 * bit(color_val, 1) + 0x06 * bit(color_val, 0);
    }
    spi.write(&data).await.unwrap();
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = true;
        config.rcc.pll = Some(Pll {
            source: PllSource::HSI, // 16 MHz
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL6, // 16 * 6 = 96 MHz
            divp: None,
            divq: None,
            divr: Some(PllRDiv::DIV2), // 96 / 2 = 48 MHz
        });
        config.rcc.sys = Sysclk::PLL1_R;
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: false,
        });
        config.rcc.mux.clk48sel = Clk48sel::HSI48;
    }

    let p = embassy_stm32::init(config);

    let mut spi_config = spi::Config::default();
    spi_config.frequency = Hertz(3_000_000);
    spi_config.mode = spi::MODE_1;

    let mut spi = Spi::new_txonly(p.SPI1, p.PA1, p.PA7, p.DMA1_CH5, Irqs, spi_config);

    let button = ExtiInput::new(p.PA5, p.EXTI5, Pull::Up, Irqs);
    let mut button = Button::new(button, ButtonConfig::default());

    const COLORS: [(u8, u8, u8); 3] = [(0, 255, 0), (255, 150, 0), (255, 0, 0)];
    const OFF_COLOR: (u8, u8, u8) = (0, 0, 0);

    let mut enabled = false;
    let mut color_id = 0;

    loop {
        if enabled && let Some(&color) = COLORS.get(color_id) {
            set_led(&mut spi, color).await;
        } else {
            set_led(&mut spi, OFF_COLOR).await;
        }
        match button.update().await {
            ButtonEvent::ShortPress { count: _ } => {
                if enabled {
                    color_id = (color_id + 1) % COLORS.len();
                }
            }
            ButtonEvent::LongPress => {
                enabled = !enabled;
            }
        }
    }
}
