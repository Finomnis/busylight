#![no_std]
#![no_main]

mod panic_handler;

use async_button_rtic::{Button, ButtonConfig, ButtonEvent};
use defmt::info;
use defmt_rtt as _;
use embassy_stm32::{
    Config, bind_interrupts, dma,
    exti::{self, ExtiInput},
    gpio::Pull,
    interrupt,
    mode::Async,
    peripherals,
    rcc::mux::Clk48sel,
    spi,
    spi::Spi,
    time::Hertz,
};

bind_interrupts!(struct Irqs {
    DMA1_CH4_7_DMA2_CH1_5_DMAMUX_OVR => dma::InterruptHandler<peripherals::DMA1_CH5>;
    EXTI4_15 => exti::InterruptHandler<interrupt::typelevel::EXTI4_15>;
});

use rtic_monotonics::stm32::prelude::*;
stm32_tim2_monotonic!(Mono, 1_000_000);

#[rtic::app(
    device = ::embassy_stm32::pac,
    peripherals = false,
    dispatchers = [TIM16] // free IRQs used by RTIC for async software tasks
)]
mod app {

    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        spi: Spi<'static, Async, spi::mode::Master>,
        button: Button<ExtiInput<'static, Async>, Mono>,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        info!("init");

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
                sync_from_usb: true,
            });
            config.rcc.mux.clk48sel = Clk48sel::HSI48;
        }

        let p = embassy_stm32::init(config);

        let tim2_hz = embassy_stm32::rcc::frequency::<embassy_stm32::peripherals::TIM2>().0;
        Mono::start(tim2_hz);

        let mut spi_config = spi::Config::default();
        spi_config.frequency = Hertz(3_000_000);
        spi_config.mode = spi::MODE_1;

        let spi = Spi::new_txonly(p.SPI1, p.PA1, p.PA7, p.DMA1_CH5, Irqs, spi_config);

        let button = ExtiInput::new(p.PA5, p.EXTI5, Pull::Up, Irqs);

        let button_config = ButtonConfig {
            double_click: 0.millis(),
            ..Default::default()
        };

        let button = Button::new(button, button_config);

        // Set the ARM SLEEPONEXIT bit to go to sleep after handling interrupts
        // See https://developer.arm.com/docs/100737/0100/power-management/sleep-mode/sleep-on-exit-bit
        cx.core.SCB.set_sleeponexit();

        led_control_loop::spawn().unwrap();

        (Shared {}, Local { spi, button })
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        info!("idle");
        loop {
            // Wait For Interrupt is used instead of a busy-wait loop
            // to allow MCU to sleep between interrupts
            // https://developer.arm.com/documentation/ddi0406/c/Application-Level-Architecture/Instruction-Details/Alphabetical-list-of-instructions/WFI
            rtic::export::wfi()
        }
    }

    #[task(priority = 1, local = [spi, button])]
    async fn led_control_loop(ctx: led_control_loop::Context) {
        info!("led_control_loop");

        let mut spi = ctx.local.spi;
        let button = ctx.local.button;

        const COLORS: [(u8, u8, u8); 3] = [(0, 255, 0), (255, 100, 0), (255, 0, 0)];
        const OFF_COLOR: (u8, u8, u8) = (0, 0, 0);

        let mut enabled = true;
        let mut color_id = 0;

        const NUM_LEDS: usize = 5;

        async fn set_led(mut spi: impl embedded_hal_async::spi::SpiBus<u8>, color: (u8, u8, u8)) {
            info!("{}: Color: {:?}", Mono::now(), color);
            let mut data = [0u8; neopixel_spi_encoder::buffer_size(NUM_LEDS)];
            let data = neopixel_spi_encoder::fill_with_color(&mut data, color);
            spi.write(data).await.unwrap();
        }

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
}
