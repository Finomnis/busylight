#![no_std]
#![no_main]

mod led_statemachine;
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
use embassy_usb::{
    UsbDevice,
    class::hid::{self, HidBootProtocol, HidReaderWriter, HidSubclass},
    msos,
};
use embassy_usb_dfu::application::{DfuAttributes, DfuState, usb_dfu};
use led_statemachine::{LedController, LedEvent};
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

const HID_REPORT_DESCRIPTOR: &[u8] = &[
    0x06, 0x00, 0xff, // Usage Page: vendor-defined 0xff00
    0x09, 0x01, // Usage: 1
    0xa1, 0x01, // Collection: Application
    //
    // Output report: 1 byte, values 0..3.
    0x09, 0x01, // Usage: busy state
    0x15, 0x00, // Logical min 0
    0x25, 0x03, // Logical max 3
    0x75, 0x08, // Report size 8 bits
    0x95, 0x01, // Report count 1
    0x91, 0x02, // Output: Data,Var,Abs
    //
    0xc0, // End collection
];

struct UsbEventHandler {
    queue: rtic_sync::channel::Sender<'static, LedEvent, 10>,
    // Only activate after the PC was connected at least once
    was_already_connected: bool,
    suspended: bool,
    addressed: bool,
    was_on_previously: bool,
}
impl UsbEventHandler {
    pub fn new(queue: rtic_sync::channel::Sender<'static, LedEvent, 10>) -> Self {
        Self {
            queue,
            was_already_connected: false,
            suspended: false,
            addressed: false,
            was_on_previously: false,
        }
    }

    fn update_leds(&mut self) {
        let should_be_on = self.addressed && !self.suspended;

        let mut success = true;

        if self.was_already_connected && (should_be_on != self.was_on_previously) {
            success = if should_be_on {
                self.queue.try_send(LedEvent::ExitSleep)
            } else {
                self.queue.try_send(LedEvent::EnterSleep)
            }
            .is_ok();
        }

        if should_be_on {
            self.was_already_connected = true;
        }
        if success {
            self.was_on_previously = should_be_on;
        }
    }
}
impl embassy_usb::Handler for UsbEventHandler {
    fn suspended(&mut self, suspended: bool) {
        self.suspended = suspended;
        self.update_leds();
    }

    fn addressed(&mut self, _addr: u8) {
        self.addressed = true;
        self.update_leds();
    }

    fn reset(&mut self) {
        self.suspended = false;
        self.addressed = false;
        self.update_leds();
    }
}

#[rtic::app(
    device = ::embassy_stm32::pac,
    peripherals = false,
    dispatchers = [TIM15_LPTIM3, TIM16, TIM7_LPTIM2] // free IRQs used by RTIC for async software tasks
)]
mod app {

    use super::*;

    #[shared]
    struct Shared {
        led_event_sender: rtic_sync::channel::Sender<'static, LedEvent, 10>,
    }

    #[local]
    struct Local {
        led_controller: LedController,
        button: Button<ExtiInput<'static, Async>, Mono>,
        log_handler: uart_logger::UartLogHandler,
        usb_dev: UsbDevice<'static, usb::Driver<'static, peripherals::USB>>,
        hid: HidReaderWriter<'static, usb::Driver<'static, peripherals::USB>, 8, 8>,
    }

    #[init(local = [
        flash: Option<embassy_sync::blocking_mutex::NoopMutex<RefCell<BlockingFlash>>> = None,
        dfu_state: Option<DfuState<DfuHandler<'static, embassy_embedded_hal::flash::partition::BlockingPartition<'static, embassy_sync::blocking_mutex::raw::NoopRawMutex, BlockingFlash>>>> = None,
        hid_state: Option<hid::State<'static>> = None,
        usb_event_handler: Option<UsbEventHandler> = None,
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
        config.serial_number = Some(embassy_stm32::uid::uid_hex());

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

        // --- HID busy-light interface ---
        let hid_state = cx.local.hid_state.insert(hid::State::new());

        let hid_config = hid::Config {
            report_descriptor: HID_REPORT_DESCRIPTOR,
            request_handler: None,
            poll_ms: 10,
            max_packet_size: 8,
            hid_subclass: HidSubclass::No,
            hid_boot_protocol: HidBootProtocol::None,
        };

        let hid = hid::HidReaderWriter::<_, 8, 8>::new(&mut builder, hid_state, hid_config);

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

        let (led_event_sender, led_event_receiver) = rtic_sync::make_channel!(LedEvent, 10);
        let led_controller = LedController::new(spi, led_event_receiver);

        let usb_event_handler = cx
            .local
            .usb_event_handler
            .insert(UsbEventHandler::new(led_event_sender.clone()));

        builder.handler(usb_event_handler);

        let usb_dev = builder.build();

        // Set the ARM SLEEPONEXIT bit to go to sleep after handling interrupts
        // See https://developer.arm.com/docs/100737/0100/power-management/sleep-mode/sleep-on-exit-bit
        cx.core.SCB.set_sleeponexit();

        led_control_loop::spawn().unwrap();
        log_handler::spawn().unwrap();
        usb_handler::spawn().unwrap();
        usb_hid_handler::spawn().unwrap();
        button_handler::spawn().unwrap();

        (
            Shared { led_event_sender },
            Local {
                led_controller,
                button,
                log_handler,
                usb_dev,
                hid,
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

    #[task(priority = 2, local = [usb_dev])]
    async fn usb_handler(ctx: usb_handler::Context) {
        let usb_dev = ctx.local.usb_dev;
        usb_dev.run().await;
    }

    #[task(priority = 2, local = [hid], shared = [&led_event_sender])]
    async fn usb_hid_handler(ctx: usb_hid_handler::Context) {
        let hid = ctx.local.hid;
        let mut event_sender = ctx.shared.led_event_sender.clone();

        let hid_report = &mut [0u8; 8];

        loop {
            hid.ready().await;
            if let Ok(val) = hid.read(hid_report).await
                && val >= 1
            {
                match hid_report[0] {
                    0 => {
                        let _ = event_sender.send(LedEvent::Off).await;
                    }
                    1 => {
                        let _ = event_sender.send(LedEvent::Green).await;
                    }
                    2 => {
                        let _ = event_sender.send(LedEvent::Yellow).await;
                    }
                    3 => {
                        let _ = event_sender.send(LedEvent::Red).await;
                    }
                    _ => (),
                }
            }
        }
    }

    #[task(priority = 2, local = [button], shared = [&led_event_sender])]
    async fn button_handler(ctx: button_handler::Context) {
        let button = ctx.local.button;
        let mut event_sender = ctx.shared.led_event_sender.clone();

        loop {
            match button.update().await {
                ButtonEvent::ShortPress { count: _ } => {
                    let _ = event_sender.send(LedEvent::ShortPress).await;
                }
                ButtonEvent::LongPress => {
                    let _ = event_sender.send(LedEvent::LongPress).await;
                }
            }
        }
    }

    #[task(priority = 3, local = [led_controller])]
    async fn led_control_loop(ctx: led_control_loop::Context) {
        ctx.local.led_controller.run().await;
    }
}
