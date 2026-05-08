use embassy_stm32::{
    mode::Async,
    spi::{self, Spi},
};

#[derive(Debug)]
pub enum LedEvent {
    Red,
    Yellow,
    Green,
    Off,
    On,
    ShortPress,
    LongPress,
}

const NUM_LEDS: usize = 5;
const COLORS: [(u8, u8, u8); 3] = [(0, 255, 0), (255, 100, 0), (255, 0, 0)];
const OFF_COLOR: (u8, u8, u8) = (0, 0, 0);

pub struct LedController {
    spi: Spi<'static, Async, spi::mode::Master>,
    events: rtic_sync::channel::Receiver<'static, LedEvent, 10>,
}

impl LedController {
    pub fn new(
        spi: Spi<'static, Async, spi::mode::Master>,
        events: rtic_sync::channel::Receiver<'static, LedEvent, 10>,
    ) -> Self {
        Self { spi, events }
    }

    async fn set_led(&mut self, color: (u8, u8, u8)) {
        log::info!("Color: {:?}", color);
        let mut data = [0u8; neopixel_spi_encoder::buffer_size(NUM_LEDS)];
        let data = neopixel_spi_encoder::fill_with_color(&mut data, color);
        self.spi.write(data).await.unwrap();
    }

    pub async fn run(&mut self) {
        log::info!("led_control_loop");

        let mut enabled = true;
        let mut color_id = 0;

        loop {
            if enabled && let Some(&color) = COLORS.get(color_id) {
                self.set_led(color).await;
            } else {
                self.set_led(OFF_COLOR).await;
            }

            let event = self.events.recv().await.unwrap();

            match event {
                LedEvent::Red => {
                    enabled = true;
                    color_id = 2;
                }
                LedEvent::Yellow => {
                    enabled = true;
                    color_id = 1;
                }
                LedEvent::Green => {
                    enabled = true;
                    color_id = 0;
                }
                LedEvent::Off => {
                    enabled = false;
                }
                LedEvent::On => {
                    enabled = true;
                }
                LedEvent::ShortPress => {
                    if enabled {
                        color_id = (color_id + 1) % COLORS.len();
                    }
                }
                LedEvent::LongPress => {
                    enabled = !enabled;
                }
            }
        }
    }
}
