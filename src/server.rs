use std::borrow::Cow;

use anyhow::Result;
use futures::{future::Either, FutureExt};
use log::{error, info, warn};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use crate::GpsMsg;

pub struct Msg<'a>(Cow<'a, [u8]>);

impl<'a> Msg<'a> {
    pub async fn from_reader<R>(r: &mut R) -> Result<Msg<'static>>
    where
        R: AsyncReadExt + Unpin,
    {
        let size = r.read_u32_le().await?;
        let mut data = vec![0u8; size as usize];

        r.read_exact(&mut data).await?;

        Ok(Msg(Cow::Owned(data)))
    }

    pub fn from_vec(vec: Vec<u8>) -> Msg<'static> {
        Msg(vec.into())
    }

    pub async fn write<W>(&self, w: &mut W) -> Result<()>
    where
        W: AsyncWriteExt + Unpin,
    {
        let len: u32 = self.0.len().try_into().unwrap();
        w.write_u32_le(len).await?;
        w.write_all(self.0.as_ref()).await?;

        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

pub struct Connection {
    stream: TcpStream,
    read_buffer: [u8; 256],
    buffer: Vec<u8>,
}

impl Connection {
    async fn read_raw(&mut self) -> Result<Vec<u8>> {
        let len = self.stream.read(&mut self.read_buffer).await?;
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&self.read_buffer[..len]);
        Ok(buffer)
    }

    async fn read(&mut self) -> Result<()> {
        let len = self.stream.read(&mut self.read_buffer).await?;
        self.buffer.extend_from_slice(&self.read_buffer[..len]);
        Ok(())
    }

    fn read_msg(&mut self) -> Option<GpsMsg<'static>> {
        let (msg, size) = GpsMsg::from_bytes(&self.buffer).ok()?;
        let msg = msg.into_owned();
        let len = self.buffer.len();
        self.buffer.copy_within(size.., 0);
        self.buffer.truncate(len - size);
        Some(msg)
    }
}

pub struct StreamServer {
    raw: bool,
    listener: TcpListener,
    connections: Vec<Connection>,
}

impl StreamServer {
    pub async fn new<A: ToSocketAddrs>(a: A, raw: bool) -> Result<Self> {
        let listener = TcpListener::bind(a).await?;
        Ok(StreamServer {
            raw,
            listener,
            connections: Vec::new(),
        })
    }

    pub async fn recv_raw(&mut self) -> Vec<u8> {
        loop {
            let msg = {
                let recv_future = futures::future::select_all(
                    self.connections
                        .iter_mut()
                        .enumerate()
                        .map(|(idx, x)| x.read_raw().map(move |x| (idx, x)).boxed()),
                );
                let accept_future = self.listener.accept();
                match futures::future::select(recv_future, accept_future.boxed()).await {
                    Either::Left((msg, _)) => {
                        let (msg, _, _) = msg;
                        Either::Left(msg)
                    }
                    Either::Right((accept, _)) => Either::Right(accept),
                }
            };

            match msg {
                Either::Left(msg) => {
                    let (idx, msg) = msg;
                    match msg {
                        Err(e) => {
                            warn!("connection error: {:?}", e);
                            self.connections.swap_remove(idx);
                        }
                        Ok(x) => {
                            return x;
                        }
                    }
                }
                Either::Right(accept) => {
                    let accept = match accept {
                        Ok(x) => x,
                        Err(e) => {
                            error!("error accepting connection `{}`", e);
                            continue;
                        }
                    };
                    let (incomming, addr) = accept;
                    info!("recieved connection from {}", addr);
                    self.connections.push(Connection {
                        stream: incomming,
                        read_buffer: [0u8; 256],
                        buffer: Vec::new(),
                    });
                }
            }
        }
    }

    pub async fn recv(&mut self) -> GpsMsg<'static> {
        loop {
            if self.connections.is_empty() {
                let accept = match self.listener.accept().await {
                    Ok(x) => x,
                    Err(e) => {
                        error!("error accepting connection `{}`", e);
                        continue;
                    }
                };
                let (incomming, addr) = accept;
                info!("recieved connection from {}", addr);
                self.connections.push(Connection {
                    stream: incomming,
                    read_buffer: [0u8; 256],
                    buffer: Vec::new(),
                });
                continue;
            }

            let msg = {
                let recv_future = futures::future::select_all(
                    self.connections
                        .iter_mut()
                        .enumerate()
                        .map(|(idx, x)| x.read().map(move |x| (idx, x)).boxed()),
                );
                let accept_future = self.listener.accept();

                match futures::future::select(recv_future, accept_future.boxed()).await {
                    Either::Left((msg, _)) => {
                        let (msg, _, _) = msg;
                        Either::Left(msg)
                    }
                    Either::Right((accept, _)) => Either::Right(accept),
                }
            };

            match msg {
                Either::Left(msg) => {
                    let (idx, msg) = msg;
                    if let Err(e) = msg {
                        warn!("connection error: {:?}", e);
                        self.connections.swap_remove(idx);
                    } else if let Some(x) = self.connections[idx].read_msg() {
                        return x;
                    }
                }
                Either::Right(accept) => {
                    let accept = match accept {
                        Ok(x) => x,
                        Err(e) => {
                            error!("error accepting connection `{}`", e);
                            continue;
                        }
                    };
                    let (incomming, addr) = accept;
                    info!("recieved connection from {}", addr);
                    self.connections.push(Connection {
                        stream: incomming,
                        read_buffer: [0u8; 256],
                        buffer: Vec::new(),
                    });
                }
            }
        }
    }

    pub async fn send_raw(&mut self, d: &[u8]) -> Result<()> {
        let future = self.connections.iter_mut().map(|x| x.stream.write_all(d));
        let res = futures::future::join_all(future).await;
        for (idx, r) in res.iter().enumerate().rev() {
            if let Err(e) = r {
                warn!("connection error: {:?}", e);
                self.connections.swap_remove(idx);
            }
        }
        Ok(())
    }

    pub async fn send(&mut self, d: &GpsMsg<'_>) -> Result<()> {
        let data = if self.raw {
            let mut res = Vec::new();
            d.write_bytes(&mut res);
            res
        } else {
            serde_json::to_vec(d)?
        };
        let len = u32::try_from(data.len()).unwrap().to_le_bytes();
        self.send_raw(&len).await?;
        self.send_raw(&data).await?;
        Ok(())
    }
}
