use hidapi::{HidApi, HidDevice, HidError};
use miette::Diagnostic;
use thiserror::Error;

pub const VID: u16 = 0x1209;
pub const PID: u16 = 0xd9d0;

#[derive(Error, Diagnostic, Debug)]
pub enum BusyLightError {
    #[error(transparent)]
    #[diagnostic(code(busylight::hid))]
    IoError(#[from] HidError),

    #[error("Device reported an unexpected state")]
    #[diagnostic(code(busylight::unexpected_state))]
    UnexpectedDeviceState,
}

pub struct BusyLight {
    device: HidDevice,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum BusyLightState {
    Off = 0,
    Green = 1,
    Yellow = 2,
    Red = 3,
}

impl TryFrom<u8> for BusyLightState {
    type Error = BusyLightError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Off),
            1 => Ok(Self::Green),
            2 => Ok(Self::Yellow),
            3 => Ok(Self::Red),
            _ => Err(BusyLightError::UnexpectedDeviceState),
        }
    }
}

impl BusyLight {
    pub fn new() -> Result<Self, BusyLightError> {
        let api = HidApi::new()?;

        let device = api.open(VID, PID)?;

        Ok(Self { device })
    }

    pub fn new_with_serial(serial: impl AsRef<str>) -> Result<Self, BusyLightError> {
        let api = HidApi::new()?;

        let device = api.open_serial(VID, PID, serial.as_ref())?;

        Ok(Self { device })
    }

    pub fn list_devices() -> Result<Vec<hidapi::DeviceInfo>, BusyLightError> {
        let api = HidApi::new()?;

        Ok(api
            .device_list()
            .filter(|dev| dev.vendor_id() == VID && dev.product_id() == PID)
            .cloned()
            .collect())
    }

    fn send_value(&self, value: u8) -> Result<(), BusyLightError> {
        self.device.write(&[0x00, value])?;
        Ok(())
    }

    pub fn set_state(&self, state: BusyLightState) -> Result<(), BusyLightError> {
        self.send_value(state as u8)
    }

    pub fn read_state(&self) -> Result<BusyLightState, BusyLightError> {
        let mut buf = [0u8; 2];
        self.device.get_feature_report(&mut buf)?;

        BusyLightState::try_from(buf[1])
    }
}
