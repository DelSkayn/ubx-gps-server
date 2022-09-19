use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

use crate::{
    impl_bitfield, impl_enum,
    parse::{read_u32, ser_bitflags, Error, ParseData, Result},
};

use clap::ValueEnum;

#[bitflags]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MsgMask {
    Error = 0x01,
    Warning = 0x02,
    Notice = 0x04,
    Test = 0x08,
    Debug = 0x010,
}

impl_bitfield!(MsgMask);

impl_enum! {
    pub enum RtkFix: u8{
        RtkFloat = 2,
        RtkFixed = 3
    }
}

macro_rules! impl_value{
    (
        pub enum Value{
        $($name:ident($(#[$m:meta])*$ty:ty) = $id:expr,)*
    }) => {

        #[derive(Debug,Clone,Copy,Eq,PartialEq, Serialize,Deserialize)]
        #[serde(tag = "kind",content="value", rename_all = "kebab-case")]
        pub enum Value{
            $($name($(#[$m])*$ty),)*
        }

        #[derive(Debug,Clone,Copy,Eq,PartialEq, Serialize,Deserialize, ValueEnum)]
        #[serde(rename_all = "kebab-case")]
        pub enum ValueKey{
            $($name,)*
        }

        impl Value{
            pub fn write_bytes(&self, buffer: &mut Vec<u8>){
                match *self{
                    $(Self::$name(x) => {
                        ($id as u32).parse_write(buffer);
                        x.parse_write(buffer);
                    },)*
                }
            }

            pub fn from_bytes(b: &[u8]) -> Result<(&[u8],Self)>{
                let (b,id) = read_u32(b)?;
                match id{
                    $($id => {
                        let(b,v) = <$ty>::parse_read(b)?;
                        Ok((b,Self::$name(v)))
                    })*
                    _ => Err(Error::Invalid)
                }
            }

            pub fn size(&self) -> usize{
                match *self{
                    $(Self::$name(_) => {
                        4 + std::mem::size_of::<$ty>()
                    })*
                }
            }

            pub fn key(&self) -> ValueKey{
                match *self{
                    $(Self::$name(_) => ValueKey::$name,)*
                }
            }
        }


        impl ValueKey{
            pub fn write_bytes(&self, buffer: &mut Vec<u8>){
                match *self{
                    $(Self::$name => {
                        ($id as u32).parse_write(buffer);
                    },)*
                }
            }
        }
    }
}

impl_enum! {
    pub enum RtkMode: u8{
        Float = 2,
        Fixed = 3
    }
}

impl_enum! {
    pub enum Tmode: u8{
        Disabled = 0,
        SurveyIn = 1,
        Fixed = 2
    }
}

impl_enum! {
    pub enum PosType: u8{
        Ecef = 0,
        Llh = 1
    }
}

impl_enum! {
    pub enum StopBits: u8{
        Half = 0,
        One = 1,
        OneHalf = 2,
        Two = 3
    }
}

impl_enum! {
    pub enum Databits: u8{
        Eight = 0,
        Seven = 1
    }
}

impl_enum! {
    pub enum Parity: u8{
        None = 0,
        Odd = 1,
        Even = 2
    }
}

impl_enum! {
    pub enum OdoProfile: u8{
        Run = 0,
        Cycl = 1,
        Swim = 2,
        Car = 3,
        Custom = 4
    }
}

impl_value! {
    pub enum Value{
        RateMeas(u16) = 0x30210001,
        RateNav(u16) = 0x30210002,

        UsbInprotUbx(bool) = 0x10770001,
        UsbInprotNmea(bool) = 0x10770002,
        UsbInprotRtcm3x(bool) = 0x10770004,

        UsbOutprotUbx(bool) = 0x10780001,
        UsbOutprotNmea(bool) = 0x10780002,
        UsbOutprotRtcm3x(bool) = 0x10780004,

        SpiInprotUbx(bool) = 0x10790001,
        SpiInprotNmea(bool) = 0x10790002,
        SpiInprotRtcm3x(bool) = 0x10790004,

        SpiOutprotUbx(bool) = 0x107a0001,
        SpiOutprotNmea(bool) = 0x107a0002,
        SpiOutprotRtcm3x(bool) = 0x107a0004,

        Uart1InprotUbx(bool) = 0x10730001,
        Uart1InprotNmea(bool) = 0x10730002,
        Uart1InprotRtcm3x(bool) = 0x10730004,

        Uart1OutprotUbx(bool) = 0x10740001,
        Uart1OutprotNmea(bool) = 0x10740002,
        Uart1OutprotRtcm3x(bool) = 0x10740004,

        Uart2InprotUbx(bool) = 0x10750001,
        Uart2InprotNmea(bool) = 0x10750002,
        Uart2InprotRtcm3x(bool) = 0x10750004,

        Uart2OutprotUbx(bool) = 0x10760001,
        Uart2OutprotNmea(bool) = 0x10760002,
        Uart2OutprotRtcm3x(bool) = 0x10760004,

        Uart1Baudrate(u32) = 0x40520001,
        Uart1StopBits(StopBits) = 0x20520002,
        Uart1Databits(Databits) = 0x20520003,
        Uart1Parity(Parity) = 0x20520004,
        Uart1Enabled(bool) = 0x20520005,

        Uart2Baudrate(u32) = 0x40530001,
        Uart2StopBits(StopBits) = 0x20530002,
        Uart2Databits(Databits) = 0x20530003,
        Uart2Parity(Parity) = 0x20530004,
        Uart2Enabled(bool) = 0x20530005,
        Uart2Remap(bool) = 0x20530006,

        InfmsgUbxUart1(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
        ) = 0x20920002,
        InfmsgUbxUart2(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
                       ) = 0x20920003,
        InfmsgUbxUsb(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
            ) = 0x20920004,
        InfmsgNmeaUart1(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
            ) = 0x20920007,
        InfmsgNmeaUart2(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
            ) = 0x20920008,
        InfmsgNmeaUsb(
            #[serde(with = "ser_bitflags")]
            BitFlags<MsgMask>
            ) = 0x20920009,
        MsgoutRtcm3xType1005Usb(u8) = 0x209102c0,
        MsgoutRtcm3xType1074Usb(u8) = 0x20910361,
        MsgoutRtcm3xType1077Usb(u8) = 0x209102cf,
        MsgoutRtcm3xType1084Usb(u8) = 0x20910366,
        MsgoutRtcm3xType1087Usb(u8) = 0x209102d4,
        MsgoutRtcm3xType1094Usb(u8) = 0x2091036b,
        MsgoutRtcm3xType1097Usb(u8) = 0x2091031b,
        MsgoutRtcm3xType1124Usb(u8) = 0x20910370,
        MsgoutRtcm3xType1127Usb(u8) = 0x209102d9,
        MsgoutRtcm3xType1230Usb(u8) = 0x20910306,
        MsgoutRtcm3xType4072_0Usb(u8) = 0x20910301,
        MsgoutRtcm3xType4072_1Usb(u8) = 0x20910384,

        MsgoutUbxLogInfoUsb(u8) = 0x2091025b,
        MsgoutUbxMonHw2Usb(u8) = 0x209101bc,
        MsgoutUbxMonHw3Usb(u8) = 0x20910357,
        MsgoutUbxMonHwUsb(u8) = 0x209101b7,
        MsgoutUbxMonIoUsb(u8) = 0x209101a8,
        MsgoutUbxMonCommsUsb(u8) = 0x20910352,
        MsgoutUbxMonMsgppUsb(u8) = 0x20910199,
        MsgoutUbxMonRfUsb(u8) = 0x2091035c,
        MsgoutUbxMonRxbufUsb(u8) = 0x209101a3,
        MsgoutUbxMonRxrUsb(u8) = 0x2091018a,
        MsgoutUbxMonTxbufUsb(u8) = 0x2091019e,
        MsgoutUbxNavClockUsb(u8) = 0x20910068,
        MsgoutUbxNavDopUsb(u8) = 0x2091003b,
        MsgoutUbxNavEoeUsb(u8) = 0x20910162,
        MsgoutUbxNavHpposecefUsb(u8) = 0x20910031,
        MsgoutUbxNavHpposllhUsb(u8) = 0x20910036,
        MsgoutUbxNavOdoUsb(u8) = 0x20910081,
        MsgoutUbxNavOrbUsb(u8) = 0x20910013,
        MsgoutUbxNavPosecefUsb(u8) = 0x20910027,
        MsgoutUbxNavPosllhUsb(u8) = 0x2091002c,
        MsgoutUbxNavPvtUsb(u8) = 0x20910009,
        MsgoutUbxNavRelPosNedUsb(u8) = 0x20910090,
        MsgoutUbxNavSatUsb(u8) = 0x20910018,
        MsgoutUbxNavSigUsb(u8) = 0x20910348,
        MsgoutUbxNavStatusUsb(u8) = 0x2091001d,
        MsgoutUbxNavSvinUsb(u8) = 0x2091008b,
        MsgoutUbxNavTimebdsUsb(u8) = 0x20910054,
        MsgoutUbxNavTimegalUsb(u8) = 0x20910059,
        MsgoutUbxNavTimegloUsb(u8) = 0x2091004f,
        MsgoutUbxNavTimegpsUsb(u8) = 0x2091004a,
        MsgoutUbxNavTimelsUsb(u8) = 0x20910063,
        MsgoutUbxNavTimeutcUsb(u8) = 0x2091005e,
        MsgoutUbxNavVelecefUsb(u8) = 0x20910040,
        MsgoutUbxNavVelnedUsb(u8) = 0x20910045,
        MsgoutUbxRxmMeasxUsb(u8) = 0x20910207,
        MsgoutUbxRxmRawxUsb(u8) = 0x209102a7,
        MsgoutUbxRxmRlmUsb(u8) = 0x20910261,
        MsgoutUbxRxmRtcmUsb(u8) = 0x2091026b,
        MsgoutUbxRxmSfrbxUsb(u8) = 0x20910234,

        OdoUseOdo(bool) = 0x10220001,
        OdoUseCog(bool) = 0x10220002,
        OdoOutlpvel(bool) = 0x10220003,
        OdoOutlpcog(bool) = 0x10220004,
        OdoProfile(OdoProfile) = 0x20220005,
        OdoCogmaxspeed(u8) = 0x20220021,
        OdoCogmaxposacc(u8) = 0x20220022,
        OdoVellpgain(u8) = 0x20220031,
        OdoCoglpgain(u8) = 0x20220032,

        NavhpgDgnssmode(RtkMode) = 0x20140011,

        TmodeMode(Tmode) = 0x20030001,
        TmodePosType(PosType) = 0x20030002,
        TmodeEcefX(i32) = 0x20030003,
        TmodeEcefY(i32) = 0x20030004,
        TmodeEcefZ(i32) = 0x20030005,
        TmodeEcefXHp(i8) = 0x20030006,
        TmodeEcefYHp(i8) = 0x20030007,
        TmodeEcefZHp(i8) = 0x20030008,
        TmodeFixedPosAcc(u32) = 0x4003000f,
        TmodeSvinMinDur(u32) = 0x40030010,
        TmodeSvinAccLimit(u32) = 0x40030011,

        SignalGpsEna(bool) = 0x1031001f,
        SignalGpsL1caEna(bool) = 0x10310001,
        SignalGpsL2cEna(bool) = 0x10310003,

        SignalGalEna(bool) = 0x10310021,
        SignalGalE1Ena(bool) = 0x10310007,
        SignalGalE5bEna(bool) = 0x1031000a,

        SignalBdsEna(bool) = 0x10310022,
        SignalBdsB1Ena(bool) = 0x1031000d,
        SignalBdsB2Ena(bool) = 0x1031000e,

        SignalQzssEna(bool) = 0x10310024,
        SignalQzssL1caEna(bool) = 0x10310012,
        SignalQzssL2cEna(bool) = 0x10310015,

        SignalGloEna(bool) = 0x10310025,
        SignalGloL1Ena(bool) = 0x10310018,
        SignalGloL2Ena(bool) = 0x1031001a,
    }
}
