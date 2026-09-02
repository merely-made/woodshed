//! The protocol driver: everything above the wire, and nothing that touches it.
//!
//! [`Guitar`] owns the parts of talking to an instrument that are true whatever
//! the radio is — the request/reply loop, acknowledgement handling for split
//! messages, the two framing generations, file transfer with its checksum, and
//! the device methods themselves. It reaches the instrument through a
//! [`Link`], so a platform supplies four I/O operations
//! and inherits all of this.
//!
//! # Why this is its own crate
//!
//! It used to live in `ringdown-ble` beside the btleplug code, which meant a
//! CoreBluetooth or Web Bluetooth client had to depend on btleplug to reach the
//! driver — the dependency it was trying to escape. Splitting it out is what
//! makes a second platform cost a transport rather than a fork.
//!
//! # What it refuses
//!
//! One method, `ReadConfig`, wedges the instrument's RPC handler until the
//! guitar is power-cycled (H18). The driver refuses it in code, for every
//! consumer, rather than each consumer keeping its own list; see
//! [`WEDGING_METHODS`] and [`Guitar::allow_wedging_calls`] for the probe's way
//! past it.
//!
//! # What a reply proves
//!
//! Only that the instrument parsed the request (H27). The typed writes return
//! a [`Sent`], not a success, and each one's doc names the receipt that does
//! verify it — the panel, the owner's ears, `ReadMetronome`, or the
//! remove-until-`false` count. The planners in [`ringdown::plan`] carry the
//! full account per write.
//!
//! Getting a connection is still the platform's business: discovery, pairing
//! and reconnection differ too much to abstract usefully. `ringdown-ble` does
//! that for desktop and hands back a [`Guitar`].

use std::time::Duration;

use ringdown::{
    effects::{BankSpec, Effect, ParamError},
    handshake::Banner,
    link::Link,
    llt::{self, Ack, LltCode},
    llt2,
    plan::{self, Call},
    rpc::{self, Method, RequestIds, Response, Status},
};
use serde_json::Value;
/// How long to wait for the device to answer one request.
///
/// Matches the vendor client's own timeout, which is the only evidence we have
/// about what the device considers a reasonable wait.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for a single LLT frame to be acknowledged.
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

/// What can go wrong talking to the instrument.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// The platform transport failed.
    ///
    /// Stringified rather than typed, and deliberately: a [`Link`]
    /// implementation chooses its own error type, and this driver has to work
    /// with all of them at once. Naming any one platform's error here is
    /// exactly what would tie the driver back to that platform. The message is
    /// the platform's own, so nothing diagnostic is lost.
    #[error("link error: {0}")]
    Link(String),

    /// No Bluetooth adapter is available.
    #[error("no bluetooth adapter found")]
    NoAdapter,

    /// Scanning finished without seeing a guitar.
    #[error("no guitar found while scanning for {0:?}")]
    NotFound(Duration),

    /// The peripheral connected but does not expose the expected GATT surface.
    #[error("connected device is missing the {0} characteristic")]
    MissingCharacteristic(&'static str),

    /// The device did not answer in time.
    ///
    /// Carries everything that *did* arrive while waiting. Without this a
    /// reply the client failed to recognise is indistinguishable from silence,
    /// and those two failures have opposite fixes.
    #[error("{}", timeout_message(.waited, .heard))]
    Timeout {
        /// How long was spent waiting.
        waited: Duration,
        /// Every notification received while waiting, as lossy UTF-8.
        heard: Vec<String>,
    },

    /// The device rejected a chunk of a split message.
    #[error("device rejected frame {frame} of a split message: {code:?}")]
    ChunkRejected {
        /// Which frame was rejected.
        frame: u32,
        /// The status the device returned.
        code: LltCode,
    },

    /// The protocol core refused to frame a message.
    #[error("framing failed: {0}")]
    Framing(#[from] llt::LltError),

    /// The compressed transport refused to prepare a message.
    #[error("LLT2 framing failed: {0}")]
    Framing2(#[from] llt2::Llt2Error),

    /// The RPC layer failed, including errors reported by the device.
    #[error("rpc failed: {0}")]
    Rpc(#[from] rpc::RpcError),

    /// A method this driver refuses to send, because it wedges the instrument.
    ///
    /// See [`WEDGING_METHODS`]. The refusal happens before anything is
    /// written, so the link is untouched.
    #[error(
        "{method} refused: it wedges the instrument's RPC handler until the guitar is \
         power-cycled (H18); Guitar::allow_wedging_calls sends it anyway"
    )]
    Refused {
        /// The method as the caller named it.
        method: String,
    },

    /// The protocol core refused to build the request, because the firmware
    /// would parse it and change nothing.
    #[error("refused before the wire: {0}")]
    Param(#[from] ParamError),

    /// The connect-time version banner could not be parsed.
    #[error("device sent an unrecognised version banner: {0:?}")]
    BadBanner(String),

    /// A reply arrived but was not the shape the method promises.
    #[error("{0}")]
    Shape(String),

    /// A file arrived, but not intact.
    #[error("file failed its checksum: device said {expected:#010x}, got {actual:#010x}")]
    ChecksumMismatch {
        /// What `GetFileInfo` reported.
        expected: u32,
        /// What the received bytes actually compute to.
        actual: u32,
    },
}

/// What the device knows about a stored file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileInfo {
    /// Length in bytes.
    pub size: u64,
    /// The device's own checksum, in its CRC-32/MPEG-2 variant.
    pub crc32: u32,
}

/// The largest single file read to attempt.
///
/// The reply is hex, so it carries two characters per byte, and it has to fit
/// one notification. Two hundred bytes leaves room for the JSON envelope
/// around it without probing for the exact ceiling.
pub const MAX_FILE_CHUNK: usize = 200;

// Hex doubles the payload, so a full chunk's reply is twice this many
// characters and must still fit one notification at the negotiated write
// length. Checked at compile time, since both sides are constants and a test
// would only discover at runtime what the compiler can refuse outright.
const _: () = assert!(MAX_FILE_CHUNK * 2 < ASSUMED_WRITE_LEN);

/// Split `/Loops/loop0031.wav` into its stem, number and extension.
///
/// Returns `None` for a name that does not end in digits before its
/// extension, since stepping such a name back is not meaningful.
fn split_numbered(path: &str) -> Option<(&str, u32, &str)> {
    let dot = path.rfind('.').unwrap_or(path.len());
    let (body, ext) = path.split_at(dot);
    let digits_start = body
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i)?;
    let number = body[digits_start..].parse().ok()?;
    Some((&body[..digits_start], number, ext))
}

/// Decode an uppercase or lowercase hex string.
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

/// Methods the driver will not send unless told to.
///
/// `ReadConfig` returns nothing and wedges the instrument's RPC handler:
/// every later request, including ones that worked moments before, is met
/// with silence until the guitar is power-cycled (H18). Its contents are
/// reachable by composing calls that work, so there is nothing to gain and a
/// power cycle to lose. The failure it causes looks like an unrelated bug,
/// which is why the refusal is code rather than a comment. Matched
/// case-insensitively, since whether the firmware is strict about case is
/// untested and the cost of guessing wrong is a wedged instrument.
pub const WEDGING_METHODS: &[&str] = &["ReadConfig"];

/// What a typed write got back: the request id and the raw reply.
///
/// Named for what it proves. A reply of `true` from this firmware means the
/// request was **parsed**, not that the instrument changed (H27): `den`
/// outside its whitelist, `SetBankName` on an empty tile and `AddEffect` into
/// a bank that cannot render all answer `true` and do nothing. What verifies
/// a write is named on the method that sent it. `RemoveEffect` is the one
/// exception where `true` does mean stored (H31).
#[derive(Debug, Clone, PartialEq)]
pub struct Sent {
    /// The request id, for correlating with a trace.
    pub id: i64,
    /// The reply's `result`, uninterpreted.
    pub reply: Value,
}

impl Sent {
    /// Whether the reply was the literal `true`: the instrument parsed the
    /// request. Says nothing about whether it was applied.
    pub fn parsed(&self) -> bool {
        self.reply == Value::Bool(true)
    }
}

/// A connected guitar.
///
/// Generic over its transport. A platform — btleplug on desktop, CoreBluetooth,
/// Web Bluetooth, Android — supplies a [`Link`] and gets this whole driver
/// unchanged. Transport crates typically alias this to a concrete name, as
/// `ringdown_ble::Guitar` does.
pub struct Guitar<L: Link> {
    link: L,
    ids: RequestIds,
    write_len: usize,
    request_timeout: Duration,
    trace: bool,
    transport: Transport,
    allow_wedging: bool,
}

/// Which of the two transports this instrument speaks.
///
/// Chosen from the firmware versions in the connect-time banner, exactly as
/// the vendor's client chooses: both processors at 1.2.2 or newer means LLT2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// JSON messages in JSON frames.
    Llt,
    /// Compressed messages in binary frames.
    Llt2,
}

impl<L: Link> Guitar<L>
where
    L::Error: std::fmt::Display,
{
    /// Drive an instrument over an already-open transport.
    ///
    /// The path for platforms other than desktop btleplug: obtain a connection
    /// however that platform does it, wrap it in a [`Link`], and the rest of
    /// the protocol works unchanged. `transport` is which framing generation to
    /// speak, normally decided from the connect-time banner.
    pub fn over(link: L, transport: Transport) -> Guitar<L> {
        let write_len = link.write_len_hint().unwrap_or(ASSUMED_WRITE_LEN);
        Guitar {
            link,
            ids: RequestIds::new(),
            write_len,
            request_timeout: REQUEST_TIMEOUT,
            trace: false,
            transport,
            allow_wedging: false,
        }
    }

    fn link_err(e: L::Error) -> TransportError {
        TransportError::Link(e.to_string())
    }

    /// The write length in use. See [`ASSUMED_WRITE_LEN`] for why it is
    /// assumed rather than negotiated.
    pub fn write_len(&self) -> usize {
        self.write_len
    }

    /// Print every notification received to stderr as it arrives.
    ///
    /// The client only surfaces messages it recognises, so when a request goes
    /// unanswered this is what distinguishes a device that said nothing from a
    /// device that said something unexpected.
    pub fn set_trace(&mut self, on: bool) {
        self.trace = on;
    }

    /// Let [`WEDGING_METHODS`] through for the rest of this connection.
    ///
    /// For the probe, whose job includes sending the instrument things that
    /// break it. A client has no reason to call this; the guard exists
    /// because the failure it prevents costs a power cycle and looks like
    /// somebody else's bug.
    pub fn allow_wedging_calls(&mut self) {
        self.allow_wedging = true;
    }

    /// The transport underneath, for what only the platform can answer.
    pub fn link(&self) -> &L {
        &self.link
    }

    /// Subscribe and print traffic for a while without sending anything.
    ///
    /// Answers the narrower question of whether the device ever emits a
    /// notification unprompted, which separates "notifications are not working"
    /// from "the request was not understood".
    pub async fn listen(&mut self, duration: Duration) -> Vec<String> {
        let deadline = tokio::time::Instant::now() + duration;
        let mut heard = Vec::new();
        while let Some(bytes) = self.next_notification(deadline).await {
            heard.push(render(&bytes));
        }
        heard
    }

    /// Write a raw message to the request characteristic, bypassing framing.
    ///
    /// For probing only: it is how an alternative encoding gets tested without
    /// the rest of the stack insisting on the recovered one.
    pub async fn write_raw(&self, bytes: &[u8], with_response: bool) -> Result<(), TransportError> {
        self.link
            .write(bytes, with_response)
            .await
            .map_err(Self::link_err)
    }

    /// How long to wait for a reply.
    ///
    /// The default matches the vendor client's, but that is evidence about
    /// what the app tolerates rather than about how slow the device can be: a
    /// large read may have to gather state from flash and compress it before
    /// it can answer at all.
    pub fn set_request_timeout(&mut self, timeout: Duration) {
        self.request_timeout = timeout;
    }

    /// Which transport this connection is using.
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// Force a transport, overriding what the banner implied.
    ///
    /// The version rule is read from the vendor's client rather than stated by
    /// the device, so being able to contradict it is what makes it testable.
    pub fn set_transport(&mut self, transport: Transport) {
        self.transport = transport;
    }

    /// Override the write length.
    ///
    /// Present because the negotiated MTU is not always discoverable, and
    /// finding the largest value the device actually accepts may be empirical.
    pub fn set_write_len(&mut self, len: usize) {
        self.write_len = len;
    }

    /// Read the connect-time version banner.
    ///
    /// This is a GATT *read* of the response characteristic, not a
    /// notification, and it happens before any RPC.
    pub async fn banner(&self) -> Result<Banner, TransportError> {
        let raw = self.link.read_response().await.map_err(Self::link_err)?;
        let text = String::from_utf8_lossy(&raw).into_owned();
        Banner::parse(&text).ok_or(TransportError::BadBanner(text))
    }

    /// Call a method and return its result.
    pub async fn call(&mut self, method: Method, params: Value) -> Result<Value, TransportError> {
        self.call_named(method.wire_name(), params).await
    }

    /// Call a method by its wire name, including one ringdown has no
    /// [`Method`] variant for.
    ///
    /// The compressor's keyword dictionary names methods the vendor's own app
    /// never calls, and the only way to learn whether they are callable, and
    /// what they want, is to ask the instrument. This is how. One name is
    /// refused whatever the caller says: see [`WEDGING_METHODS`].
    pub async fn call_named(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, TransportError> {
        Ok(self.call_raw(method, params).await?.1)
    }

    /// The request/reply loop under every call: refuse what wedges, allocate
    /// an id, encode, send over whichever transport, wait for the answer.
    async fn call_raw(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(i64, Value), TransportError> {
        if !self.allow_wedging
            && WEDGING_METHODS
                .iter()
                .any(|m| m.eq_ignore_ascii_case(method))
        {
            return Err(TransportError::Refused {
                method: method.to_string(),
            });
        }
        let id = self.ids.next_id();
        let encoded = serde_json::to_string(&serde_json::json!({
            "jsonrpc": ringdown::rpc::JSONRPC_VERSION,
            "id": id,
            "method": method,
            "params": params,
        }))
        .map_err(|e| rpc::RpcError::Encode(e.to_string()))?;

        match self.transport {
            Transport::Llt => self.send_llt(&encoded, id).await?,
            Transport::Llt2 => self.send_llt2(&encoded, id).await?,
        }

        let response = self.await_response(id).await?;
        Ok((id, response.into_result()?))
    }

    /// Send via the older transport: JSON frames, acknowledged as JSON.
    async fn send_llt(&mut self, encoded: &str, id: i64) -> Result<(), TransportError> {
        let outbound = llt::frame_message(encoded, id, self.write_len)?;
        let chunked = outbound.is_chunked();
        let frames: Vec<Vec<u8>> = outbound
            .frames()
            .iter()
            .map(|f| f.as_bytes().to_vec())
            .collect();

        for (index, frame) in frames.iter().enumerate() {
            self.write_frame(frame).await?;
            // Only split messages are acknowledged frame by frame; an unsplit
            // one is answered directly by its RPC reply.
            if chunked {
                let frame_no = (index + 1) as u32;
                let ack = self.await_llt_ack(id, frame_no).await?;
                if !ack.code.is_continue() && !ack.code.is_terminal_success() {
                    return Err(TransportError::ChunkRejected {
                        frame: frame_no,
                        code: ack.code,
                    });
                }
            }
        }
        Ok(())
    }

    /// Send via LLT2: compressed, in binary frames acknowledged as six bytes.
    async fn send_llt2(&mut self, encoded: &str, id: i64) -> Result<(), TransportError> {
        let object_id = id as u8;
        let outbound = llt2::prepare(encoded, object_id, self.write_len)?;
        let framed = outbound.is_framed();
        let frames = outbound.frames().to_vec();

        for (index, frame) in frames.iter().enumerate() {
            self.write_frame(frame).await?;
            if framed {
                let frame_no = (index + 1) as u16;
                let ack = self.await_llt2_ack(object_id, frame_no).await?;
                if !ack.code.is_continue() && !ack.code.is_terminal_success() {
                    return Err(TransportError::ChunkRejected {
                        frame: u32::from(frame_no),
                        code: ack.code,
                    });
                }
            }
        }
        Ok(())
    }

    async fn write_frame(&self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.trace {
            eprintln!("      -> {} bytes", bytes.len());
        }
        self.link.write(bytes, true).await.map_err(Self::link_err)?;
        Ok(())
    }

    /// Drain any notifications that arrive within `window` after a call.
    ///
    /// A reply is not necessarily the whole answer. `call` returns on the first
    /// message matching the request id and stops listening, so a device that
    /// acknowledges first and sends data afterwards would have its data
    /// discarded, the same shape of mistake that made a working `GetStatus`
    /// look like silence. This is how to check rather than assume.
    pub async fn drain(&mut self, window: Duration) -> Vec<String> {
        self.listen(window).await
    }

    /// Read the instrument's status.
    ///
    /// This is the control run that matters: if it answers, the recovered
    /// protocol map is confirmed against hardware.
    pub async fn status(&mut self) -> Result<Status, TransportError> {
        let value = self.call(Method::GetStatus, rpc::params::none()).await?;
        Ok(serde_json::from_value(value).map_err(|e| rpc::RpcError::Decode(e.to_string()))?)
    }

    // -- Typed writes ------------------------------------------------------
    //
    // One method per planner in `ringdown::plan`. The planner's doc is the
    // full account of what the write does and which receipt proves it;
    // these repeat only the receipt.

    /// Send a planned call and carry back what the instrument replied.
    ///
    /// The one path under every typed write below, and the way to send a
    /// [`Call`] built elsewhere.
    pub async fn send(&mut self, call: Call) -> Result<Sent, TransportError> {
        let (id, reply) = self.call_raw(call.method.wire_name(), call.params).await?;
        Ok(Sent { id, reply })
    }

    /// Append `effect` to the chain in `slot`. **Receipt: ears.** See
    /// [`plan::add_effect`].
    pub async fn add_effect(&mut self, slot: i64, effect: &Effect) -> Result<Sent, TransportError> {
        self.send(plan::add_effect(slot, effect)).await
    }

    /// Replace the effect at `index` in `slot`. **Receipt: ears**, less
    /// established than `add_effect`. See [`plan::update_effect`].
    pub async fn update_effect(
        &mut self,
        slot: i64,
        index: i64,
        effect: &Effect,
    ) -> Result<Sent, TransportError> {
        self.send(plan::update_effect(slot, index, effect)).await
    }

    /// Remove the effect at `index` from `slot`. **Receipt: the reply**, the
    /// one `true` that means stored; `false` on an empty chain is how a chain
    /// is counted (H31). See [`plan::remove_effect`].
    pub async fn remove_effect(&mut self, slot: i64, index: i64) -> Result<Sent, TransportError> {
        self.send(plan::remove_effect(slot, index)).await
    }

    /// Move the effect at `from` in `slot` to `to`. **Receipt: none from
    /// software.** See [`plan::move_effect`].
    pub async fn move_effect(
        &mut self,
        slot: i64,
        from: i64,
        to: i64,
    ) -> Result<Sent, TransportError> {
        self.send(plan::move_effect(slot, from, to)).await
    }

    /// Select `slot` on the panel. **Receipt: panel.** Never while the owner
    /// is comparing by ear. See [`plan::switch_bank`].
    pub async fn switch_bank(&mut self, slot: i64) -> Result<Sent, TransportError> {
        self.send(plan::switch_bank(slot)).await
    }

    /// Rename the bank in `slot`. **Receipt: panel**, and only on a slot that
    /// already holds an effect (H33). See [`plan::set_bank_name`].
    pub async fn set_bank_name(&mut self, slot: i64, name: &str) -> Result<Sent, TransportError> {
        self.send(plan::set_bank_name(slot, name)).await
    }

    /// Set the output gain of `slot` in decibels. **Receipt: none.** See
    /// [`plan::set_gain_bank`].
    pub async fn set_gain_bank(&mut self, slot: i64, gain_db: f32) -> Result<Sent, TransportError> {
        self.send(plan::set_gain_bank(slot, gain_db)).await
    }

    /// Sustain-killer state for `slot`. **Receipt: none.** See
    /// [`plan::sustain_killer`].
    pub async fn sustain_killer(
        &mut self,
        slot: i64,
        killed: Option<bool>,
        reset: Option<bool>,
    ) -> Result<Sent, TransportError> {
        self.send(plan::sustain_killer(slot, killed, reset)).await
    }

    /// Move the bank at `from` to `to`. **Receipt: panel**; unexercised. See
    /// [`plan::move_bank`].
    pub async fn move_bank(&mut self, from: i64, to: i64) -> Result<Sent, TransportError> {
        self.send(plan::move_bank(from, to)).await
    }

    /// Remove the bank in `slot`, shifting later slots down. **Receipt:
    /// panel**; unexercised. See [`plan::remove_bank`].
    pub async fn remove_bank(&mut self, slot: i64) -> Result<Sent, TransportError> {
        self.send(plan::remove_bank(slot)).await
    }

    /// **Insert** `bank` at `slot`, renumbering every later slot (H38).
    /// **Receipt: panel, then ears.** Whether the object renders is the open
    /// research question; see [`plan::add_bank`].
    pub async fn add_bank(&mut self, slot: i64, bank: &BankSpec) -> Result<Sent, TransportError> {
        self.send(plan::add_bank(slot, bank)).await
    }

    /// Start the metronome. **Receipt: `ReadMetronome`.** A `den` the
    /// firmware would drop is refused before the wire (H24). See
    /// [`plan::start_metronome`].
    pub async fn start_metronome(
        &mut self,
        bpm: i64,
        num: Option<i64>,
        den: Option<i64>,
        bars: Option<i64>,
    ) -> Result<Sent, TransportError> {
        self.send(plan::start_metronome(bpm, num, den, bars)?).await
    }

    /// Change the running metronome. **Receipt: `ReadMetronome`.** See
    /// [`plan::update_metronome`].
    pub async fn update_metronome(
        &mut self,
        bpm: i64,
        num: Option<i64>,
        den: Option<i64>,
        bars: Option<i64>,
    ) -> Result<Sent, TransportError> {
        self.send(plan::update_metronome(bpm, num, den, bars)?)
            .await
    }

    /// Stop the metronome. **Receipt: ears.**
    pub async fn stop_metronome(&mut self) -> Result<Sent, TransportError> {
        self.send(plan::stop_metronome()).await
    }

    /// Size and checksum of a stored file.
    pub async fn file_info(&mut self, name: &str) -> Result<FileInfo, TransportError> {
        let reply = self
            .call_named("GetFileInfo", serde_json::json!({ "name": name }))
            .await?;
        let field = |key: &str| -> Result<u64, TransportError> {
            reply
                .get(key)
                .and_then(Value::as_u64)
                .ok_or_else(|| TransportError::Shape(format!("missing {key} in {reply}")))
        };
        Ok(FileInfo {
            size: field("size")?,
            crc32: field("crc32")? as u32,
        })
    }

    /// The name the *next* recording will take.
    ///
    /// Named as the device names it, and deliberately not called
    /// `last_recording_name`: with 31 loops on the instrument this returns
    /// `loop0032.wav`, a file that does not exist yet. Asking `GetFileInfo`
    /// about it fails, which is a confusing result if you believed the method
    /// name. See [`Guitar::latest_recording_name`] for the one you can open.
    pub async fn next_recording_name(&mut self) -> Result<String, TransportError> {
        let reply = self
            .call_named("GetLastRecordingName", rpc::params::none())
            .await?;
        reply
            .as_str()
            .map(String::from)
            .ok_or_else(|| TransportError::Shape(format!("expected a path, got {reply}")))
    }

    /// The most recent recording that actually exists, or `None` if there is
    /// none.
    ///
    /// Derived by stepping back from [`Guitar::next_recording_name`] and
    /// confirming the file opens, because the device's own answer is one past
    /// the end and arithmetic alone would only move the off-by-one rather than
    /// remove it.
    pub async fn latest_recording_name(&mut self) -> Result<Option<String>, TransportError> {
        let next = self.next_recording_name().await?;
        let Some((stem, number, ext)) = split_numbered(&next) else {
            return Ok(None);
        };
        if number == 0 {
            return Ok(None);
        }
        let candidate = format!("{stem}{:04}{ext}", number - 1);
        match self.file_info(&candidate).await {
            Ok(_) => Ok(Some(candidate)),
            // A device error here means "no such file", which is an answer
            // rather than a failure: there simply is no previous recording.
            Err(TransportError::Rpc(rpc::RpcError::Device(_))) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Read part of a file.
    ///
    /// `DumpFile` answers with the bytes as an uppercase hex string, so a
    /// reply carries half as many bytes as it has characters. That doubling,
    /// against a notification that must fit the MTU, is what caps a single
    /// read at a couple of hundred bytes and makes a whole loop a long
    /// transfer rather than a quick one.
    pub async fn read_file_range(
        &mut self,
        name: &str,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, TransportError> {
        let reply = self
            .call_named(
                "DumpFile",
                serde_json::json!({ "name": name, "offset": offset, "size": len }),
            )
            .await?;
        let hex = reply
            .as_str()
            .ok_or_else(|| TransportError::Shape(format!("expected hex, got {reply}")))?;
        decode_hex(hex).ok_or_else(|| {
            TransportError::Shape(format!("reply was not hex ({} chars)", hex.len()))
        })
    }

    /// Read a whole file, verifying it against the checksum the device reports.
    ///
    /// `progress` is called with (bytes so far, total) after each chunk, since
    /// a multi-megabyte loop takes minutes over this transport and a silent
    /// wait is indistinguishable from a hang.
    ///
    /// The checksum is the device's own (`CRC-32/MPEG-2`, see
    /// [`ringdown::crc32`]) and is verified over the assembled file. A
    /// transfer that arrives corrupt fails here rather than being written to
    /// disk and discovered later.
    pub async fn read_file(
        &mut self,
        name: &str,
        chunk: usize,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<Vec<u8>, TransportError> {
        let info = self.file_info(name).await?;
        let chunk = chunk.clamp(1, MAX_FILE_CHUNK);
        let mut out = Vec::with_capacity(info.size as usize);

        while (out.len() as u64) < info.size {
            let remaining = info.size - out.len() as u64;
            let want = core::cmp::min(remaining, chunk as u64) as usize;
            let piece = self.read_file_range(name, out.len() as u64, want).await?;
            if piece.is_empty() {
                return Err(TransportError::Shape(format!(
                    "device returned nothing at offset {} of {}",
                    out.len(),
                    info.size
                )));
            }
            out.extend_from_slice(&piece);
            progress(out.len() as u64, info.size);
        }

        let actual = ringdown::crc32::compute(&out);
        if actual != info.crc32 {
            return Err(TransportError::ChecksumMismatch {
                expected: info.crc32,
                actual,
            });
        }
        Ok(out)
    }

    /// Disconnect cleanly.
    ///
    /// Consuming, because the instrument serves one client at a time: a link
    /// that has been handed back cannot be used again, and the type system is
    /// the cheapest place to say so.
    pub async fn disconnect(self) -> Result<(), TransportError> {
        self.link.disconnect().await.map_err(Self::link_err)
    }

    async fn await_llt_ack(&mut self, object_id: i64, frame: u32) -> Result<Ack, TransportError> {
        let deadline = tokio::time::Instant::now() + ACK_TIMEOUT;
        let mut heard = Vec::new();
        loop {
            let Some(bytes) = self.next_notification(deadline).await else {
                return Err(TransportError::Timeout {
                    waited: ACK_TIMEOUT,
                    heard,
                });
            };
            let text = String::from_utf8_lossy(&bytes).into_owned();
            if let Some(ack) = Ack::parse(&text) {
                // Acks for other transfers, or for frames already past, are
                // noise rather than errors.
                if ack.object_id == object_id && ack.message_id == frame {
                    return Ok(ack);
                }
            }
            heard.push(render(&bytes));
        }
    }

    async fn await_llt2_ack(
        &mut self,
        object_id: u8,
        frame: u16,
    ) -> Result<llt2::Ack2, TransportError> {
        let deadline = tokio::time::Instant::now() + ACK_TIMEOUT;
        let mut heard = Vec::new();
        loop {
            let Some(bytes) = self.next_notification(deadline).await else {
                return Err(TransportError::Timeout {
                    waited: ACK_TIMEOUT,
                    heard,
                });
            };
            if let Some(ack) = llt2::Ack2::parse(&bytes)
                && ack.answers(object_id, frame)
            {
                return Ok(ack);
            }
            heard.push(render(&bytes));
        }
    }

    async fn await_response(&mut self, id: i64) -> Result<Response, TransportError> {
        let deadline = tokio::time::Instant::now() + self.request_timeout;
        let mut heard = Vec::new();
        loop {
            let Some(bytes) = self.next_notification(deadline).await else {
                return Err(TransportError::Timeout {
                    waited: self.request_timeout,
                    heard,
                });
            };

            // A reply may arrive compressed or as plain JSON. Try decompressing
            // first: the start-nibble check makes that a cheap, unambiguous
            // test rather than a guess, and plain JSON simply fails it.
            let candidate = match ringdown::compress::decode(&bytes) {
                Some(json) => Some(json),
                None => core::str::from_utf8(&bytes).ok().map(String::from),
            };

            if let Some(text) = candidate {
                // The response characteristic multiplexes acknowledgements with
                // replies, so anything that parses as an ack is not our answer.
                if Ack::parse(&text).is_none()
                    && let Ok(response) = Response::decode(&text)
                    && response.answers(id)
                {
                    return Ok(response);
                }
            }
            heard.push(render(&bytes));
        }
    }

    /// The next notification's raw bytes, or `None` once `deadline` passes.
    ///
    /// Deliberately **not** decoded to text here. A compressed reply is binary,
    /// and a lossy UTF-8 conversion would replace every byte outside ASCII with
    /// a replacement character, destroying the payload before anything had a
    /// chance to decompress it. Every notification passes through this one
    /// place so that tracing sees the traffic as it actually arrived.
    async fn next_notification(&mut self, deadline: tokio::time::Instant) -> Option<Vec<u8>> {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let bytes = self.link.next_notification(remaining).await?;
        if self.trace {
            eprintln!("      <- {} bytes: {}", bytes.len(), render(&bytes));
        }
        Some(bytes)
    }
}

/// Present a notification readably, decompressing it when it is compressed.
///
/// Used for tracing and for the evidence a timeout carries, so that a
/// compressed frame reads as JSON rather than as a wall of replacement
/// characters.
fn render(bytes: &[u8]) -> String {
    if let Some(json) = ringdown::compress::decode(bytes) {
        return format!("[compressed] {json}");
    }
    match core::str::from_utf8(bytes) {
        Ok(text) => format!("{text:?}"),
        Err(_) => {
            let hex: Vec<String> = bytes.iter().take(32).map(|b| format!("{b:02x}")).collect();
            format!(
                "[binary] {}{}",
                hex.join(" "),
                if bytes.len() > 32 { " ..." } else { "" }
            )
        }
    }
}

/// Render a timeout together with whatever was overheard.
///
/// The difference between "nothing arrived" and "something arrived that we
/// failed to recognise" is the entire diagnosis, and those two have opposite
/// fixes, so the error carries the evidence rather than discarding it.
fn timeout_message(waited: &Duration, heard: &[String]) -> String {
    if heard.is_empty() {
        format!("timed out after {waited:?}; the device sent nothing at all")
    } else {
        format!(
            "timed out after {waited:?}; the device sent {} message(s), none of which was the              expected reply: {heard:?}",
            heard.len()
        )
    }
}

/// The write length assumed when the platform will not say what it negotiated.
///
/// btleplug 0.11 exposes no MTU accessor at all — not to request one, not even
/// to read what the platform agreed (deviceplug/btleplug#246 is still open). So
/// this cannot be discovered the way the vendor's client discovers it, and a
/// number has to be chosen.
///
/// The choice is 514, matching what the vendor's client gets after asking for
/// an MTU of 517, on the reasoning that the device is built to accept it and
/// modern platform stacks negotiate high MTUs unprompted. That is an inference,
/// not a measurement, and it is the assumption most likely to be wrong on this
/// page.
///
/// The two failure modes are not symmetric, which is why the optimistic value
/// wins: assuming the 20-byte floor would make every message fail, since 20 is
/// too small to carry even one LLT frame, whereas assuming too much fails
/// visibly on the write that overruns. Neither corrupts anything. If writes are
/// rejected, lower it with [`Guitar::set_write_len`] until they are not — and
/// record the value that worked.
pub const ASSUMED_WRITE_LEN: usize = 514;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numbered_recording_splits_for_stepping_back() {
        let (stem, n, ext) = split_numbered("/Loops/loop0032.wav").unwrap();
        assert_eq!(stem, "/Loops/loop");
        assert_eq!(n, 32);
        assert_eq!(ext, ".wav");
        // Reassembling one back is the whole point: the device reports the
        // next name, and the latest existing one is beneath it.
        assert_eq!(format!("{stem}{:04}{ext}", n - 1), "/Loops/loop0031.wav");
    }

    #[test]
    fn an_unnumbered_name_declines_rather_than_guessing() {
        assert!(split_numbered("/Loops/take.wav").is_none());
        assert!(split_numbered("/Loops/").is_none());
    }

    #[test]
    fn hex_replies_decode_and_bad_ones_are_refused() {
        // The opening of a real reply: "RIFF".
        assert_eq!(decode_hex("52494646").unwrap(), b"RIFF");
        assert_eq!(decode_hex("").unwrap(), Vec::<u8>::new());
        assert!(decode_hex("52494").is_none(), "odd length is not hex");
        assert!(decode_hex("52ZZ4646").is_none(), "non-hex digits");
    }

    use core::cell::RefCell;
    use ringdown::effects::EffectKind;
    use std::collections::VecDeque;

    /// A link that answers from a script instead of a radio.
    ///
    /// Every write is recorded as the text it was, and the next scripted
    /// result is queued as a reply under the id that request carried. Unsplit
    /// LLT is the bare JSON message, so the id is there to read.
    struct ScriptedLink {
        written: RefCell<Vec<String>>,
        results: RefCell<VecDeque<Value>>,
        inbox: RefCell<VecDeque<Vec<u8>>>,
    }

    impl ScriptedLink {
        fn answering(results: impl IntoIterator<Item = Value>) -> Self {
            ScriptedLink {
                written: RefCell::new(Vec::new()),
                results: RefCell::new(results.into_iter().collect()),
                inbox: RefCell::new(VecDeque::new()),
            }
        }

        fn written(&self) -> Vec<String> {
            self.written.borrow().clone()
        }
    }

    impl Link for ScriptedLink {
        type Error = core::convert::Infallible;

        async fn write(&self, bytes: &[u8], _with_response: bool) -> Result<(), Self::Error> {
            let text = String::from_utf8(bytes.to_vec()).expect("unsplit LLT is JSON text");
            let request: Value = serde_json::from_str(&text).expect("a well-formed request");
            let id = request["id"].as_i64().expect("an integer id");
            self.written.borrow_mut().push(text);
            if let Some(result) = self.results.borrow_mut().pop_front() {
                let reply = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
                self.inbox
                    .borrow_mut()
                    .push_back(reply.to_string().into_bytes());
            }
            Ok(())
        }

        async fn read_response(&self) -> Result<Vec<u8>, Self::Error> {
            Ok(b"S1.2.2_E1.3.0\n".to_vec())
        }

        async fn next_notification(&mut self, _within: Duration) -> Option<Vec<u8>> {
            self.inbox.borrow_mut().pop_front()
        }

        async fn disconnect(self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn guitar(results: impl IntoIterator<Item = Value>) -> Guitar<ScriptedLink> {
        Guitar::over(ScriptedLink::answering(results), Transport::Llt)
    }

    /// The guard fires before anything touches the link, by name in any case,
    /// and the override is what lets the probe through.
    #[tokio::test]
    async fn read_config_is_refused_before_anything_is_written() {
        let mut g = guitar([Value::Bool(true)]);

        let err = g
            .call(Method::ReadConfig, rpc::params::none())
            .await
            .unwrap_err();
        assert!(
            matches!(err, TransportError::Refused { ref method } if method == "ReadConfig"),
            "{err}"
        );
        assert!(g.link().written().is_empty(), "nothing may reach the link");

        let err = g
            .call_named("readconfig", rpc::params::none())
            .await
            .unwrap_err();
        assert!(matches!(err, TransportError::Refused { .. }));
        assert!(format!("{err}").contains("allow_wedging_calls"), "{err}");

        g.allow_wedging_calls();
        let reply = g
            .call(Method::ReadConfig, rpc::params::none())
            .await
            .unwrap();
        assert_eq!(reply, Value::Bool(true));
        assert_eq!(g.link().written().len(), 1);
    }

    /// A typed write sends exactly the planner's bytes under the next id, and
    /// carries the reply back uninterpreted: `false` is an answer, not an
    /// error, because that is how a chain is counted (H31).
    #[tokio::test]
    async fn a_typed_write_sends_the_planned_bytes_and_carries_the_reply() {
        let mut g = guitar([Value::Bool(true), Value::Bool(false)]);
        let octave = Effect::new(EffectKind::Pitch).with("Shift", -12.0).unwrap();

        let sent = g.add_effect(4, &octave).await.unwrap();
        assert_eq!(sent.id, 1);
        assert!(sent.parsed());
        assert_eq!(
            g.link().written()[0],
            r#"{"jsonrpc":2.0,"id":1,"method":"AddEffect","params":{"bank_num":4,"effect":{"preset":"default","type":"Pitch","bypass":false,"params":[{"key":"Shift","value":-12.0}]}}}"#
        );

        let sent = g.remove_effect(4, 0).await.unwrap();
        assert_eq!(sent.id, 2);
        assert!(!sent.parsed());
        assert_eq!(sent.reply, Value::Bool(false));
    }

    /// A `den` the firmware would silently drop is refused in the core and
    /// never reaches the link; the whitelist goes through.
    #[tokio::test]
    async fn a_refused_den_never_reaches_the_link() {
        let mut g = guitar([Value::Bool(true)]);
        let err = g
            .update_metronome(96, Some(6), Some(8), None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, TransportError::Param(ParamError::DenNotAccepted(8))),
            "{err}"
        );
        assert!(g.link().written().is_empty());

        let sent = g
            .update_metronome(96, Some(6), Some(4), None)
            .await
            .unwrap();
        assert!(sent.parsed());
        assert!(g.link().written()[0].ends_with(r#""params":{"bpm":96,"num":6,"den":4}}"#));
    }

    #[tokio::test]
    async fn bank_methods_send_their_documented_params() {
        let mut g = guitar([Value::Bool(true), Value::Bool(true), Value::Bool(true)]);
        g.switch_bank(3).await.unwrap();
        g.set_bank_name(8, "ringdown").await.unwrap();
        g.add_bank(4, &BankSpec::new("octave")).await.unwrap();
        let w = g.link().written();
        assert!(
            w[0].ends_with(r#""method":"SwitchBank","params":{"bank_num":3}}"#),
            "{}",
            w[0]
        );
        assert!(
            w[1].ends_with(r#""method":"SetBankName","params":{"bank_num":8,"name":"ringdown"}}"#),
            "{}",
            w[1]
        );
        assert!(
            w[2].ends_with(r#""method":"AddBank","params":{"bank_num":4,"bank":{"name":"octave","effects":[]}}}"#),
            "{}",
            w[2]
        );
    }
}
