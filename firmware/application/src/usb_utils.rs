use core::sync::atomic::Ordering;

use crate::led_statemachine::LedEvent;

const fn str_to_two(s: &str) -> u16 {
    let value = match u16::from_str_radix(s, 10) {
        Ok(value) => value,
        Err(_) => panic!("invalid BCD value"),
    };

    assert!(value <= 99, "BCD value must fit in two decimal digits");

    ((value / 10) << 4) | (value % 10)
}

const fn str_to_one(s: &str) -> u16 {
    let value = match u16::from_str_radix(s, 10) {
        Ok(value) => value,
        Err(_) => panic!("invalid BCD value"),
    };

    assert!(value <= 9, "BCD value must fit in one decimal digits");

    value
}

pub const USB_BCD_DEVICE_VERSION: u16 = (str_to_two(env!("CARGO_PKG_VERSION_MAJOR")) << 8)
    | (str_to_one(env!("CARGO_PKG_VERSION_MINOR")) << 4)
    | str_to_one(env!("CARGO_PKG_VERSION_PATCH"));

// This is a randomly generated GUID to allow clients on Windows to find your device.
pub const DEVICE_INTERFACE_GUIDS: &[&str] = &["{1d58b148-7511-410d-84b5-698f7ee0532b}"];

pub struct HidRequestHandler;

impl HidRequestHandler {
    pub const fn new() -> Self {
        Self
    }
}
impl embassy_usb::class::hid::RequestHandler for HidRequestHandler {
    fn get_report(
        &mut self,
        _id: embassy_usb::class::hid::ReportId,
        buf: &mut [u8],
    ) -> Option<usize> {
        if buf.is_empty() {
            return None;
        }

        buf[0] = crate::LED_STATE.load(Ordering::Relaxed);
        Some(1)
    }
}

pub struct UsbEventHandler {
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
