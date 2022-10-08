use std::{
    collections::BTreeMap,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::{Context as ErrorContext, Result};
use bluer::{
    adv::{Advertisement, AdvertisementHandle},
    l2cap::{Stream, StreamListener},
    Adapter, AddressType, Session,
};
use futures::{Sink, Stream as StreamTrait};
use log::{error, info};
use pin_project::pin_project;

use crate::{
    bluetooth::{MANUFACTURER_ID, SERVICE_UUID},
    connection::{MessageSink, MessageStream},
};

#[pin_project]
pub struct BluetoothServer {
    session: Session,
    adapter: Adapter,
    advert_handle: AdvertisementHandle,
    #[pin]
    listener: StreamListener,
    streams: Vec<MessageSink<MessageStream<Stream>>>,
}

impl BluetoothServer {
    pub async fn new() -> Result<Self> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        let address = adapter.address().await?;

        info!(
            "running on bluetooth adapter `{}` with address `{}`",
            adapter.name(),
            address,
        );

        let mut manufacturer_data = BTreeMap::new();
        manufacturer_data.insert(MANUFACTURER_ID, vec![0x21, 0x22, 0x23, 0x24]);

        let advert = Advertisement {
            service_uuids: Some(SERVICE_UUID).into_iter().collect(),
            manufacturer_data,
            discoverable: Some(true),
            local_name: Some("gps_server".to_string()),
            ..Default::default()
        };
        let ad_handle = adapter.advertise(advert).await?;

        let address =
            bluer::l2cap::SocketAddr::new(address, AddressType::LePublic, super::PSM_LE_ADDR);
        let listener = StreamListener::bind(address)
            .await
            .context("failed to create bluetooth stream listener")?;

        Ok(BluetoothServer {
            session,
            adapter,
            advert_handle: ad_handle,
            listener,
            streams: Vec::new(),
        })
    }

    pub fn poll_accept(&mut self, cx: &mut Context) -> Result<()> {
        loop {
            match self.listener.poll_accept(cx) {
                Poll::Ready(Ok((stream, addr))) => {
                    info!("new bluetooth connection from {:?}", addr);
                    self.streams
                        .push(MessageSink::new(MessageStream::new(stream)));
                }
                Poll::Ready(Err(e)) => {
                    error!("error accepting bluetooth connection: {e:?}");
                }
                Poll::Pending => return Ok(()),
            }
        }
    }
}

impl StreamTrait for BluetoothServer {
    type Item = Vec<u8>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.as_mut().poll_accept(cx) {
            Ok(()) => {}
            Err(e) => {
                error!("error accepting incomming bluetooth messages: {e}");
            }
        }

        for idx in (0..self.streams.len()).rev() {
            match Pin::new(&mut self.streams[idx]).poll_next(cx) {
                Poll::Pending => {}
                Poll::Ready(Some(Ok(x))) => return Poll::Ready(Some(x)),
                Poll::Ready(Some(Err(e))) => {
                    error!("error reading bluetooth messages: {e}");
                    self.streams.swap_remove(idx);
                }
                Poll::Ready(None) => {
                    info!("bluetooth reader quit");
                    self.streams.swap_remove(idx);
                }
            }
        }

        Poll::Pending
    }
}

impl Sink<Vec<u8>> for BluetoothServer {
    type Error = ();

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.streams
            .retain_mut(|i| match Pin::new(i).poll_ready(cx) {
                Poll::Ready(Ok(())) => true,
                Poll::Pending => {
                    res = Poll::Pending;
                    true
                }
                Poll::Ready(Err(e)) => {
                    error!("error writing to bluetooth connection: {e}");
                    false
                }
            });

        res
    }

    fn start_send(mut self: std::pin::Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        self.streams
            .retain_mut(|i| match Pin::new(i).start_send(item.clone()) {
                Ok(_) => true,
                Err(e) => {
                    error!("error writing message to bluetooth sink; {e:?}");
                    false
                }
            });
        Ok(())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.streams
            .retain_mut(|i| match Pin::new(i).poll_flush(cx) {
                Poll::Ready(Ok(())) => true,
                Poll::Pending => {
                    res = Poll::Pending;
                    true
                }
                Poll::Ready(Err(e)) => {
                    error!("error flushing bluetooth connection: {e}");
                    false
                }
            });

        res
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.streams
            .retain_mut(|i| match Pin::new(i).poll_flush(cx) {
                Poll::Ready(Ok(())) => true,
                Poll::Pending => {
                    res = Poll::Pending;
                    true
                }
                Poll::Ready(Err(e)) => {
                    error!("error flushin bluetooth connection: {e}");
                    false
                }
            });

        res
    }
}
