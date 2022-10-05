use std::{
    pin::Pin,
    result::Result as StdResult,
    task::{Context, Poll},
};

use futures::{stream::FusedStream, Sink, Stream};
use log::{error, info, trace};
use tokio::net::TcpListener;

use super::Connection;

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

    fn poll_flush_out(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        trace!("ConnectionPool::poll_flush_out");
        loop {
            if let Some((idx, data)) = self.send.as_mut() {
                match self.connections[*idx].as_mut().poll_ready(cx) {
                    Poll::Ready(Err(e)) => {
                        error!("error sending to connection: {}", e);
                        self.connections.swap_remove(*idx);
                        if *idx == 0 {
                            self.send = None;
                        } else {
                            *idx -= 1;
                        }
                    }
                    Poll::Ready(Ok(())) => {
                        if let Err(e) = self.connections[*idx].as_mut().start_send(data.clone()) {
                            error!("error sending to connection: {}", e);
                            self.connections.swap_remove(*idx);
                        }
                        if *idx == 0 {
                            self.send = None;
                        } else {
                            *idx -= 1;
                        }
                    }
                    Poll::Pending => return Poll::Pending,
                }
            } else {
                return Poll::Ready(());
            }
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

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this: &mut Self = &mut *self;

        trace!("ConnectionPoll::poll_next");

        loop {
            match this.listener.poll_accept(cx) {
                Poll::Ready(Ok((x, addr))) => {
                    info!("new connection from {}", addr);
                    if let Err(e) = x.set_nodelay(true) {
                        error!("error setting no delay for connection {e}");
                        continue;
                    }
                    this.connections.push(Box::pin(Connection::new(x)));
                    continue;
                }
                Poll::Ready(Err(e)) => {
                    error!("error accepting connection {}", e);
                }
                Poll::Pending => {}
            }

            // reverse to make swap remove work
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

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        trace!("ConnectionPool::poll_ready");
        self.poll_flush_out(cx).map(Ok)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Vec<u8>) -> StdResult<(), Self::Error> {
        trace!("ConnectionPool::start_send");
        let this: &mut Self = &mut *self;
        if !this.connections.is_empty() {
            this.send = Some((this.connections.len() - 1, item));
        }
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        trace!("ConnectionPool::poll_flush");
        let this: &mut Self = &mut *self;
        match this.poll_flush_out(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                for c in (0..this.connections.len()).rev() {
                    match this.connections[c].as_mut().poll_flush(cx) {
                        Poll::Ready(Ok(())) => {}
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => {
                            error!("error connection {e}");
                            this.connections.swap_remove(c);
                        }
                    }
                }
                return Poll::Ready(Ok(()));
            }
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<StdResult<(), Self::Error>> {
        let this: &mut Self = &mut *self;
        match this.poll_flush_out(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                for c in (0..this.connections.len()).rev() {
                    match this.connections[c].as_mut().poll_close(cx) {
                        Poll::Ready(Ok(())) => {
                            this.connections.pop();
                        }
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => {
                            error!("error connection {e}");
                            this.connections.pop();
                        }
                    }
                }
                return Poll::Ready(Ok(()));
            }
        }
    }
}
