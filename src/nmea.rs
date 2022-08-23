use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::parse::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct NmeaFrame<'a>(Cow<'a, str>);

impl<'a> NmeaFrame<'a> {
    const NMEA_PREAMBLE: u8 = b'$';

    pub fn valid_prefix(b: &[u8]) -> bool {
        !b.is_empty() && b[0] == Self::NMEA_PREAMBLE
    }

    pub fn from_bytes(b: &'a [u8]) -> Result<(Self, usize), Error> {
        if !Self::valid_prefix(b) {
            return Err(Error::InvalidHeader);
        }

        let mut idx = 0;
        while b.len() > idx + 1 {
            if b[idx] == b'\r' {
                if b[idx + 1] == b'\n' {
                    let data = &b[..=idx + 1];
                    let data = std::str::from_utf8(data).map_err(|_| Error::Invalid)?;
                    return Ok((NmeaFrame(Cow::from(data)), data.len()));
                } else {
                    idx += 2;
                }
            } else {
                idx += 1;
            }
        }
        Err(Error::NotEnoughData)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref().as_bytes()
    }

    pub fn into_owned(self) -> NmeaFrame<'static> {
        NmeaFrame(Cow::from(self.0.into_owned()))
    }
}
