use std::{net::SocketAddr, str::FromStr};

use anyhow::{anyhow, Context, Result};
use clap::{arg, value_parser, ArgAction, Command};
use futures::{
    future::{self, Either},
    SinkExt, StreamExt,
};
use gps::{
    connection::{ConnectionPool, OutgoingConnection},
    msg::GpsMsg,
    parse::ParseData,
};
use log::{error, trace};
use tokio::net::TcpListener;

async fn run() -> Result<()> {
    let matches = Command::new("gps format")
        .version("0.1")
        .arg(
            arg!(
                -p --port <PORT> "Set the port to host the server on"
            )
            .required(false)
            .default_value("9166")
            .value_parser(value_parser!(u16)),
        )
        .arg(
            arg!(
                [ADDRESS] "Connect to an other server."
            )
            .required(true)
            .value_parser(SocketAddr::from_str),
        )
        .arg(
            arg!(
                -h --host <ADDRESS> "The address to host the server on"
            )
            .required(false)
            .default_value("0.0.0.0"),
        )
        .arg(
            arg!(
                -D --deamon "run the server as a deamon"
            )
            .action(ArgAction::SetTrue),
        )
        .get_matches();

    let address = matches.get_one::<SocketAddr>("ADDRESS").unwrap();
    let server_address = matches.get_one::<String>("host").unwrap();
    let server_port = *matches.get_one::<u16>("port").unwrap();

    let listener = TcpListener::bind((server_address.as_str(), server_port))
        .await
        .context("failed to create server")?;

    let mut connections = ConnectionPool::new(listener);

    let mut outgoing = OutgoingConnection::new(Some(*address));

    if *matches.get_one::<bool>("deamon").unwrap() {
        gps::deamonize()
            .map_err(|_| anyhow!("deamon creation error"))
            .context("failed to create a deamon")?;
    }

    loop {
        match future::select(connections.next(), outgoing.next()).await {
            // Just to ensure that connections are accepting, messages are ignored.
            Either::Left((Some(x), _)) => match serde_json::from_slice::<GpsMsg>(&x) {
                Ok(x) => {
                    let mut buffer = Vec::<u8>::new();
                    x.parse_write(&mut buffer).unwrap();
                    outgoing.try_send_message(&buffer).await;
                }
                Err(e) => {
                    error!("error deserializing incomming message {e}");
                }
            },
            Either::Right((Some(x), _)) => match GpsMsg::parse_read(&x) {
                Ok((_, x)) => {
                    trace!("message: {:?}", x);
                    match serde_json::to_vec(&x) {
                        Ok(data) => {
                            connections.send(data).await.unwrap();
                        }
                        Err(e) => {
                            error!("error serializing message {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("error parsing message: {e}");
                }
            },
            _ => unreachable!(),
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
