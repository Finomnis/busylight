use fugit::ExtU32;

/// [`Button`](super::Button) configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ButtonConfig<Mono: rtic_time::Monotonic> {
    /// Time the button should be down in order to count it as a press.
    pub debounce: Mono::Duration,
    /// Time between consecutive presses to count as a press in the same sequence instead of a new
    /// sequence.
    pub double_click: Mono::Duration,
    /// Time the button is held before a long press is detected.
    pub long_press: Mono::Duration,
    /// Button direction.
    pub mode: Mode,
}

impl<Mono: rtic_time::Monotonic> ButtonConfig<Mono> {
    /// Returns a new [ButtonConfig].
    pub fn new(
        debounce: Mono::Duration,
        double_click: Mono::Duration,
        long_press: Mono::Duration,
        mode: Mode,
    ) -> Self {
        Self {
            debounce,
            double_click,
            long_press,
            mode,
        }
    }
}

impl<
    const NOM: u32,
    const DENOM: u32,
    Mono: rtic_time::Monotonic<Duration = fugit::Duration<u32, NOM, DENOM>>,
> Default for ButtonConfig<Mono>
{
    fn default() -> Self {
        Self {
            debounce: 10.millis(),
            double_click: 350.millis(),
            long_press: 1000.millis(),
            mode: Mode::default(),
        }
    }
}

/// Button direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Mode {
    /// Button is connected to a pin with a pull-up resistor. Button pressed it logic 0.
    #[default]
    PullUp,
    /// Button is connected to a pin with a pull-down resistor. Button pressed it logic 1.
    PullDown,
}

impl Mode {
    /// Is button connected to a pin with a pull-up resistor?
    pub const fn is_pullup(&self) -> bool {
        matches!(self, Mode::PullUp)
    }

    /// Is button connected to a pin with a pull-down resistor?
    pub const fn is_pulldown(&self) -> bool {
        !self.is_pullup()
    }
}
