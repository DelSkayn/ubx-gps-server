use std::io::Write;

use crate::{
    impl_bitfield, impl_struct,
    parse::{ser_bitflags, ParseData, ParseError, Result},
};
use anyhow::bail;
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Clock{
    i_tow: u32,
    clk_b: i32,
    clk_d: i32,
    t_acc: u32,
    f_acc: u32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Dop{
    i_tow: u32,
    g_dop: u16,
    p_dop: u16,
    t_dop: u16,
    v_dop: u16,
    h_dop: u16,
    n_dop: u16,
    e_dop: u16,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Eoe{
    i_tow: u32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,Default)]
#[serde(default)]
pub struct Hpposecef{
    version:u8,
    res1: [u8;3],
    i_tow: u32,
    ecef_x: i32,
    ecef_y: i32,
    ecef_z: i32,
    ecef_x_hp: i8,
    ecef_y_hp: i8,
    ecef_z_hp: i8,
    res2: u8,
    p_acc: i32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,Default)]
#[serde(default)]
pub struct Hpposllh{
    version:u8,
    res1: [u8;3],
    i_tow: u32,
    lon: i32,
    lat: i32,
    height: i32,
    h_msl: i32,
    lon_hp: i8,
    lat_hp: i8,
    height_hp: i8,
    h_msl_hp: i8,
    h_acc: i32,
    v_acc: i32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Odo{
    version:u8,
    res1: [u8;3],
    i_tow: u32,
    distance: u32,
    total_distance: u32,
    distance_std: u32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Posecef{
    i_tow: u32,
    ecef_x: i32,
    ecef_y: i32,
    ecef_z: i32,
    p_acc: u32,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Posllh{
    i_tow: u32,
    lon: i32,
    lat: i32,
    height: i32,
    h_msl: i32,
    h_acc: u32,
}
}

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Valid {
    Date = 0b0001,
    Time = 0b0010,
    FullyResolved = 0b0100,
    Mag = 0b1000,
}

impl_bitfield!(Valid);

#[bitflags]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelFlags {
    GnssFixOk = 0b0000000001,
    DiffSoln = 0b0000000010,
    RelPosValid = 0b0000000100,
    CarrSolnFloat = 0b0000001000,
    CarrSolnFixed = 0b0000010000,
    IsMoving = 0b0000100000,
    RefPosMiss = 0b0001000000,
    RefObsMiss = 0b0010000000,
    RelPosHeadingValid = 0b0100000000,
    RelPosNormalized = 0b1000000000,
}

impl_bitfield!(RelFlags);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsmState {
    NotActive = 0,
    Enabled = 1,
    Acquisition = 2,
    Tracking = 3,
    PowerOptimizedTracking = 4,
    Inactive = 5,
}

impl Default for PsmState {
    fn default() -> Self {
        PsmState::NotActive
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrierPhaseSol {
    NoSolution = 0,
    Float = 1,
    Fixed = 2,
}

impl Default for CarrierPhaseSol {
    fn default() -> Self {
        CarrierPhaseSol::NoSolution
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixType {
    NoFix,
    DeadReckoning,
    Fix2D,
    Fix3D,
    Gnss,
    Time,
    Reserved(u8),
}

impl Default for FixType {
    fn default() -> Self {
        FixType::NoFix
    }
}

impl ParseData for FixType {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, d) = u8::parse_read(b)?;
        let res = match d {
            0 => Self::NoFix,
            1 => Self::DeadReckoning,
            2 => Self::Fix2D,
            3 => Self::Fix3D,
            4 => Self::Gnss,
            5 => Self::Time,
            x => Self::Reserved(x),
        };
        Ok((b, res))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        match self {
            Self::NoFix => 0u8.parse_write(b),
            Self::DeadReckoning => 1u8.parse_write(b),
            Self::Fix2D => 2u8.parse_write(b),
            Self::Fix3D => 3u8.parse_write(b),
            Self::Gnss => 4u8.parse_write(b),
            Self::Time => 5u8.parse_write(b),
            Self::Reserved(x) => x.parse_write(b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FixStatus {
    pub car_sol: CarrierPhaseSol,
    pub head_veh_valid: bool,
    pub psm_state: PsmState,
    pub diff_soln: bool,
    pub gnss_fix_ok: bool,
}

impl ParseData for FixStatus {
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, data) = u8::parse_read(b)?;
        let psm_state = match (data >> 2) & 0b111 {
            0 => PsmState::NotActive,
            1 => PsmState::Enabled,
            2 => PsmState::Acquisition,
            3 => PsmState::Tracking,
            4 => PsmState::PowerOptimizedTracking,
            5 => PsmState::Inactive,
            _ => bail!(ParseError::Invalid),
        };

        let car_sol = match (data >> 6) & 0b11 {
            0 => CarrierPhaseSol::NoSolution,
            1 => CarrierPhaseSol::Float,
            2 => CarrierPhaseSol::Fixed,
            _ => bail!(ParseError::Invalid),
        };

        Ok((
            b,
            FixStatus {
                car_sol,
                head_veh_valid: (data >> 5) & 0b1 != 0,
                psm_state,
                diff_soln: (data >> 1) & 0b1 != 0,
                gnss_fix_ok: data & 0b1 != 0,
            },
        ))
    }

    fn parse_write<W: Write>(&self, b: &mut W) -> Result<()> {
        let data = (self.car_sol as u8) << 6
            | (self.head_veh_valid as u8) << 5
            | (self.psm_state as u8) << 2
            | (self.diff_soln as u8) << 1
            | self.gnss_fix_ok as u8;

        data.parse_write(b)
    }
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,Default)]
#[serde(default)]
pub struct Pvt{
        i_tow: u32,
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        min: u8,
        sec: u8,
        #[serde(with = "ser_bitflags")]
        valid: BitFlags<Valid>,
        t_acc: u32,
        nano: i32,
        fix_type: FixType,
        flags: FixStatus,
        flags2: u8,
        numsv: u8,
        lon: i32,
        lat: i32,
        height: i32,
        height_sea: i32,
        h_acc: u32,
        v_acc: u32,
        vel_n: i32,
        vel_e: i32,
        vel_d: i32,
        g_speed: i32,
        heading_mot: i32,
        s_acc: u32,
        head_acc: u32,
        p_dop: u16,
        flags3: u8,
        res1: [u8;5],
        head_veh: i32,
        mag_dec: i16,
        mag_acc: u16,
}
}

impl_struct! {
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize,Default)]
#[serde(default)]
    pub struct RelPosNed {
        version: u8,
        res1: u8,
        ref_station_id: u16,
        i_tow: u32,
        rel_pos_n: i32,
        rel_pos_e: i32,
        rel_pos_d: i32,
        rel_pos_length: i32,
        rel_pos_heading: i32,
        res2: [u8;4],
        rel_pos_n_hp: i8,
        rel_pos_e_hp: i8,
        rel_pos_d_hp: i8,
        rel_pos_length_hp: i8,
        acc_n: i32,
        acc_e: i32,
        acc_d: i32,
        acc_length: i32,
        acc_heading: i32,
        res3: [u8;4],
        #[serde(with = "ser_bitflags")]
        flags: BitFlags<RelFlags>,
    }
}

impl_class! {
    pub enum Nav: PollNav{
        Clock(Clock)[20u16] = 0x22u8,
        Dop(Dop)[18u16] = 0x04u8,
        Eoe(Eoe)[4u16] = 0x61u8,
        Hpposecef(Hpposecef)[28u16] = 0x13u8,
        Hpposllh(Hpposllh)[36u16] = 0x14u8,
        Odo(Odo)[20u16] = 0x09u8,
        Posecef(Posecef)[20u16] = 0x01u8,
        Posllh(Posllh)[28u16] = 0x02u8,
        Pvt(Pvt)[92u16] = 0x07u8,
        RelPosNed(RelPosNed)[64u16] = 0x3Cu8,
    }
}
