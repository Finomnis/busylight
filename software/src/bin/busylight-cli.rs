use busylight::{BusyLight, BusyLightError, BusyLightState};
use clap::{Parser, Subcommand};

/// Controls the BusyLight
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Specify device serial
    #[arg(short, long, global = true)]
    serial: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
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
    /// Lists all available devices
    List,
    /// Shows the current LED state
    Get,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let device = || -> Result<BusyLight, BusyLightError> {
        if let Some(serial) = cli.serial {
            BusyLight::new_with_serial(serial)
        } else {
            BusyLight::new()
        }
    };

    let command = cli.command.unwrap_or(Commands::List);
    match command {
        Commands::Red => device()?.set_state(BusyLightState::Red)?,
        Commands::Yellow => device()?.set_state(BusyLightState::Yellow)?,
        Commands::Green => device()?.set_state(BusyLightState::Green)?,
        Commands::Off => device()?.set_state(BusyLightState::Off)?,
        Commands::Get => println!("{:?}", device()?.read_state()?),
        Commands::List => {
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
    }

    Ok(())
}
