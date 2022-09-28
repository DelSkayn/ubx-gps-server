use anyhow::{Context, Result};
use clap::{arg, value_parser, ArgAction, ArgMatches, Command};
use log::info;

use crate::server::StreamServer;

pub fn subcmd<'help>() -> Command<'help> {
    Command::new("proxy")
        .about("Work with device config")
        .arg(
            arg!(
                [address] "The address to host the server on"
            )
            .required(false)
            .default_value("0.0.0.0"),
        )
        .arg(
            arg!(
                -p --port <PORT> "Set the port to run the data server on"
            )
            .required(false)
            .default_value("9165")
            .value_parser(value_parser!(u16)),
        )
        .arg(arg!( -r --raw "Dont format message but send raw bytes").action(ArgAction::SetTrue))
}

pub async fn cmd(data: &mut super::CmdData, m: &ArgMatches) -> Result<()> {
    let raw = *m.get_one::<bool>("raw").unwrap();
    let addr = m.get_one::<String>("address").unwrap();
    let port = *m.get_one::<u16>("port").unwrap();

    let mut server = StreamServer::new((addr.clone(), port), raw)
        .await
        .context("Failed to create server")?;

    info!("starting proxy");

    if raw {
        loop {
            tokio::select! {
                msg = data.device.read_bytes() => {
                    let msg = msg?;
                    server.send_raw(&msg).await?;
                }
                msg = server.recv_raw() => {
                    data.device.write_raw(&msg).await?;
                }
            }
        }
    } else {
        loop {
            tokio::select! {
                msg = data.device.read() => {
                    let msg = msg?;
                    msg.log();
                    server.send(&msg).await?;
                }
                msg = server.recv() => {
                    data.device.write(msg).await?;
                }
            }
        }
    }
}
