use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::gatt::peripheral_event::{
    PeripheralEvent, PeripheralRequest, ReadRequestResponse, RequestResponse, WriteRequestResponse,
};
use crate::peripheral::winrt::win_utils::{
    buffer_to_vec, device_id_from_session, to_uuid, vec_to_buffer,
};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use uuid::Uuid;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattProtocolError, GattServiceProviderAdvertisementStatus,
};
use windows::Devices::Radios::{Radio, RadioState};
use windows::core::IInspectable;
use windows::{
    Devices::Bluetooth::GenericAttributeProfile::{
        GattLocalCharacteristic, GattReadRequestedEventArgs, GattServiceProvider,
        GattServiceProviderAdvertisementStatusChangedEventArgs, GattWriteRequestedEventArgs,
    },
    Foundation::TypedEventHandler,
};

pub struct WinEventHandler {
    sender_tx: Sender<PeripheralEvent>,
    connected_clients: Arc<
        RwLock<
            HashMap<
                (Uuid, Uuid),
                Vec<(
                    String,
                    u64,
                    windows::Devices::Bluetooth::GenericAttributeProfile::GattSubscribedClient,
                )>,
            >,
        >,
    >,
    generation: Arc<std::sync::atomic::AtomicU64>,
    runtime: tokio::runtime::Handle,
}

impl WinEventHandler {
    pub fn new(sender_tx: Sender<PeripheralEvent>) -> Self {
        Self {
            sender_tx,
            runtime: tokio::runtime::Handle::current(),
            connected_clients: Arc::new(RwLock::new(HashMap::new())),
            generation: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    pub fn create_radio_listener(&self) -> TypedEventHandler<Radio, IInspectable> {
        let sender_tx: Sender<PeripheralEvent> = self.sender_tx.clone();

        return TypedEventHandler::new(
            move |originator: &Option<Radio>, _: &Option<IInspectable>| {
                let radio = originator
                    .as_ref()
                    .ok_or_else(windows::core::Error::empty)?;
                let is_on = radio.State()? == RadioState::On;
                sender_tx
                    .try_send(PeripheralEvent::StateUpdate { is_powered: is_on })
                    .map_err(|_| windows::core::Error::empty())?;
                Ok(())
            },
        );
    }

    pub fn create_advertisement_status_handler(
        &self,
    ) -> TypedEventHandler<
        GattServiceProvider,
        GattServiceProviderAdvertisementStatusChangedEventArgs,
    > {
        TypedEventHandler::new(move |originator: &Option<GattServiceProvider>, args: &Option<GattServiceProviderAdvertisementStatusChangedEventArgs>| {
            let service = originator.as_ref().ok_or_else(windows::core::Error::empty)?;
            let event_args = args.as_ref().ok_or_else(windows::core::Error::empty)?;
            let status = event_args.Status()?;
            log::debug!("Advertisement Status: {:?}: Started: {:?}", 
                to_uuid(&service.Service()?.Uuid()?),
                status == GattServiceProviderAdvertisementStatus::Started);
            Ok(())
        })
    }

    pub fn current_target(
        &self,
        service: Uuid,
        characteristic: Uuid,
        client: &str,
        generation: u64,
    ) -> Option<windows::Devices::Bluetooth::GenericAttributeProfile::GattSubscribedClient> {
        self.connected_clients.read().ok().and_then(|peers| {
            peers.get(&(service, characteristic)).and_then(|list| {
                list.iter()
                    .find(|(id, current, _)| id == client && *current == generation)
                    .map(|(_, _, target)| target.clone())
            })
        })
    }

    pub fn create_subscribe_handler(
        &self,
        service_uuid: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, IInspectable> {
        let peers = self.connected_clients.clone();
        let generation = self.generation.clone();
        let sender = self.sender_tx.clone();
        TypedEventHandler::new(move |originator: &Option<GattLocalCharacteristic>, _| {
            let characteristic = originator
                .as_ref()
                .ok_or_else(windows::core::Error::empty)?;
            let characteristic_uuid = to_uuid(&characteristic.Uuid()?);
            let mut current = Vec::new();
            for client in characteristic.SubscribedClients()? {
                current.push((
                    device_id_from_session(client.Session()?)?,
                    generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                    client,
                ));
            }
            let previous = {
                let mut guard = peers.write().map_err(|_| windows::core::Error::empty())?;
                guard
                    .insert((service_uuid, characteristic_uuid), current.clone())
                    .unwrap_or_default()
            };
            // Rotate leases on every OS change; queued commands cannot target a
            // replacement connection merely because the device ID is unchanged.
            for (clients, subscribed) in [(previous, false), (current, true)] {
                for (client, generation, _) in clients {
                    if sender
                        .try_send(PeripheralEvent::CharacteristicSubscriptionUpdate {
                            request: PeripheralRequest {
                                client,
                                generation,
                                service: service_uuid,
                                characteristic: characteristic_uuid,
                            },
                            subscribed,
                        })
                        .is_err()
                    {
                        // Invalidate leases if event delivery overflows; a stale
                        // application snapshot must never authorize a send.
                        if let Ok(mut guard) = peers.write() {
                            guard.remove(&(service_uuid, characteristic_uuid));
                        }
                        return Err(windows::core::Error::empty());
                    }
                }
            }
            Ok(())
        })
    }

    pub fn create_read_handler(
        &self,
        service: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, GattReadRequestedEventArgs> {
        let sender = self.sender_tx.clone();
        let runtime = self.runtime.clone();
        TypedEventHandler::new(
            move |origin: &Option<GattLocalCharacteristic>,
                  args: &Option<GattReadRequestedEventArgs>| {
                let characteristic = origin
                    .as_ref()
                    .ok_or_else(windows::core::Error::empty)?
                    .clone();
                let args = args
                    .as_ref()
                    .ok_or_else(windows::core::Error::empty)?
                    .clone();
                let deferral = args.GetDeferral()?;
                let sender = sender.clone();
                runtime.spawn(async move {
                    let result: windows::core::Result<()> = async {
                        let request = args.GetRequestAsync()?.await?;
                        let (reply, response) = oneshot::channel::<ReadRequestResponse>();
                        let event = PeripheralEvent::ReadRequest {
                            request: PeripheralRequest {
                                client: device_id_from_session(args.Session()?)?,
                                generation: 0,
                                service,
                                characteristic: to_uuid(&characteristic.Uuid()?),
                            },
                            offset: request.Offset()? as u64,
                            responder: reply,
                        };
                        let response = if sender.try_send(event).is_ok() {
                            tokio::time::timeout(std::time::Duration::from_millis(200), response)
                                .await
                                .ok()
                                .and_then(Result::ok)
                        } else {
                            None
                        };
                        match response {
                            Some(result) if result.response == RequestResponse::Success => {
                                request.RespondWithValue(&vec_to_buffer(result.value)?)
                            },
                            Some(result) => request
                                .RespondWithProtocolError(result.response.to_gatt_protocol_error()),
                            None => request
                                .RespondWithProtocolError(GattProtocolError::UnlikelyError()?),
                        }
                    }
                    .await;
                    if let Err(error) = result {
                        log::error!("GATT read failed: {error}");
                    }
                    if let Err(error) = deferral.Complete() {
                        log::error!("GATT read deferral failed: {error}");
                    }
                });
                Ok(())
            },
        )
    }

    pub fn create_write_handler(
        &self,
        service: Uuid,
    ) -> TypedEventHandler<GattLocalCharacteristic, GattWriteRequestedEventArgs> {
        let sender = self.sender_tx.clone();
        let runtime = self.runtime.clone();
        TypedEventHandler::new(
            move |origin: &Option<GattLocalCharacteristic>,
                  args: &Option<GattWriteRequestedEventArgs>| {
                let characteristic = origin
                    .as_ref()
                    .ok_or_else(windows::core::Error::empty)?
                    .clone();
                let args = args
                    .as_ref()
                    .ok_or_else(windows::core::Error::empty)?
                    .clone();
                let deferral = args.GetDeferral()?;
                let sender = sender.clone();
                runtime.spawn(async move {
                let result: windows::core::Result<()> = async {
                    let request = args.GetRequestAsync()?.await?;
                    let (reply, response) = oneshot::channel::<WriteRequestResponse>();
                    let event = PeripheralEvent::WriteRequest {
                        request: PeripheralRequest { client: device_id_from_session(args.Session()?)?, generation: 0, service, characteristic: to_uuid(&characteristic.Uuid()?) },
                        value: buffer_to_vec(&request.Value()?)?, offset: request.Offset()? as u64, responder: reply,
                    };
                    let response = if sender.try_send(event).is_ok() {
                        tokio::time::timeout(std::time::Duration::from_millis(200), response).await.ok().and_then(Result::ok)
                    } else { None };
                    // Write Without Response has no ATT response operation.
                    if request.Option()? == windows::Devices::Bluetooth::GenericAttributeProfile::GattWriteOption::WriteWithoutResponse { return Ok(()); }
                    match response {
                        Some(result) if result.response == RequestResponse::Success => request.Respond(),
                        Some(result) => request.RespondWithProtocolError(result.response.to_gatt_protocol_error()),
                        None => request.RespondWithProtocolError(GattProtocolError::UnlikelyError()?),
                    }
                }.await;
                if let Err(error) = result { log::error!("GATT write failed: {error}"); }
                if let Err(error) = deferral.Complete() { log::error!("GATT write deferral failed: {error}"); }
            });
                Ok(())
            },
        )
    }
}

impl RequestResponse {
    fn to_gatt_protocol_error(self) -> u8 {
        let result = match self {
            RequestResponse::Success => Ok(0),
            RequestResponse::InvalidHandle => GattProtocolError::InvalidHandle(),
            RequestResponse::RequestNotSupported => GattProtocolError::RequestNotSupported(),
            RequestResponse::InvalidOffset => GattProtocolError::InvalidOffset(),
            RequestResponse::UnlikelyError => GattProtocolError::UnlikelyError(),
        };
        if let Ok(value) = result {
            return value;
        }
        return 0;
    }
}
