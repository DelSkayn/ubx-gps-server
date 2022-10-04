use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::parse::{self, ParseData, ParseError, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Nmea(String);

impl Nmea {
    const NMEA_PREAMBLE: u8 = b'$';

    pub fn contains_prefix(b: &[u8]) -> bool {
        !b.is_empty() && b[0] == Self::NMEA_PREAMBLE
    }

    pub fn message_usage(b: &[u8]) -> Option<usize> {
        if !Self::contains_prefix(b) {
            return None;
        }

        let mut iter = b.iter().copied().enumerate();
        while let Some((_, b)) = iter.next() {
            if b == b'\r' {
                if let Some((idx, b'\n')) = iter.next() {
                    return Some(idx + 1);
                }
            }
        }
        None
    }
}

impl ParseData for Nmea {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let mut b = parse::tag(b, Self::NMEA_PREAMBLE)?;
        let mut next;
        let mut res = String::new();
        res.push('$');
        loop {
            (b, next) = u8::parse_read(b)?;
            res.push(char::try_from(next).map_err(|_| ParseError::Invalid)?);
            if next == b'\r' {
                (b, next) = u8::parse_read(b)?;
                res.push(char::try_from(next).map_err(|_| ParseError::Invalid)?);
                if next == b'\n' {
                    break;
                }
            }
        }
        Ok((b, Self(res)))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        for s in self.0.as_bytes() {
            s.parse_write(b)?;
        }
        Ok(())
    }
}
