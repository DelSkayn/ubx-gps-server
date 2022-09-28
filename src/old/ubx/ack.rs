use serde::{Deserialize, Serialize};

use crate::{
    parse::{read_u8, tag, Error, ParseData, Result, ResultExt},
    pwrite,
};

#[derive(Debug, Serialize, Deserialize)]
pub enum Ack {
    Ack { msg_id: u8, cls_id: u8 },
    Nak { msg_id: u8, cls_id: u8 },
}

impl Ack {
    pub fn write_bytes(&self, b: &mut Vec<u8>) {
        match *self {
            Self::Ack { msg_id, cls_id } => {
                pwrite!(b =>{
                    0x01u8, 2u16,cls_id,msg_id,
                });
            }
            Self::Nak { msg_id, cls_id } => {
                pwrite!(b =>{
                    0x00u8, 2u16,cls_id,msg_id,
                });
            }
        }
    }

    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, kind) = read_u8(b)?;
        let b = tag(b, 2u16).map_invalid(Error::InvalidLen)?;
        let (b, cls_id) = read_u8(b)?;
        let (b, msg_id) = read_u8(b)?;

        let this = match kind {
            0x00 => Ack::Nak { msg_id, cls_id },
            0x01 => Ack::Ack { msg_id, cls_id },
            x => return Err(Error::InvalidMsg(x)),
        };

        Ok((b, this))
    }
}
