use std::time::Duration;

use anyhow::{Context, Result};
use clap::{arg, value_parser, AppSettings, ArgAction, Command};
use serialport::{DataBits, Parity, StopBits};

use crate::device::GpsDevice;

mod cat;
mod config;
mod server;

pub struct CmdData {
    verbose: bool,
    device: GpsDevice,
}

pub async fn run() -> Result<()> {
    let matches = Command::new("gps")
        .version("0.1")
        .arg(
            arg!(
            -v --verbose "Enable verbose output"
            )
            .action(ArgAction::SetTrue),
        )
        .arg(
            arg!(
                -s --serial <PATH> "Set the serial port"
            )
            .required(false)
            .default_value("/dev/ttyACM0"),
        )
        .arg(
            arg!(
                -b --baud <BOUD> "Set the baud rate for the serial port"
            )
            .required(false)
            .default_value("9600")
            .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(
            -c --config <CONFIG> "apply the settings from a config file to the device"
            )
            .required(false),
        )
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(config::subcmd())
        .subcommand(cat::subcmd())
        .subcommand(server::subcmd())
        .get_matches();

    let verbose = *matches.get_one::<bool>("verbose").unwrap();
    let port_path = matches.get_one::<String>("serial").unwrap();
    let port_baud = *matches.get_one::<u32>("baud").unwrap();
    let port = serialport::new(port_path, port_baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_secs(1));

    let device = GpsDevice::new(port).context("failed to create gps device")?;

    let mut data = CmdData { verbose, device };

    match matches.subcommand() {
        Some(("config", matches)) => config::cmd(&mut data, matches).await,
        Some(("cat", matches)) => cat::cmd(&mut data, matches).await,
        Some(("server", matches)) => server::cmd(&mut data, matches).await,
        _ => unreachable!(),
    }
}
