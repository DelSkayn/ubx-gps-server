use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use anyhow::{bail, Result};
use bluer::{
    gatt::{remote::Characteristic, CharacteristicReader, CharacteristicWriter},
    Adapter, AdapterEvent, Device, Session,
};
use futures::{pin_mut, Sink, Stream, StreamExt};
use log::{error, info};
use pin_project::pin_project;
use tokio::time::sleep;

use crate::{
    bluetooth::{CHARACTERISTIC_UUID, SERVICE_UUID},
    connection::{MessageSink, MessageStream},
};

#[pin_project]
pub struct BluetoothClient {
    session: Session,
    adapter: Adapter,
    #[pin]
    writer: MessageSink<CharacteristicWriter>,
    #[pin]
    reader: MessageStream<CharacteristicReader>,
}

impl BluetoothClient {
    async fn find_characteristic(device: &Device) -> Result<Option<Characteristic>> {
        let addr = device.address();
        let uuids = device.uuids().await?.unwrap_or_default();
        let md = device.manufacturer_data().await?;
        info!(
            "discovered bluetooth device {} with service UUID {:?}\n\t manufacture data{:x?}",
            addr, &uuids, &md
        );

        if uuids.contains(&SERVICE_UUID) {
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
            }
        } else {
            info!("already connected to bluetooth device!");
        }

        info!("enumerating services");
        for service in device.services().await? {
            let uuid = service.uuid().await?;

            info!("\tservice uuid: {}", &uuid);
            info!("\tservice data: {:?}", service.all_properties().await?);
            if uuid == SERVICE_UUID {
                info!("\tfound service");
                for chari in service.characteristics().await? {
                    let uuid = chari.uuid().await?;
                    info!("\tcharacteristics uuid: {}", &uuid);
                    info!(
                        "\tcharacteristics data: {:?}",
                        chari.all_properties().await?
                    );
                    if uuid == CHARACTERISTIC_UUID {
                        info!("found our characteristics");
                        return Ok(Some(chari));
                    }
                }
            }
        }
        info!("\t not found");
        Ok(None)
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

        let char = loop {
            if let Some(evt) = discover.next().await {
                match evt {
                    AdapterEvent::DeviceAdded(addr) => {
                        let device = adapter.device(addr)?;
                        match Self::find_characteristic(&device).await {
                            Ok(Some(char)) => {
                                break char;
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

        info!("acquiring connection");
        let reader = char.notify_io().await?;
        let reader = MessageStream::new(reader);
        let writer = char.write_io().await?;
        let writer = MessageSink::new(writer);

        Ok(BluetoothClient {
            session,
            adapter,
            reader,
            writer,
        })
    }
}

impl Stream for BluetoothClient {
    type Item = Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .reader
            .poll_next(cx)
            .map_err(anyhow::Error::from)
    }
}

impl Sink<Vec<u8>> for BluetoothClient {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: Vec<u8>) -> Result<(), Self::Error> {
        self.project().writer.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project().writer.poll_close(cx)
    }
}
