use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

use crate::{impl_bitfield, impl_struct, parse::ParseData};

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RtcmFlags {
    CrcFailed = 0b1,
}

impl_bitfield!(RtcmFlags);

impl_struct! {
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rtcm {
    version: u8,
    flags: BitFlags<RtcmFlags>,
    res1: [u8; 2],
    ref_stations: u16,
    msg_type: u16,
}
}

impl_class! {
    pub enum Rxm: PollRxm{
        Rtcm(Rtcm)[0x8] = 0x32,
    }
}
