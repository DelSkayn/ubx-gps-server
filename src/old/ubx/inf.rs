use serde::{Deserialize, Serialize};

use crate::{
    parse::{Error, ParseData, Result},
    pwrite,
};

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Inf {
    Debug(String),
    Error(String),
    Notice(String),
    Test(String),
    Warning(String),
}

impl Inf {
    pub fn id(&self) -> u8 {
        match *self {
            Self::Debug(_) => 0x04,
            Self::Error(_) => 0x00,
            Self::Notice(_) => 0x02,
            Self::Test(_) => 0x03,
            Self::Warning(_) => 0x01,
        }
    }

    pub fn write_bytes(&self, b: &mut Vec<u8>) {
        let id = self.id();
        match *self {
            Self::Debug(ref x)
            | Self::Error(ref x)
            | Self::Notice(ref x)
            | Self::Test(ref x)
            | Self::Warning(ref x) => {
                pwrite!(b => {
                    id,
                    x.len() as u16,
                });
                b.extend_from_slice(x.as_bytes())
            }
        }
    }

    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, id) = u8::parse_read(b)?;
        let (b, len) = u16::parse_read(b)?;

        if len as usize > b.len() {
            return Err(Error::NotEnoughData);
        }

        let (str, b) = b.split_at(len as usize);
        let str = std::str::from_utf8(str).map_err(|_| Error::Invalid)?;
        let res = match id {
            0x0 => Self::Error(str.to_string()),
            0x1 => Self::Warning(str.to_string()),
            0x2 => Self::Notice(str.to_string()),
            0x3 => Self::Test(str.to_string()),
            0x4 => Self::Debug(str.to_string()),
            _ => return Err(Error::Invalid),
        };

        Ok((b, res))
    }
}
