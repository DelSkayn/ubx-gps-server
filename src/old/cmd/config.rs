use anyhow::{bail, Context, Result};
use clap::{arg, value_parser, ArgMatches, Command};
use futures::FutureExt;
use log::debug;
use tokio::fs;

use crate::{
    ubx::{
        self,
        cfg::{BitLayer, Layer, Value, ValueKey},
        Cfg,
    },
    GpsMsg,
};

pub fn subcmd<'help>() -> Command<'help> {
    Command::new("config")
        .about("Work with device config")
        .subcommand(
            Command::new("set")
                .about("set config values")
                .arg(arg!([PATH]).required(true)),
        )
        .subcommand(
            Command::new("get").about("get set config values").arg(
                arg!([VALUE])
                    .required(true)
                    .value_parser(value_parser!(ValueKey))
                    .multiple_values(true),
            ),
        )
}

pub async fn cmd(data: &mut super::CmdData, matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("set", sub_matches)) => {
            let v = sub_matches.get_one::<String>("PATH").unwrap();
            set(data, v).await
        }
        Some(("get", sub_matches)) => {
            let v = sub_matches
                .get_many::<ValueKey>("VALUE")
                .unwrap()
                .copied()
                .collect();
            get(data, v).await
        }
        _ => unreachable!(),
    }
}

pub async fn set(data: &mut super::CmdData, value: &str) -> Result<()> {
    let file = fs::read(value)
        .await
        .context("failed to read config file")?;
    let values: Vec<Value> = serde_json::from_slice(&file).context("failed to parse config")?;

    for vals in values.chunks(64) {
        let cfg = Cfg::ValSet {
            version: 0,
            layer: BitLayer::Ram.into(),
            values: vals.into(),
        };
        debug!("config: {:?}", cfg);
        let ack = data
            .device
            .config(cfg)
            .await
            .context("could not write config to device")?;
        let ack = ack.shared();

        loop {
            tokio::select! {
                acked = ack.clone() => {
                    if let Ok(false) = acked{
                        bail!("config not ack'd")
                    }else{
                        return Ok(())
                    }
                }
                msg = data.device.read() => {
                    msg.context("failed to parse message from device")?.log();
                }
            }
        }
    }
    Ok(())
}

async fn get(data: &mut super::CmdData, values: Vec<ValueKey>) -> Result<()> {
    let cfg = Cfg::ValGetReq {
        version: 0,
        layer: Layer::Ram,
        values,
    };
    let ack = data.device.config(cfg).await?;
    ack.await.ok();

    loop {
        let msg = data.device.read().await?;
        msg.log();
        if let GpsMsg::Ubx(ubx::Msg::Cfg(Cfg::ValGetRes { values, .. })) = msg {
            for v in values.iter() {
                println!("{:?}", v)
            }
            return Ok(());
        }
    }
}
