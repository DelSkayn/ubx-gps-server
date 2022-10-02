use std::{
    fmt,
    io::{self, Write},
    result::Result as StdResult,
};

pub mod ser_bitflags {
    use enumflags2::{BitFlag, BitFlags};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T: BitFlag + Serialize, S>(
        flags: &BitFlags<T>,
        s: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let list: Vec<_> = flags.iter().collect();
        list.serialize(s)
    }

    pub fn deserialize<'de, D, T>(d: D) -> Result<BitFlags<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de> + BitFlag,
    {
        let v = Vec::<T>::deserialize(d)?;
        let mut res = BitFlags::empty();
        for v in v {
            res |= v
        }
        Ok(res)
    }
}

#[macro_export]
macro_rules! pread {
    ($buf:ident => { $($name:ident : $t:ty,)* })=> {
        $(let ($buf,$name) = <$t>::parse_read($buf)?;)*
    };
}

#[macro_export]
macro_rules! pread_struct {
    ($buf:ident => $name:ident$(::$p:ident)*{ $($field:ident : $t:ty,)* })=> {
        {
            $(let ($buf,$field) = <$t>::parse_read($buf)?;)*
            ($buf,$name$(::$p)*{ $($field,)*})
        }
    };
}

#[macro_export]
macro_rules! pwrite {
    ($buf:ident => { $($name:expr,)* })=> {
        $($name.parse_write($buf);)*
    };

}

#[macro_export]
macro_rules! impl_struct{
    (
        $(#[$m:meta])*
        pub struct $name:ident{
            $(
                $(#[$at:meta])*
            $field:ident: $ty:ty,
            )*
        }
    ) => {
        $(#[$m])*
        pub struct $name{
            $(
                $(#[$at])*
                pub $field: $ty,
            )*
        }

        impl ParseData for $name {
            fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8], Self)> {
                $(let (b,$field) = <$ty>::parse_read(b)?;)*
                Ok((b,$name{
                    $($field,)*
                }))
            }

            fn parse_write<W: std::io::Write>(&self, b: &mut W) -> crate::parse::Result<()> {
                $(ParseData::parse_write(&self.$field,b)?;)*
                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! impl_bitfield {
    ($name:ty) => {
        impl ParseData for enumflags2::BitFlags<$name> {
            fn parse_read(b: &[u8]) -> crate::parse::Result<(&[u8], Self)> {
                let (b, v) = ParseData::parse_read(b)?;
                Ok((b, Self::from_bits_truncate(v)))
            }

            fn parse_write<W: std::io::Write>(&self, b: &mut W) -> crate::parse::Result<()> {
                ParseData::parse_write(&self.bits(), b)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_enum{
    (pub enum $name:ident: $repr:ident{
        $($kind:ident = $v:expr),*
    }) => {
        #[repr($repr)]
        #[derive(Debug,Clone,Copy,Eq,PartialEq,Serialize,Deserialize)]
        pub enum $name{
            $($kind = $v),*
        }

        impl ParseData for $name{
            fn parse_read(b: &[u8]) -> Result<(&[u8], Self)>{
                let (b,v) = $repr::parse_read(b)?;
                match v{
                    $($v => Ok((b,Self::$kind)),)*
                    _  => return Err(Error::Invalid)
                }
            }

            fn parse_write<W: std::io::Write>(&self, b: &mut W) -> Result<()> {
                ParseData::parse_write(&(*self as $repr),b)
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    NotEnoughData,
    InvalidChecksum,
    InvalidHeader,
    InvalidClass(u8),
    InvalidMsg(u8),
    InvalidLen,
    Invalid,
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::NotEnoughData => write!(f, "not enough data in buffer to parse structure"),
            Error::InvalidChecksum => write!(f, "checksum is not valid"),
            Error::InvalidHeader => write!(f, "header is not valid"),
            Error::InvalidClass(x) => write!(f, "encountered unknown ubx message class `{}`", x),
            Error::InvalidMsg(x) => write!(f, "encountered unknown ubx message id `{}`", x),
            Error::InvalidLen => write!(f, "ubx message length is not as specified in spec"),
            Error::Invalid => write!(f, "failed to parse buffer"),
            Error::Io(ref e) => write!(f, "io error: {}", e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = StdResult<T, Error>;

pub trait ResultExt {
    fn map_invalid(self, e: Error) -> Self;
}

impl<T> ResultExt for Result<T> {
    fn map_invalid(self, e: Error) -> Self {
        match self {
            Err(Error::Invalid) => Err(e),
            x => x,
        }
    }
}

pub trait Offset {
    fn offset(&self, other: &Self) -> usize;
}

impl Offset for [u8] {
    fn offset(&self, other: &Self) -> usize {
        let start = self.as_ptr() as usize;
        let end = start + self.len();
        let ptr = other.as_ptr() as usize;
        assert!(ptr >= start && ptr <= end);
        unsafe { other.as_ptr().offset_from(self.as_ptr()) as usize }
    }
}

pub trait ParseData: Sized {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)>;

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()>;
}

impl ParseData for u64 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.len() < 8 {
            return Err(Error::NotEnoughData)?;
        }
        let d = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        let d = u64::from_le_bytes(d);
        Ok((&b[4..], d))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl ParseData for u32 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.len() < 4 {
            return Err(Error::NotEnoughData)?;
        }
        let d = [b[0], b[1], b[2], b[3]];
        let d = u32::from_le_bytes(d);
        Ok((&b[4..], d))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl ParseData for u16 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.len() < 2 {
            return Err(Error::NotEnoughData)?;
        }
        let d = [b[0], b[1]];
        let d = u16::from_le_bytes(d);
        Ok((&b[2..], d))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl ParseData for u8 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.is_empty() {
            return Err(Error::NotEnoughData);
        }
        Ok((&b[1..], b[0]))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&[*self])?;
        Ok(())
    }
}

impl ParseData for i32 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.len() < 4 {
            return Err(Error::NotEnoughData)?;
        }
        let d = [b[0], b[1], b[2], b[3]];
        let d = i32::from_le_bytes(d);
        Ok((&b[4..], d))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl ParseData for i16 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        if b.len() < 2 {
            return Err(Error::NotEnoughData)?;
        }
        let d = [b[0], b[1]];
        let d = i16::from_le_bytes(d);
        Ok((&b[2..], d))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl ParseData for i8 {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let d = *b.first().ok_or(Error::NotEnoughData)?;
        Ok((&b[1..], d as i8))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&[*self as u8])?;
        Ok(())
    }
}

impl ParseData for bool {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, v) = u8::parse_read(b)?;
        Ok((b, v != 0))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        b.write_all(&[*self as u8])?;
        Ok(())
    }
}

impl<T: ParseData, const N: usize> ParseData for [T; N] {
    fn parse_read(mut b: &[u8]) -> Result<(&[u8], Self)> {
        let mut tmp = std::mem::MaybeUninit::<[T; N]>::uninit();
        for i in 0..N {
            let (nb, t) = T::parse_read(b)?;
            b = nb;
            unsafe {
                tmp.as_mut_ptr().cast::<T>().add(i).write(t);
            }
        }
        Ok((b, unsafe { tmp.assume_init() }))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        for v in self.iter() {
            v.parse_write(b)?;
        }
        Ok(())
    }
}

impl<T: ParseData> ParseData for Vec<T> {
    fn parse_read(mut b: &[u8]) -> Result<(&[u8], Self)> {
        let mut res = Vec::new();
        while !b.is_empty() {
            match T::parse_read(b) {
                Ok((bn, v)) => {
                    res.push(v);
                    b = bn;
                }
                Err(Error::NotEnoughData) => return Err(Error::Invalid),
                Err(x) => return Err(x),
            }
        }

        Ok((b, res))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        for v in self.iter() {
            v.parse_write(b)?;
        }
        Ok(())
    }
}

pub fn tag<T: ParseData + PartialEq>(b: &[u8], tag: T) -> Result<&[u8]> {
    let (b, t) = T::parse_read(b)?;
    if t == tag {
        Ok(b)
    } else {
        Err(Error::Invalid)
    }
}

pub fn collect<T: ParseData>(mut b: &[u8], cnt: usize) -> Result<(&[u8], Vec<T>)> {
    let mut res = Vec::with_capacity(cnt);
    for _ in 0..cnt {
        let (nb, t) = T::parse_read(b)?;
        res.push(t);
        b = nb;
    }
    Ok((b, res))
}

pub fn eat<T: ParseData>(b: &[u8], len: usize) -> Result<&[u8]> {
    if b.len() < len {
        Err(Error::NotEnoughData)
    } else {
        Ok(&b[len..])
    }
}
