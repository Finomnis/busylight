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
    /// Continuously print LED changes
    Listen,
}

async fn async_main() -> miette::Result<()> {
    let cli = Cli::parse();

    let device = async || -> Result<BusyLight, BusyLightError> {
        if let Some(serial) = cli.serial {
            BusyLight::new_with_serial(serial).await
        } else {
            BusyLight::new().await
        }
    };

    let command = cli.command.unwrap_or(Commands::List);
    match command {
        Commands::Red => device().await?.set_state(BusyLightState::Red).await?,
        Commands::Yellow => device().await?.set_state(BusyLightState::Yellow).await?,
        Commands::Green => device().await?.set_state(BusyLightState::Green).await?,
        Commands::Off => device().await?.set_state(BusyLightState::Off).await?,
        Commands::Get => println!("{:?}", device().await?.read_state().await?),
        Commands::Listen => {
            let mut device = device().await?;
            println!("{:?}", device.read_state().await?);
            loop {
                println!("{:?}", device.wait_for_state_change().await?);
            }
        }
        Commands::List => {
            let devices = BusyLight::list_devices().await?;
            println!("Available BusyLights:");
            println!();
            if devices.is_empty() {
                println!("   -- none --");
            }
            for device in devices {
                let [major, minor] = ['?', '?']; //device.release_number.to_be_bytes();
                println!(
                    "   - {} v{}.{} (serial: {})",
                    device.name,
                    major,
                    minor,
                    device.serial_number.as_deref().unwrap_or("??"),
                )
            }
            println!();
        }
    }

    Ok(())
}

fn main() -> miette::Result<()> {
    async_io::block_on(async_main())
}
