use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::parse::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct RtcmFrame<'a> {
    kind: u16,
    data: Cow<'a, [u8]>,
}

impl<'a> RtcmFrame<'a> {
    const RTCM_PREAMBLE: u8 = 0xd3;

    pub fn valid_prefix(b: &[u8]) -> bool {
        !b.is_empty() && b[0] == Self::RTCM_PREAMBLE
    }

    pub fn from_bytes(b: &'a [u8]) -> Result<(Self, usize), Error> {
        if b.len() < 6 {
            return Err(Error::NotEnoughData);
        }

        if b[0] != Self::RTCM_PREAMBLE {
            return Err(Error::InvalidHeader);
        }

        let size = (((b[1] & 0b11) as usize) << 8) | b[2] as usize;
        let size = size + 6;
        let kind = ((b[3] as u16) << 4) | b[3] as u16 >> 4;

        if b.len() < size {
            return Err(Error::NotEnoughData);
        }

        if !crc24q_check(&b[..size]) {
            return Err(Error::InvalidChecksum);
        }

        let res = Self {
            data: b[..size].into(),
            kind,
        };

        Ok((res, size))
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_ref()
    }

    pub fn into_owned(self) -> RtcmFrame<'static> {
        RtcmFrame {
            data: self.data.into_owned().into(),
            kind: self.kind,
        }
    }
}

fn crc24q_check(d: &[u8]) -> bool {
    static CRC_TAB: [u32; 16] = [
        0x00000000, 0x01864CFB, 0x038AD50D, 0x020C99F6, 0x0793E6E1, 0x0615AA1A, 0x041933EC,
        0x059F7F17, 0x0FA18139, 0x0E27CDC2, 0x0C2B5434, 0x0DAD18CF, 0x083267D8, 0x09B42B23,
        0x0BB8B2D5, 0x0A3EFE2E,
    ];

    let mut crc = 0u32;

    for b in d.iter().copied() {
        crc ^= (b as u32) << 16;
        crc = (crc << 4) ^ CRC_TAB[((crc >> 20) & 0x0F) as usize];
        crc = (crc << 4) ^ CRC_TAB[((crc >> 20) & 0x0F) as usize];
    }

    (crc & 0xffffff) == 0
}
