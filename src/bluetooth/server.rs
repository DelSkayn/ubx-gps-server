use std::{
    collections::BTreeMap,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::{Context as ErrorContext, Result};
use bluer::{
    adv::{Advertisement, AdvertisementHandle},
    gatt::{
        local::{
            characteristic_control, service_control, Application, ApplicationHandle,
            Characteristic, CharacteristicControl, CharacteristicNotify,
            CharacteristicNotifyMethod, CharacteristicWrite, CharacteristicWriteMethod, Service,
            ServiceControl,
        },
        CharacteristicReader, CharacteristicWriter,
    },
    Adapter, Session,
};
use futures::{Sink, Stream};
use log::{debug, error, info};
use pin_project::pin_project;

use crate::{
    bluetooth::{CHARACTERISTIC_UUID, MANUFACTURER_ID, SERVICE_UUID},
    connection::{MessageSink, MessageStream},
};

#[pin_project]
pub struct BluetoothServer {
    session: Session,
    adapter: Adapter,
    advert_handle: AdvertisementHandle,
    #[pin]
    ctrl: CharacteristicControl,
    srvs: ServiceControl,
    app_handle: ApplicationHandle,
    writers: Vec<MessageSink<CharacteristicWriter>>,
    readers: Vec<MessageStream<CharacteristicReader>>,
}

impl BluetoothServer {
    pub async fn new() -> Result<Self> {
        let session = Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        info!(
            "running on bluetooth adapter `{}` with address `{}`",
            adapter.name(),
            adapter.address().await?
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

        info!("serving GATT service");
        let (srvs, srvs_handle) = service_control();
        let (ctrl, ctrl_handle) = characteristic_control();
        let app = Application {
            services: vec![Service {
                uuid: SERVICE_UUID,
                primary: true,
                characteristics: vec![Characteristic {
                    uuid: CHARACTERISTIC_UUID,
                    write: Some(CharacteristicWrite {
                        write: true,
                        write_without_response: true,
                        method: CharacteristicWriteMethod::Io,
                        ..Default::default()
                    }),
                    notify: Some(CharacteristicNotify {
                        notify: true,
                        method: CharacteristicNotifyMethod::Io,
                        ..Default::default()
                    }),
                    control_handle: ctrl_handle,
                    ..Default::default()
                }],
                control_handle: srvs_handle,
                ..Default::default()
            }],
            ..Default::default()
        };
        let app_handle = adapter.serve_gatt_application(app).await?;

        debug!("service handle is 0x{:x}", srvs.handle()?);
        debug!("characteristic handle is 0x{:x}", ctrl.handle()?);

        Ok(BluetoothServer {
            session,
            adapter,
            advert_handle: ad_handle,
            ctrl,
            srvs,
            app_handle,
            writers: Vec::new(),
            readers: Vec::new(),
        })
    }

    pub fn poll_accept(&mut self, cx: &mut Context) -> Result<()> {
        use bluer::gatt::local::CharacteristicControlEvent::*;

        loop {
            let p = Pin::new(&mut self.ctrl);
            match p.poll_next(cx) {
                Poll::Ready(Some(Write(x))) => {
                    let x: CharacteristicReader =
                        x.accept().context("failed to accept write request")?;
                    info!("accepted new bluetooth sender");
                    self.readers.push(MessageStream::new(x));
                }
                Poll::Ready(Some(Notify(x))) => {
                    info!("accepted new bluetooth reciever");
                    self.writers.push(MessageSink::new(x));
                }
                Poll::Ready(None) => {
                    panic!("bluetooth controller quit")
                }
                Poll::Pending => return Ok(()),
            }
        }
    }
}

impl Stream for BluetoothServer {
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

        for idx in (0..self.readers.len()).rev() {
            match Pin::new(&mut self.readers[idx]).poll_next(cx) {
                Poll::Pending => {}
                Poll::Ready(Some(Ok(x))) => return Poll::Ready(Some(x)),
                Poll::Ready(Some(Err(e))) => {
                    error!("error reading bluetooth messages: {e}");
                    self.readers.swap_remove(idx);
                }
                Poll::Ready(None) => {
                    info!("bluetooth reader quit");
                    self.readers.swap_remove(idx);
                }
            }
        }

        Poll::Pending
    }
}

impl Sink<Vec<u8>> for BluetoothServer {
    type Error = anyhow::Error;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.writers
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
        for i in self.writers.iter_mut() {
            Pin::new(i).start_send(item.clone())?;
        }
        Ok(())
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.writers
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

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let mut res = Poll::Ready(Ok(()));

        self.writers
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
