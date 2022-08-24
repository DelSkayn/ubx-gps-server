use crate::{
    impl_bitfield, impl_enum,
    parse::{read_u16, read_u8, tag, Error, Offset, ParseData, Result, ResultExt, ser_bitflags},
    pread,
};
use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

impl_enum! {
    pub enum FixType: u8 {
        NoFix = 0,
        DeadReckoning = 1,
        Fix2D = 2,
        Fix3D = 3,
        Gnss = 4,
        Time = 5
    }
}

#[bitflags]
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize,Deserialize)]
pub enum Valid {
    Date = 0b0001,
    Time = 0b0010,
    FullyResolved = 0b0100,
    Mag = 0b1000,
}

impl_bitfield!(Valid);

#[bitflags]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize,Deserialize)]
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

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum PsmState{
    NotActive = 0,
    Enabled = 1,
    Acquisition = 2,
    Tracking = 3,
    PowerOptimizedTracking = 4,
    Inactive = 5
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum CarrierPhaseSol{
    NoSolution = 0,
    Float = 1,
    Fixed = 2,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct FixStatus{
    car_sol: CarrierPhaseSol,
    head_veh_valid: bool,
    psm_state: PsmState,
    diff_soln: bool,
    gnss_fix_ok: bool,
}

impl ParseData for FixStatus{
    fn parse_read(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b,data) = u8::parse_read(b)?;
        let psm_state = match (data >> 2) & 0b111{
            0 => PsmState::NotActive,
            1 => PsmState::Enabled,
            2 => PsmState::Acquisition,
            3 => PsmState::Tracking,
            4 => PsmState::PowerOptimizedTracking,
            5 => PsmState::Inactive,
            _ => return Err(Error::Invalid)
        };

        let car_sol = match (data >> 6) & 0b11{
            0 => CarrierPhaseSol::NoSolution,
            1 => CarrierPhaseSol::Float,
            2 => CarrierPhaseSol::Fixed,
            _ => return Err(Error::Invalid)
        };

        Ok((b,FixStatus{
            car_sol,
            head_veh_valid: (data >> 5) & 0b1 != 0,
            psm_state,
            diff_soln: (data >> 1) & 0b1 != 0,
            gnss_fix_ok: data & 0b1 != 0,
        }))
    }

    fn parse_write(self, b: &mut Vec<u8>) {
        let data = (self.car_sol as u8) << 6
            | (self.head_veh_valid as u8) << 5
            | (self.psm_state as u8) << 2
            | (self.diff_soln as u8) << 1
            | self.gnss_fix_ok as u8;

        b.push(data);
    }
}


impl_bitfield!(RelFlags);

#[derive(Debug, Serialize, Deserialize)]
pub struct Satellite {
    gnss_id: u8,
    sv_id: u8,
    cno: u8,
    elev: i8,
    azim: i16,
    pr_res: i16,
    flags: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Nav {
    Clock,
    Dop {
        i_tow: u32,
        g_dop: u16,
        p_dop: u16,
        t_dop: u16,
        v_dop: u16,
        h_dop: u16,
        n_dop: u16,
        e_dop: u16,
    },
    Eoe {
        i_tow: u32,
    },
    Geofence,
    HPPOSecef {
        version: u8,
        i_tow: u32,
        ecef_x: i32,
        ecef_y: i32,
        ecef_z: i32,

        ecef_x_hp: i8,
        ecef_y_hp: i8,
        ecef_z_hp: i8,

        flags: u8,
        p_acc: u32,
    },
    HPPOSllh {
        version: u8,
        flags: u8,
        i_tow: u32,
        lon: i32,
        lat: i32,
        height: i32,
        height_sea: i32,
        lon_hp: i8,
        lat_hp: i8,
        height_hp: i8,
        height_sea_hp: i8,
        h_acc: i32,
        v_acc: i32,
    },
    Posecef {
        i_tow: u32,
        ecef_x: i32,
        ecef_y: i32,
        ecef_z: i32,
        p_acc: u32,
    },
    Pvt {
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
        _reserved: u32,
        _reserved_ext: u8,
        head_veh: i32,
        mag_dec: i16,
        mag_acc: u16,
    },
    RelPosNed {
        version: u8,
        ref_station_id: u16,
        i_tow: u32,
        rel_pos_n: i32,
        rel_pos_e: i32,
        rel_pos_d: i32,
        rel_pos_length: i32,
        rel_pos_heading: i32,
        rel_pos_n_hp: i8,
        rel_pos_e_hp: i8,
        rel_pos_d_hp: i8,
        rel_pos_length_hp: i8,
        acc_n: i32,
        acc_e: i32,
        acc_d: i32,
        acc_length: i32,
        acc_heading: i32,
        #[serde(with = "ser_bitflags")]
        flags: BitFlags<RelFlags>,
    },
    Sat {
        i_tow: u32,
        version: u8,
        num_sat: u8,
        sats: Vec<Satellite>,
    },
    Svin {
        version: u8,
        i_tow: u32,
        dur: u32,
        mean_x: i32,
        mean_y: i32,
        mean_z: i32,
        mean_xhp: i8,
        mean_yhp: i8,
        mean_zhp: i8,
        mean_acc: u32,
        obs: u32,
        valid: u8,
        active: u8,
    },
    TimeGps {
        i_tow: u32,
        ftow: i32,
        week: i16,
        leap_seconds: i8,
        valid: u8,
        t_acc: u32,
    },
    TimeLs {
        i_tow: u32,
        version: u8,
        src_of_cur_ls: u8,
        cur_ls: i8,
        src_of_ls_change: u8,
        ls_change: i8,
        time_to_ls_event: i32,
        dat_of_ls_gps_wn: u16,
        dat_of_ls_gps_dn: u16,
        valid: u8,
    },
    Velecef {
        i_tow: u32,
        ecef_v_x: i32,
        ecef_v_y: i32,
        ecef_v_z: i32,
        s_acc: u32,
    },
}

impl Nav {
    pub fn from_bytes(b: &[u8]) -> Result<(&[u8], Self)> {
        let (b, msg) = read_u8(b)?;
        match msg {
            0x22 => Ok((b, Nav::Clock)),
            0x04 => {
                let b = tag(b, 18u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    i_tow: u32,
                    g_dop: u16,
                    p_dop: u16,
                    t_dop: u16,
                    v_dop: u16,
                    h_dop: u16,
                    n_dop: u16,
                    e_dop: u16,
                });

                Ok((
                    b,
                    Nav::Dop {
                        i_tow,
                        g_dop,
                        p_dop,
                        t_dop,
                        v_dop,
                        h_dop,
                        n_dop,
                        e_dop,
                    },
                ))
            }
            0x61 => {
                let b = tag(b, 4u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    i_tow: u32,
                });
                Ok((b, Nav::Eoe { i_tow }))
            }
            0x39 => Ok((b, Nav::Geofence)),
            0x13 => {
                let b = tag(b, 28u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    version: u8,
                    i_tow: u32,
                    _res: u8,
                    _res: u16,
                    ecef_x: i32,
                    ecef_y: i32,
                    ecef_z: i32,
                    ecef_x_hp: i8,
                    ecef_y_hp: i8,
                    ecef_z_hp: i8,
                    flags: u8,
                    p_acc: u32,
                });
                Ok((
                    b,
                    Nav::HPPOSecef {
                        version,
                        i_tow,
                        ecef_x,
                        ecef_y,
                        ecef_z,
                        ecef_x_hp,
                        ecef_y_hp,
                        ecef_z_hp,
                        flags,
                        p_acc,
                    },
                ))
            }
            0x14 => {
                let b = tag(b, 28u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    version: u8,
                    _res: u16,
                    flags: u8,
                    i_tow: u32,
                    lon: i32,
                    lat: i32,
                    height: i32,
                    height_sea: i32,
                    lon_hp: i8,
                    lat_hp: i8,
                    height_hp: i8,
                    height_sea_hp: i8,
                    h_acc: i32,
                    v_acc: i32,
                });
                Ok((
                    b,
                    Nav::HPPOSllh {
                        version,
                        flags,
                        i_tow,
                        lon,
                        lat,
                        height,
                        height_sea,
                        lon_hp,
                        lat_hp,
                        height_hp,
                        height_sea_hp,
                        h_acc,
                        v_acc,
                    },
                ))
            }
            0x01 => {
                let b = tag(b, 20u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    i_tow: u32,
                    ecef_x: i32,
                    ecef_y: i32,
                    ecef_z: i32,
                    p_acc: u32,
                });
                Ok((
                    b,
                    Nav::Posecef {
                        i_tow,
                        ecef_x,
                        ecef_y,
                        ecef_z,
                        p_acc,
                    },
                ))
            }
            0x07 => {
                let b = tag(b, 92u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    i_tow: u32,
                    year: u16,
                    month: u8,
                    day: u8,
                    hour: u8,
                    min: u8,
                    sec: u8,
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
                    _reserved: u32,
                    _reserved_ext: u8,
                    head_veh: i32,
                    mag_dec: i16,
                    mag_acc: u16,
                });

                let res = Nav::Pvt {
                    i_tow,
                    year,
                    month,
                    day,
                    hour,
                    min,
                    sec,
                    valid,
                    t_acc,
                    nano,
                    fix_type,
                    flags,
                    flags2,
                    numsv,
                    lon,
                    lat,
                    height,
                    height_sea,
                    h_acc,
                    v_acc,
                    vel_n,
                    vel_e,
                    vel_d,
                    g_speed,
                    heading_mot,
                    s_acc,
                    head_acc,
                    p_dop,
                    flags3,
                    _reserved,
                    _reserved_ext,
                    head_veh,
                    mag_dec,
                    mag_acc,
                };
                Ok((b, res))
            }
            0x3c => {
                let b = tag(b, 0x40u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    version: u8,
                    _res: u8,
                    ref_station_id: u16,
                    i_tow: u32,
                    rel_pos_n: i32,
                    rel_pos_e: i32,
                    rel_pos_d: i32,
                    rel_pos_length: i32,
                    rel_pos_heading: i32,
                    _res: u32,
                    rel_pos_n_hp: i8,
                    rel_pos_e_hp: i8,
                    rel_pos_d_hp: i8,
                    rel_pos_length_hp: i8,
                    acc_n: i32,
                    acc_e: i32,
                    acc_d: i32,
                    acc_length: i32,
                    acc_heading: i32,
                    _res: u32,
                    flags: BitFlags<RelFlags>,
                });
                Ok((
                    b,
                    Nav::RelPosNed {
                        version,
                        ref_station_id,
                        i_tow,
                        rel_pos_n,
                        rel_pos_e,
                        rel_pos_d,
                        rel_pos_length,
                        rel_pos_heading,
                        rel_pos_n_hp,
                        rel_pos_e_hp,
                        rel_pos_d_hp,
                        rel_pos_length_hp,
                        acc_n,
                        acc_e,
                        acc_d,
                        acc_length,
                        acc_heading,
                        flags,
                    },
                ))
            }
            0x35 => {
                let (b, len) = read_u16(b)?;
                pread!(b => {
                    i_tow: u32,
                    version: u8,
                    num_sat: u8,
                    _res: u16,
                });
                if len != 8 + 12 * num_sat as u16 {
                    return Err(Error::InvalidLen);
                }
                let mut sats = Vec::new();
                let mut sb = b;
                for _ in 0..num_sat {
                    let tb = sb;
                    pread!(tb => {
                        gnss_id: u8,
                        sv_id:u8,
                        cno:u8,
                        elev: i8,
                        azim: i16,
                        pr_res: i16,
                        flags: u32,
                    });
                    assert_eq!(sb.offset(tb), 12);
                    sb = tb;
                    sats.push(Satellite {
                        gnss_id,
                        sv_id,
                        cno,
                        elev,
                        azim,
                        pr_res,
                        flags,
                    });
                }
                assert_eq!(b.offset(sb), 12 * num_sat as usize);
                Ok((
                    sb,
                    Nav::Sat {
                        i_tow,
                        version,
                        num_sat,
                        sats,
                    },
                ))
            }
            0x3b => {
                let b = tag(b, 40u16).map_invalid(Error::InvalidLen)?;
                pread!(b =>{
                    version: u8,
                    _res0: [u8:3],
                    i_tow: u32,
                    dur: u32,
                    mean_x: i32,
                    mean_y: i32,
                    mean_z: i32,
                    mean_xhp: i8,
                    mean_yhp: i8,
                    mean_zhp: i8,
                    _res1: u8,
                    mean_acc: u32,
                    obs: u32,
                    valid: u8,
                    active: u8,
                    _res2: [u8;2],
                });

                Ok((
                    b,
                    Nav::Svin {
                        version,
                        i_tow,
                        dur,
                        mean_x,
                        mean_y,
                        mean_z,
                        mean_xhp,
                        mean_yhp,
                        mean_zhp,
                        mean_acc,
                        obs,
                        valid,
                        active,
                    },
                ))
            }
            0x26 => {
                let b = tag(b, 24u16).map_invalid(Error::InvalidLen)?;
                pread!(b =>{
                    i_tow: u32,
                    version:u8,
                    _res0: u8,
                    _res1: u16,
                    src_of_cur_ls: u8,
                    cur_ls: i8,
                    src_of_ls_change: u8,
                    ls_change: i8,
                    time_to_ls_event: i32,
                    dat_of_ls_gps_wn: u16,
                    dat_of_ls_gps_dn: u16,
                    _res2: u8,
                    _res3: u16,
                    valid: u8,
                });
                Ok((
                    b,
                    Nav::TimeLs {
                        i_tow,
                        version,
                        src_of_cur_ls,
                        cur_ls,
                        src_of_ls_change,
                        ls_change,
                        time_to_ls_event,
                        dat_of_ls_gps_wn,
                        dat_of_ls_gps_dn,
                        valid,
                    },
                ))
            }
            0x20 => {
                let b = tag(b, 16u16).map_invalid(Error::InvalidLen)?;
                pread!(b =>{
                i_tow: u32,
                ftow: i32,
                week: i16,
                leap_seconds: i8,
                valid: u8,
                t_acc: u32,
                        });

                Ok((
                    b,
                    Nav::TimeGps {
                        i_tow,
                        ftow,
                        week,
                        leap_seconds,
                        valid,
                        t_acc,
                    },
                ))
            }
            0x11 => {
                let b = tag(b, 20u16).map_invalid(Error::InvalidLen)?;
                pread!(b => {
                    i_tow: u32,
                    ecef_v_x: i32,
                    ecef_v_y: i32,
                    ecef_v_z: i32,
                    s_acc: u32,
                });
                Ok((
                    b,
                    Nav::Velecef {
                        i_tow,
                        ecef_v_x,
                        ecef_v_y,
                        ecef_v_z,
                        s_acc,
                    },
                ))
            }
            x => Err(Error::InvalidMsg(x)),
        }
    }
}
