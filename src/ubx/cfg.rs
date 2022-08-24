use std::mem;

use crate::{
    impl_bitfield, impl_enum,
    parse::{self, read_u16, read_u8, ser_bitflags, tag, Error, ParseData, Result, ResultExt},
    pread, pwrite,
};
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

pub mod value;
pub use self::value::ValueKey;
pub use value::Value;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StopBits {
    Bit1 = 0b00,
    Bit1_5 = 0b01,
    Bit2 = 0b10,
    Bit0_5 = 0b11,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Parity {
    Even,
    Odd,
    None,
    Reserved,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CharLen {
    Bit5 = 0b00,
    Bit6 = 0b01,
    Bit7 = 0b10,
    Bit8 = 0b11,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Mode {
    pub char_len: CharLen,
    pub parity: Parity,
    pub stop_bits: StopBits,
}

impl Mode {
    pub fn from_u32(v: u32) -> Self {
        let char_len = v >> 6 & 0b11;
        let char_len = match char_len {
            0b00 => CharLen::Bit5,
            0b01 => CharLen::Bit6,
            0b10 => CharLen::Bit7,
            0b11 => CharLen::Bit8,
            _ => unreachable!(),
        };

        let parity = v >> 9 & 0b111;
        let parity = match parity {
            0b000 => Parity::Even,
            0b001 => Parity::Odd,
            0b100 | 0b101 => Parity::None,
            _ => Parity::Reserved,
        };

        let stop_bits = v >> 12 & 0b11;
        let stop_bits = match stop_bits {
            0b00 => StopBits::Bit1,
            0b01 => StopBits::Bit1_5,
            0b10 => StopBits::Bit2,
            0b11 => StopBits::Bit0_5,
            _ => unreachable!(),
        };

        Mode {
            char_len,
            parity,
            stop_bits,
        }
    }

    pub fn to_u32(self) -> u32 {
        let mut res = 0u32;
        res |= (self.stop_bits as u8 as u32) << 12;
        res |= match self.parity {
            Parity::Even => 0b000,
            Parity::Odd => 0b001,
            Parity::None => 0b100,
            Parity::Reserved => 0b010,
        } << 9;
        res |= (self.char_len as u8 as u32) << 6;
        res
    }
}

#[bitflags]
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtoMask {
    Ubx = 0b000001,
    Nmea = 0b000010,
    Rtcm = 0b000100,
    Rtcm3 = 0b100000,
}

impl_bitfield!(ProtoMask);

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitLayer {
    Ram = 0b001,
    Bbr = 0b010,
    Flash = 0b100,
}

impl_bitfield!(BitLayer);

impl_enum! {
    pub enum Layer: u8{
        Ram = 0,
        Bbr = 1,
        Flash = 2,
        Default = 7
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TMode {
    Disabled,
    SurvayIn,
    FixedMode,
    Reserved(u8),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TModeFlags {
    lla: bool,
    mode: TMode,
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

    fn parse_write(self, b: &mut Vec<u8>) {
        let mode = match self.mode {
            TMode::Disabled => 0,
            TMode::SurvayIn => 1,
            TMode::FixedMode => 2,
            TMode::Reserved(x) => x,
        };

        let data = ((self.lla as u16) << 8) | mode as u16;
        data.parse_write(b);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Cfg {
    Ant {
        flags: u16,
        pins: u16,
    },
    Cfg {
        clear_mask: u32,
        save_mask: u32,
        load_mask: u32,
        dev_mask: Option<u8>,
    },
    PrtPoll {
        port_id: u8,
    },
    PrtUart {
        port_id: u8,
        tx_ready: u16,
        mode: Mode,
        baud_rate: u32,
        #[serde(with = "ser_bitflags")]
        in_proto: BitFlags<ProtoMask>,
        #[serde(with = "ser_bitflags")]
        out_proto: BitFlags<ProtoMask>,
        flags: u16,
    },
    PrtUsb {
        port_id: u8,
        tx_ready: u16,
        #[serde(with = "ser_bitflags")]
        in_proto: BitFlags<ProtoMask>,
        #[serde(with = "ser_bitflags")]
        out_proto: BitFlags<ProtoMask>,
    },
    ValGetRes {
        version: u8,
        layer: Layer,
        values: Vec<Value>,
    },
    ValGetReq {
        version: u8,
        layer: Layer,
        values: Vec<ValueKey>,
    },
    ValSet {
        version: u8,
        #[serde(with = "ser_bitflags")]
        layer: BitFlags<BitLayer>,
        values: Vec<Value>,
    },
    ValDel {
        version: u8,
        #[serde(with = "ser_bitflags")]
        layer: BitFlags<BitLayer>,
        values: Vec<ValueKey>,
    },
    TMode3 {
        version: u8,
        flags: TModeFlags,
        ecefx_or_lat: i32,
        ecefy_or_lon: i32,
        ecefz_or_alt: i32,
        ecefx_or_lat_hp: i8,
        ecefy_or_lon_hp: i8,
        ecefz_or_alt_hp: i8,
        fixed_pos_acc: u32,
        svin_min_dur: u32,
        svin_accl_limit: u32,
    },
}

impl Cfg {
    pub fn write_bytes(&self, b: &mut Vec<u8>) {
        match *self {
            Cfg::Ant { flags, pins } => {
                pwrite!(b => {
                0x13u8,
                4u16,
                flags,
                pins,
                });
            }
            Cfg::Cfg {
                clear_mask,
                save_mask,
                load_mask,
                dev_mask,
            } => {
                pwrite!(b => {
                    0x09u8,
                    dev_mask.map(|_| 13u16).unwrap_or(12u16),
                    clear_mask,
                    save_mask,
                    load_mask,
                });
                if let Some(dev_mask) = dev_mask {
                    b.push(dev_mask);
                }
            }
            Cfg::PrtPoll { port_id } => {
                pwrite!(b => {
                    0x00u8,
                    1u16,
                    port_id,
                });
            }
            Cfg::ValGetReq {
                version,
                layer,
                ref values,
            } => {
                let len = u16::try_from(values.len() * mem::size_of::<u32>() + 4).unwrap();
                pwrite!(b => {
                    0x8Bu8,
                    len,
                    version,
                    layer,
                    [0u8;2],
                });
                for v in values {
                    v.write_bytes(b);
                }
            }
            Cfg::ValGetRes { .. } => {
                panic!("ValGetRes should not be written to a buffer, it should only be recieved");
            }
            Cfg::ValSet {
                version,
                layer,
                ref values,
            } => {
                let len: usize = values.iter().map(|x| x.size()).sum();
                let len = u16::try_from(len + 4).unwrap();
                pwrite!(b => {
                    0x8au8,
                    len,
                    version,
                    layer,
                    0u16,
                });
                for v in values {
                    v.write_bytes(b);
                }
            }
            Cfg::ValDel {
                version,
                layer,
                ref values,
            } => {
                let len: usize = values.len() * 4;
                let len = u16::try_from(len + 4).unwrap();
                pwrite!(b => {
                    0x8cu8,
                    len,
                    version,
                    layer,
                    0x0u16,
                });
                for v in values {
                    v.write_bytes(b);
                }
            }
            Cfg::PrtUart {
                port_id,
                tx_ready,
                mode,
                baud_rate,
                in_proto,
                out_proto,
                flags,
            } => {
                pwrite!(b => {
                    0x00u8,
                    20u16,
                    port_id,
                    0u8,
                    tx_ready,
                    mode.to_u32(),
                    baud_rate,
                    in_proto.bits(),
                    out_proto.bits(),
                    flags,
                    0u16,
                });
            }
            Cfg::PrtUsb {
                port_id,
                tx_ready,
                in_proto,
                out_proto,
            } => {
                pwrite!(b => {
                    0x0u8,
                    20u16,
                    port_id,
                    0u8,
                    tx_ready,
                    [0u8;8],
                    in_proto,
                    out_proto,
                    [0u8;2],
                    [0u8;2],
                });
            }

            Cfg::TMode3 {
                version,
                flags,
                ecefx_or_lat,
                ecefy_or_lon,
                ecefz_or_alt,
                ecefx_or_lat_hp,
                ecefy_or_lon_hp,
                ecefz_or_alt_hp,
                fixed_pos_acc,
                svin_min_dur,
                svin_accl_limit,
            } => {
                pwrite!(b => {
                    40u16,
                    version,
                    0u8,
                    flags,
                    ecefx_or_lat,
                    ecefy_or_lon,
                    ecefz_or_alt,
                    ecefx_or_lat_hp,
                    ecefy_or_lon_hp,
                    ecefz_or_alt_hp,
                    0u8,
                    fixed_pos_acc,
                    svin_min_dur,
                    svin_accl_limit,
                    [0u8;8],
                });
            }
        }
    }

    pub fn id(&self) -> u8 {
        match *self {
            Self::Ant { .. } => 0x13,
            Self::Cfg { .. } => 0x09,
            Self::PrtPoll { .. } => 0x0,
            Self::ValGetRes { .. } => 0x8b,
            Self::ValGetReq { .. } => 0x8b,
            Self::ValSet { .. } => 0x8a,
            Self::ValDel { .. } => 0x8c,
            Self::PrtUart { .. } => 0x0,
            Self::PrtUsb { .. } => 0x0,
            Self::TMode3 { .. } => 0x71,
        }
    }

    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, kind) = read_u8(b)?;

        match kind {
            0x13 => {
                let b = tag(b, 4u16).map_invalid(Error::InvalidLen)?;
                pread!(b =>{
                    flags: u16,
                    pins: u16,
                });
                Ok((b, Cfg::Ant { flags, pins }))
            }
            0x9 => {
                let (b, len) = read_u16(b)?;
                if !(len == 12 || len == 13) {
                    return Err(Error::InvalidLen);
                }
                pread!(b =>{
                    clear_mask: u32,
                    save_mask: u32,
                    load_mask: u32,
                });
                if len == 13 {
                    let (b, dev_mask) = read_u8(b)?;
                    Ok((
                        b,
                        Cfg::Cfg {
                            clear_mask,
                            save_mask,
                            load_mask,
                            dev_mask: Some(dev_mask),
                        },
                    ))
                } else {
                    Ok((
                        b,
                        Cfg::Cfg {
                            clear_mask,
                            save_mask,
                            load_mask,
                            dev_mask: None,
                        },
                    ))
                }
            }
            0x8b => {
                let (b, len) = read_u16(b)?;
                if b.len() < len as usize {
                    return Err(Error::NotEnoughData);
                }
                let (b, rem) = b.split_at(len as usize);
                pread!(b => {
                    version: u8,
                    layer: Layer,
                    _res:u16,
                });

                let mut b = b;
                let mut values = Vec::new();
                while !b.is_empty() {
                    match value::Value::from_bytes(b) {
                        Ok((nb, val)) => {
                            values.push(val);
                            b = nb
                        }
                        Err(Error::NotEnoughData) => return Err(Error::Invalid),
                        Err(x) => return Err(x),
                    }
                }
                Ok((
                    rem,
                    Cfg::ValGetRes {
                        version,
                        layer,
                        values,
                    },
                ))
            }
            0x00 => {
                let (b, len) = read_u16(b)?;
                match len {
                    1 => {
                        let (b, port_id) = read_u8(b)?;
                        Ok((b, Cfg::PrtPoll { port_id }))
                    }
                    20 => {
                        pread!(b => {
                            port_id: u8,
                            _res: u8,
                            tx_ready: u16,
                            mode:u32,
                            baud_rate: u32,
                            in_proto: BitFlags<ProtoMask>,
                            out_proto: BitFlags<ProtoMask>,
                            flags: u16,
                            _res2: u16,
                        });
                        let mode = Mode::from_u32(mode);
                        let res = Cfg::PrtUart {
                            port_id,
                            tx_ready,
                            mode,
                            baud_rate,
                            in_proto,
                            out_proto,
                            flags,
                        };
                        Ok((b, res))
                    }
                    _ => Err(Error::InvalidLen),
                }
            }
            0x71 => {
                parse::tag(b, 40u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    version: u8,
                    _res: u8,
                    flags: TModeFlags,
                    ecefx_or_lat: i32,
                    ecefy_or_lon: i32,
                    ecefz_or_alt: i32,
                    ecefx_or_lat_hp: i8,
                    ecefy_or_lon_hp: i8,
                    ecefz_or_alt_hp: i8,
                    _res: u8,
                    fixed_pos_acc: u32,
                    svin_min_dur: u32,
                    svin_accl_limit: u32,
                    _res: [u8;8],
                });
                Ok((
                    b,
                    Cfg::TMode3 {
                        version,
                        flags,
                        ecefx_or_lat,
                        ecefy_or_lon,
                        ecefz_or_alt,
                        ecefx_or_lat_hp,
                        ecefy_or_lon_hp,
                        ecefz_or_alt_hp,
                        fixed_pos_acc,
                        svin_min_dur,
                        svin_accl_limit,
                    },
                ))
            }
            x => Err(Error::InvalidMsg(x)),
        }
    }
}
