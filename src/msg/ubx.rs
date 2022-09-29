use crate::parse::{self, Error, ParseData, Result, ResultExt};
use serde::{Deserialize, Serialize};
use std::io::Write;

macro_rules! impl_class {
    (pub enum $class:ident: $pollname:ident{
        $($var:ident( $t:ty )$([$len:expr])* = $e:expr,)*
    }) => {

        #[derive(Debug,serde::Serialize,serde::Deserialize, Clone)]
        pub enum $class {
            $($var($t),)*
            Unknown{ id: u8, payload:Vec<u8> }
        }

        #[derive(Debug,serde::Serialize,serde::Deserialize, Clone, Copy, Eq,PartialEq)]
        pub enum $pollname{
            $($var,)*
        }

        impl crate::parse::ParseData for $class{
            fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8],Self)>{
                #[allow(unused_imports)]
                use crate::parse::ResultExt;
                let (b,msg) = u8::parse_read(b)?;
                match msg{
                    $($e => {
                        $(let b = crate::parse::tag(b,($len as u16)).map_invalid(crate::parse::Error::InvalidLen)?;)*
                        let (b,res) = <$t>::parse_read(b)?;
                        Ok((b,Self::$var(res)))
                    })*
                    x => {
                        let (b,len) = u16::parse_read(b)?;
                        let (b,payload) = crate::parse::collect(b,len as usize)?;
                        Ok((b,Self::Unknown{
                            id: x,
                            payload,
                        }))
                    }
                }
            }

            fn parse_write<W: std::io::Write>(&self, w: &mut W) -> crate::parse::Result<()>{
                match *self{
                    $(Self::$var(ref x) => {
                        ($e as u8).parse_write(w)?;
                        x.parse_write(w)
                    })*
                    Self::Unknown{ id, ref payload } => {
                        id.parse_write(w)?;
                        (payload.len() as u16).parse_write(w)?;
                        payload.parse_write(w)
                    }
                }
            }
        }

        impl crate::parse::ParseData for $pollname{
            fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8],Self)>{
                let (b,msg) = u8::parse_read(b)?;
                match msg{
                    $($e => {
                        Ok((b,Self::$var))
                    })*
                    _ => Err(crate::parse::Error::Invalid),
                }
            }

            fn parse_write<W: std::io::Write>(&self, w: &mut W) -> crate::parse::Result<()>{
                match *self{
                    $(Self::$var => {
                        ($e as u8).parse_write(w)?;
                        0u16.parse_write(w)
                    })*
                }
            }
        }
    };
}

pub mod cfg;
use cfg::Cfg;

pub mod nav;
use nav::Nav;

pub mod ack;
use ack::Ack;

pub mod mon;
use mon::Mon;

macro_rules! impl_ubx {
    (pub enum Ubx{
        $($var:ident($t:ty) = $class_id:expr,)*
    }) => {

        #[derive(Debug,Serialize,Deserialize, Clone)]
        pub enum Ubx{
            $(
                $var($t),
            )*
            Unknown{
                class: u8,
                msg: u8,
                len: u16,
                payload: Vec<u8>,
                ck_a: u8,
                ck_b: u8,
            }
        }

        impl Ubx{
            fn checksum(data: &[u8]) -> (u8, u8) {
                let mut a = 0u8;
                let mut b = 0u8;
                for byte in data {
                    a = a.wrapping_add(*byte);
                    b = b.wrapping_add(a);
                }
                (a, b)
            }
        }

        impl ParseData for Ubx{

            fn parse_read(b: &[u8]) -> Result<(&[u8],Self)>{
                let b = parse::tag(b,0xb5u8).map_invalid(Error::InvalidHeader)?;
                let b = parse::tag(b,0x62u8).map_invalid(Error::InvalidHeader)?;

                let (b,class) = u8::parse_read(b)?;
                match class{
                    $($class_id => {
                        let (b,inner) = <$t>::parse_read(b)?;
                        Ok((b,Ubx::$var(inner)))
                    },)*
                    _ => {
                        let (b,msg) = u8::parse_read(b)?;
                        let (b,len) = u16::parse_read(b)?;
                        let (b,payload) = parse::collect(b,len as usize)?;
                        let (b,ck_a) = u8::parse_read(b)?;
                        let (b,ck_b) = u8::parse_read(b)?;
                        Ok((b,Ubx::Unknown{
                            class,
                            msg,
                            len,
                            payload,
                            ck_a,
                            ck_b
                        }))
                    }
                }
            }

            fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
                0xb5u8.parse_write(b)?;
                0x62u8.parse_write(b)?;

                match *self{
                    $(Self::$var(ref x) => {
                        let mut buffer = Vec::<u8>::new();
                        ($class_id as u8).parse_write(&mut buffer).unwrap();
                        x.parse_write(&mut buffer).unwrap();
                        let (ck_a,ck_b) = Self::checksum(&buffer);
                        b.write_all(&buffer)?;
                        b.write_all(&[ck_a,ck_b])?;
                        Ok(())
                    })*
                    Ubx::Unknown{ class,msg,len,ref payload,ck_a,ck_b } => {
                        class.parse_write(b)?;
                        msg.parse_write(b)?;
                        len.parse_write(b)?;
                        payload.parse_write(b)?;
                        ck_a.parse_write(b)?;
                        ck_b.parse_write(b)?;
                        Ok(())
                    }
                }
            }
        }

    };
}

impl_ubx! {
    pub enum Ubx {
        Cfg(Cfg) = 0x06,
        Nav(Nav) = 0x01,
        Ack(Ack) = 0x05,
        Mon(Mon) = 0x0A,
    }
}

impl Ubx {
    pub fn contains_prefix(b: &[u8]) -> bool {
        b.len() >= 2 && b[0] == 0xb5 && b[1] == 0x62
    }

    pub fn message_usage(b: &[u8]) -> Option<usize> {
        if !Self::contains_prefix(b) {
            return None;
        }
        if b.len() < 6 {
            return None;
        }
        let (_, len) = u16::parse_read(&b[4..]).unwrap();
        let len = len as usize;
        if b.len() < len + 8 {
            None
        } else {
            Some(len + 8)
        }
    }
}
