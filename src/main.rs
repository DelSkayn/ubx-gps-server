#![allow(dead_code)]

use anyhow::Result;

use log::{info, debug, warn, error};
use nmea::NmeaFrame;
use parse::Error;
use rtcm::RtcmFrame;

use serde::{Deserialize, Serialize};
use ubx::Msg;

mod device;
mod nmea;
mod parse;
mod rtcm;
mod server;
mod ubx;
mod ntrip;

mod cmd;

#[derive(Debug, Serialize, Deserialize)]
pub enum GpsMsg<'a> {
    #[serde(borrow)]
    Nmea(NmeaFrame<'a>),
    #[serde(borrow)]
    Rtcm(RtcmFrame<'a>),
    Ubx(ubx::Msg),
}

impl<'a> GpsMsg<'a> {
    pub fn from_bytes(b: &'a [u8]) -> parse::Result<(Self, usize)> {
        if ubx::Msg::valid_prefix(b) {
            Msg::from_bytes(b).map(|x| (GpsMsg::Ubx(x.0), x.1))
        } else if RtcmFrame::valid_prefix(b) {
            RtcmFrame::from_bytes(b).map(|x| (GpsMsg::Rtcm(x.0), x.1))
        } else if NmeaFrame::valid_prefix(b) {
            NmeaFrame::from_bytes(b).map(|x| (GpsMsg::Nmea(x.0), x.1))
        } else {
            Err(Error::InvalidHeader)
        }
    }

    pub fn write_bytes(&self,b: &mut Vec<u8>){
        match *self{
            GpsMsg::Ubx(ref x) => {
                x.write_bytes(b);
            }
            Self::Nmea(ref x) => {
                b.extend_from_slice(x.as_bytes())
            }
            Self::Rtcm(ref x) => {
                b.extend_from_slice(x.as_bytes())
            }
        }
    }

    pub fn into_owned(self) -> GpsMsg<'static> {
        match self {
            GpsMsg::Nmea(x) => GpsMsg::Nmea(x.into_owned()),
            GpsMsg::Rtcm(x) => GpsMsg::Rtcm(x.into_owned()),
            GpsMsg::Ubx(x) => GpsMsg::Ubx(x),
        }
    }

    pub fn log(&self) {
        if let GpsMsg::Ubx(ubx::Msg::Inf(ref x)) = *self {
            match *x {
                ubx::Inf::Test(ref x) => info!("ubx test: {}", x),
                ubx::Inf::Debug(ref x) => debug!("ubx debug: {}", x),
                ubx::Inf::Error(ref x) => error!("ubx error: {}", x),
                ubx::Inf::Warning(ref x) => warn!("ubx warn: {}", x),
                ubx::Inf::Notice(ref x) => info!("ubx notice: {}", x),
            }
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(cmd::run())
}
