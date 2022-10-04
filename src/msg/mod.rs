use std::io::Write;

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

pub mod ubx;
pub use ubx::{Ubx, UbxPoll};

pub mod rtcm;
pub use rtcm::Rtcm;

pub mod nmea;
pub use nmea::Nmea;

pub mod server;
pub use server::Server;

use crate::parse::{ParseData, ParseError, Result as ParseResult};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GpsMsg {
    Ubx(Ubx),
    UbxPoll(UbxPoll),
    Rtcm3(Rtcm),
    Nmea(Nmea),
    Server(Server),
}

impl GpsMsg {
    pub fn parse_gps_msg(b: &[u8]) -> ParseResult<(&[u8], Self)> {
        match Ubx::parse_read(b).map(|(a, b)| (a, GpsMsg::Ubx(b))) {
            Ok(x) => Ok(x),
            Err(e) => {
                let error = e.downcast_ref::<ParseError>();
                match error {
                    Some(ParseError::Invalid) | Some(ParseError::InvalidLen) => {
                        UbxPoll::parse_read(b).map(|(a, b)| (a, GpsMsg::UbxPoll(b)))
                    }
                    _ => bail!(e),
                }
            }
        }
    }
}

impl ParseData for GpsMsg {
    fn parse_read(b: &[u8]) -> ParseResult<(&[u8], Self)> {
        if Ubx::contains_prefix(b) {
            GpsMsg::parse_gps_msg(b).context("failed to parse ubx message")
        } else if Rtcm::contains_prefix(b) {
            Rtcm::parse_read(b)
                .map(|(a, b)| (a, GpsMsg::Rtcm3(b)))
                .context("failed to parse rtcm message")
        } else if Nmea::contains_prefix(b) {
            Nmea::parse_read(b)
                .map(|(a, b)| (a, GpsMsg::Nmea(b)))
                .context("failed to parse Nmea message")
        } else if Server::contains_prefix(b) {
            Server::parse_read(b)
                .map(|(a, b)| (a, GpsMsg::Server(b)))
                .context("failed parse server message")
        } else {
            bail!(ParseError::Invalid);
        }
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> ParseResult<()> {
        match *self {
            Self::Ubx(ref x) => x.parse_write(b),
            Self::UbxPoll(ref x) => x.parse_write(b),
            Self::Rtcm3(ref x) => x.parse_write(b),
            Self::Nmea(ref x) => x.parse_write(b),
            Self::Server(ref x) => x.parse_write(b),
        }
    }
}

impl GpsMsg {
    pub fn contains_prefix(b: &[u8]) -> bool {
        Ubx::contains_prefix(b)
            || Rtcm::contains_prefix(b)
            || Nmea::contains_prefix(b)
            || Server::contains_prefix(b)
    }

    pub fn message_usage(b: &[u8]) -> Option<usize> {
        Ubx::message_usage(b)
            .or_else(|| Rtcm::message_usage(b))
            .or_else(|| Nmea::message_usage(b))
            .or_else(|| Server::message_usage(b))
    }

    fn into_server(self) -> Result<Server, Self> {
        match self {
            GpsMsg::Server(x) => Ok(x),
            x => Err(x),
        }
    }
}
