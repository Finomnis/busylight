#![no_std]
#![no_main]

mod panic_handler;
mod uart_logger;

use async_button_rtic::{Button, ButtonConfig, ButtonEvent};
use core::cell::RefCell;
use embassy_boot_stm32::{AlignedBuffer, BlockingFirmwareState, FirmwareUpdaterConfig};
use embassy_stm32::{
    Config, bind_interrupts, dma,
    exti::{self, ExtiInput},
    flash::{Flash, WRITE_SIZE},
    gpio::Pull,
    interrupt,
    mode::Async,
    peripherals,
    rcc::mux::Clk48sel,
    spi,
    spi::Spi,
    time::Hertz,
    usart, usb,
};
use embassy_usb::{UsbDevice, msos};
use embassy_usb_dfu::application::{DfuAttributes, DfuState, usb_dfu};
use log::info;
use static_cell::ConstStaticCell;

bind_interrupts!(struct Irqs {
    DMA1_CHANNEL2_3 =>
        dma::InterruptHandler<peripherals::DMA1_CH2>,
        dma::InterruptHandler<peripherals::DMA1_CH3>;
    EXTI4_15 => exti::InterruptHandler<interrupt::typelevel::EXTI4_15>;
    USB_DRD_FS => usb::InterruptHandler<peripherals::USB>;
});

// This is a randomly generated GUID to allow clients on Windows to find your device.
const DEVICE_INTERFACE_GUIDS: &[&str] = &["{1d58b148-7511-410d-84b5-698f7ee0532b}"];

struct DfuHandler<'d, FLASH: embedded_storage::nor_flash::NorFlash> {
    firmware_state: BlockingFirmwareState<'d, FLASH>,
}

impl<FLASH: embedded_storage::nor_flash::NorFlash> embassy_usb_dfu::application::Handler
    for DfuHandler<'_, FLASH>
{
    fn enter_dfu(&mut self) {
        match self.firmware_state.mark_dfu() {
            Ok(()) => cortex_m::peripheral::SCB::sys_reset(),
            Err(_) => {
                log::error!("failed to mark DFU mode");
            }
        }
    }
}

static CONFIG_DESCRIPTOR: ConstStaticCell<[u8; 256]> = ConstStaticCell::new([0u8; 256]);
static BOS_DESCRIPTOR: ConstStaticCell<[u8; 256]> = ConstStaticCell::new([0u8; 256]);
static MSOS_DESCRIPTOR: ConstStaticCell<[u8; 1024]> = ConstStaticCell::new([0u8; 1024]);
static CONTROL_BUF: ConstStaticCell<[u8; 4096]> = ConstStaticCell::new([0u8; 4096]);
static MAGIC: ConstStaticCell<embassy_boot_stm32::AlignedBuffer<WRITE_SIZE>> =
    ConstStaticCell::new(AlignedBuffer([0; WRITE_SIZE]));

use rtic_monotonics::stm32::prelude::*;
stm32_tim2_monotonic!(Mono, 1_000_000);

type BlockingFlash = Flash<'static, embassy_stm32::flash::Blocking>;

#[rtic::app(
    device = ::embassy_stm32::pac,
    peripherals = false,
    dispatchers = [TIM15_LPTIM3, TIM16, TIM7_LPTIM2] // free IRQs used by RTIC for async software tasks
)]
mod app {

    use super::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        spi: Spi<'static, Async, spi::mode::Master>,
        button: Button<ExtiInput<'static, Async>, Mono>,
        log_handler: uart_logger::UartLogHandler,
        usb_dev: UsbDevice<'static, usb::Driver<'static, peripherals::USB>>,
    }

    #[init(local = [
        flash: Option<embassy_sync::blocking_mutex::NoopMutex<RefCell<BlockingFlash>>> = None,
        dfu_state: Option<DfuState<DfuHandler<'static, embassy_embedded_hal::flash::partition::BlockingPartition<'static, embassy_sync::blocking_mutex::raw::NoopRawMutex, BlockingFlash>>>> = None,
    ])]
    fn init(mut cx: init::Context) -> (Shared, Local) {
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

        let mut uart_config = usart::Config::default();
        uart_config.baudrate = 115200;
        let uart = usart::UartTx::new(p.USART2, p.PA2, p.DMA1_CH2, Irqs, uart_config).unwrap();
        let log_handler = uart_logger::init(uart);
        info!("init");

        let mut spi_config = spi::Config::default();
        spi_config.frequency = Hertz(3_000_000);
        spi_config.mode = spi::MODE_1;

        let spi = Spi::new_txonly(p.SPI1, p.PA1, p.PA7, p.DMA1_CH3, Irqs, spi_config);

        let button = ExtiInput::new(p.PA5, p.EXTI5, Pull::Up, Irqs);

        let button_config = ButtonConfig {
            double_click: 0.millis(),
            ..Default::default()
        };

        let button = Button::new(button, button_config);

        let flash = Flash::new_blocking(p.FLASH);
        let flash = embassy_sync::blocking_mutex::NoopMutex::new(RefCell::new(flash));
        let flash = cx.local.flash.insert(flash);

        let usb_driver = embassy_stm32::usb::Driver::new(p.USB, Irqs, p.PA12, p.PA11);

        let mut config = embassy_usb::Config::new(0x1209, 0xd9d0);
        config.manufacturer = Some("Finomnis");
        config.product = Some("BusyLight");

        let firmware_updater_config = FirmwareUpdaterConfig::from_linkerfile_blocking(flash, flash);
        let magic = MAGIC.take();
        let mut firmware_state =
            BlockingFirmwareState::from_config(firmware_updater_config, &mut magic.0);
        firmware_state.mark_booted().expect("Failed to mark booted");

        let dfu_handler = DfuHandler { firmware_state };
        let dfu_state = DfuState::new(
            dfu_handler,
            DfuAttributes::CAN_DOWNLOAD | DfuAttributes::WILL_DETACH,
            embassy_time::Duration::from_millis(2500),
        );
        let dfu_state = cx.local.dfu_state.insert(dfu_state);

        let config_descriptor = CONFIG_DESCRIPTOR.take();
        let bos_descriptor = BOS_DESCRIPTOR.take();
        let msos_descriptor = MSOS_DESCRIPTOR.take();
        let control_buf = CONTROL_BUF.take();

        let mut builder = embassy_usb::Builder::new(
            usb_driver,
            config,
            config_descriptor,
            bos_descriptor,
            msos_descriptor,
            control_buf,
        );

        // We add MSOS headers so that the device automatically gets assigned the WinUSB driver on Windows.
        // Otherwise users need to do this manually using a tool like Zadig.
        //
        // It seems these always need to be at added at the device level for this to work and for
        // composite devices they also need to be added on the function level (as shown later).
        //
        builder.msos_descriptor(msos::windows_version::WIN8_1, 2);
        builder.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
        builder.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
            "DeviceInterfaceGUIDs",
            msos::PropertyData::RegMultiSz(DEVICE_INTERFACE_GUIDS),
        ));

        usb_dfu(&mut builder, dfu_state, |func| {
            // You likely don't have to add these function level headers if your USB device is not composite
            // (i.e. if your device does not expose another interface in addition to DFU)
            func.msos_feature(msos::CompatibleIdFeatureDescriptor::new("WINUSB", ""));
            func.msos_feature(msos::RegistryPropertyFeatureDescriptor::new(
                "DeviceInterfaceGUIDs",
                msos::PropertyData::RegMultiSz(DEVICE_INTERFACE_GUIDS),
            ));
        });

        let usb_dev = builder.build();

        // Set the ARM SLEEPONEXIT bit to go to sleep after handling interrupts
        // See https://developer.arm.com/docs/100737/0100/power-management/sleep-mode/sleep-on-exit-bit
        cx.core.SCB.set_sleeponexit();

        led_control_loop::spawn().unwrap();
        log_handler::spawn().unwrap();
        usb_handler::spawn().unwrap();

        (
            Shared {},
            Local {
                spi,
                button,
                log_handler,
                usb_dev,
            },
        )
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

    #[task(priority = 1, local = [log_handler])]
    async fn log_handler(ctx: log_handler::Context) {
        let log_handler = ctx.local.log_handler;
        log_handler.run().await;
    }

    #[task(priority = 3, local = [usb_dev])]
    async fn usb_handler(ctx: usb_handler::Context) {
        let usb_dev = ctx.local.usb_dev;
        usb_dev.run().await;
    }

    #[task(priority = 2, local = [spi, button])]
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
            info!(
                "{} | {}: Color: {:?}",
                Mono::now(),
                embassy_time::Instant::now().as_millis(),
                color
            );
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
