use serde::{Deserialize, Serialize};

use crate::{
    impl_struct,
    parse::{self, ParseData},
    pread,
};

impl_struct! {
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct CommBlock {
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
    res2: [u8; 8],
    skipped: u32,
}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comms {
    pub version: u8,
    pub n_ports: u8,
    pub tx_errors: u8,
    pub res1: u8,
    pub prot_ids: [u8; 4],
    pub blocks: Vec<CommBlock>,
}

impl ParseData for Comms {
    fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8], Self)> {
        pread!(b => {
            _len: u16,
            version: u8,
            n_ports: u8,
            tx_errors: u8,
            res1: u8,
            prot_ids: [u8; 4],
        });
        let (b, blocks) = parse::collect(b, n_ports as usize)?;
        Ok((
            b,
            Self {
                version,
                n_ports,
                tx_errors,
                res1,
                prot_ids,
                blocks,
            },
        ))
    }

    fn parse_write<W: std::io::Write>(&self, b: &mut W) -> crate::parse::Result<()> {
        let len =
            u16::try_from(self.blocks.len() * 40 + 8).map_err(|_| crate::parse::Error::Invalid)?;
        len.parse_write(b)?;
        self.version.parse_write(b)?;
        self.n_ports.parse_write(b)?;
        self.tx_errors.parse_write(b)?;
        self.res1.parse_write(b)?;
        self.prot_ids.parse_write(b)?;
        self.blocks.parse_write(b)?;
        Ok(())
    }

    fn parse_to_vec(&self) -> crate::parse::Result<Vec<u8>> {
        let mut res = Vec::new();
        self.parse_write(&mut res)?;
        Ok(res)
    }
}

impl_struct! {
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Msgpp{
    msg1:[u16; 8],
    msg2:[u16; 8],
    msg3:[u16; 8],
    msg4:[u16; 8],
    msg5:[u16; 8],
    skipped:[u16; 6],
}
}

impl_class! {
    pub enum Mon: PollMon{
        Msgpp(Msgpp)[120] = 0x06,
        Comms(Comms) = 0x36,
    }
}
