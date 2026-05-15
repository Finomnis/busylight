use std::collections::HashMap;

use async_hid::{AsyncHidFeatureHandle, AsyncHidRead, AsyncHidWrite, HidBackend, HidError};
use futures::StreamExt;
use miette::Diagnostic;
use thiserror::Error;

pub const VID: u16 = 0x1209;
pub const PID: u16 = 0xd9d0;

#[derive(Error, Diagnostic, Debug)]
pub enum BusyLightError {
    #[error(transparent)]
    #[diagnostic(code(busylight::hid))]
    IoError(#[from] HidError),

    #[error("Device not found")]
    #[diagnostic(code(busylight::no_device))]
    DeviceNotFound,

    #[error("Device reported an unexpected state")]
    #[diagnostic(code(busylight::unexpected_state))]
    UnexpectedDeviceState,

    #[error("Device responded with an invalid feature report")]
    #[diagnostic(code(busylight::invalid_feature_report))]
    InvalidFeatureReport,

    #[error("Device sent an invalid input report")]
    #[diagnostic(code(busylight::invalid_input_report))]
    InvalidInputReport,
}

pub struct BusyLight {
    reader: async_hid::DeviceReader,
    writer: async_hid::DeviceWriter,
    feature: async_hid::DeviceFeatureHandle,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BusyLightDeviceInfo {
    pub name: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub version: Option<u16>,
}

impl BusyLight {
    pub async fn new() -> Result<Self, BusyLightError> {
        let backend = HidBackend::default();
        let mut devices = backend.enumerate().await?;

        let device = loop {
            match devices.next().await {
                Some(dev) if dev.vendor_id == VID && dev.product_id == PID => {
                    break dev;
                }
                Some(_) => continue,
                None => return Err(BusyLightError::DeviceNotFound),
            }
        };

        let (reader, writer) = device.open().await?;

        let feature = device.open_feature_handle().await?;

        Ok(Self {
            reader,
            writer,
            feature,
        })
    }

    pub async fn new_with_serial(serial: impl AsRef<str>) -> Result<Self, BusyLightError> {
        let serial = serial.as_ref();

        let backend = HidBackend::default();
        let mut devices = backend.enumerate().await?;

        let device = loop {
            match devices.next().await {
                Some(dev)
                    if dev.vendor_id == VID
                        && dev.product_id == PID
                        && dev.serial_number.as_deref() == Some(serial) =>
                {
                    break dev;
                }
                Some(_) => continue,
                None => return Err(BusyLightError::DeviceNotFound),
            }
        };

        let (reader, writer) = device.open().await?;

        let feature = device.open_feature_handle().await?;

        Ok(Self {
            reader,
            writer,
            feature,
        })
    }

    pub async fn list_devices() -> Result<Vec<BusyLightDeviceInfo>, BusyLightError> {
        let versions = nusb::list_devices().await.ok().map(|devs| {
            devs.filter_map(|dev| {
                if dev.vendor_id() == VID && dev.product_id() == PID {
                    dev.serial_number()
                        .map(|serial| (serial.to_string(), dev.device_version()))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>()
        });

        let devices = HidBackend::default()
            .enumerate()
            .await?
            .filter_map(async |dev| {
                if dev.vendor_id != VID || dev.product_id != PID {
                    return None;
                }

                let version =
                    if let (Some(versions), Some(serial)) = (&versions, &dev.serial_number) {
                        versions.get(serial).copied()
                    } else {
                        None
                    };

                Some(BusyLightDeviceInfo {
                    name: dev.name.clone(),
                    vendor_id: dev.vendor_id,
                    product_id: dev.product_id,
                    serial_number: dev.serial_number.clone(),
                    version,
                })
            })
            .collect::<Vec<_>>()
            .await;

        Ok(devices)
    }

    async fn send_value(&mut self, value: u8) -> Result<(), BusyLightError> {
        self.writer
            .write_output_report(&[0x01, value])
            .await
            .map_err(Into::into)
    }

    pub async fn set_state(&mut self, state: BusyLightState) -> Result<(), BusyLightError> {
        self.send_value(state as u8).await
    }

    pub async fn read_state(&mut self) -> Result<BusyLightState, BusyLightError> {
        let mut buf = [0x02, 0x00];
        let read_len = self.feature.read_feature_report(&mut buf).await?;

        match read_len {
            // Backends that return report ID + one-byte payload.
            2 => BusyLightState::try_from(buf[1]),

            // Backends that return only the one-byte payload.
            1 => BusyLightState::try_from(buf[0]),

            _ => Err(BusyLightError::InvalidFeatureReport),
        }
    }

    pub async fn wait_for_state_change(&mut self) -> Result<BusyLightState, BusyLightError> {
        let mut buf = [0u8; 2];
        let read_len = self.reader.read_input_report(&mut buf).await?;

        if read_len != 2 || buf[0] != 0x03 {
            Err(BusyLightError::InvalidInputReport)
        } else {
            BusyLightState::try_from(buf[1])
        }
    }
}
