use crate::{
    pread, pread_struct, pwrite,
    parse::{self, Error, ParseData, Result, ResultExt}, impl_enum, impl_struct,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct IoBlock {
    rx_bytes: u32,
    tx_bytes: u32,
    parity_errs: u16,
    framing_errs: u16,
    overrun_errs: u16,
    break_cond: u16,
}

impl ParseData for IoBlock {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        pread!(b => {
        rx_bytes: u32,
        tx_bytes: u32,
        parity_errs: u16,
        framing_errs: u16,
        overrun_errs: u16,
        break_cond: u16,
        _res: [u8;4],
            });
        Ok((
            b,
            Self {
                rx_bytes,
                tx_bytes,
                parity_errs,
                framing_errs,
                overrun_errs,
                break_cond,
            },
        ))
    }

    fn parse_write(self, b: &mut Vec<u8>) {
        pwrite!(b => {
            self.rx_bytes,
            self.tx_bytes,
            self.parity_errs,
            self.framing_errs,
            self.overrun_errs,
            self.break_cond,
        });
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommsBlock {
    port_id: u16,
    tx_pending: u16,
    tx_bytes: u32,
    tx_usage: u8,
    tx_peak_usage: u8,
    rx_pending: u16,
    rx_bytes: u32,
    rx_usage: u8,
    rx_peak_usage: u8,
    overrun_errs: u16,
    msgs: [u16; 4],
    skipped: u32,
}

impl_enum! {
    pub enum AntStatus: u8{
        Init = 0x00,
        DontKnow = 0x01,
        Ok = 0x02,
        Short = 0x03,
        Open = 0x04
    }
}


impl_enum! {
    pub enum AntPower: u8{
        Off = 0x00,
        On = 0x01,
        DontKnow = 0x02
    }
}
impl_struct!{
    #[derive(Debug, Serialize, Deserialize)]
    pub struct RfBlock{
        block_id: u8,
        flags: u8,
        ant_status: AntStatus,
        ant_power: AntPower,
        pos_status: u32,
        res1: [u8; 4],
        agc_cnt: u16,
        jam_ind: u16,
        ofs_i: i8,
        mag_i: u8,
        ofs_q: i8,
        mag_q: u8,
        res2: [u8; 3],
    }
}

impl ParseData for CommsBlock {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        pread!(b => {
        port_id: u16,
        tx_pending: u16,
        tx_bytes: u32,
        tx_usage: u8,
        tx_peak_usage: u8,
        rx_pending: u16,
        rx_bytes: u32,
        rx_usage: u8,
        rx_peak_usage: u8,
        overrun_errs: u16,
        msgs: [u16;4],
        _res: [u8;8],
        skipped: u32,
        });
        Ok((
            b,
            Self {
                port_id,
                tx_pending,
                tx_bytes,
                tx_usage,
                tx_peak_usage,
                rx_pending,
                rx_bytes,
                rx_usage,
                rx_peak_usage,
                overrun_errs,
                msgs,
                skipped,
            },
        ))
    }

    fn parse_write(self, b: &mut Vec<u8>) {
        pwrite!(b => {
            self.port_id,
            self.tx_pending,
            self.tx_bytes,
            self.tx_usage,
            self.tx_peak_usage,
            self.rx_pending,
            self.rx_bytes,
            self.rx_usage,
            self.rx_peak_usage,
            self.overrun_errs,
            self.msgs,
            [0u8;8],
            0u32,
        });
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Mon {
    Io(Vec<IoBlock>),
    Comms {
        version: u8,
        n_ports: u8,
        tx_errors: u8,
        prot_ids: [u8; 4],
        blocks: Vec<CommsBlock>,
    },
    Msgpp {
        msg1: [u16; 8],
        msg2: [u16; 8],
        msg3: [u16; 8],
        msg4: [u16; 8],
        msg5: [u16; 8],
        msg6: [u16; 8],
        skipped: [u32; 6],
    },
    Rf{
        version:u8,
        n_blocks:u8,
        res: [u8;2],
        blocks: Vec<RfBlock>,
    }
}

impl Mon {
    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, msg) = u8::parse_read(b)?;
        match msg {
            0x02 => {
                let (b, len) = u16::parse_read(b)?;

                if len % 20 != 0 {
                    return Err(Error::Invalid);
                }

                let cnt = len / 20;

                let (b, blocks) = parse::collect::<IoBlock>(b, cnt as usize)?;
                Ok((b, Mon::Io(blocks)))
            }
            0x36 => {
                let (b, len) = u16::parse_read(b)?;
                pread!(b => {
                    version: u8,
                    n_ports: u8,
                    tx_errors: u8,
                    _res: u8,
                    prot_ids: [u8;4],
                });
                if len as usize != 8 + 40 * n_ports as usize {
                    return Err(Error::Invalid);
                }
                let (b, blocks) = parse::collect::<CommsBlock>(b, n_ports as usize)?;
                Ok((
                    b,
                    Self::Comms {
                        version,
                        n_ports,
                        tx_errors,
                        prot_ids,
                        blocks,
                    },
                ))
            }
            0x06 => {
                parse::tag(b, 120u16).map_invalid(Error::InvalidLen)?;
                let res = pread_struct!(b => Self::Msgpp{
                    msg1: [u16; 8],
                    msg2: [u16; 8],
                    msg3: [u16; 8],
                    msg4: [u16; 8],
                    msg5: [u16; 8],
                    msg6: [u16; 8],
                    skipped: [u32; 6],
                });
                Ok(res)
            }
            0x38 => {
                let (b,len) = u16::parse_read(b)?;
                pread!(b => {
                    version: u8,
                    n_blocks: u8,
                    res: [u8;2],
                });
                let blocks = Vec::with_capacity(n_blocks as usize);
                let mut loop_b = b;
                for n in 0..n_blocks{
                    let (b,block) = RfBlock::parse_read(loop_b)?;
                    loop_b = b;
                    blocks.push(block);
                }
                Ok((loop_b,Self::Rf { version, n_blocks, res, blocks }))
            }
            x => Err(Error::InvalidMsg(x)),
        }
    }
}
