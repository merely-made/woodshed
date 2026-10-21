// SPDX-License-Identifier: MPL-2.0
//! Standard short-message BLE-MIDI transport. Native GATT lives in the supplier.
use super::diagnostics;
use std::{fmt, time::Duration};

pub const SERVICE_UUID: &str = "03b80e5a-ede8-4b33-a751-6ce34ec4c700";
pub const IO_UUID: &str = "7772e5db-3868-4112-a1a9-f2669d106bf3";
/// A selectable transport in the existing output picker. No radio is opened by listing it.
pub const OUTPUT_NAME: &str = "Bluetooth MIDI pedal";
pub const COMPAT_OUTPUT_NAME: &str = "Bluetooth MIDI pedal (no encryption)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    Encrypted,
    /// Explicit owner choice for receivers that did not negotiate encryption.
    UnencryptedCompatibility,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub security: Security,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Ringdown".into(),
            security: Security::Encrypted,
        }
    }
}

impl Config {
    fn validate(&self) -> Result<(), Error> {
        // Flags + 128-bit service + name must fit a legacy 31-byte advertisement.
        if self.name.is_empty()
            || self.name.len() > 8
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Error::Invalid(
                "BLE name must be 1-8 ASCII letters, digits, '-' or '_'".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Advertising,
    Subscribed,
    Unavailable,
    Closed,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub phase: Phase,
    pub security: Security,
    pub subscribers: usize,
    pub generation: u64,
    pub accepted_messages: u64,
    pub received_messages: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Error {
    Invalid(String),
    NotSubscribed,
    WouldBlock,
    Expired,
    /// The OS operation started but its completion was not observed. Do not replay.
    Unknown,
    Transport(String),
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "invalid BLE-MIDI message/configuration: {s}"),
            Self::NotSubscribed => write!(f, "BLE-MIDI requires exactly one current subscriber"),
            Self::WouldBlock => write!(
                f,
                "BLE-MIDI notification queue is full; message was not queued for retry"
            ),
            Self::Expired => write!(f, "BLE-MIDI command expired before sending"),
            Self::Unknown => write!(
                f,
                "BLE-MIDI send outcome is unknown; do not replay automatically"
            ),
            Self::Transport(s) => write!(f, "BLE-MIDI transport: {s}"),
            Self::Unsupported => write!(
                f,
                "BLE-MIDI peripheral role is unavailable on this platform"
            ),
        }
    }
}
impl std::error::Error for Error {}

/// One complete short MIDI message per notification; no running status, SysEx
/// or stream splitting. Maximum packet is five bytes, below the ATT minimum.
pub fn encode_short_message(message: &[u8], elapsed_millis: u64) -> Result<Vec<u8>, Error> {
    let expected = match message.first().copied() {
        Some(0x80..=0xbf | 0xe0..=0xef | 0xf2) => 3,
        Some(0xc0..=0xdf | 0xf1 | 0xf3) => 2,
        Some(0xf6 | 0xf8 | 0xfa..=0xfc | 0xfe | 0xff) => 1,
        _ => {
            return Err(Error::Invalid(
                "expected a complete short MIDI message".into(),
            ));
        },
    };
    if message.len() != expected || message[1..].iter().any(|byte| *byte >= 0x80) {
        return Err(Error::Invalid(
            "incorrect message length or non-data byte".into(),
        ));
    }
    let timestamp = elapsed_millis & 0x1fff;
    let mut packet = Vec::with_capacity(expected + 2);
    packet.push(0x80 | ((timestamp >> 7) as u8 & 0x3f));
    packet.push(0x80 | (timestamp as u8 & 0x7f));
    packet.extend_from_slice(message);
    Ok(packet)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod native {
    use super::*;
    use ble_peripheral_rust::{
        Peripheral, PeripheralImpl,
        gatt::{
            characteristic::Characteristic,
            peripheral_event::{
                PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
            },
            properties::{AttributePermission as Permission, CharacteristicProperty as Property},
            service::Service,
        },
    };
    use std::{
        collections::HashMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread::JoinHandle,
        time::Instant,
    };
    use tokio::sync::mpsc as async_mpsc;
    use uuid::Uuid;

    static ACTIVE: AtomicBool = AtomicBool::new(false);
    struct Owner;
    impl Drop for Owner {
        fn drop(&mut self) {
            ACTIVE.store(false, Ordering::Release);
        }
    }

    struct View {
        status: Status,
        peer: Option<(String, u64)>,
    }
    struct Command {
        message: Vec<u8>,
        peer: String,
        generation: u64,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
        reply: mpsc::SyncSender<Result<(), Error>>,
    }

    pub(crate) struct Output {
        tx: Option<async_mpsc::Sender<Command>>,
        view: Arc<Mutex<View>>,
        thread: Option<JoinHandle<()>>,
    }

    fn supplier_error(error: ble_peripheral_rust::error::Error) -> Error {
        use ble_peripheral_rust::error::ErrorType;
        match error.kind() {
            ErrorType::NotSubscribed => Error::NotSubscribed,
            ErrorType::WouldBlock => Error::WouldBlock,
            ErrorType::Unsupported => Error::Unsupported,
            ErrorType::Expired => Error::Expired,
            _ => Error::Transport(error.to_string()),
        }
    }

    impl Output {
        pub(crate) fn open(config: Config) -> Result<Self, Error> {
            config.validate()?;
            if ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(Error::Transport(
                    "A BLE-MIDI output is already open; close it before reopening".into(),
                ));
            }
            let owner = Owner;
            let view = Arc::new(Mutex::new(View {
                status: Status {
                    phase: Phase::Unavailable,
                    security: config.security,
                    subscribers: 0,
                    generation: 0,
                    accepted_messages: 0,
                    received_messages: 0,
                    last_error: None,
                },
                peer: None,
            }));
            let (tx, receiver) = async_mpsc::channel(1);
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let shared = view.clone();
            let thread = std::thread::Builder::new()
                .name("instrument-ble-midi".into())
                .spawn(move || {
                    let _owner = owner;
                    let runtime = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => runtime,
                        Err(error) => {
                            let _ = ready_tx.send(Err(Error::Transport(error.to_string())));
                            return;
                        },
                    };
                    runtime.block_on(run(config, receiver, shared, ready_tx));
                })
                .map_err(|error| Error::Transport(error.to_string()))?;
            match ready_rx.recv_timeout(Duration::from_secs(15)) {
                Ok(Ok(())) => Ok(Self {
                    tx: Some(tx),
                    view,
                    thread: Some(thread),
                }),
                Ok(Err(error)) => {
                    drop(tx);
                    let _ = thread.join();
                    Err(error)
                },
                Err(_) => {
                    drop(tx);
                    Err(Error::Transport(
                        "BLE-MIDI startup timed out; no output opened".into(),
                    ))
                },
            }
        }

        pub(crate) fn status(&self) -> Status {
            self.view.lock().unwrap().status.clone()
        }

        pub(crate) fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
            self.send_inner(bytes, None)
        }
        pub(crate) fn send_for_generation(
            &mut self,
            bytes: &[u8],
            generation: Option<u64>,
        ) -> Result<(), Error> {
            self.send_inner(bytes, Some(generation.ok_or(Error::NotSubscribed)?))
        }
        fn send_inner(&mut self, bytes: &[u8], expected: Option<u64>) -> Result<(), Error> {
            encode_short_message(bytes, 0)?;
            let (peer, generation) = self
                .view
                .lock()
                .unwrap()
                .peer
                .clone()
                .ok_or(Error::NotSubscribed)?;
            if expected.is_some_and(|expected| expected != generation) {
                return Err(Error::NotSubscribed);
            }
            let (reply, response) = mpsc::sync_channel(1);
            let cancelled = Arc::new(AtomicBool::new(false));
            let command = Command {
                message: bytes.to_vec(),
                peer,
                generation,
                deadline: Instant::now() + Duration::from_millis(100),
                cancelled: cancelled.clone(),
                reply,
            };
            self.tx
                .as_ref()
                .ok_or(Error::NotSubscribed)?
                .try_send(command)
                .map_err(|error| match error {
                    async_mpsc::error::TrySendError::Full(_) => Error::WouldBlock,
                    async_mpsc::error::TrySendError::Closed(_) => Error::Transport(
                        "BLE-MIDI worker is unavailable; message was not queued".into(),
                    ),
                })?;
            match response.recv_timeout(Duration::from_millis(400)) {
                Ok(result) => result,
                Err(_) => {
                    cancelled.store(true, Ordering::Release);
                    Err(Error::Unknown)
                },
            }
        }

        pub(crate) fn close(&mut self) -> Result<(), Error> {
            self.tx.take();
            if let Some(thread) = self.thread.take() {
                thread
                    .join()
                    .map_err(|_| Error::Transport("BLE-MIDI worker stopped unexpectedly".into()))?;
            }
            let status = self.status();
            if let Some(error) = status.last_error.filter(|_| status.phase != Phase::Closed) {
                Err(Error::Transport(error))
            } else {
                Ok(())
            }
        }
    }
    impl Drop for Output {
        fn drop(&mut self) {
            let _ = self.close();
        }
    }

    fn validate_command(
        command: &Command,
        current: Option<&(String, u64)>,
        now: Instant,
    ) -> Result<(), Error> {
        if command.cancelled.load(Ordering::Acquire) || now > command.deadline {
            return Err(Error::Expired);
        }
        if current != Some(&(command.peer.clone(), command.generation)) {
            return Err(Error::NotSubscribed);
        }
        Ok(())
    }

    async fn run(
        config: Config,
        mut commands: async_mpsc::Receiver<Command>,
        view: Arc<Mutex<View>>,
        ready: mpsc::SyncSender<Result<(), Error>>,
    ) {
        let (events_tx, mut events) = async_mpsc::channel(64);
        let mut peripheral =
            match tokio::time::timeout(Duration::from_secs(5), Peripheral::new(events_tx)).await {
                Ok(Ok(peripheral)) => peripheral,
                Ok(Err(error)) => {
                    let _ = ready.send(Err(supplier_error(error)));
                    return;
                },
                Err(_) => {
                    let _ = ready.send(Err(Error::Transport(
                        "Bluetooth adapter initialization timed out".into(),
                    )));
                    return;
                },
            };
        let service = Uuid::parse_str(SERVICE_UUID).unwrap();
        let io = Uuid::parse_str(IO_UUID).unwrap();
        let mut properties = vec![
            Property::Read,
            Property::WriteWithoutResponse,
            Property::Notify,
        ];
        let permissions = match config.security {
            Security::Encrypted => {
                properties.push(Property::NotifyEncryptionRequired);
                vec![
                    Permission::ReadEncryptionRequired,
                    Permission::WriteEncryptionRequired,
                ]
            },
            Security::UnencryptedCompatibility => vec![Permission::Readable, Permission::Writeable],
        };
        let registration = Service {
            uuid: service,
            primary: true,
            characteristics: vec![Characteristic {
                uuid: io,
                properties,
                permissions,
                value: None,
                descriptors: vec![],
            }],
        };
        let startup = tokio::time::timeout(Duration::from_secs(12), async {
            while !peripheral.is_powered().await.map_err(supplier_error)? {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            peripheral
                .add_service(&registration)
                .await
                .map_err(supplier_error)?;
            peripheral
                .start_advertising(&config.name, &[service])
                .await
                .map_err(supplier_error)
        })
        .await
        .unwrap_or_else(|_| {
            Err(Error::Transport(
                "BLE-MIDI service/advertising startup timed out".into(),
            ))
        });
        if let Err(error) = startup {
            let _ = ready.send(Err(error));
            let _ = tokio::time::timeout(Duration::from_secs(2), peripheral.shutdown()).await;
            return;
        }
        view.lock().unwrap().status.phase = Phase::Advertising;
        if ready.send(Ok(())).is_err() {
            let _ = tokio::time::timeout(Duration::from_secs(2), peripheral.shutdown()).await;
            return;
        }
        let mut peers = HashMap::<String, u64>::new();
        let epoch = Instant::now();
        loop {
            tokio::select! {
                biased;
                event = events.recv() => {
                    match event {
                        Some(PeripheralEvent::StateUpdate { is_powered: false }) => {
                            let mut state = view.lock().unwrap(); state.peer = None; state.status.phase = Phase::Unavailable;
                            state.status.last_error = Some("Bluetooth adapter became unavailable; reconnect explicitly".into()); break;
                        },
                        Some(PeripheralEvent::StateUpdate { .. }) => {},
                        Some(PeripheralEvent::CharacteristicSubscriptionUpdate { request, subscribed }) if request.characteristic == io && request.service == service => {
                            if subscribed {
                                let current = peers.entry(request.client).or_default();
                                if request.generation > *current { *current = request.generation; }
                            } else if peers.get(&request.client) == Some(&request.generation) { peers.remove(&request.client); }
                            let active = peers.len();
                            {
                                let mut state = view.lock().unwrap(); state.status.subscribers = active;
                                state.peer = if active == 1 { peers.iter().next().map(|(id, gen)| (id.clone(), *gen)) } else { None };
                                state.status.generation = state.peer.as_ref().map_or(0, |(_, gen)| *gen);
                                diagnostics::record("ble-subscription", None, None, Some(state.status.generation), &[], || format!("subscribed_event={subscribed}; subscribers={active}"));
                                state.status.phase = if active == 1 { Phase::Subscribed } else if active == 0 { Phase::Advertising } else { Phase::Unavailable };
                                state.status.last_error = if active > 1 { Some("Multiple subscribers; sending is disabled".into()) } else { None };
                            }
                            let result = tokio::time::timeout(Duration::from_secs(6), async {
                                if active == 0 { peripheral.start_advertising(&config.name, &[service]).await }
                                else { peripheral.stop_advertising().await }
                            }).await;
                            let error = match result {
                                Ok(Ok(())) => None,
                                Ok(Err(error)) => Some(error.to_string()),
                                Err(_) => Some("BLE advertising transition timed out; reopen the output".into()),
                            };
                            if let Some(error) = error {
                                let mut state = view.lock().unwrap(); state.peer = None;
                                state.status.phase = Phase::Unavailable; state.status.last_error = Some(error);
                                break;
                            }
                        },
                        Some(PeripheralEvent::ReadRequest { request, offset, responder }) => {
                            let response = if request.service != service || request.characteristic != io { RequestResponse::InvalidHandle } else if offset != 0 { RequestResponse::InvalidOffset } else { RequestResponse::Success };
                            let _ = responder.send(ReadRequestResponse { value: vec![], response });
                        },
                        Some(PeripheralEvent::WriteRequest { request, value, offset, responder }) => {
                            let valid = request.service == service && request.characteristic == io && offset == 0 && value.len() >= 3 && value.len() <= 5 && value[0] & 0xc0 == 0x80 && value[1] & 0x80 != 0 && encode_short_message(&value[2..], 0).is_ok();
                            if valid { view.lock().unwrap().status.received_messages += 1; }
                            let _ = responder.send(WriteRequestResponse { response: if valid { RequestResponse::Success } else { RequestResponse::RequestNotSupported } });
                        },
                        Some(_) => {},
                        None => break,
                    }
                },
                command = commands.recv() => {
                    let Some(command) = command else { break; };
                    let current = view.lock().unwrap().peer.clone();
                    let result = if let Err(error) = validate_command(&command, current.as_ref(), Instant::now()) { Err(error) }
                    else {
                        let packet = encode_short_message(&command.message, epoch.elapsed().as_millis() as u64).unwrap();
                        diagnostics::record("ble-notify-start", None, None, Some(command.generation), &packet, || format!("midi={:?}; OS notification submission, not device acknowledgement",command.message));
                        match tokio::time::timeout(Duration::from_millis(250), peripheral.update_characteristic(io, command.peer, command.generation, command.deadline, packet)).await {
                            Ok(result) => result.map_err(supplier_error), Err(_) => Err(Error::Unknown),
                        }
                    };
                    diagnostics::record("ble-notify-result", None, None, Some(command.generation), &command.message, || format!("{result:?}; OS result, not device acknowledgement"));
                    if result.is_ok() { let mut state = view.lock().unwrap(); state.status.accepted_messages += 1; state.status.last_error = None; }
                    else { view.lock().unwrap().status.last_error = result.as_ref().err().map(ToString::to_string); }
                    let _ = command.reply.send(result);
                }
            }
        }
        drop(events); // release pending native ATT request responders during shutdown
        let shutdown = tokio::time::timeout(Duration::from_secs(2), peripheral.shutdown()).await;
        let mut state = view.lock().unwrap();
        state.peer = None;
        state.status.subscribers = 0;
        match shutdown {
            Ok(Ok(())) => {
                state.status.phase = Phase::Closed;
            },
            Ok(Err(error)) => {
                state.status.phase = Phase::Unavailable;
                state.status.last_error = Some(error.to_string());
            },
            Err(_) => {
                state.status.phase = Phase::Unavailable;
                state.status.last_error = Some("BLE shutdown outcome unknown".into());
            },
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn fixture(peer: bool) -> (Output, async_mpsc::Receiver<Command>) {
            let (tx, rx) = async_mpsc::channel(1);
            let output = Output {
                tx: Some(tx),
                thread: None,
                view: Arc::new(Mutex::new(View {
                    peer: peer.then(|| ("fixture".into(), 2)),
                    status: Status {
                        phase: Phase::Subscribed,
                        security: Security::Encrypted,
                        subscribers: usize::from(peer),
                        generation: 2,
                        accepted_messages: 0,
                        received_messages: 0,
                        last_error: None,
                    },
                })),
            };
            (output, rx)
        }
        fn queued() -> Command {
            let (reply, _) = mpsc::sync_channel(1);
            Command {
                message: vec![0xb0, 67, 127],
                peer: "fixture".into(),
                generation: 2,
                deadline: Instant::now() + Duration::from_millis(100),
                cancelled: Arc::new(AtomicBool::new(false)),
                reply,
            }
        }
        #[test]
        fn stale_generation_is_refused_before_queueing() {
            let (mut out, mut rx) = fixture(true);
            assert!(matches!(
                out.send_for_generation(&[0xb0, 67, 127], Some(1)),
                Err(Error::NotSubscribed)
            ));
            assert!(rx.try_recv().is_err());
        }
        #[test]
        fn disconnected_message_is_not_queued() {
            let (mut out, mut rx) = fixture(false);
            assert!(matches!(
                out.send(&[0xb0, 67, 127]),
                Err(Error::NotSubscribed)
            ));
            assert!(rx.try_recv().is_err());
        }
        #[test]
        fn queue_pressure_is_refused_without_replacing_pending_message() {
            let (mut out, mut rx) = fixture(true);
            out.tx.as_ref().unwrap().try_send(queued()).unwrap();
            assert!(matches!(out.send(&[0xb0, 67, 0]), Err(Error::WouldBlock)));
            assert_eq!(rx.try_recv().unwrap().message, vec![0xb0, 67, 127]);
            assert!(rx.try_recv().is_err());
        }
        #[test]
        fn closed_worker_is_distinct_from_queue_pressure() {
            let (mut out, rx) = fixture(true);
            drop(rx);
            assert!(matches!(
                out.send(&[0xb0, 67, 127]),
                Err(Error::Transport(_))
            ));
        }
        #[test]
        fn timed_out_command_is_cancelled_and_rejected_when_worker_resumes() {
            let (mut out, mut rx) = fixture(true);
            assert!(matches!(out.send(&[0xb0, 67, 127]), Err(Error::Unknown)));
            let command = rx.try_recv().unwrap();
            assert!(matches!(
                validate_command(&command, Some(&("fixture".into(), 2)), Instant::now()),
                Err(Error::Expired)
            ));
            assert!(rx.try_recv().is_err());
        }
        #[test]
        fn replacement_and_missing_subscription_reject_old_command() {
            let command = queued();
            assert!(matches!(
                validate_command(&command, Some(&("fixture".into(), 3)), Instant::now()),
                Err(Error::NotSubscribed)
            ));
            assert!(matches!(
                validate_command(&command, None, Instant::now()),
                Err(Error::NotSubscribed)
            ));
            assert!(
                validate_command(&command, Some(&("fixture".into(), 2)), Instant::now()).is_ok()
            );
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) use native::Output;
