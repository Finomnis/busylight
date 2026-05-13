use busylight::{BusyLight, BusyLightState};
use clap::{Parser, Subcommand};

/// Controls the BusyLight
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// List available devices
    #[arg(short, long, global = true)]
    list: bool,

    /// Specify device serial
    #[arg(short, long, global = true)]
    serial: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,

    /// Retreives the current LED state
    #[arg(long)]
    get: bool,
}

#[derive(Subcommand, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Commands {
    /// Set light to red
    Red,
    /// Set light to yellow
    Yellow,
    /// Set light to green
    Green,
    /// Switch light off
    Off,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    if let Some(command) = cli.command
        && !cli.list
    {
        let device = if let Some(serial) = cli.serial {
            BusyLight::new_with_serial(serial)
        } else {
            BusyLight::new()
        }?;
        device.set_state(match command {
            Commands::Red => BusyLightState::Red,
            Commands::Yellow => BusyLightState::Yellow,
            Commands::Green => BusyLightState::Green,
            Commands::Off => BusyLightState::Off,
        })?;
    } else if cli.get {
        let device = if let Some(serial) = cli.serial {
            BusyLight::new_with_serial(serial)
        } else {
            BusyLight::new()
        }?;
        let state = device.read_state()?;
        println!("{:?}", state);
    } else {
        let devices = BusyLight::list_devices()?;
        println!("Available BusyLights:");
        println!();
        if devices.is_empty() {
            println!("   -- none --");
        }
        for device in devices {
            let [major, minor] = device.release_number().to_be_bytes();
            println!(
                "   - {} {} v{}.{} (serial: {})",
                device.manufacturer_string().unwrap_or("??"),
                device.product_string().unwrap_or("??"),
                major,
                minor,
                device.serial_number().unwrap_or("??"),
            )
        }
        println!();
    }

    Ok(())
}
