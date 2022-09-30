use std::io::Write;

use crate::{
    impl_bitfield, impl_enum, impl_struct,
    parse::{ser_bitflags, Error, ParseData, Result},
};
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

mod values;
pub use values::{Value, ValueKey};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TMode {
    Disabled,
    SurvayIn,
    FixedMode,
    Reserved(u8),
}

impl Default for TMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct TModeFlags {
    pub lla: bool,
    pub mode: TMode,
}

impl ParseData for TModeFlags {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, d) = u16::parse_read(b)?;

        let mode = match (d & 0xff) as u8 {
            0 => TMode::Disabled,
            1 => TMode::SurvayIn,
            2 => TMode::FixedMode,
            x => TMode::Reserved(x),
        };

        let lla = (d >> 8 & 0b1) != 0;
        Ok((b, TModeFlags { lla, mode }))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        let mode = match self.mode {
            TMode::Disabled => 0,
            TMode::SurvayIn => 1,
            TMode::FixedMode => 2,
            TMode::Reserved(x) => x,
        };

        let data = ((self.lla as u16) << 8) | mode as u16;
        data.parse_write(b)
    }
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
    pub struct TMode3 {
        version: u8,
        res1: u8,
        flags: TModeFlags,
        ecefx_or_lat: i32,
        ecefy_or_lon: i32,
        ecefz_or_alt: i32,
        ecefx_or_lat_hp: i8,
        ecefy_or_lon_hp: i8,
        ecefz_or_alt_hp: i8,
        res2:u8,
        fixed_pos_acc: u32,
        svin_min_dur: u32,
        svin_accl_limit: u32,
        res3:[u8;8],
    }
}

impl_enum! {
    pub enum Layer: u8{
        Ram = 0,
        Bbr = 1,
        Flash = 2,
        Default = 7
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::Default
    }
}

impl_struct! {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
    pub struct ValGetRequest {
        layer: Layer,
        res1: [u8;2],
        keys: Vec<ValueKey>,
    }
}

impl_struct! {
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
    pub struct ValGetResponse{
        layer: Layer,
        res1: [u8;2],
        keys: Vec<Value>,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValGet {
    Request(ValGetRequest),
    Response(ValGetResponse),
}

impl ParseData for ValGet {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, len) = u16::parse_read(b)?;
        if b.len() < len as usize {
            return Err(Error::NotEnoughData);
        }
        let (b, rem) = b.split_at(len.into());
        let (b, version) = u8::parse_read(b)?;
        match version {
            0 => {
                let (_, res) = ValGetRequest::parse_read(b)?;
                Ok((rem, Self::Request(res)))
            }
            1 => {
                let (_, res) = ValGetResponse::parse_read(b)?;
                Ok((rem, Self::Response(res)))
            }
            _ => Err(Error::Invalid),
        }
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        let mut buffer = Vec::new();
        match *self {
            Self::Request(ref x) => {
                0u8.parse_write(&mut buffer).unwrap();
                x.parse_write(&mut buffer).unwrap();
            }
            Self::Response(ref x) => {
                1u8.parse_write(&mut buffer).unwrap();
                x.parse_write(&mut buffer).unwrap();
            }
        }
        let len = u16::try_from(buffer.len()).map_err(|_| Error::InvalidLen)?;
        len.parse_write(b)?;
        b.write_all(&buffer)?;
        Ok(())
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitLayer {
    Ram = 0b001,
    Bbr = 0b010,
    Flash = 0b100,
}

impl_bitfield!(BitLayer);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ValSet {
    pub version: u8,
    #[serde(with = "ser_bitflags")]
    pub layers: BitFlags<BitLayer>,
    pub res1: [u8; 2],
    pub values: Vec<Value>,
}

impl ParseData for ValSet {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, len) = u16::parse_read(b)?;
        if b.len() < len as usize {
            return Err(Error::NotEnoughData);
        }
        let (b, rem) = b.split_at(len.into());
        let (b, version) = u8::parse_read(b)?;
        if version != 0 {
            return Err(Error::Invalid);
        }
        let (b, layers) = ParseData::parse_read(b)?;
        let (b, res1) = ParseData::parse_read(b)?;
        let (_, values) = ParseData::parse_read(b)?;
        Ok((
            rem,
            ValSet {
                version,
                layers,
                res1,
                values,
            },
        ))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        let mut buffer = Vec::new();

        self.version.parse_write(&mut buffer).unwrap();
        self.layers.parse_write(&mut buffer).unwrap();
        self.res1.parse_write(&mut buffer).unwrap();
        self.values.parse_write(&mut buffer).unwrap();

        let len = u16::try_from(buffer.len()).map_err(|_| Error::InvalidLen)?;
        len.parse_write(b)?;
        b.write_all(&buffer)?;
        Ok(())
    }
}

impl_class! {
    pub enum Cfg: PollCfg {
        TMode3(TMode3)[40] = 0x71,
        ValGet(ValGet) = 0x8b,
        ValSet(ValSet) = 0x8a,
    }
}
