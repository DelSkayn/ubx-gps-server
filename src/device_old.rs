use std::{
    io::{ErrorKind, Write},
    thread,
};

use anyhow::{bail, Context, Result};

use log::{trace, warn};
use serialport::{SerialPort, SerialPortBuilder};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot,
};

use crate::{
    parse::Error,
    ubx::{self, Cfg},
    GpsMsg,
};

#[derive(Debug)]
pub struct AckMsg {
    send: oneshot::Sender<bool>,
    id: u8,
    class: u8,
}

pub struct GpsDevice {
    ack_send: Sender<AckMsg>,
    send: Sender<GpsMsg<'static>>,
    recv: Receiver<GpsMsg<'static>>,
}

impl GpsDevice {
    const CFG_VALSET_CLASS: u8 = 0x06;

    fn read_buff(port: &mut impl SerialPort, buffer: &mut Vec<u8>) -> Result<usize> {
        let read = port.bytes_to_read()?;
        if read == 0 {
            let read = 512;
            let len = buffer.len();
            buffer.resize(read as usize + len, 0u8);
            let read = loop {
                let x = match port.read(&mut buffer[len..]) {
                    Err(e) => {
                        if let ErrorKind::TimedOut = e.kind() {
                            continue;
                        }
                        bail!(e);
                    }
                    Ok(x) => x,
                };
                break x;
            };
            buffer.truncate(len + read);
            Ok(read)
        } else {
            let len = buffer.len();
            buffer.resize(read as usize + len, 0u8);

            loop {
                match port.read_exact(&mut buffer[len..]) {
                    Err(e) if matches!(e.kind(), ErrorKind::TimedOut) => continue,
                    Err(e) => {
                        bail!(e)
                    }
                    Ok(_) => {
                        break;
                    }
                }
            }
            Ok(read as usize)
        }
    }

    fn read_msg(port: &mut impl SerialPort, buffer: &mut Vec<u8>) -> Result<GpsMsg<'static>> {
        let mut offset = 0;
        loop {
            if buffer.len() < 2 || buffer.len() >= offset {
                Self::read_buff(port, buffer)?;
            }

            match GpsMsg::from_bytes(&buffer[offset..]) {
                Ok((x, len)) => {
                    let x = x.into_owned();
                    let new_len = buffer.len() - len;
                    buffer.copy_within(len.., 0);
                    buffer.truncate(new_len);
                    return Ok(x);
                }
                Err(Error::NotEnoughData) => {
                    Self::read_buff(port, buffer)?;
                }
                Err(Error::InvalidHeader) => {
                    offset += 1;
                }
                Err(e) => {
                    warn!("read invalid message: {:?}",e);
                    let new_len = buffer.len() - 1;
                    buffer.copy_within(1.., 0);
                    buffer.truncate(new_len);
                    bail!("{:?}", e);
                }
            }
        }
    }

    pub fn new(port: SerialPortBuilder) -> Result<Self> {
        trace!("opening serial port");
        let mut port = port.open_native().context("Failed to open serial port")?;

        let mut write = port.try_clone_native()?;

        let (send, mut t_recv) = mpsc::channel::<GpsMsg<'static>>(16);

        trace!("spawning write thread");
        thread::spawn(move || {
            let mut buffer = Vec::new();
            while let Some(x) = t_recv.blocking_recv() {
                trace!("write msg: {:?}", x);
                match x {
                    GpsMsg::Ubx(x) => {
                        buffer.clear();
                        x.write_bytes(&mut buffer);
                        write
                            .write_all(&buffer)
                            .map_err(|e| warn!("write error:{}", e))
                            .ok();
                    }
                    GpsMsg::Nmea(x) => {
                        write
                            .write_all(x.as_bytes())
                            .map_err(|e| warn!("write error:{}", e))
                            .ok();
                    }
                    GpsMsg::Rtcm(x) => {
                        write
                            .write_all(x.as_bytes())
                            .map_err(|e| warn!("write error:{}", e))
                            .ok();
                    }
                }
            }
        });

        let (t_send, recv) = mpsc::channel::<GpsMsg<'static>>(16);
        let (ack_send, mut ack_recv) = mpsc::channel::<AckMsg>(1);

        trace!("spawning read thread");
        thread::spawn(move || {
            let mut buffer = Vec::new();
            let mut ack = None;
            loop {
                match Self::read_msg(&mut port, &mut buffer) {
                    Err(e) => {
                        warn!("read error: {}", e)
                    }
                    Ok(x) => {
                        trace!("read msg: {:?}",x);
                        if let GpsMsg::Ubx(ubx::Msg::Ack(msg)) = x {
                            ack = ack.or_else(|| ack_recv.try_recv().ok());
                            if let Some(x) = ack.as_ref() {
                                match msg {
                                    ubx::Ack::Ack { msg_id, cls_id } => {
                                        if msg_id == x.id && cls_id == x.class {
                                            ack.take().unwrap().send.send(true).ok();
                                        }
                                    }
                                    ubx::Ack::Nak { msg_id, cls_id } => {
                                        if msg_id == x.id && cls_id == x.class {
                                            ack.take().unwrap().send.send(false).ok();
                                        }
                                    }
                                }
                            }
                        } else {
                            let x = x.into_owned();
                            if let Err(mpsc::error::TrySendError::Closed(_)) = t_send.try_send(x) {
                                return;
                            }
                        }
                    }
                }
            }
        });

        Ok(GpsDevice {
            ack_send,
            send,
            recv,
        })
    }

    pub async fn read(&mut self) -> GpsMsg {
        self.recv
            .recv()
            .await
            .expect("read thread quit unexpectedly")
    }

    pub async fn config(&self, cfg: Cfg) -> oneshot::Receiver<bool> {
        let (send, recv) = oneshot::channel();
        self.ack_send
            .send(AckMsg {
                send,
                id: cfg.id(),
                class: Self::CFG_VALSET_CLASS,
            })
            .await
            .expect("read thread quit unexpectedly");
        self.send
            .send(GpsMsg::Ubx(ubx::Msg::Cfg(cfg)))
            .await
            .expect("read thread quit unexpectedly");
        recv
    }

    pub async fn write(&self, msg: GpsMsg<'_>) {
        self.send
            .send(msg.into_owned())
            .await
            .expect("write thread quit unexpectedly");
    }
}
