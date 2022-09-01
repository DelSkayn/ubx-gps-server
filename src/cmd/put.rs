use anyhow::{Result, Context};
use clap::{arg, ArgMatches, Command};

use crate::GpsMsg;

pub fn subcmd<'help>() -> Command<'help> {
    Command::new("put")
        .about("write a list of commands to gps")
        .arg(arg!(<FILE>).required(true))
}

pub async fn cmd(data: &mut super::CmdData, m: &ArgMatches) -> Result<()> {
    let file = m.get_one::<String>("FILE").unwrap();

    let string = tokio::fs::read_to_string(file).await
        .context("failed to read command file")?;

    let commands: Vec<GpsMsg> = serde_json::from_str(&string)
        .context("failed to parse command file")?;

    for cmd in commands{
        data.device.write(cmd).await?;
    }
    Ok(())
}
