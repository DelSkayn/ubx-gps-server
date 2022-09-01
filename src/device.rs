use anyhow::{bail, Result};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{oneshot, Semaphore},
};

use crate::{
    parse,
    ubx,
    GpsMsg,
};

pub struct GpsDevice<F> {
    stream: F,
    read_buffer: [u8; 1024],
    write_buffer: Vec<u8>,
    buffer: Vec<u8>,
    ack_sem: Semaphore,
    pending_ack: Option<(u8, oneshot::Sender<bool>)>,
}

impl<F> GpsDevice<F> {
    pub fn new(stream: F) -> Self {
        GpsDevice {
            stream,
            read_buffer: [0u8; 1024],
            write_buffer: Vec::new(),
            buffer: Vec::new(),
            ack_sem: Semaphore::new(1),
            pending_ack: None,
        }
    }
}

impl<F> GpsDevice<F>
where
    F: AsyncRead + AsyncWrite + AsyncReadExt + Unpin,
{
    pub async fn extend_buffer(&mut self) -> Result<()> {
        let len = self.stream.read(&mut self.read_buffer).await?;
        self.buffer.extend_from_slice(&self.read_buffer[..len]);
        Ok(())
    }

    async fn read_raw(&mut self) -> Result<GpsMsg<'static>> {
        loop {
            match GpsMsg::from_bytes(self.buffer.as_slice()) {
                Ok((msg, b)) => {
                    let msg = msg.into_owned();
                    let len = self.buffer.len();
                    self.buffer.copy_within(b.., 0);
                    self.buffer.truncate(len - b);
                    return Ok(msg);
                }
                Err(parse::Error::InvalidLen) => {}
                Err(e) => bail!(e),
            }
        }
    }

    pub async fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let len = self.stream.read(&mut self.read_buffer).await?;
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.read_buffer[..len]);
        Ok(buffer)
    }

    pub async fn read(&mut self) -> Result<GpsMsg<'static>> {
        if let Some(&(id,_)) = self.pending_ack.as_ref() {
            match self.read_raw().await? {
                GpsMsg::Ubx(ubx::Msg::Ack(ubx::Ack::Ack { msg_id, cls_id })) => {
                    if msg_id == id {
                        self.pending_ack.take().unwrap().1.send(true).ok();
                        self.ack_sem.add_permits(1);
                    }
                    Ok(GpsMsg::Ubx(ubx::Msg::Ack(ubx::Ack::Ack { msg_id, cls_id })))
                }
                GpsMsg::Ubx(ubx::Msg::Ack(ubx::Ack::Nak { msg_id, cls_id })) => {
                    if msg_id == id {
                        self.pending_ack.take().unwrap().1.send(false).ok();
                        self.ack_sem.add_permits(1);
                    }
                    Ok(GpsMsg::Ubx(ubx::Msg::Ack(ubx::Ack::Nak { msg_id, cls_id })))
                }
                x => Ok(x),
            }
        } else {
            self.read_raw().await
        }
    }

    pub async fn write(&mut self, m: GpsMsg<'_>) -> Result<()> {
        self.write_buffer.clear();
        m.write_bytes(&mut self.write_buffer);
        self.stream.write_all(&self.write_buffer).await?;
        Ok(())
    }

    pub async fn write_raw(&mut self, m: &[u8]) -> Result<()> {
        self.stream.write_all(m).await?;
        Ok(())
    }

    pub async fn config(&mut self, msg: ubx::Cfg) -> Result<oneshot::Receiver<bool>> {
        let perm = self.ack_sem.acquire().await.unwrap();

        let (send, recv) = oneshot::channel();

        self.pending_ack = Some((msg.id(), send));

        self.write_buffer.clear();
        GpsMsg::Ubx(ubx::Msg::Cfg(msg)).write_bytes(&mut self.write_buffer);

        self.stream.write_all(&self.write_buffer).await?;
        perm.forget();

        Ok(recv)
    }
}
