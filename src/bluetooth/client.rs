use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use anyhow::{bail, Context as ErrorContext, Result};
use bluer::{
    l2cap::{SocketAddr, Stream},
    Adapter, AdapterEvent, Device, Session,
};
use futures::{pin_mut, Sink, Stream as StreamTrait, StreamExt};
use log::{error, info};
use pin_project::pin_project;
use tokio::time::sleep;

use crate::connection::{MessageSink, MessageStream};

#[pin_project]
pub struct BluetoothClient {
    session: Session,
    adapter: Adapter,
    #[pin]
    source: MessageSink<MessageStream<Stream>>,
}

impl BluetoothClient {
    async fn find_address(device: &Device) -> Result<Option<SocketAddr>> {
        let addr = device.address();
        let uuids = device.uuids().await?.unwrap_or_default();
        let md = device.manufacturer_data().await?;
        info!(
            "discovered bluetooth device {} with service UUID {:?}\n\t manufacture data{:x?}",
            addr, &uuids, &md
        );

        if !uuids.contains(&super::SERVICE_UUID) {
            return Ok(None);
        }
        info!("found device with our service");

        sleep(Duration::from_secs(2)).await;
        if !device.is_connected().await? {
            info!("trying to connect to device");
            loop {
                match device.connect().await {
                    Ok(()) => break,
                    Err(err) => {
                        error!("error connecting to device: {}", err);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            info!("connected to bluetooth device!");
        } else {
            info!("already connected to device");
        }

        Ok(Some(SocketAddr::new(
            addr,
            bluer::AddressType::LePublic,
            super::PSM_LE_ADDR,
        )))
    }

    pub async fn new() -> Result<Self> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;

        adapter.set_powered(true).await?;

        info!(
            "discovering on bluetooth adapter {} with address {}",
            adapter.name(),
            adapter.address().await?
        );

        let discover = adapter.discover_devices().await?;
        pin_mut!(discover);

        let address = loop {
            if let Some(evt) = discover.next().await {
                match evt {
                    AdapterEvent::DeviceAdded(addr) => {
                        let device = adapter.device(addr)?;
                        match Self::find_address(&device).await {
                            Ok(Some(address)) => {
                                break address;
                            }
                            Ok(None) => {}
                            Err(err) => {
                                info!("device connection failed {err}");
                                adapter.remove_device(device.address()).await.ok();
                            }
                        }
                    }
                    AdapterEvent::DeviceRemoved(addr) => {
                        info!("device removed {addr}")
                    }
                    _ => {}
                }
            } else {
                bail!("discovery quit")
            }
        };

        let stream = Stream::connect(address)
            .await
            .context("could not connect to bluetooth client")?;

        let source = MessageSink::new(MessageStream::new(stream));

        Ok(BluetoothClient {
            session,
            adapter,
            source,
        })
    }
}

impl StreamTrait for BluetoothClient {
    type Item = Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .source
            .poll_next(cx)
            .map_err(anyhow::Error::from)
    }
}

impl Sink<Vec<u8>> for BluetoothClient {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().source.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        self.project().source.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().source.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().source.poll_close(cx)
    }
}
