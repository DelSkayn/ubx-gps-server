use std::{
    io::Error as IoError,
    mem::MaybeUninit,
    pin::Pin,
    task::{Context, Poll},
};

use crate::VecExt;

use anyhow::Error;
use futures::{Sink, Stream};
use pin_project::pin_project;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};

pub mod pool;
pub use pool::ConnectionPool;

pub mod outgoing;
pub use outgoing::OutgoingConnection;

pub struct MessageStream<T> {
    pending: Option<u32>,
    buffer: Vec<u8>,
    pub source: T,
}

impl<T> MessageStream<T> {
    pub fn new(t: T) -> Self {
        MessageStream {
            pending: None,
            buffer: Vec::new(),
            source: t,
        }
    }
}

impl<T: AsyncRead + Unpin> Stream for MessageStream<T> {
    type Item = Result<Vec<u8>, IoError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        loop {
            if this.pending.is_none() && this.buffer.len() >= 4 {
                let array = <[u8; 4]>::try_from(&this.buffer[..4]).unwrap();
                let len = u32::from_le_bytes(array);
                // shift the len bytes out
                this.buffer.shift(4);
                this.pending = Some(len);
            }

            if let Some(pending) = this.pending.take() {
                if this.buffer.len() >= pending as usize {
                    let mut res = this.buffer.split_off(pending as usize);
                    std::mem::swap(&mut res, &mut this.buffer);
                    return Poll::Ready(Some(Ok(res)));
                }
                this.pending = Some(pending);
            }

            let mut read_buffer = [MaybeUninit::uninit(); 4096];
            let mut buffer = ReadBuf::uninit(&mut read_buffer);
            match Pin::new(&mut this.source).poll_read(cx, &mut buffer) {
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
        }
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for MessageStream<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, IoError>> {
        Pin::new(&mut self.source).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
        Pin::new(&mut self.source).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
        Pin::new(&mut self.source).poll_shutdown(cx)
    }
}

pub enum WriteState {
    Ready,
    WritingLength { written: usize, data: Vec<u8> },
    WritingData { written: usize, data: Vec<u8> },
}

#[pin_project]
pub struct MessageSink<T> {
    state: WriteState,
    #[pin]
    pub source: T,
}

impl<T> MessageSink<T> {
    pub fn new(t: T) -> Self {
        MessageSink {
            state: WriteState::Ready,
            source: t,
        }
    }
}

impl<T: AsyncWrite + Unpin> MessageSink<T> {
    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        loop {
            match std::mem::replace(&mut self.state, WriteState::Ready) {
                WriteState::Ready => return Poll::Ready(Ok(())),
                WriteState::WritingLength { mut written, data } => {
                    let len = u32::try_from(data.len())
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    let buffer = len.to_le_bytes();

                    match Pin::new(&mut self.source).poll_write(cx, &buffer[written..]) {
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::from(e))),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(x)) => {
                            written += x;

                            if written >= 4 {
                                self.state = WriteState::WritingData { written: 0, data };
                            } else {
                                self.state = WriteState::WritingLength { written, data };
                            }
                        }
                    }
                }
                WriteState::WritingData { mut written, data } => {
                    match Pin::new(&mut self.source).poll_write(cx, &data[written..]) {
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::from(e))),
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(x)) => {
                            written += x;
                            if written >= data.len() {
                                return Poll::Ready(Ok(()));
                            }
                            self.state = WriteState::WritingData { written, data };
                        }
                    }
                }
            }
        }
    }
}

impl<T: AsyncWrite + Unpin> Sink<Vec<u8>> for MessageSink<T> {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.poll_flush(cx).map_err(anyhow::Error::from)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Error> {
        let mut this: &mut Self = &mut *self;

        this.state = WriteState::WritingLength {
            written: 0,
            data: item,
        };
        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this: &mut Self = &mut *self;
        match this.poll_flush(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Ready(Ok(())) => Pin::new(&mut this.source)
                .poll_flush(cx)
                .map_err(Error::from),
        }
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this: &mut Self = &mut *self;

        match this.poll_flush(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Ready(Ok(())) => Pin::new(&mut this.source)
                .poll_shutdown(cx)
                .map_err(Error::from),
        }
    }
}

impl<T: Stream> Stream for MessageSink<T> {
    type Item = T::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.source.poll_next(cx)
    }
}

#[pin_project]
pub struct Connection {
    #[pin]
    inner: MessageSink<MessageStream<TcpStream>>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Connection {
            inner: MessageSink::new(MessageStream::new(stream)),
        }
    }

    pub async fn write_message(&mut self, data: &[u8]) -> Result<(), IoError> {
        self.inner.source.flush().await?;
        let len = u32::try_from(data.len())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let len = len.to_le_bytes();
        self.inner.source.write_all(&len).await?;
        self.inner.source.write_all(data).await?;
        self.inner.source.flush().await
    }
}

impl Stream for Connection {
    type Item = Result<Vec<u8>, IoError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let write = this.inner.project();
        write.source.poll_next(cx)
    }
}

impl Sink<Vec<u8>> for Connection {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.project();

        this.inner.poll_flush(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Error> {
        let this = self.project();

        this.inner.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.project();

        this.inner.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.project();

        this.inner.poll_close(cx)
    }
}
