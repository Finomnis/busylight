#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;

pub use config::{ButtonConfig, Mode};

mod config;

use rtic_time::Monotonic;

/// A generic button that asynchronously detects [`ButtonEvent`]s.
#[derive(Clone)]
pub struct Button<P, Mono: Monotonic> {
    pin: P,
    state: State,
    count: usize,
    config: ButtonConfig<Mono>,
    mono: PhantomData<Mono>,
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// Initial state.
    Unknown,
    /// Debounced press.
    Pressed,
    /// The button was just released, waiting for more presses in the same sequence, or for the
    /// sequence to end.
    Released,
    /// Fully released state, idle.
    Idle,
    /// Waiting for the button to be released.
    PendingRelease,
}

/// Detected button events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ButtonEvent {
    /// A sequence of 1 or more short presses.
    ShortPress {
        /// The number of short presses in the sequence.
        count: usize,
    },
    /// A long press. This event is returned directly when the button is held for more than
    /// [`ButtonConfig::long_press`].
    LongPress,
}

impl<P, Mono> Button<P, Mono>
where
    P: embedded_hal_async::digital::Wait + embedded_hal::digital::InputPin,
    Mono: rtic_time::Monotonic,
{
    /// Creates a new button with the provided config.
    pub const fn new(pin: P, config: ButtonConfig<Mono>) -> Self {
        Self {
            pin,
            state: State::Unknown,
            count: 0,
            config,
            mono: PhantomData,
        }
    }

    /// Updates the button and returns the detected event.
    ///
    /// Awaiting this blocks execution of the task until a [`ButtonEvent`] is detected so it should
    /// **not** be called from tasks where blocking for long periods of time is not desireable.
    pub async fn update(&mut self) -> ButtonEvent {
        loop {
            if let Some(event) = self.update_step().await {
                return event;
            }
        }
    }

    async fn update_step(&mut self) -> Option<ButtonEvent> {
        match self.state {
            State::Unknown => {
                if self.is_pin_pressed() {
                    self.state = State::Pressed;
                } else {
                    self.state = State::Idle;
                }
                None
            }

            State::Pressed => {
                match Mono::timeout_after(self.config.long_press, self.wait_for_release()).await {
                    Ok(_) => {
                        // Short press
                        self.debounce_delay().await;
                        if self.is_pin_released() {
                            self.state = State::Released;
                        }
                        None
                    }
                    Err(_) => {
                        // Long press detected
                        self.count = 0;
                        self.state = State::PendingRelease;
                        Some(ButtonEvent::LongPress)
                    }
                }
            }

            State::Released => {
                match Mono::timeout_after(self.config.double_click, self.wait_for_press()).await {
                    Ok(_) => {
                        // Continue sequence
                        self.debounce_delay().await;
                        if self.is_pin_pressed() {
                            self.count += 1;
                            self.state = State::Pressed;
                        }
                        None
                    }
                    Err(_) => {
                        // Sequence ended
                        let count = self.count;
                        self.count = 0;
                        self.state = State::Idle;
                        Some(ButtonEvent::ShortPress { count })
                    }
                }
            }

            State::Idle => {
                self.wait_for_press().await;
                self.debounce_delay().await;
                if self.is_pin_pressed() {
                    self.count = 1;
                    self.state = State::Pressed;
                }
                None
            }

            State::PendingRelease => {
                self.wait_for_release().await;
                self.debounce_delay().await;
                if self.is_pin_released() {
                    self.state = State::Idle;
                }
                None
            }
        }
    }

    fn is_pin_pressed(&mut self) -> bool {
        self.pin.is_low().unwrap_or(self.config.mode.is_pulldown()) == self.config.mode.is_pullup()
    }

    fn is_pin_released(&mut self) -> bool {
        !self.is_pin_pressed()
    }

    async fn wait_for_release(&mut self) {
        match self.config.mode {
            Mode::PullUp => self.pin.wait_for_high().await.unwrap_or_default(),
            Mode::PullDown => self.pin.wait_for_low().await.unwrap_or_default(),
        }
    }

    async fn wait_for_press(&mut self) {
        match self.config.mode {
            Mode::PullUp => self.pin.wait_for_low().await.unwrap_or_default(),
            Mode::PullDown => self.pin.wait_for_high().await.unwrap_or_default(),
        }
    }

    async fn debounce_delay(&self) {
        Mono::delay(self.config.debounce).await;
    }
}
