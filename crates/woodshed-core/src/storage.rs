//! The storage seam (genet-host plan W0.2) and the persisted session.
//!
//! The core describes what survives a restart as one serde struct; a host
//! supplies a [`Storage`] that moves the serialized form — filesystem on
//! desktop, OPFS in the browser. Every field is `#[serde(default)]`-safe
//! so old files keep loading as the struct grows.

use muniment::{Backend, StoreError};
use crate::loop_capture::{
    EncodedCapture, LoopCaptureCodec, LoopCaptureRef, MAX_CAPTURE_FRAMES,
    MAX_PCM_BYTES, PCM_HEADER_BYTES, decode_capture, encode_capture, encode_pcm, pcm_digest,
    pcm_key, validate_namespace,
};
pub use crate::settings::RelatedSettings;

mod session;
pub use session::{AppSection, PersistedSession, SessionLoad, decode_session};

/// The two things woodshed keeps between runs, as named slots on a
/// [`muniment::Backend`].
///
/// This used to be two traits, `Storage` and `SettingsStorage`, each a single
/// hardcoded slot moving a `String`. That was a restatement of what muniment
/// already provides, and its lack of a key space is why a second thing to store
/// needed a second trait rather than a second name. A third needs neither now.
///
/// The host supplies the backend, which is where the platform split lives:
/// filesystem on desktop, OPFS or IndexedDB in the browser, redb or zip where a
/// real store is wanted. Sealing is [`crate::sealed_backend::SealedBackend`],
/// composed in by the host rather than known about here.
pub struct SessionStore<B> {
    backend: B,
    // Admission is shared by clones: reopening an uncertain slot through a
    // second handle must not silently authorize a default-state overwrite.
    refusal: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    write_errors:
        std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<&'static str, String>>>,
}

impl<B: Clone> Clone for SessionStore<B> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            refusal: self.refusal.clone(),
            write_errors: self.write_errors.clone(),
        }
    }
}

/// The session slot: artifact state, what the practitioner was doing.
const SESSION_SLOT: &str = "session";
/// The settings slot: install and persona-facing preferences. Separate from the
/// session because they answer different questions and are written at different
/// times, which is the distinction the two old traits were really drawing.
const SETTINGS_SLOT: &str = "settings";

pub const MAX_PRACTICE_WAV_BYTES: usize = 16 * 1024 * 1024;

pub fn practice_wav_key(digest: &str) -> Result<String, String> {
    if digest.len() != 64
        || !digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("Invalid practice recording digest.".into());
    }
    Ok(format!("practice-wav/{digest}"))
}

impl<B: Backend> SessionStore<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            refusal: Default::default(),
            write_errors: Default::default(),
        }
    }

    /// The backend, for a host that also drives it directly.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn load(&self) -> Option<String> {
        self.read(SESSION_SLOT)
    }

    /// Missing is a valid new session; unreadable is retained and quarantined.
    pub fn try_load(&self) -> Result<Option<String>, StoreError> {
        self.try_read(SESSION_SLOT)
    }

    /// A host calls this after semantic/JSON admission fails. There is no
    /// in-process reset: only a deliberate reopen can admit the original again.
    pub fn refuse_writes(&self, reason: impl Into<String>) {
        let mut refusal = self
            .refusal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        refusal.get_or_insert_with(|| reason.into());
    }

    pub fn write_refusal(&self) -> Option<String> {
        self.refusal
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// A failed slot remains unconfirmed until that same slot is verified.
    pub fn save_problem(&self) -> Option<String> {
        self.write_refusal().or_else(|| {
            let errors = self
                .write_errors
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            (!errors.is_empty()).then(|| errors.values().cloned().collect::<Vec<_>>().join("; "))
        })
    }

    /// A host-side preparation/admission failure also leaves the current
    /// session unconfirmed. Only a verified session write clears this outcome.
    pub fn session_unconfirmed(&self, reason: impl Into<String>) {
        self.write_errors
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(SESSION_SLOT, reason.into());
    }

    fn writable(&self) -> Result<(), StoreError> {
        match self.write_refusal() {
            Some(reason) => Err(StoreError::Backend(format!(
                "Saved data is retained; writes are blocked until reopen: {reason}"
            ))),
            None => Ok(()),
        }
    }

    pub fn save(&self, contents: &str) {
        self.write(SESSION_SLOT, contents);
    }

    /// Commit a session reference only after its companion blobs have been
    /// accepted. Readback detects a backend that reports success but drops a
    /// changed write. The host still checks its custody seal before claiming
    /// durability; already-existing identical bytes cannot prove a new write.
    pub fn try_save(&self, contents: &str) -> Result<(), StoreError> {
        self.try_write_slot(SESSION_SLOT, contents)
    }

    pub fn try_read_practice_wav(&self, digest: &str, length: usize) -> Result<Vec<u8>, String> {
        let key = practice_wav_key(digest)?;
        let bytes = pollster::block_on(self.backend.get(&key))
            .map_err(|error| format!("Could not read practice recording: {error}"))?
            .ok_or("Saved practice recording is missing or could not be unsealed.")?;
        if bytes.is_empty()
            || bytes.len() > MAX_PRACTICE_WAV_BYTES
            || bytes.len() != length
            || blake3::hash(&bytes).to_hex().as_str() != digest
        {
            return Err("Saved practice recording failed its integrity check.".into());
        }
        Ok(bytes)
    }

    /// Publish the immutable recording before committing its Set reference.
    pub fn try_write_practice_wav(&self, bytes: &[u8]) -> Result<String, String> {
        self.writable().map_err(|error| error.to_string())?;
        if bytes.is_empty() || bytes.len() > MAX_PRACTICE_WAV_BYTES {
            return Err("Practice recording exceeds its bounded byte budget.".into());
        }
        let digest = blake3::hash(bytes).to_hex().to_string();
        let key = practice_wav_key(&digest)?;
        if pollster::block_on(self.backend.get(&key))
            .map_err(|error| format!("Could not inspect practice recording: {error}"))?
            .is_none()
        {
            pollster::block_on(self.backend.put(&key, bytes))
                .map_err(|error| format!("Could not write practice recording: {error}"))?;
        }
        if self.try_read_practice_wav(&digest, bytes.len())? != bytes {
            return Err("Practice recording failed its readback integrity check.".into());
        }
        Ok(digest)
    }

    /// Keep each sealed value within the store's normal blob budget. For longer
    /// selections the content address names an ordered list of verified chunk hashes.
    pub fn try_write_wav_region(&self, bytes: &[u8]) -> Result<String, String> {
        if bytes.len() <= MAX_PRACTICE_WAV_BYTES {
            return self.try_write_practice_wav(bytes);
        }
        let mut index = Vec::new();
        for chunk in bytes.chunks(MAX_PRACTICE_WAV_BYTES) {
            let hash = self.try_write_practice_wav(chunk)?;
            let hash = blake3::Hash::from_hex(hash).map_err(|e| e.to_string())?;
            index.extend_from_slice(hash.as_bytes());
        }
        let digest = blake3::hash(bytes).to_hex().to_string();
        let key = practice_wav_key(&digest)?;
        pollster::block_on(self.backend.put(&key, &index)).map_err(|e| e.to_string())?;
        if pollster::block_on(self.backend.get(&key)).map_err(|e| e.to_string())?.as_ref() != Some(&index) {
            return Err("Practice recording index did not persist.".into());
        }
        Ok(digest)
    }

    pub fn try_read_wav_region(&self, digest: &str, length: usize) -> Result<Vec<u8>, String> {
        if length <= MAX_PRACTICE_WAV_BYTES {
            return self.try_read_practice_wav(digest, length);
        }
        let key = practice_wav_key(digest)?;
        let index = pollster::block_on(self.backend.get(&key)).map_err(|e| e.to_string())?
            .ok_or("Practice recording index is missing.")?;
        if index.len() != length.div_ceil(MAX_PRACTICE_WAV_BYTES) * 32
            || length as u64 > u64::from(u32::MAX) + 8 {
            return Err("Invalid practice recording index.".into());
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|e| e.to_string())?;
        for hash in index.chunks_exact(32) {
            let hash = blake3::Hash::from_bytes(hash.try_into().expect("32-byte chunk digest"));
            let size = MAX_PRACTICE_WAV_BYTES.min(length - bytes.len());
            bytes.extend_from_slice(&self.try_read_practice_wav(hash.to_hex().as_str(), size)?);
        }
        if blake3::hash(&bytes).to_hex().as_str() != digest {
            return Err("Practice recording failed its integrity check.".into());
        }
        Ok(bytes)
    }

    pub fn try_read_capture(
        &self,
        namespace: &str,
        reference: &LoopCaptureRef,
    ) -> Result<Vec<f32>, String> {
        reference.validate()?;
        let key = pcm_key(namespace, &reference.digest)?;
        let bytes = pollster::block_on(self.backend.get(&key))
            .map_err(|error| format!("Could not read loop PCM: {error}"))?
            .ok_or("Loop PCM companion is missing or could not be unsealed.")?;
        decode_capture(&bytes, reference)
    }

    /// Write the blob before publishing its reference in the session. Sealing
    /// is composed at Backend, so both write and readback use plaintext here.
    /// A failed later session write may leave an unreferenced blob; this slice
    /// deliberately performs no collection or deletion.
    /// The host backend must publish companion keys without overwriting an
    /// existing file: sealing may hide another persona's bytes from `get`.
    pub fn try_write_capture(
        &self,
        namespace: &str,
        rate: u32,
        samples: &[f32],
    ) -> Result<(String, usize), String> {
        validate_namespace(namespace)?;
        let bytes = encode_pcm(rate, samples)?;
        let digest = self.try_write_capture_blob(namespace, &bytes)?;
        Ok((digest, samples.len()))
    }

    /// Store exact PCM using the current lossless compression policy. Legacy
    /// raw companions keep their existing addresses and remain readable.
    pub fn try_write_capture_compressed(
        &self,
        namespace: &str,
        rate: u32,
        samples: &[f32],
    ) -> Result<(LoopCaptureCodec, String, usize), String> {
        validate_namespace(namespace)?;
        let encoded = encode_capture(rate, samples)?;
        self.try_write_encoded_capture(namespace, rate, samples.len(), &encoded)
    }

    /// Publish a prepared result from `encode_capture`, allowing hosts to keep
    /// codec work on a worker without moving persona custody or storage there.
    /// Full validation of an untrusted stored blob remains `decode_capture`.
    pub fn try_write_encoded_capture(
        &self,
        namespace: &str,
        expected_rate: u32,
        expected_frames: usize,
        encoded: &EncodedCapture,
    ) -> Result<(LoopCaptureCodec, String, usize), String> {
        validate_namespace(namespace)?;
        let bytes = &encoded.bytes;
        if !(PCM_HEADER_BYTES..=MAX_PCM_BYTES).contains(&bytes.len()) {
            return Err("Prepared loop capture exceeds its storage bound.".into());
        }
        let magic = match encoded.codec {
            LoopCaptureCodec::RawF32V1 => b"WSHPCM\0\0",
            LoopCaptureCodec::WavpackF32V1 => b"WSHWVC\0\0",
            _ => return Err("Prepared loop capture has an unsupported codec.".into()),
        };
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let rate = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let frames = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into().unwrap()))
            .map_err(|_| "Prepared loop capture frame count is unsupported.")?;
        if &bytes[..8] != magic
            || version != 1
            || !(1_000..=384_000).contains(&rate)
            || frames == 0
            || frames > MAX_CAPTURE_FRAMES
            || rate != expected_rate
            || frames != expected_frames
        {
            return Err("Prepared loop capture has an invalid header.".into());
        }
        let raw_len = PCM_HEADER_BYTES + frames * 4;
        if match encoded.codec {
            LoopCaptureCodec::RawF32V1 => bytes.len() != raw_len,
            LoopCaptureCodec::WavpackF32V1 => {
                bytes.len() <= PCM_HEADER_BYTES || bytes.len() >= raw_len
            },
            _ => true,
        } {
            return Err("Prepared loop capture length differs from its codec policy.".into());
        }
        let digest = self.try_write_capture_blob(namespace, bytes)?;
        Ok((encoded.codec.clone(), digest, frames))
    }

    fn try_write_capture_blob(&self, namespace: &str, bytes: &[u8]) -> Result<String, String> {
        self.writable().map_err(|error| error.to_string())?;
        let digest = pcm_digest(bytes);
        let key = pcm_key(namespace, &digest)?;
        if let Some(stored) = pollster::block_on(self.backend.get(&key))
            .map_err(|error| format!("Could not inspect existing loop PCM: {error}"))?
        {
            if stored == bytes && pcm_digest(&stored) == digest {
                return Ok(digest);
            }
            return Err(
                "Existing loop PCM does not match its address; it remains unchanged.".into(),
            );
        }
        pollster::block_on(self.backend.put(&key, bytes))
            .map_err(|error| format!("Could not write loop PCM: {error}"))?;
        let stored = pollster::block_on(self.backend.get(&key))
            .map_err(|error| format!("Could not verify loop PCM: {error}"))?
            .ok_or("Loop PCM write was not retained by its backend.")?;
        if stored != bytes || pcm_digest(&stored) != digest {
            return Err("Loop PCM write failed its readback integrity check.".into());
        }
        Ok(digest)
    }

    pub fn load_settings(&self) -> Option<String> {
        self.read(SETTINGS_SLOT)
    }

    pub fn save_settings(&self, contents: &str) {
        self.write(SETTINGS_SLOT, contents);
    }

    /// A preference is acknowledged only after backend acceptance and exact
    /// readback, just like the session that may depend on it.
    pub fn try_save_settings(&self, contents: &str) -> Result<(), muniment::StoreError> {
        self.try_write_slot(SETTINGS_SLOT, contents)
    }

    fn try_write_slot(&self, slot: &'static str, contents: &str) -> Result<(), StoreError> {
        let result = (|| {
            self.writable()?;
            pollster::block_on(self.backend.put(slot, contents.as_bytes()))?;
            if pollster::block_on(self.backend.get(slot))?.as_deref() != Some(contents.as_bytes()) {
                return Err(StoreError::Backend(format!(
                    "{slot} write could not be verified by readback."
                )));
            }
            Ok(())
        })();
        let mut errors = self
            .write_errors
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match &result {
            Ok(()) => {
                errors.remove(slot);
            },
            Err(error) => {
                errors.insert(slot, format!("{slot}: {error}"));
            },
        }
        result
    }

    /// Compatibility optional read: errors are logged and quarantine writes.
    /// Hosts needing a visible admission result use `try_load`.
    fn read(&self, slot: &str) -> Option<String> {
        self.try_read(slot).unwrap_or_else(|error| {
            eprintln!("[woodshed] could not read {slot}: {error}");
            None
        })
    }

    fn try_read(&self, slot: &str) -> Result<Option<String>, StoreError> {
        let result = pollster::block_on(self.backend.get(slot)).and_then(|bytes| {
            bytes
                .map(|bytes| {
                    String::from_utf8(bytes).map_err(|error| {
                        StoreError::Backend(format!("{slot} is not UTF-8: {error}"))
                    })
                })
                .transpose()
        });
        if let Err(error) = &result {
            self.refuse_writes(format!("Could not read {slot}: {error}"));
        }
        result
    }

    /// Write one slot, logging rather than surfacing failure, because a broken
    /// disk must not strand practice.
    fn write(&self, slot: &str, contents: &str) {
        let result = if slot == SESSION_SLOT {
            self.try_save(contents)
        } else {
            self.try_save_settings(contents)
        };
        if let Err(error) = result {
            eprintln!("[woodshed] could not persist {slot}: {error}");
        }
    }
}

/// Associated-data + persona-derivation context binding woodshed's sealed session
/// to its purpose. Also the salt the sealing key derives under, so the key is
/// specific to this use.
pub(crate) const WOODSHED_SEAL_CONTEXT: &[u8] = b"woodshed.practice-session.seal.v1";

#[cfg(test)]
mod tests;
