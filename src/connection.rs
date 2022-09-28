use std::{
    io::Result,
    mem::MaybeUninit,
    net::SocketAddr,
    pin::Pin,
    result::Result as StdResult,
    task::{Context, Poll},
    time::Duration,
};

use crate::VecExt;

use futures::{stream::FusedStream, Future, FutureExt, Sink, Stream, StreamExt};
use log::{error, info};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    time::Sleep,
};

pub enum OutgoingConnectionState {
    Start,
    Waiting(Pin<Box<Sleep>>),
    Connecting(Pin<Box<dyn Future<Output = Result<TcpStream>>>>),
    Connected(Pin<Box<Connection>>),
}

#[pin_project]
pub struct OutgoingConnection {
    connection: OutgoingConnectionState,
    address: Option<SocketAddr>,
}

impl OutgoingConnection {
    pub fn new(address: Option<SocketAddr>) -> Self {
        OutgoingConnection {
            connection: OutgoingConnectionState::Start,
            address,
        }
    }

    pub async fn try_send_message(&mut self, message: &[u8]) -> bool {
        if let OutgoingConnectionState::Connected(ref mut x) = self.connection {
            if let Err(e) = x.write_message(message).await {
                error!("error writing to outgoing connection {e}");
                let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                self.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                false
            } else {
                true
            }
        } else {
            false
        }
    }
}

impl FusedStream for OutgoingConnection {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl Stream for OutgoingConnection {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        loop {
            match *this.connection {
                OutgoingConnectionState::Start => {
                    if let Some(x) = this.address.as_ref() {
                        let open = TcpStream::connect(x.clone());
                        *this.connection = OutgoingConnectionState::Connecting(Box::pin(open));
                    } else {
                        return Poll::Pending;
                    }
                }
                OutgoingConnectionState::Waiting(ref mut x) => match x.poll_unpin(cx) {
                    Poll::Ready(_) => {
                        *this.connection = OutgoingConnectionState::Start;
                    }
                    Poll::Pending => return Poll::Pending,
                },
                OutgoingConnectionState::Connecting(ref mut x) => match x.poll_unpin(cx) {
                    Poll::Ready(Ok(x)) => {
                        let connection = Connection::new(x);
                        *this.connection = OutgoingConnectionState::Connected(Box::pin(connection));
                    }
                    Poll::Ready(Err(e)) => {
                        error!("error connecting to outgoing server {}", e);
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        *this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Pending => return Poll::Pending,
                },
                OutgoingConnectionState::Connected(ref mut x) => match x.poll_next_unpin(cx) {
                    Poll::Ready(None) => {
                        info!("outgoing connection quit");
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        *this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Ready(Some(Err(e))) => {
                        error!("error reading from outgoing connection {}", e);
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        *this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Ready(Some(Ok(x))) => return Poll::Ready(Some(x)),
                    Poll::Pending => return Poll::Pending,
                },
            }
        }
    }
}

pub enum WriteState {
    None,
    PendingLen { remaining_len: usize, data: Vec<u8> },
    Pending(Vec<u8>),
}

#[pin_project]
pub struct ConnectionPool {
    listener: TcpListener,
    connections: Vec<Pin<Box<Connection>>>,
    send: Option<(usize, Vec<u8>)>,
}

impl ConnectionPool {
    pub fn new(listener: TcpListener) -> Self {
        ConnectionPool {
            listener,
            connections: Vec::new(),
            send: None,
        }
    }
}

impl FusedStream for ConnectionPool {
    fn is_terminated(&self) -> bool {
        false
    }
}

impl Stream for ConnectionPool {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        loop {
            match this.listener.poll_accept(cx) {
                Poll::Ready(Ok((x, addr))) => {
                    info!("new connection from {}", addr);
                    this.connections.push(Box::pin(Connection::new(x)));
                    continue;
                }
                Poll::Ready(Err(e)) => {
                    error!("error accepting connection {}", e);
                }
                Poll::Pending => {}
            }

            for i in (0..this.connections.len()).rev() {
                match this.connections[i].as_mut().poll_next(cx) {
                    Poll::Ready(Some(Ok(x))) => return Poll::Ready(Some(x)),
                    Poll::Ready(Some(Err(e))) => {
                        error!("error from connection {:?}", e);
                        this.connections.swap_remove(i);
                    }
                    Poll::Ready(None) => {
                        info!("connection quit");
                        this.connections.swap_remove(i);
                    }
                    Poll::Pending => {}
                }
            }

            return Poll::Pending;
        }
    }
}

impl Sink<Vec<u8>> for ConnectionPool {
    type Error = ();

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        let this = self.project();

        loop {
            if let Some((idx, data)) = this.send.as_mut() {
                match this.connections[*idx].as_mut().poll_ready(cx) {
                    Poll::Ready(Err(e)) => {
                        error!("error sending to connection: {}", e);
                        this.connections.swap_remove(*idx);
                        if *idx == 0 {
                            *this.send = None;
                        } else {
                            *idx -= 1;
                        }
                    }
                    Poll::Ready(Ok(())) => {
                        if let Err(e) = this.connections[*idx].as_mut().start_send(data.clone()) {
                            error!("error sending to connection: {}", e);
                            this.connections.swap_remove(*idx);
                        }
                        if *idx == 0 {
                            *this.send = None;
                        } else {
                            *idx -= 1;
                        }
                    }
                    Poll::Pending => return Poll::Pending,
                }
            } else {
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> StdResult<(), Self::Error> {
        let this = self.project();
        *this.send = Some((this.connections.len() - 1, item));
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        self.poll_ready(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<StdResult<(), Self::Error>> {
        self.poll_ready(cx)
    }
}

#[pin_project]
pub struct Connection {
    pending: Option<u32>,
    buffer: Vec<u8>,
    read_buffer: [MaybeUninit<u8>; 4096],
    write_pending: WriteState,
    tcp: Pin<Box<TcpStream>>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Connection {
            pending: None,
            buffer: Vec::new(),
            read_buffer: [MaybeUninit::uninit(); 4096],
            write_pending: WriteState::None,
            tcp: Box::pin(stream),
        }
    }

    pub async fn write_message(&mut self, data: &[u8]) -> Result<()> {
        let len = u32::try_from(data.len()).expect("message length to long");
        let len = len.to_le_bytes();
        self.tcp.write_all(&len).await?;
        self.tcp.write_all(data).await
    }
}

impl Stream for Connection {
    type Item = Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        loop {
            let mut buffer = ReadBuf::uninit(this.read_buffer);
            match this.tcp.as_mut().poll_read(cx, &mut buffer) {
                Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let filled = buffer.filled();
                    if filled.is_empty() {
                        return Poll::Ready(None);
                    }
                    this.buffer.extend(filled);
                }
                Poll::Pending => return Poll::Pending,
            }

            if this.pending.is_none() {
                match <[u8; 4]>::try_from(this.buffer.as_slice()) {
                    Ok(x) => {
                        this.buffer.shift(4);
                        *this.pending = Some(u32::from_le_bytes(x));
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }

            if let Some(pending) = this.pending {
                if this.buffer.len() >= *pending as usize {
                    let mut res = this.buffer.split_off(*pending as usize);
                    std::mem::swap(&mut res, this.buffer);
                    return Poll::Ready(Some(Ok(res)));
                }
            }
        }
    }
}

impl Sink<Vec<u8>> for Connection {
    type Error = std::io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();

        loop {
            match std::mem::replace(this.write_pending, WriteState::None) {
                WriteState::None => return Poll::Ready(Ok(())),
                WriteState::PendingLen {
                    remaining_len,
                    data,
                } => {
                    let buffer = (data.len() as u32).to_le_bytes();
                    match this.tcp.as_mut().poll_write(cx, &buffer) {
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(x)) => {
                            let new_len = remaining_len - x;
                            if new_len == 0 {
                                *this.write_pending = WriteState::Pending(data);
                            } else {
                                *this.write_pending = WriteState::PendingLen {
                                    remaining_len: new_len,
                                    data,
                                };
                            }
                        }
                    }
                }
                WriteState::Pending(mut data) => match this.tcp.as_mut().poll_write(cx, &data) {
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(x)) => {
                        let len = data.len();
                        if x == len {
                            return Poll::Ready(Ok(()));
                        }
                        data.shift(x);
                        *this.write_pending = WriteState::Pending(data);
                    }
                },
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<()> {
        self.write_pending = WriteState::PendingLen {
            remaining_len: 4,
            data: item,
        };
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();

        loop {
            match std::mem::replace(this.write_pending, WriteState::None) {
                WriteState::None => return this.tcp.as_mut().poll_flush(cx),
                WriteState::PendingLen {
                    remaining_len,
                    data,
                } => {
                    let buffer = (data.len() as u32).to_le_bytes();
                    match this.tcp.as_mut().poll_write(cx, &buffer) {
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(x)) => {
                            let new_len = remaining_len - x;
                            if new_len == 0 {
                                *this.write_pending = WriteState::Pending(data);
                            } else {
                                *this.write_pending = WriteState::PendingLen {
                                    remaining_len: new_len,
                                    data,
                                };
                            }
                        }
                    }
                }
                WriteState::Pending(mut data) => match this.tcp.as_mut().poll_write(cx, &data) {
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(x)) => {
                        let len = data.len();
                        if x == len {
                            return Poll::Ready(Ok(()));
                        }
                        data.shift(x);
                        *this.write_pending = WriteState::Pending(data);
                    }
                },
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = self.project();

        loop {
            match std::mem::replace(this.write_pending, WriteState::None) {
                WriteState::None => return this.tcp.as_mut().poll_shutdown(cx),
                WriteState::PendingLen {
                    remaining_len,
                    data,
                } => {
                    let buffer = (data.len() as u32).to_le_bytes();
                    match this.tcp.as_mut().poll_write(cx, &buffer) {
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(x)) => {
                            let new_len = remaining_len - x;
                            if new_len == 0 {
                                *this.write_pending = WriteState::Pending(data);
                            } else {
                                *this.write_pending = WriteState::PendingLen {
                                    remaining_len: new_len,
                                    data,
                                };
                            }
                        }
                    }
                }
                WriteState::Pending(mut data) => match this.tcp.as_mut().poll_write(cx, &data) {
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(x)) => {
                        let len = data.len();
                        if x == len {
                            return Poll::Ready(Ok(()));
                        }
                        data.shift(x);
                        *this.write_pending = WriteState::Pending(data);
                    }
                },
            }
        }
    }
}
