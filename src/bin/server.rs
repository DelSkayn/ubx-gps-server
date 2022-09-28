use std::{net::SocketAddr, str::FromStr, time::Duration};

use anyhow::{anyhow, Context as ErrorContext, Result};
use clap::{arg, value_parser, ArgAction, Command};
use futures::{FutureExt, SinkExt, StreamExt};
use gps::{
    connection::{ConnectionPool, OutgoingConnection},
    msg::GpsMsg,
    VecExt,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_serial::{DataBits, Parity, SerialStream, StopBits};

fn find_message(b: &mut Vec<u8>) {
    if b.len() < 2 {
        return;
    }
    if GpsMsg::contains_prefix(b) {
        return;
    }
    let mut idx = 1;
    while b.len() > idx {
        if GpsMsg::contains_prefix(&b[idx..]) {
            b.shift(idx);
            return;
        }
        idx += 1;
    }
    b.clear();
}

async fn run() -> Result<()> {
    let matches = Command::new("gps server")
        .version("0.1")
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
            .requires("serial")
            .default_value("9600")
            .value_parser(value_parser!(u32)),
        )
        .arg(
            arg!(
                -p --port <PORT> "Set the port to host the server on"
            )
            .required(false)
            .default_value("9165")
            .value_parser(value_parser!(u16)),
        )
        .arg(
            arg!(
                -c --connect <ADDRESS> "Connect to an other server."
            )
            .required(false),
        )
        .arg(
            arg!(
                [address] "The address to host the server on"
            )
            .required(false)
            .default_value("0.0.0.0"),
        )
        .arg(
            arg!(
                -D --deamonize "run the server as a deamon"
            )
            .action(ArgAction::SetTrue),
        )
        .get_matches();

    let address = matches.get_one::<String>("address").unwrap();
    let server_port = *matches.get_one::<u16>("port").unwrap();

    let port_path = matches.get_one::<String>("serial").unwrap();
    let port_baud = *matches.get_one::<u32>("baud").unwrap();

    let connection_address = matches
        .get_one::<String>("connect")
        .map(|x| x.as_str())
        .map(SocketAddr::from_str)
        .transpose()
        .context("error parsing connection address")?;

    let port = tokio_serial::new(port_path, port_baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_secs(1));

    let mut port = SerialStream::open(&port).context("failed to open serial port")?;

    let listener = TcpListener::bind((address.as_str(), server_port))
        .await
        .context("failed to create server")?;

    let mut outgoing_connection = OutgoingConnection::new(connection_address);

    let mut connections = ConnectionPool::new(listener);

    if *matches.get_one::<bool>("deamon").unwrap() {
        gps::deamonize()
            .map_err(|_| anyhow!("deamon creation error"))
            .context("failed to create a deamon")?;
    }

    let mut port_read_buffer = [0u8; 4096];
    let mut pending_read_bytes = Vec::new();

    loop {
        let mut outgoing_connection_future = Box::pin(outgoing_connection.next());
        let mut device_future = Box::pin(port.read(&mut port_read_buffer).fuse());
        let mut connection_future = connections.next();

        futures::select! {
            x = device_future => {
                let x = x?;
                pending_read_bytes.extend(&port_read_buffer[..x]);
                find_message(&mut pending_read_bytes);
                if let Some(x) = GpsMsg::message_usage(&pending_read_bytes){

                    let mut buf = pending_read_bytes.split_off(x);
                    std::mem::swap(&mut buf,&mut pending_read_bytes);

                    connections.send(buf).await.unwrap();
                }
            },
            x = outgoing_connection_future => {
                port.write_all(&x.unwrap()).await.context("error writing to device")?;
            },
            out = connection_future => {
                port.write_all(&out.unwrap()).await.context("error writing to device")?;
            }
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
