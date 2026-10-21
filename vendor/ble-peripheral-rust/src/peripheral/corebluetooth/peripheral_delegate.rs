use super::mac_extensions::UuidExtension;
use crate::{
    error::{Error, ErrorType},
    gatt::peripheral_event::{
        PeripheralEvent, PeripheralRequest, ReadRequestResponse, RequestResponse,
        WriteRequestResponse,
    },
};
use objc2::{ClassType, DeclaredClass, declare_class, msg_send_id, mutability, rc::Retained};
use objc2_core_bluetooth::{
    CBATTError, CBATTRequest, CBCentral, CBCharacteristic, CBManagerState, CBPeripheralManager,
    CBPeripheralManagerDelegate, CBService,
};
use objc2_foundation::{NSArray, NSData, NSError, NSObject, NSObjectProtocol};
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc::Sender, oneshot};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

pub struct IVars {
    pub sender: Sender<PeripheralEvent>,
    pub subscribers: Mutex<HashMap<(Uuid, String), (u64, Retained<CBCentral>)>>,
    pub generation: std::sync::atomic::AtomicU64,
    pub services_resolver: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Option<String>>>>>,
    pub advertisement_resolver: Arc<Mutex<Option<oneshot::Sender<Option<String>>>>>,
}

declare_class!(
    #[derive(Debug)]
    pub struct PeripheralDelegate;

    unsafe impl ClassType for PeripheralDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "PeripheralManagerDelegate";
    }

    impl DeclaredClass for PeripheralDelegate {
        type Ivars = IVars;
    }

    unsafe impl NSObjectProtocol for PeripheralDelegate {}

    unsafe impl CBPeripheralManagerDelegate for PeripheralDelegate {
        #[method(peripheralManagerDidUpdateState:)]
         fn delegate_peripheralmanagerdidupdatestate(&self, peripheral: &CBPeripheralManager){
                let state = unsafe { peripheral.state() };
                if state != CBManagerState::PoweredOn { self.clear_subscribers(); }
                self.send_event(PeripheralEvent::StateUpdate { is_powered : state == CBManagerState::PoweredOn });
         }

        #[method(peripheralManagerDidStartAdvertising:error:)]
        fn delegate_peripheralmanagerdidstartadvertising_error(&self, _: &CBPeripheralManager,error: Option<&NSError>){
            let mut error_desc: Option<String> = None;
            if let Some(error) = error {
                error_desc = Some(error.localizedDescription().to_string());
            }
            log::debug!("Advertising, Error: {error_desc:?}");
            if let Ok(mut resolver) = self.ivars().advertisement_resolver.lock() {
                let sender_opt = resolver.take();
                drop(resolver);
                if let Some(sender) = sender_opt {
                    let _ = sender.send(error_desc);
                }
            }
        }

        #[method(peripheralManager:didAddService:error:)]
         fn delegate_peripheralmanager_didaddservice_error(&self, _: &CBPeripheralManager,service: &CBService, error: Option<&NSError>){
            let mut error_desc: Option<String> = None;
            if let Some(error) = error {
                error_desc = Some(error.localizedDescription().to_string());
            }
            log::debug!("AddServices, Error: {error_desc:?}");

            if let  Ok(mut resolver) = self.ivars().services_resolver.lock() {
                if let Some(sender) = resolver.remove(&service.get_uuid()) {
                    drop(resolver);
                    let _ = sender.send(error_desc);
                }
            }
        }

        #[method(peripheralManager:central:didSubscribeToCharacteristic:)]
         fn delegate_peripheralmanager_central_didsubscribetocharacteristic(
            &self,
            _: &CBPeripheralManager,
            central: &CBCentral,
            characteristic: &CBCharacteristic,
        ){
            unsafe{
                let service: Option<Retained<CBService>> = characteristic.service();
                if service.is_none() {
                    return;
                }
                let generation = self.ivars().generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if let Ok(mut peers) = self.ivars().subscribers.lock() {
                    peers.insert((characteristic.get_uuid(), central.identifier().to_string()), (generation, Retained::retain(central as *const _ as *mut _).unwrap()));
                }
                self.send_event(PeripheralEvent::CharacteristicSubscriptionUpdate {
                    request: PeripheralRequest {
                        client: central.identifier().to_string(),
                        generation,
                        service: characteristic.service().unwrap().get_uuid(),
                        characteristic: characteristic.get_uuid(),
                    },
                    subscribed: true,
                });
            }
        }

        #[method(peripheralManager:central:didUnsubscribeFromCharacteristic:)]
         fn delegate_peripheralmanager_central_didunsubscribefromcharacteristic(
            &self,
            _: &CBPeripheralManager,
            central: &CBCentral,
            characteristic: &CBCharacteristic,
        ){  unsafe{
            let service: Option<Retained<CBService>> = characteristic.service();
            if service.is_none() {
                return;
            }

            let old = self.ivars().subscribers.lock().ok().and_then(|mut peers| peers.remove(&(characteristic.get_uuid(), central.identifier().to_string())));
            let generation = old.map_or(0, |(generation, _)| generation);
            self.send_event(PeripheralEvent::CharacteristicSubscriptionUpdate {
               request: PeripheralRequest {
                    client: central.identifier().to_string(),
                    generation,
                    service: characteristic.service().unwrap().get_uuid(),
                    characteristic: characteristic.get_uuid(),
                },
                subscribed: false,
            });
        }}

        #[method(peripheralManager:didReceiveReadRequest:)]
         fn delegate_peripheralmanager_didreceivereadrequest(
            &self,
            manager: &CBPeripheralManager,
            request: &CBATTRequest,
        ){
            unsafe{
                let service = request.characteristic().service();
                if service.is_none() {
                    return;
                }
                let central = request.central();
                let characteristic = request.characteristic();

                self.send_read_request(
                    PeripheralRequest{
                        client: central.identifier().to_string(),
                        generation: 0,
                        service: characteristic.service().unwrap().get_uuid(),
                        characteristic: characteristic.get_uuid(),
                    },
                    manager,
                    request,
                );
            }
        }

        #[method(peripheralManager:didReceiveWriteRequests:)]
         fn delegate_peripheralmanager_didreceivewriterequests(
            &self,
            manager: &CBPeripheralManager,
            requests: &NSArray<CBATTRequest>,
        ){
            // The shared transport accepts one complete short message at a time.
            // Reject a batch atomically and respond once to its first request.
            if requests.len() > 1 {
                if let Some(first) = requests.first() {
                    unsafe { manager.respondToRequest_withResult(first, CBATTError::RequestNotSupported); }
                }
                return;
            }
            for request in requests {
                unsafe{
                    let service = request.characteristic().service();
                    if service.is_none() {
                        return;
                    }
                    let mut value: Vec<u8> = Vec::new();
                    if let Some(ns_data) = request.value() {
                       value = ns_data.bytes().to_vec();
                    }
                    let central = request.central();
                    let characteristic = request.characteristic();

                    self.send_write_request(
                        PeripheralRequest{
                             client: central.identifier().to_string(),
                            generation: 0,
                            service: characteristic.service().unwrap().get_uuid(),
                            characteristic: characteristic.get_uuid(),
                        },
                        manager,
                        request,
                        value,
                    );
                }
            }
        }
    }
);

impl PeripheralDelegate {
    pub fn new(sender: Sender<PeripheralEvent>) -> Retained<PeripheralDelegate> {
        let this = PeripheralDelegate::alloc().set_ivars(IVars {
            sender,
            subscribers: Mutex::new(HashMap::new()),
            generation: std::sync::atomic::AtomicU64::new(1),
            services_resolver: Arc::new(Mutex::new(HashMap::new())),
            advertisement_resolver: Arc::new(Mutex::new(None)),
        });
        return unsafe { msg_send_id![super(this), init] };
    }

    pub fn is_waiting_for_advertisement_result(&self) -> bool {
        if let Ok(resolver) = self.ivars().advertisement_resolver.lock() {
            return resolver.is_some();
        }
        return false;
    }

    pub fn clear_subscribers(&self) {
        if let Ok(mut peers) = self.ivars().subscribers.lock() {
            peers.clear();
        }
    }

    pub fn prepare_advertisement(&self) -> Result<oneshot::Receiver<Option<String>>, Error> {
        let (sender, receiver) = oneshot::channel();
        let mut resolver = self
            .ivars()
            .advertisement_resolver
            .lock()
            .map_err(|_| Error::from_type(ErrorType::ChannelError))?;
        *resolver = Some(sender);
        Ok(receiver)
    }
    pub async fn finish_advertisement(
        &self,
        receiver: oneshot::Receiver<Option<String>>,
    ) -> Result<(), Error> {
        let event = timeout(Duration::from_secs(5), receiver).await;
        if let Ok(mut resolver) = self.ivars().advertisement_resolver.lock() {
            *resolver = None;
        }
        self.resolve_event(event)
    }

    pub fn is_waiting_for_service_result(&self, service: Uuid) -> bool {
        if let Ok(resolver) = self.ivars().services_resolver.lock() {
            return resolver.get(&service).is_some();
        }
        return false;
    }

    pub fn prepare_service(
        &self,
        service: Uuid,
    ) -> Result<oneshot::Receiver<Option<String>>, Error> {
        let (sender, receiver) = oneshot::channel();
        self.ivars()
            .services_resolver
            .lock()
            .map_err(|_| Error::from_type(ErrorType::ChannelError))?
            .insert(service, sender);
        Ok(receiver)
    }
    pub async fn finish_service(
        &self,
        service: Uuid,
        receiver: oneshot::Receiver<Option<String>>,
    ) -> Result<(), Error> {
        let event = timeout(Duration::from_secs(5), receiver).await;
        if let Ok(mut resolver) = self.ivars().services_resolver.lock() {
            resolver.remove(&service);
        }
        self.resolve_event(event)
    }

    fn resolve_event(
        &self,
        event: Result<
            Result<Option<String>, oneshot::error::RecvError>,
            tokio::time::error::Elapsed,
        >,
    ) -> Result<(), Error> {
        let event = match event {
            Ok(Ok(event)) => event,
            Ok(Err(e)) => {
                return Err(Error::from_string(
                    format!("Channel error while waiting: {}", e),
                    ErrorType::CoreBluetooth,
                ));
            },
            Err(_) => {
                return Err(Error::from_string(
                    "Timeout waiting for event".to_string(),
                    ErrorType::CoreBluetooth,
                ));
            },
        };

        if let Some(error) = event {
            return Err(Error::from_string(error, ErrorType::CoreBluetooth));
        }

        return Ok(());
    }
}

/// Event handler
impl PeripheralDelegate {
    fn send_event(&self, event: PeripheralEvent) {
        if let Err(error) = self.ivars().sender.try_send(event) {
            log::error!("Peripheral event could not be queued: {error}");
        }
    }

    fn send_read_request(
        &self,
        peripheral_request: PeripheralRequest,
        manager: &CBPeripheralManager,
        request: &CBATTRequest,
    ) {
        let sender = self.ivars().sender.clone();
        unsafe {
            futures::executor::block_on(async {
                let (resp_tx, resp_rx) = oneshot::channel::<ReadRequestResponse>();

                if let Err(e) = sender.try_send(PeripheralEvent::ReadRequest {
                    request: peripheral_request,
                    offset: request.offset() as u64,
                    responder: resp_tx,
                }) {
                    log::error!("Error sending delegate event: {}", e);
                    return;
                }

                let mut cb_att_error = CBATTError::InvalidHandle;
                if let Some(result) = bounded_response(resp_rx) {
                    cb_att_error = result.response.to_cb_error();
                    request.setValue(Some(&NSData::from_vec(result.value)));
                }
                manager.respondToRequest_withResult(request, cb_att_error);
            });
        };
    }

    fn send_write_request(
        &self,
        peripheral_request: PeripheralRequest,
        manager: &CBPeripheralManager,
        request: &CBATTRequest,
        value: Vec<u8>,
    ) {
        let sender = self.ivars().sender.clone();
        unsafe {
            futures::executor::block_on(async {
                let (resp_tx, resp_rx) = oneshot::channel::<WriteRequestResponse>();

                if let Err(e) = sender.try_send(PeripheralEvent::WriteRequest {
                    request: peripheral_request,
                    value,
                    offset: request.offset() as u64,
                    responder: resp_tx,
                }) {
                    log::error!("Error sending delegate event: {}", e);
                    return;
                }

                let mut cb_att_error = CBATTError::InvalidHandle;
                if let Some(result) = bounded_response(resp_rx) {
                    cb_att_error = result.response.to_cb_error();
                }

                manager.respondToRequest_withResult(request, cb_att_error);
            });
        };
    }
}

// Callback threads have no Tokio timer context. Keep the wait bounded while
// the async owner validates ATT requests; never block its serial queue forever.
fn bounded_response<T>(mut receiver: oneshot::Receiver<T>) -> Option<T> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
    loop {
        match receiver.try_recv() {
            Ok(value) => return Some(value),
            Err(oneshot::error::TryRecvError::Closed) => return None,
            Err(oneshot::error::TryRecvError::Empty) => {},
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

impl RequestResponse {
    fn to_cb_error(self) -> CBATTError {
        match self {
            RequestResponse::Success => CBATTError::Success,
            RequestResponse::InvalidHandle => CBATTError::InvalidHandle,
            RequestResponse::RequestNotSupported => CBATTError::RequestNotSupported,
            RequestResponse::InvalidOffset => CBATTError::InvalidOffset,
            RequestResponse::UnlikelyError => CBATTError::UnlikelyError,
        }
    }
}
