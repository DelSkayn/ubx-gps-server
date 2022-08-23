use crate::{
    parse::{self, Error, ParseData, Result},
    pread, pwrite,
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
    skipped: u32
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
            x => Err(Error::InvalidMsg(x)),
        }
    }
}
