use std::{
    io::Result,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures::{stream::FusedStream, Future, FutureExt, Stream, StreamExt};
use log::{error, info};
use tokio::{net::TcpStream, time::Sleep};

use super::Connection;

pub enum OutgoingConnectionState {
    Start,
    Waiting(Pin<Box<Sleep>>),
    Connecting(Pin<Box<dyn Future<Output = Result<TcpStream>>>>),
    Connected(Pin<Box<Connection>>),
}

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

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this: &mut Self = &mut *self;

        loop {
            match this.connection {
                OutgoingConnectionState::Start => {
                    if let Some(x) = this.address.as_ref() {
                        let open = TcpStream::connect(x.clone());
                        this.connection = OutgoingConnectionState::Connecting(Box::pin(open));
                    } else {
                        return Poll::Pending;
                    }
                }
                OutgoingConnectionState::Waiting(ref mut x) => match x.poll_unpin(cx) {
                    Poll::Ready(_) => {
                        this.connection = OutgoingConnectionState::Start;
                    }
                    Poll::Pending => return Poll::Pending,
                },
                OutgoingConnectionState::Connecting(ref mut x) => match x.poll_unpin(cx) {
                    Poll::Ready(Ok(x)) => {
                        if let Err(e) = x.set_nodelay(true) {
                            error!("error setting connection to nodelay {e}");
                            let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                            this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                        } else {
                            let connection = Connection::new(x);
                            this.connection =
                                OutgoingConnectionState::Connected(Box::pin(connection));
                        }
                    }
                    Poll::Ready(Err(e)) => {
                        error!("error connecting to outgoing server {}", e);
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Pending => return Poll::Pending,
                },
                OutgoingConnectionState::Connected(ref mut x) => match x.poll_next_unpin(cx) {
                    Poll::Ready(None) => {
                        info!("outgoing connection quit");
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Ready(Some(Err(e))) => {
                        error!("error reading from outgoing connection {}", e);
                        let wait = tokio::time::sleep(Duration::from_secs_f32(0.5));
                        this.connection = OutgoingConnectionState::Waiting(Box::pin(wait));
                    }
                    Poll::Ready(Some(Ok(x))) => return Poll::Ready(Some(x)),
                    Poll::Pending => return Poll::Pending,
                },
            }
        }
    }
}
