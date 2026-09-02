//! Desktop Bluetooth for [`ringdown`]: discovery, connection, and the btleplug
//! [`Link`].
//!
//! This crate is the platform half. It finds an instrument, opens a GATT
//! connection, and hands back a [`Guitar`] from `ringdown-client` — which owns
//! the protocol and never learns what radio it is speaking over.
//!
//! ```no_run
//! # async fn run() -> Result<(), ringdown_ble::TransportError> {
//! let found = ringdown_ble::discover(std::time::Duration::from_secs(10)).await?;
//! let mut guitar = ringdown_ble::connect(&found[0]).await?;
//! println!("{:?}", guitar.status().await?);
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use btleplug::api::{
    Central, CentralEvent, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Manager, Peripheral};
use futures::{Stream, StreamExt};
use ringdown::{handshake::Banner, link::Link, llt2};
use uuid::Uuid;

pub use ringdown_client::{
    ASSUMED_WRITE_LEN, FileInfo, MAX_DRAIN, MAX_FILE_CHUNK, REQUEST_TIMEOUT, Restored, Sent,
    Transport, TransportError, WEDGING_METHODS,
};

/// A guitar reached over desktop Bluetooth.
///
/// The protocol driver with this crate's [`Link`] filled in, so callers write
/// `Guitar` and never mention the type parameter.
pub type Guitar = ringdown_client::Guitar<BtleplugLink>;

/// Map a btleplug failure into the driver's platform-agnostic error.
///
/// `TransportError` cannot name `btleplug::Error` — that is the whole point of
/// the split — and a blanket `From` impl is not ours to write, since both the
/// trait and the error type are foreign here.
trait Ble<T> {
    fn ble(self) -> Result<T, TransportError>;
}

impl<T> Ble<T> for Result<T, btleplug::Error> {
    fn ble(self) -> Result<T, TransportError> {
        self.map_err(|e| TransportError::Link(e.to_string()))
    }
}
/// How many times to attempt a connection before giving up.
const CONNECT_ATTEMPTS: u32 = 3;

/// How long to wait between connection attempts.
const CONNECT_BACKOFF: Duration = Duration::from_millis(800);

/// Why a scanned device was taken to be a guitar.
///
/// Worth surfacing rather than collapsing to a boolean: a device matched only
/// by name is a weaker identification than one advertising the service, and a
/// caller staring at a failed connection deserves to know which it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchedBy {
    /// The advertisement carried the guitar service UUID. Strongest signal.
    AdvertisedService,
    /// The advertised name looks like a HyVibe. Weaker, but many BLE devices
    /// never advertise their service UUIDs, so this is not a fallback to be
    /// embarrassed about.
    Name,
}

/// A guitar seen while scanning.
#[derive(Clone)]
pub struct Found {
    peripheral: Peripheral,
    /// The advertised local name, when the device provided one.
    pub name: Option<String>,
    /// The peripheral's address, as the platform reports it.
    pub address: String,
    /// How this device was identified.
    pub matched_by: MatchedBy,
}

impl std::fmt::Debug for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Found")
            .field("name", &self.name)
            .field("address", &self.address)
            .field("matched_by", &self.matched_by)
            .finish()
    }
}

/// Whether an advertised name looks like a HyVibe system.
///
/// The vendor's System Menu calls this the "BT ID" and the manual's example is
/// `H2-SE614`, so the model prefix plus a unit suffix is the shape to expect.
/// The bare product name is also accepted, since older units and other models
/// may not use the `H2-` prefix.
fn name_looks_like_a_guitar(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower.starts_with("h2-") || lower.contains("hyvibe") || lower.starts_with("lag")
}

fn service_uuid() -> Uuid {
    // The constants are strings in the core so it can stay dependency-light;
    // they are fixed and valid, so a parse failure here would be a build-time
    // typo rather than a runtime condition.
    Uuid::parse_str(ringdown::GUITAR_SERVICE).expect("service uuid constant is malformed")
}

fn request_uuid() -> Uuid {
    Uuid::parse_str(ringdown::GUITAR_CHARACTERISTIC_REQUEST)
        .expect("request characteristic uuid constant is malformed")
}

fn response_uuid() -> Uuid {
    Uuid::parse_str(ringdown::GUITAR_CHARACTERISTIC_RESPONSE)
        .expect("response characteristic uuid constant is malformed")
}

/// Scan for guitars.
///
/// Scans for the whole `timeout` rather than returning on the first hit, so a
/// caller with more than one instrument in range sees all of them.
///
/// # Why the scan is unfiltered
///
/// The obvious implementation asks the adapter to filter on the guitar's
/// service UUID. That is wrong here, and wrong in a way that would be easy to
/// misread: a great many BLE peripherals do not put their service UUIDs in the
/// advertisement at all, exposing them only on service discovery after a
/// connection. Filtering on the service would then return nothing, and
/// "no guitar found" would look like evidence against the recovered protocol
/// map when it was only evidence about advertising behaviour.
///
/// So the scan is unfiltered and the matching happens here, on either the
/// service UUID or the advertised name, with [`Found::matched_by`] recording
/// which. A device is better identified imperfectly than missed silently.
pub async fn discover(timeout: Duration) -> Result<Vec<Found>, TransportError> {
    let manager = Manager::new().await.ble()?;
    let adapter = manager
        .adapters()
        .await
        .ble()?
        .into_iter()
        .next()
        .ok_or(TransportError::NoAdapter)?;

    adapter.start_scan(ScanFilter::default()).await.ble()?;

    // Watch scan events rather than sleeping blind, so a caller sees devices as
    // the adapter reports them.
    let mut events = adapter.events().await.ble()?;
    let deadline = tokio::time::Instant::now() + timeout;
    let mut seen = Vec::new();

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, events.next()).await {
            Ok(Some(CentralEvent::DeviceDiscovered(id)))
            | Ok(Some(CentralEvent::DeviceUpdated(id))) => {
                if seen.iter().any(|f: &Found| f.peripheral.id() == id) {
                    continue;
                }
                if let Ok(peripheral) = adapter.peripheral(&id).await {
                    let props = peripheral.properties().await.ok().flatten();
                    let name = props.as_ref().and_then(|p| p.local_name.clone());

                    let matched_by = if props
                        .as_ref()
                        .map(|p| p.services.contains(&service_uuid()))
                        .unwrap_or(false)
                    {
                        Some(MatchedBy::AdvertisedService)
                    } else if name.as_deref().is_some_and(name_looks_like_a_guitar) {
                        Some(MatchedBy::Name)
                    } else {
                        None
                    };

                    if let Some(matched_by) = matched_by {
                        seen.push(Found {
                            name,
                            address: props
                                .as_ref()
                                .map(|p| p.address.to_string())
                                .unwrap_or_default(),
                            peripheral,
                            matched_by,
                        });
                    }
                }
            }
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let _ = adapter.stop_scan().await;

    if seen.is_empty() {
        return Err(TransportError::NotFound(timeout));
    }
    Ok(seen)
}

/// An open btleplug connection, as the protocol driver sees it.
///
/// This is the desktop [`Link`]: everything btleplug-specific about carrying a
/// message lives here, and nothing above it names btleplug at all.
pub struct BtleplugLink {
    peripheral: Peripheral,
    request: Characteristic,
    response: Characteristic,
    notifications: std::pin::Pin<Box<dyn Stream<Item = btleplug::api::ValueNotification> + Send>>,
}

impl Link for BtleplugLink {
    type Error = btleplug::Error;

    async fn write(&self, bytes: &[u8], with_response: bool) -> Result<(), btleplug::Error> {
        let kind = if with_response {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };
        self.peripheral.write(&self.request, bytes, kind).await
    }

    async fn read_response(&self) -> Result<Vec<u8>, btleplug::Error> {
        self.peripheral.read(&self.response).await
    }

    async fn next_notification(&mut self, within: Duration) -> Option<Vec<u8>> {
        if within.is_zero() {
            return None;
        }
        let note = tokio::time::timeout(within, self.notifications.next())
            .await
            .ok()
            .flatten()?;
        Some(note.value)
    }

    async fn disconnect(self) -> Result<(), btleplug::Error> {
        self.peripheral.disconnect().await
    }
}

/// Connect, set up notifications, and learn the usable write length.
///
/// Follows the connect order the vendor's client uses: discover, subscribe,
/// then read the version banner. The one step it cannot follow is
/// requesting an MTU — see [`Guitar::write_len`].
pub async fn connect(found: &Found) -> Result<Guitar, TransportError> {
    connect_with_retries(found, CONNECT_ATTEMPTS).await
}

/// Connect, retrying transient failures.
///
/// A BLE connect fails for reasons that have nothing to do with the peer
/// being wrong: the platform hands out a peripheral handle that has gone
/// stale since the scan, the device is mid-advertisement-interval, or the
/// adapter is still tearing down a previous session. "Not connected"
/// arriving from `connect` itself is the usual shape. One attempt turns
/// those into a failed run and an investigation of the wrong thing.
pub async fn connect_with_retries(found: &Found, attempts: u32) -> Result<Guitar, TransportError> {
    let mut last = None;
    for attempt in 1..=attempts.max(1) {
        match connect_once(found).await {
            Ok(guitar) => return Ok(guitar),
            Err(e) => {
                if attempt < attempts {
                    eprintln!(
                        "      connect attempt {attempt} failed ({e}); retrying in {:?}",
                        CONNECT_BACKOFF
                    );
                    tokio::time::sleep(CONNECT_BACKOFF).await;
                }
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or(TransportError::NoAdapter))
}

async fn connect_once(found: &Found) -> Result<Guitar, TransportError> {
    let peripheral = found.peripheral.clone();
    if !peripheral.is_connected().await.ble()? {
        peripheral.connect().await.ble()?;
    }
    peripheral.discover_services().await.ble()?;

    let characteristics = peripheral.characteristics();
    let request = characteristics
        .iter()
        .find(|c| c.uuid == request_uuid())
        .cloned()
        .ok_or(TransportError::MissingCharacteristic("request"))?;
    let response = characteristics
        .iter()
        .find(|c| c.uuid == response_uuid())
        .cloned()
        .ok_or(TransportError::MissingCharacteristic("response"))?;

    // Subscribe before anything is sent, so no reply can be missed.
    peripheral.subscribe(&response).await.ble()?;
    let notifications = peripheral.notifications().await.ble()?;

    // Read the banner here rather than leaving it to the caller, because
    // the firmware versions in it decide which transport to speak, and
    // that has to be settled before the first message goes out. A banner
    // that will not parse leaves the older transport in force: small
    // messages are wire-identical either way, so the conservative choice
    // costs nothing until a large message needs sending.
    let transport = match peripheral.read(&response).await {
        Ok(raw) => Banner::parse(&String::from_utf8_lossy(&raw))
            .filter(|b| llt2::selects_llt2(b.stm, b.esp))
            .map(|_| Transport::Llt2)
            .unwrap_or(Transport::Llt),
        Err(_) => Transport::Llt,
    };

    Ok(Guitar::over(
        BtleplugLink {
            peripheral,
            request,
            response,
            notifications,
        },
        transport,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uuid_constants_parse() {
        // These are `expect`ed at runtime, so a typo must fail a test rather
        // than a user's connection.
        let _ = service_uuid();
        let _ = request_uuid();
        let _ = response_uuid();
    }

    #[test]
    fn the_uuids_are_distinct() {
        assert_ne!(request_uuid(), response_uuid());
        assert_ne!(service_uuid(), request_uuid());
    }

    #[test]
    fn the_manuals_bt_id_example_is_recognised() {
        // The H2 manual's System Menu screenshot shows "BT ID: H2-SE614".
        assert!(name_looks_like_a_guitar("H2-SE614"));
        assert!(name_looks_like_a_guitar("h2-se614"));
        assert!(name_looks_like_a_guitar("  H2-ABC123  "));
    }

    #[test]
    fn other_product_spellings_are_recognised() {
        assert!(name_looks_like_a_guitar("HyVibe"));
        assert!(name_looks_like_a_guitar("LAG HyVibe"));
        assert!(name_looks_like_a_guitar("Lag-Guitar"));
    }

    #[test]
    fn unrelated_devices_are_not_matched() {
        for name in ["AirPods", "Galaxy Buds", "MX Master 3", "", "Bose QC45"] {
            assert!(
                !name_looks_like_a_guitar(name),
                "{name} should not look like a guitar"
            );
        }
    }
}
