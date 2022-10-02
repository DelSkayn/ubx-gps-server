use crate::parse::{self, ParseData};
use serde::{Deserialize, Serialize};

macro_rules! impl_inf {
    ($($name:ident),*) => {$(
        #[derive(Serialize, Deserialize, Clone, Debug)]
        pub struct $name(String);

        impl ParseData for $name {
            fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8], Self)> {
                let (b, len) = u16::parse_read(b)?;
                let (b, str) = parse::collect::<u8>(b, len as usize)?;
                let res = String::from_utf8(str).map_err(|_| crate::parse::Error::Invalid)?;
                Ok((b, $name(res)))
            }

            fn parse_write<W: std::io::Write>(&self, b: &mut W) -> crate::parse::Result<()> {
                let len = u16::try_from(self.0.len()).map_err(|_| crate::parse::Error::Invalid)?;
                len.parse_write(b)?;
                for byte in self.0.as_bytes() {
                    byte.parse_write(b)?;
                }
                Ok(())
            }
        }
        )*};
}

impl_inf!(Debug, Error, Notice, Test, Warning);

impl_class! {
    pub enum Inf: PollInf{
        Debug(Debug) = 0x04,
        Error(Error) = 0x00,
        Notice(Notice) = 0x02,
        Test(Test) = 0x03,
        Warning(Warning) = 0x04,
    }
}
