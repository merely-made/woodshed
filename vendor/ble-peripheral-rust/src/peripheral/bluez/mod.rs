mod bluez_utils;
mod characteristic_utils;

use crate::{
    error::{Error, ErrorType},
    gatt::{
        peripheral_event::{PeripheralEvent, PeripheralRequest},
        service,
    },
};
use async_trait::async_trait;
use bluer::{
    Adapter, AdapterEvent, AdapterProperty,
    adv::{Advertisement, AdvertisementHandle},
    gatt::{
        CharacteristicWriter,
        local::{Application, ApplicationHandle, CharacteristicControlEvent},
    },
};
use bluez_utils::CharNotifyHandler;
use characteristic_utils::parse_services;
use futures::{StreamExt, channel::oneshot};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use super::PeripheralImpl;

#[derive(Debug)]
pub struct Peripheral {
    adapter: Adapter,
    services: Vec<service::Service>,
    adv_handle: Option<AdvertisementHandle>,
    app_handle: Option<ApplicationHandle>,
    sender_tx: Sender<PeripheralEvent>,
    writers: Arc<Mutex<HashMap<Uuid, (String, u64, Arc<CharacteristicWriter>)>>>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    generation: Arc<std::sync::atomic::AtomicU64>,
    _drop_tx: oneshot::Sender<()>,
}

#[async_trait]
impl PeripheralImpl for Peripheral {
    type Peripheral = Self;

    async fn new(sender_tx: Sender<PeripheralEvent>) -> Result<Self, Error> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        if !adapter.is_powered().await? {
            return Err(Error::from_string(
                "Bluetooth adapter is powered off".into(),
                ErrorType::Bluez,
            ));
        }
        log::debug!(
            "Initialize Bluetooth adapter {} with address {}",
            adapter.name(),
            adapter.address().await?
        );

        let (drop_tx, drop_rx) = oneshot::channel();
        if let Ok(mut adapter_stream) = adapter.events().await {
            let sender = sender_tx.clone();
            tokio::spawn(async move {
                let stream_future = async {
                    while let Some(event) = adapter_stream.next().await {
                        let AdapterEvent::PropertyChanged(event) = event else {
                            continue;
                        };
                        match event {
                            AdapterProperty::ActiveAdvertisingInstances(i) => {
                                log::debug!("ActiveAdvertisingInstances: {i}")
                            },
                            AdapterProperty::Powered(powered) => {
                                if let Err(err) = sender
                                    .send(PeripheralEvent::StateUpdate {
                                        is_powered: powered,
                                    })
                                    .await
                                {
                                    log::error!("Error sending state update event: {:?}", err);
                                }
                            },
                            _ => {},
                        }
                    }
                };
                tokio::select! {
                    _ = stream_future => {},
                    _ = drop_rx => {}
                }
            });
        }

        Ok(Peripheral {
            adapter,
            services: Vec::new(),
            adv_handle: None,
            app_handle: None,
            sender_tx,
            writers: Arc::new(Mutex::new(HashMap::new())),
            tasks: Vec::new(),
            generation: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            _drop_tx: drop_tx,
        })
    }

    async fn is_powered(&mut self) -> Result<bool, Error> {
        let result = self.adapter.is_powered().await?;
        return Ok(result);
    }

    async fn is_advertising(&mut self) -> Result<bool, Error> {
        let result = self.adapter.active_advertising_instances().await?;
        return Ok(result > 0 && self.adv_handle.is_some());
    }

    async fn start_advertising(&mut self, name: &str, uuids: &[Uuid]) -> Result<(), Error> {
        let manufacturer_data = BTreeMap::new();

        let mut services: BTreeSet<Uuid> = BTreeSet::new();
        for uuid in uuids {
            services.insert(*uuid);
        }

        let le_advertisement = Advertisement {
            service_uuids: services,
            manufacturer_data,
            discoverable: Some(true),
            local_name: Some(name.to_string()),
            ..Default::default()
        };
        // Publish services before advertising. StopAdvertising must not remove
        // the application while a central is using its characteristic.
        if self.app_handle.is_none() {
            let (handlers, services) =
                parse_services(self.services.clone(), self.sender_tx.clone());
            self.app_handle = Some(
                self.adapter
                    .serve_gatt_application(Application {
                        services,
                        ..Default::default()
                    })
                    .await?,
            );
            self.setup_char_handlers(handlers);
        }
        let adv_handle: AdvertisementHandle = self.adapter.advertise(le_advertisement).await?;

        self.adv_handle = Some(adv_handle);
        Ok(())
    }

    async fn stop_advertising(&mut self) -> Result<(), Error> {
        self.adv_handle = None;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Error> {
        self.cleanup();
        Ok(())
    }

    async fn add_service(&mut self, service: &service::Service) -> Result<(), Error> {
        self.services.push(service.clone());
        Ok(())
    }

    async fn update_characteristic(
        &mut self,
        characteristic: Uuid,
        client: String,
        generation: u64,
        deadline: std::time::Instant,
        value: Vec<u8>,
    ) -> Result<(), Error> {
        if std::time::Instant::now() > deadline {
            return Err(Error::from_type(ErrorType::Expired));
        }
        let writer = {
            let writers = self
                .writers
                .lock()
                .map_err(|_| Error::from_type(ErrorType::ChannelError))?;
            let (peer, current, writer) = writers
                .get(&characteristic)
                .ok_or_else(|| Error::from_type(ErrorType::NotSubscribed))?;
            if *peer != client || *current != generation {
                return Err(Error::from_type(ErrorType::NotSubscribed));
            }
            writer.clone()
        };
        // This writer belongs to the original connection, not a later lookup.
        writer
            .send(&value)
            .await
            .map(|_| ())
            .map_err(|error| Error::from_string(error.to_string(), ErrorType::Bluez))
    }
}

impl Peripheral {
    // Handle Characteristic Subscriptions
    fn setup_char_handlers(&mut self, handlers: Vec<CharNotifyHandler>) {
        for mut handler in handlers {
            let sender_tx = self.sender_tx.clone();
            let writers = self.writers.clone();

            let generation = self.generation.clone();
            self.tasks.push(tokio::spawn(async move {
                while let Some(CharacteristicControlEvent::Notify(writer)) =
                    handler.control.next().await
                {
                    let writer = Arc::new(writer);
                    let current = generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                    let peripheral_request = PeripheralRequest {
                        client: writer.device_address().to_string(),
                        generation: current,
                        service: handler.service_uuid,
                        characteristic: handler.characteristic_uuid,
                    };

                    if let Ok(mut writers_lock) = writers.lock() {
                        writers_lock.insert(
                            handler.characteristic_uuid,
                            (peripheral_request.client.clone(), current, writer.clone()),
                        );
                    } else {
                        break;
                    }
                    if sender_tx
                        .send(PeripheralEvent::CharacteristicSubscriptionUpdate {
                            request: peripheral_request.clone(),
                            subscribed: true,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }

                    if let Err(err) = writer.closed().await {
                        log::error!("NotifyClosedErr {err:?}");
                    }

                    if let Ok(mut writers_lock) = writers.lock() {
                        writers_lock.remove(&handler.characteristic_uuid);
                    } else {
                        log::error!("Failed to lock writers for removing a writer");
                    }

                    if let Err(err) = sender_tx
                        .send(PeripheralEvent::CharacteristicSubscriptionUpdate {
                            request: peripheral_request,
                            subscribed: false,
                        })
                        .await
                    {
                        log::error!("Error sending read request event: {:?}", err);
                    }
                }
            }));
        }
    }

    fn cleanup(&mut self) {
        self.adv_handle = None;
        self.app_handle = None;
        for task in self.tasks.drain(..) {
            task.abort();
        }
        if let Ok(mut writers) = self.writers.lock() {
            writers.clear();
        }
    }
}

impl Drop for Peripheral {
    fn drop(&mut self) {
        self.cleanup();
    }
}
