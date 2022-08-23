use std::time::Duration;

use anyhow::{Context, Result};
use clap::{arg, value_parser, ArgMatches, Command};
use log::{info, warn};
use tokio::{
    io::BufReader,
    net::TcpStream,
    sync::mpsc::{self, Receiver},
};

use crate::{
    rtcm::RtcmFrame,
    server::{Msg, StreamServer},
    GpsMsg,
};

use super::CmdData;

pub fn subcmd<'help>() -> Command<'help> {
    Command::new("server")
        .arg(
            arg!(
                -p --port <PORT> "Set the port to run the data server on"
            )
            .required(false)
            .default_value("9165")
            .value_parser(value_parser!(u16)),
        )
        .arg(
            arg!(
                -t --rtcmaddress <ADDRESS> "The address to connect to for recieving RTCM packets"
            )
            .required(false),
        )
        .arg(
            arg!(
                [address] "The address to host the server on"
            )
            .required(false)
            .default_value("localhost"),
        )
        .arg(
            arg!(
                -c --config <PATH> "Apply config file before running server"
                )
            )
}

pub async fn rtcm_stream(stream: TcpStream, send: &mpsc::Sender<RtcmFrame<'static>>) -> Result<()> {
    let mut buf = BufReader::new(stream);
    loop {
        let msg = Msg::from_reader(&mut buf).await?;
        let (gps_msg, len) = match GpsMsg::from_bytes(msg.as_bytes()) {
            Ok(x) => x,
            Err(e) => {
                warn!("retrieved invalid rtcm message: {:?}", e);
                continue;
            }
        };
        if len != msg.as_bytes().len() {
            warn!("rtcm message did not use full message");
        }
        if let GpsMsg::Rtcm(rtcm) = gps_msg {
            if send.send(rtcm.into_owned()).await.is_err() {
                return Ok(());
            };
        }
    }
}

pub fn connect_rtcm(addr: String) -> Receiver<RtcmFrame<'static>> {
    let (send, recv) = mpsc::channel(16);

    tokio::spawn(async move {
        let addr = addr;
        loop {
            match TcpStream::connect(&addr).await {
                Ok(x) => {
                    if let Err(e) = rtcm_stream(x, &send).await {
                        warn!("error rtcm socket: {}", e);
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    warn!("error connecting rtcm socket: {}", e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    });

    recv
}

pub async fn cmd(data: &mut CmdData, arg: &ArgMatches) -> Result<()> {
    let address = arg.get_one::<String>("address").unwrap();
    let port = arg.get_one::<u16>("port").unwrap();

    if let Some(x) = arg.get_one::<String>("config"){
        info!("applying config");
        super::config::set(data,x).await?;
    }

    let mut server = StreamServer::new((address.as_str(), *port))
        .await
        .context("failed to create server")?;

    let mut rtcm_stream = arg
        .get_one::<String>("rtcmaddress")
        .map(|x| connect_rtcm(x.clone()));

    loop {
        if let Some(x) = rtcm_stream.as_mut() {
            tokio::select! {
                msg = x.recv() => {
                    info!("rtcm msg: {:?}", msg);
                    data.device.write(crate::GpsMsg::Rtcm(msg.expect("rtcm stream quit unexpectedly"))).await;
                }
                msg = data.device.read() => {
                    info!("msg: {:?}", msg);
                    server.send(&msg).await?;
                }
                _ = server.accept() => {}
            }
        } else {
            tokio::select! {
                msg = data.device.read() => {
                    msg.log();
                    info!("msg: {:?}", msg);
                    server.send(&msg).await?;
                }
                _ = server.accept() => {}
            }
        }
    }
}
