use std::borrow::Cow;

use anyhow::Result;
use log::{warn, info};
use serde::Serialize;
use tokio::{
    io::{AsyncWriteExt, AsyncReadExt},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};

use crate::GpsMsg;


pub struct Msg<'a>(Cow<'a,[u8]>);

impl<'a> Msg<'a>{
    pub async fn from_reader<R>(r: &mut R) -> Result<Msg<'static>>
        where R: AsyncReadExt + Unpin,
    {
        let size = r.read_u32_le().await?;
        let mut data = vec![0u8; size as usize];

        r.read_exact(&mut data).await?;

        Ok(Msg(Cow::Owned(data)))
    }

    pub fn from_vec(vec: Vec<u8>) -> Msg<'static>{
        Msg(vec.into())
    }


    pub async fn write<W>(&self,w: &mut W) -> Result<()>
        where W: AsyncWriteExt + Unpin,
    {
        let len: u32 = self.0.len().try_into().unwrap();
        w.write_u32_le(len).await?;
        w.write_all(self.0.as_ref()).await?;

        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8]{
        self.0.as_ref()
    }
}

pub struct StreamServer {
    raw: bool,
    listener: TcpListener,
    connections: Vec<TcpStream>,
}

impl StreamServer {
    pub async fn new<A: ToSocketAddrs>(a: A,raw: bool) -> Result<Self> {
        let listener = TcpListener::bind(a).await?;
        Ok(StreamServer {
            raw,
            listener,
            connections: Vec::new(),
        })
    }

    pub async fn recv(&mut self) -> Result<GpsMsg<'static>> {
        loop {
            let (incomming,addr) = self.listener.accept().await?;
            info!("recieved connection from {}",addr);
            self.connections.push(incomming);
        }
    }

    pub async fn send(&mut self, d: &GpsMsg<'_>) -> Result<()> {
        let data = if self.raw{
            let mut res = Vec::new();
            d.write_bytes(&mut res);
            res
        } else {
            serde_json::to_vec(d)?
        };
        let msg = Msg::from_vec(data);
        let future = self.connections.iter_mut().map(|x| 
            msg.write(x)
        );
        let res = futures::future::join_all(future).await;
        for (idx, r) in res.iter().enumerate().rev() {
            if let Err(e) = r {
                warn!("connection error: {:?}", e);
                self.connections.swap_remove(idx);
            }
        }
        Ok(())
    }
}
