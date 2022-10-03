use crate::{
    impl_enum,
    parse::{self, Error, ParseData, Result},
};
use serde::{Deserialize, Serialize};

impl_enum! {
pub enum ServerMsg: u8 {
    ResetPort = 0,
    Quit = 1
}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Server {
    pub msg: ServerMsg,
}

impl Server {
    pub const PREFIX: u8 = b'%';

    pub fn contains_prefix(b: &[u8]) -> bool {
        !b.is_empty() && b[0] == Self::PREFIX
    }

    pub fn message_usage(b: &[u8]) -> Option<usize> {
        if !Self::contains_prefix(b) {
            return None;
        }

        if b.len() < 2 {
            return None;
        }
        Some(2)
    }
}

impl ParseData for Server {
    fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8], Self)> {
        let b = parse::tag(b, Server::PREFIX)?;
        ServerMsg::parse_read(b).map(|(a, msg)| (a, Server { msg }))
    }

    fn parse_write<W: std::io::Write>(&self, b: &mut W) -> crate::parse::Result<()> {
        Server::PREFIX.parse_write(b)?;
        self.msg.parse_write(b)
    }
}
