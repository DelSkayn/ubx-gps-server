mod ack;
pub use ack::Ack;

pub mod cfg;
pub use cfg::Cfg;

pub mod nav;
use log::error;
pub use nav::Nav;

pub mod inf;
pub use inf::Inf;

pub mod mon;
pub use mon::Mon;

pub mod rxm;
pub use rxm::Rxm;

use serde::{Deserialize, Serialize};

use crate::parse::{read_u8, tag, Error, Offset, Result, ResultExt};

#[derive(Debug, Serialize, Deserialize)]
pub enum Msg {
    Nav(Nav),
    Rxm(Rxm),
    Inf(Inf),
    Ack(Ack),
    Cfg(Cfg),
    Upd,
    Mon(Mon),
    Tim,
    Mga,
    Log,
    Sec,
}

impl Msg {
    const SYNC_CHAR_1: u8 = 0xb5;
    const SYNC_CHAR_2: u8 = 0x62;

    fn checksum(data: &[u8]) -> (u8, u8) {
        let mut a = 0u8;
        let mut b = 0u8;
        for byte in data {
            a = a.wrapping_add(*byte);
            b = b.wrapping_add(a);
        }
        (a, b)
    }

    pub fn valid_prefix(b: &[u8]) -> bool {
        b.len() > 1 && b[0] == Self::SYNC_CHAR_1 && b[1] == Self::SYNC_CHAR_2
    }

    pub fn write_bytes(&self, buffer: &mut Vec<u8>) {
        buffer.push(Self::SYNC_CHAR_1);
        buffer.push(Self::SYNC_CHAR_2);

        let len = buffer.len();

        match *self {
            Self::Nav(ref x) => {
                buffer.push(0x01);
                x.write_bytes(buffer);
            }
            Self::Rxm(_) => {
                buffer.push(0x02);
                todo!();
            }
            Self::Inf(ref x) => {
                buffer.push(0x04);
                x.write_bytes(buffer);
            }
            Self::Ack(ref x) => {
                buffer.push(0x05);
                x.write_bytes(buffer);
            }
            Self::Cfg(ref x) => {
                buffer.push(0x06);
                x.write_bytes(buffer);
            }
            Self::Upd => {
                buffer.push(0x09);
                todo!();
            }
            Self::Mon(_) => {
                buffer.push(0x0A);
                todo!();
            }
            Self::Tim => {
                buffer.push(0x0D);
                todo!();
            }
            Self::Mga => {
                buffer.push(0x13);
                todo!();
            }
            Self::Log => {
                buffer.push(0x21);
                todo!();
            }
            Self::Sec => {
                buffer.push(0x27);
                todo!();
            }
        }

        let (ck_a, ck_b) = Self::checksum(&buffer[len..]);
        buffer.push(ck_a);
        buffer.push(ck_b);
    }

    pub fn from_bytes(b: &[u8]) -> Result<(Self, usize)> {
        let b = tag(b, Self::SYNC_CHAR_1).map_invalid(Error::InvalidHeader)?;
        let b = tag(b, Self::SYNC_CHAR_2).map_invalid(Error::InvalidHeader)?;

        let before = b;

        let (b, class) = read_u8(b)?;

        let (b, this) = match class {
            0x01 => Nav::from_bytes(b).map(|(a, b)| (a, Self::Nav(b)))?,
            0x02 => Rxm::from_bytes(b).map(|(a, b)| (a, Self::Rxm(b)))?,
            0x04 => Inf::from_bytes(b).map(|(a, b)| (a, Self::Inf(b)))?,
            0x05 => Ack::from_bytes(b).map(|(a, b)| (a, Self::Ack(b)))?,
            0x06 => Cfg::from_bytes(b).map(|(a, b)| (a, Self::Cfg(b)))?,
            0x09 => return Ok((Self::Upd, 0)),
            0x0A => Mon::from_bytes(b).map(|(a, b)| (a, Self::Mon(b)))?,
            0x0D => return Ok((Self::Tim, 0)),
            0x13 => return Ok((Self::Mga, 0)),
            0x21 => return Ok((Self::Log, 0)),
            0x27 => return Ok((Self::Sec, 0)),
            x => return Err(Error::InvalidClass(x)),
        };

        let payload_len = before.offset(b);
        let payload = &before[..payload_len];
        let (ck_a, ck_b) = Self::checksum(payload);

        let (b, checksum_a) = read_u8(b)?;
        let (_, checksum_b) = read_u8(b)?;

        if ck_a != checksum_a || ck_b != checksum_b {
            error!(
                "invalid checksum {} != {} || {} != {}\n{:?}",
                ck_a, checksum_a, ck_b, checksum_b, this
            );
            return Err(Error::InvalidChecksum);
        }

        Ok((this, payload_len + 4))
    }
}
