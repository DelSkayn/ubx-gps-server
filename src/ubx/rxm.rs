use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

use crate::{
    impl_bitfield,
    parse::{self, Error, ParseData, Result, ResultExt},
    pread,
};

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flags {
    CrcFailed = 0b1,
}

impl_bitfield!(Flags);

#[derive(Debug, Serialize, Deserialize)]
pub enum Rxm {
    Rtcm {
        version: u8,
        flags: BitFlags<Flags>,
        sub_type: u16,
        ref_station: u16,
        msg_type: u16,
    },
}

impl Rxm {
    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, msg) = u8::parse_read(b)?;
        match msg {
            0x32 => {
                parse::tag(b, 8u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    version: u8,
                    flags: BitFlags<Flags>,
                    sub_type: u16,
                    ref_station: u16,
                    msg_type: u16,
                });

                Ok((
                    b,
                    Rxm::Rtcm {
                        version,
                        flags,
                        sub_type,
                        ref_station,
                        msg_type,
                    },
                ))
            }
            x => Err(Error::InvalidMsg(x)),
        }
    }
}
