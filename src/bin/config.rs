use anyhow::{Context, Result};
use clap::{arg, Command};
use enumflags2::BitFlags;
use futures::StreamExt;
use gps::{
    connection::Connection,
    msg::{
        self,
        ubx::{
            self,
            ack::Ack,
            cfg::{
                BbrMask, BitLayer, Cfg, Layer, Rst, ValGet, ValGetRequest, ValSet, Value, ValueKey,
            },
        },
        GpsMsg, Ubx,
    },
    parse::ParseData,
};
use log::{error, info, trace};
use serde_json::Error as JsonError;
use std::result::Result as StdResult;
use tokio::net::TcpStream;

fn parse_config_value(v: &str) -> StdResult<ubx::cfg::ValueKey, JsonError> {
    serde_json::from_str(&format!("\"{v}\""))
}

async fn reconnect(mut tcp: Connection) -> Result<()> {
    let bytes = msg::Server {
        msg: msg::server::ServerMsg::ResetPort,
    }
    .parse_to_vec()
    .unwrap();

    info!("sending reconnect message");
    tcp.write_message(&bytes)
        .await
        .context("failed to send message to server")?;
    info!("finished sending");

    Ok(())
}

async fn reset(mut tcp: Connection) -> Result<()> {
    let msg = ubx::Ubx::Cfg(Cfg::Rst(Rst {
        reset_mode: ubx::cfg::ResetMode::HardwareImmediately,
        nav_bbr_mask: BitFlags::<BbrMask>::all(),
        res1: 0,
    }));
    let bytes = msg.parse_to_vec().unwrap();
    info!("sending reset message");
    tcp.write_message(&bytes)
        .await
        .context("failed to send message to server")?;
    info!("finished sending");

    Ok(())
}

async fn set(mut tcp: Connection, path: &str) -> Result<()> {
    info!("reading config file");
    let file = tokio::fs::read(path)
        .await
        .context("failed to read config file")?;

    let keys: Vec<Value> = serde_json::from_slice(&file).context("failed to parse config file")?;

    let mut i = 0;
    for v in keys.chunks(64) {
        i += v.len();
        info!("writing up to `{i}` configuration values");
        let msg = ubx::Ubx::Cfg(Cfg::ValSet(ValSet {
            version: 0,
            res1: [0; 2],
            values: v.into(),
            layers: BitLayer::Ram.into(),
        }));
        let bytes = msg.parse_to_vec().unwrap();

        tcp.write_message(&bytes)
            .await
            .context("failed to send message to server")?;

        info!("waiting for ack...");
        loop {
            if let Some(x) = tcp.next().await {
                let x = match x {
                    Ok(x) => x,
                    Err(e) => {
                        error!("error reading from server: {:?}", e);
                        continue;
                    }
                };
                let msg = GpsMsg::parse_read(&x).map(|x| x.1);
                trace!("msg: {:?}", msg);
                match msg {
                    Ok(GpsMsg::Ubx(Ubx::Ack(Ack::Ack(x)))) => {
                        if x.cls_id == 0x06 && x.msg_id == 0x8a {
                            info!("recieved acknowledgement");
                            break;
                        }
                    }
                    Ok(GpsMsg::Ubx(Ubx::Ack(Ack::Nak(x)))) => {
                        if x.cls_id == 0x06 && x.msg_id == 0x8a {
                            error!("device did not acknowledge config");
                            return Ok(());
                        }
                    }
                    Ok(x) => {
                        info!("message {:?}", x)
                    }
                    Err(e) => {
                        error!("error parsing message {:?}", e)
                    }
                }
            } else {
                error!("server connection quit unexpectedly");
                return Ok(());
            }
        }
    }

    Ok(())
}

async fn get(mut tcp: Connection, value: Vec<ubx::cfg::ValueKey>) -> Result<()> {
    for v in value.chunks(64) {
        let msg = ubx::Ubx::Cfg(Cfg::ValGet(ValGet::Request(ValGetRequest {
            layer: Layer::Ram,
            res1: [0u8; 2],
            keys: v.into(),
        })));
        let mut bytes = Vec::<u8>::new();
        msg.parse_write(&mut bytes).unwrap();

        tcp.write_message(&bytes)
            .await
            .context("failed to send message to server")?;

        while let Some(x) = tcp.next().await {
            let x = match x {
                Ok(x) => x,
                Err(e) => {
                    error!("error reading from server: {:?}", e);
                    continue;
                }
            };
            match GpsMsg::parse_read(&x).map(|x| x.1) {
                Ok(GpsMsg::Ubx(Ubx::Cfg(Cfg::ValGet(ValGet::Response(x))))) => {
                    for k in x.keys {
                        println!("{:?}", k);
                    }
                    break;
                }
                Ok(GpsMsg::Ubx(Ubx::Ack(Ack::Nak(x)))) => {
                    if x.cls_id == 0x06 && x.msg_id == 0x8b {
                        error!("could not get value, one of the requested values might not be known to the gps device");
                        return Ok(());
                    }
                }
                Ok(x) => {
                    info!("message {:?}", x)
                }
                Err(e) => {
                    error!("error parsing message {:?}", e)
                }
            }
        }
    }
    Ok(())
}

async fn run() -> Result<()> {
    let matches = Command::new("gps config")
        .version("0.1")
        .arg(
            arg!(
                [address] "The address to connect too"
            )
            .required(false)
            .default_value("0.0.0.0:9165"),
        )
        .subcommand(
            Command::new("get").arg(
                arg!(
                        <VALUE> "The value(s) to get the value from"
                )
                .multiple_values(true)
                .value_parser(parse_config_value),
            ),
        )
        .subcommand(Command::new("set").arg(arg!(
            <FILE> "the file to read the configuration from"
        )))
        .subcommand(Command::new("reset"))
        .subcommand(Command::new("reconnect"))
        .subcommand_required(true)
        .get_matches();

    let address = matches.get_one::<String>("address").unwrap();

    let tcp = TcpStream::connect(address)
        .await
        .context("failed to connect to server")?;

    let tcp = Connection::new(tcp);

    match matches.subcommand() {
        Some(("get", sub_m)) => {
            let values = sub_m
                .get_many::<ValueKey>("VALUE")
                .unwrap()
                .copied()
                .collect();
            get(tcp, values).await?;
        }
        Some(("set", sub_m)) => {
            let file = sub_m.get_one::<String>("FILE").unwrap();
            set(tcp, file).await?;
        }
        Some(("reset", _)) => {
            reset(tcp).await?;
        }
        Some(("reconnect", _)) => {
            reconnect(tcp).await?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
