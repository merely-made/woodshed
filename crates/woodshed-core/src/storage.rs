//! The storage seam (genet-host plan W0.2) and the persisted session.
//!
//! The core describes what survives a restart as one serde struct; a host
//! supplies a [`Storage`] that moves the serialized form — filesystem on
//! desktop, OPFS in the browser. Every field is `#[serde(default)]`-safe
//! so old files keep loading as the struct grows.

use muniment::{Backend, StoreError};
use serde::{Deserialize, Serialize};

use crate::arpeggio::ArpeggioDirection;
use crate::loop_capture::{
    EncodedCapture, LoopCaptureCodec, LoopCaptureManifest, LoopCaptureRef, MAX_CAPTURE_FRAMES,
    MAX_PCM_BYTES, PCM_HEADER_BYTES, decode_capture, encode_capture, encode_pcm, pcm_digest,
    pcm_key, validate_namespace,
};
use crate::settings::AppSettings;
pub use crate::settings::RelatedSettings;
use crate::{Lens, StageState};

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
            }
            Err(error) => {
                errors.insert(slot, format!("{slot}: {error}"));
            }
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

/// The top-level product section. Legacy Practice and Song values migrate at
/// deserialization into Stage and Looper respectively.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppSection {
    #[default]
    #[serde(alias = "Practice")]
    Stage,
    Rehearsal,
    #[serde(alias = "Song")]
    Looper,
    Tools,
    Settings,
}

impl AppSection {
    pub const ALL: [AppSection; 5] = [
        AppSection::Stage,
        AppSection::Rehearsal,
        AppSection::Looper,
        AppSection::Tools,
        AppSection::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AppSection::Stage => "Stage",
            AppSection::Rehearsal => "Rehearsal",
            AppSection::Looper => "Looper",
            AppSection::Tools => "Tools",
            AppSection::Settings => "Settings",
        }
    }
}

/// The artifact/session state that survives a restart. Application preferences
/// are stored through [`SettingsStorage`] instead of being flattened here.
/// `decode_session` still exposes legacy flattened settings for migration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedSession {
    #[serde(alias = "tab")]
    pub section: AppSection,
    pub lens: Lens,
    pub root_idx: usize,
    pub scale_idx: usize,
    pub chord_idx: usize,
    pub arpeggio_idx: usize,
    pub arpeggio_position_idx: usize,
    pub arpeggio_direction: ArpeggioDirection,
    pub arpeggio_inversion: u8,
    pub progression_idx: Option<usize>,
    pub progression_expanded: usize,
    pub exercise_idx: usize,
    pub exercise_starting_fret: u8,
    /// The rehearsal set (cards + cursor + loop mode).
    pub set: woodshedding::rehearsal::Set,
    /// Explicit retained instruction snapshots, distinct from the working Set.
    pub retained_sets: crate::retained_sets::RetainedSets,
    pub working_sets: crate::working_sets::WorkingSets,
    pub catalog_explorations: crate::catalog_explorations::CatalogExplorations,
    /// Authored nearby catalog material, separate from view presentation.
    pub musical_context: crate::musical_context::MusicalContext,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_exploration: Option<crate::catalog_explorations::CatalogExplorationState>,
    /// The song lane: bars + song-level flags.
    pub song: crate::song::SongDoc,
    /// Source selection and explicit clock; derived plans are transient.
    /// Absent on legacy saves, which retain their backing song.
    #[serde(
        default,
        skip_serializing_if = "crate::loop_plan::LooperConfig::is_default"
    )]
    pub looper: crate::loop_plan::LooperConfig,
    /// Owner-qualified Set takes whose exact PCM lives in companion blobs.
    /// Missing/corrupt references remain authored data until explicitly cleared.
    #[serde(default, skip_serializing_if = "LoopCaptureManifest::is_empty")]
    pub loop_captures: LoopCaptureManifest,
    /// Typed catalog engagement used by Related ranking and future history
    /// views. Defaults empty for sessions written before the field existed.
    pub practice_history: crate::history::PracticeHistory,
    /// Desktop-host workspace presentation policy, encoded by Woodshed's view
    /// layer. The portable core retains these opaque bytes in the existing
    /// session slot without learning the shared Workbench schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_json: Option<String>,
    /// Shared relational recipe snapshot plus its bounded disclosed evidence.
    /// An embedding validates this payload through the shared compiler when
    /// mounting it. Persistence retains the opaque bytes beside the owner's
    /// Sets and does not acquire any foreign source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship_reading_json: Option<String>,
    /// View-owned Mere camera, arrangement and presentation roles. This payload
    /// cannot mutate the retained musical material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview_presentation_json: Option<String>,
    /// Session-owned personal relation assertions. The journal, not a cached
    /// graph projection, is the authority for retained assertion lifecycle.
    /// Missing in legacy sessions; the trusted host assigns and persists a
    /// source UUID before enabling personal relation writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal_relations: Option<crate::personal_relations::PersistedPersonalRelationJournal>,
}

impl Default for PersistedSession {
    fn default() -> Self {
        Self::capture(
            &StageState::new(),
            AppSection::Stage,
            &woodshedding::rehearsal::Set::default(),
            &crate::song::SongDoc::default(),
            &crate::history::PracticeHistory::default(),
        )
    }
}

impl PersistedSession {
    /// Snapshot the persistable subset of the app state.
    pub fn capture(
        stage: &StageState,
        section: AppSection,
        set: &woodshedding::rehearsal::Set,
        song: &crate::song::SongDoc,
        practice_history: &crate::history::PracticeHistory,
    ) -> Self {
        Self {
            set: set.clone(),
            retained_sets: crate::retained_sets::RetainedSets::default(),
            working_sets: crate::working_sets::WorkingSets::default(),
            catalog_explorations: crate::catalog_explorations::CatalogExplorations::default(),
            musical_context: crate::musical_context::MusicalContext::default(),
            active_exploration: None,
            song: song.clone(),
            looper: crate::loop_plan::LooperConfig::default(),
            loop_captures: LoopCaptureManifest::default(),
            practice_history: practice_history.clone(),
            workspace_json: None,
            relationship_reading_json: None,
            overview_presentation_json: None,
            personal_relations: None,
            section,
            lens: stage.lens,
            root_idx: stage.root_idx,
            scale_idx: stage.scale_idx,
            chord_idx: stage.chord_idx,
            arpeggio_idx: stage.arpeggio_idx,
            arpeggio_position_idx: stage.arpeggio_position_idx,
            arpeggio_direction: stage.arpeggio_direction,
            arpeggio_inversion: stage.arpeggio_inversion,
            progression_idx: stage.progression_idx,
            progression_expanded: stage.progression_expanded,
            exercise_idx: stage.exercise_idx,
            exercise_starting_fret: stage.exercise_starting_fret,
        }
    }

    /// Restore the persisted subset onto a fresh state. Indices route
    /// through the clamping setters so a session written against a larger
    /// future catalog degrades instead of panicking.
    pub fn restore(&self, stage: &mut StageState, settings: &AppSettings) {
        stage.set_lens(self.lens);
        stage.set_tuning(settings.tuning.tuning_idx);
        stage.set_root(self.root_idx);
        stage.select_scale(self.scale_idx);
        stage.select_chord(self.chord_idx);
        stage.select_arpeggio(self.arpeggio_idx);
        stage.arpeggio_position_idx = self.arpeggio_position_idx;
        stage.arpeggio_direction = self.arpeggio_direction;
        stage.arpeggio_inversion = self.arpeggio_inversion;
        if let Some(i) = self.progression_idx {
            stage.select_progression(i);
            stage.progression_expand(self.progression_expanded);
        }
        stage.select_exercise(self.exercise_idx);
        stage.exercise_starting_fret = self.exercise_starting_fret;
    }
}

/// The result of reading a session file. New files contain only
/// [`PersistedSession`]; old flat files may also return application settings so
/// a host can migrate them to its separate settings file on the next save.
#[derive(Debug)]
pub struct SessionLoad {
    pub session: PersistedSession,
    pub legacy_settings: Option<AppSettings>,
}

/// Decode the current session format and detect the pre-C5 flattened settings
/// payload without making those fields part of the new session type.
pub fn decode_session(contents: &str) -> Result<SessionLoad, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(contents)?;
    let session = serde_json::from_value(value.clone())?;
    let has_legacy_settings = [
        "theme",
        "tuning_idx",
        "bpm",
        "settings_page",
        "reduce_motion",
        "board_layout",
        "show_set_sequence_edges",
    ]
    .iter()
    .any(|key| value.get(key).is_some());
    let legacy_settings = has_legacy_settings
        .then(|| serde_json::from_value(value))
        .transpose()?;
    Ok(SessionLoad {
        session,
        legacy_settings,
    })
}

/// Associated-data + persona-derivation context binding woodshed's sealed session
/// to its purpose. Also the salt the sealing key derives under, so the key is
/// specific to this use.
pub(crate) const WOODSHED_SEAL_CONTEXT: &[u8] = b"woodshed.practice-session.seal.v1";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_capture::decode_pcm;
    const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn missing_session_initializes_but_unreadable_slot_quarantines_every_clone() {
        let inner = muniment::MemoryBackend::default();
        let store = SessionStore::new(inner.clone());
        let sibling = store.clone();
        assert!(store.try_load().unwrap().is_none());
        store.try_save("original session").unwrap();
        pollster::block_on(inner.put(SETTINGS_SLOT, &[0xff])).unwrap();
        assert!(store.load_settings().is_none());
        assert!(sibling.write_refusal().is_some());
        assert!(sibling.try_save("default replacement").is_err());
        assert!(sibling.try_save_settings("{}").is_err());
        assert!(
            sibling
                .try_write_capture(NAMESPACE, 48_000, &[0.25])
                .is_err()
        );
        store.save("another default");
        assert_eq!(
            pollster::block_on(inner.get(SESSION_SLOT))
                .unwrap()
                .unwrap(),
            b"original session"
        );
        assert_eq!(
            pollster::block_on(inner.get(SETTINGS_SLOT))
                .unwrap()
                .unwrap(),
            [0xff]
        );
    }

    #[test]
    fn wrong_persona_quarantines_without_clobbering_original_session() {
        let inner = muniment::MemoryBackend::default();
        let first = SessionStore::new(crate::sealed_backend::SealedBackend::new(
            inner.clone(),
            [1; 32],
        ));
        first.try_save("original session").unwrap();
        let ciphertext = pollster::block_on(inner.get(SESSION_SLOT)).unwrap();
        let second = SessionStore::new(crate::sealed_backend::SealedBackend::new(
            inner.clone(),
            [2; 32],
        ));
        assert!(second.try_load().is_err());
        second.save("default session");
        assert!(second.try_save_settings("{}").is_err());
        assert_eq!(
            pollster::block_on(inner.get(SESSION_SLOT)).unwrap(),
            ciphertext
        );
        assert_eq!(
            first.try_load().unwrap().as_deref(),
            Some("original session")
        );
    }

    #[test]
    fn dropped_settings_write_is_not_acknowledged() {
        let store = SessionStore::new(FaultBackend {
            inner: muniment::MemoryBackend::default(),
            fault: Fault::Drop,
        });
        assert!(store.try_save_settings("{}").is_err());
        assert!(store.save_problem().unwrap().contains("settings"));
    }

    fn capture_ref(digest: String, frames: usize) -> LoopCaptureRef {
        LoopCaptureRef {
            clock: None,
            id: crate::loop_plan::LoopSegmentId {
                set: crate::working_sets::WorkingSetId(3),
                card: woodshedding::rehearsal::CardId(7),
            },
            bpm: 83.75,
            beats: 7,
            bars: 3,
            sample_rate: 48_000,
            frames,
            digest,
            codec: LoopCaptureCodec::LegacyRawF32V1,
        }
    }

    #[test]
    fn legacy_capture_defaults_and_unavailable_references_survive_session_roundtrip() {
        let mut session = decode_session("{}").unwrap().session;
        assert!(session.loop_captures.is_empty());
        assert!(
            serde_json::to_value(&session)
                .unwrap()
                .get("loop_captures")
                .is_none()
        );
        session
            .loop_captures
            .takes
            .push(capture_ref("a".repeat(64), 3));
        session.loop_captures.namespace = Some(NAMESPACE.into());
        let loaded = decode_session(&serde_json::to_string(&session).unwrap())
            .unwrap()
            .session;
        assert_eq!(loaded.loop_captures, session.loop_captures);
        let store = SessionStore::new(muniment::MemoryBackend::default());
        assert!(
            store
                .try_read_capture(NAMESPACE, &loaded.loop_captures.takes[0])
                .is_err()
        );
        assert_eq!(loaded.loop_captures, session.loop_captures);
        session.loop_captures.version = 99;
        session.loop_captures.takes.clear();
        assert!(
            serde_json::to_value(&session)
                .unwrap()
                .get("loop_captures")
                .is_some()
        );
    }

    #[test]
    fn sealed_companion_roundtrips_exact_pcm_and_wrong_persona_refuses_it() {
        let inner = muniment::MemoryBackend::default();
        let store = SessionStore::new(crate::sealed_backend::SealedBackend::new(
            inner.clone(),
            [1; 32],
        ));
        let samples = [0.0, -0.0, 0.25, -1.5];
        let (digest, frames) = store
            .try_write_capture(NAMESPACE, 48_000, &samples)
            .unwrap();
        let reference = capture_ref(digest.clone(), frames);
        let decoded = store.try_read_capture(NAMESPACE, &reference).unwrap();
        assert_eq!(
            decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            samples.iter().map(|x| x.to_bits()).collect::<Vec<_>>()
        );
        let raw = pollster::block_on(inner.get(&pcm_key(NAMESPACE, &digest).unwrap()))
            .unwrap()
            .unwrap();
        assert_ne!(raw, encode_pcm(48_000, &samples).unwrap());
        assert!(decode_pcm(&raw, &reference).is_err());
        store.try_save("{\"take\":\"sealed\"}").unwrap();
        let other = SessionStore::new(crate::sealed_backend::SealedBackend::new(inner, [2; 32]));
        assert!(
            other
                .try_read_capture(NAMESPACE, &reference)
                .unwrap_err()
                .contains("unsealed")
        );
    }

    #[test]
    fn compressed_companion_is_sealed_and_reopens_with_exact_bits() {
        let inner = muniment::MemoryBackend::default();
        let store = SessionStore::new(crate::sealed_backend::SealedBackend::new(
            inner.clone(),
            [19; 32],
        ));
        let samples: Vec<f32> = (0..96_000)
            .map(|index| [0.125, -0.25, -0.0, f32::from_bits(1)][index % 4])
            .collect();
        let (codec, digest, frames) = store
            .try_write_capture_compressed(NAMESPACE, 48_000, &samples)
            .unwrap();
        assert_eq!(codec, LoopCaptureCodec::WavpackF32V1);
        let mut reference = capture_ref(digest.clone(), frames);
        reference.codec = codec;
        let key = pcm_key(NAMESPACE, &digest).unwrap();
        let sealed = pollster::block_on(inner.get(&key)).unwrap().unwrap();
        assert!(sealed.len() < encode_pcm(48_000, &samples).unwrap().len());
        let reopened = SessionStore::new(crate::sealed_backend::SealedBackend::new(
            inner.clone(),
            [19; 32],
        ));
        let decoded = reopened.try_read_capture(NAMESPACE, &reference).unwrap();
        assert!(
            decoded
                .iter()
                .zip(&samples)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert!(decode_capture(&sealed, &reference).is_err());
        let other = SessionStore::new(crate::sealed_backend::SealedBackend::new(inner, [20; 32]));
        assert!(other.try_read_capture(NAMESPACE, &reference).is_err());
        assert!(
            reopened
                .try_read_capture("fedcba9876543210fedcba9876543210", &reference)
                .is_err()
        );
        let mut relabelled = reference;
        relabelled.codec = LoopCaptureCodec::RawF32V1;
        assert!(reopened.try_read_capture(NAMESPACE, &relabelled).is_err());
    }

    #[test]
    fn prepared_capture_must_match_its_source_before_writing() {
        let inner = muniment::MemoryBackend::default();
        let store = SessionStore::new(inner.clone());
        let encoded = encode_capture(48_000, &vec![0.25; 96_000]).unwrap();
        assert!(
            store
                .try_write_encoded_capture(NAMESPACE, 44_100, 96_000, &encoded)
                .is_err()
        );
        assert!(
            store
                .try_write_encoded_capture(NAMESPACE, 48_000, 48_000, &encoded)
                .is_err()
        );
        assert!(
            pollster::block_on(inner.list("loop-pcm/"))
                .unwrap()
                .is_empty()
        );
    }

    #[derive(Clone)]
    enum Fault {
        Drop,
        DropSettings(std::sync::Arc<std::sync::atomic::AtomicBool>),
        CorruptCapture,
        RefuseSession,
    }

    #[derive(Clone)]
    struct FaultBackend {
        inner: muniment::MemoryBackend,
        fault: Fault,
    }

    #[async_trait::async_trait]
    impl Backend for FaultBackend {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
            self.inner.get(key).await
        }
        async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
            match &self.fault {
                Fault::Drop => Ok(()),
                Fault::DropSettings(drop)
                    if key == SETTINGS_SLOT && drop.load(std::sync::atomic::Ordering::Relaxed) =>
                {
                    Ok(())
                }
                Fault::RefuseSession if key == SESSION_SLOT => {
                    Err(StoreError::Backend("refused session replacement".into()))
                },
                Fault::CorruptCapture if key.starts_with("loop-pcm/") => {
                    let mut bad = bytes.to_vec();
                    *bad.last_mut().unwrap() ^= 1;
                    self.inner.put(key, &bad).await
                },
                _ => self.inner.put(key, bytes).await,
            }
        }
        async fn delete(&self, key: &str) -> Result<(), StoreError> {
            self.inner.delete(key).await
        }
        async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
            self.inner.list(prefix).await
        }
        async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
            self.inner.scan(start, end).await
        }
        async fn apply(&self, _ops: &[muniment::backend::WriteOp]) -> Result<(), StoreError> {
            Err(StoreError::Backend("no multi-file transaction".into()))
        }
    }

    #[test]
    fn settings_failure_survives_successful_session_until_same_slot_verifies_on_a_clone() {
        let dropping = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let store = SessionStore::new(FaultBackend {
            inner: muniment::MemoryBackend::default(),
            fault: Fault::DropSettings(dropping.clone()),
        });
        let sibling = store.clone();
        assert!(store.try_save_settings("authored settings").is_err());
        sibling.try_save("authored session").unwrap();
        assert!(store.save_problem().unwrap().contains("settings"));
        assert!(sibling.save_problem().is_some());
        dropping.store(false, std::sync::atomic::Ordering::Relaxed);
        sibling.try_save_settings("authored settings").unwrap();
        assert!(store.save_problem().is_none());
        assert!(sibling.save_problem().is_none());
        assert_eq!(store.load().as_deref(), Some("authored session"));
        assert_eq!(store.load_settings().as_deref(), Some("authored settings"));
    }

    #[test]
    fn dropped_and_corrupt_writes_cannot_claim_durability() {
        for fault in [Fault::Drop, Fault::CorruptCapture] {
            let store = SessionStore::new(FaultBackend {
                inner: muniment::MemoryBackend::default(),
                fault,
            });
            assert!(store.try_write_capture(NAMESPACE, 48_000, &[0.25]).is_err());
            assert!(
                store
                    .try_write_capture_compressed(NAMESPACE, 48_000, &vec![0.25; 48_000])
                    .is_err()
            );
        }
        let store = SessionStore::new(FaultBackend {
            inner: muniment::MemoryBackend::default(),
            fault: Fault::Drop,
        });
        assert!(store.try_save("changed session").is_err());
    }

    #[test]
    fn refused_session_replacement_retains_previous_reference_and_orphan_blob() {
        let inner = muniment::MemoryBackend::default();
        pollster::block_on(inner.put(SESSION_SLOT, b"previous authored session")).unwrap();
        let store = SessionStore::new(FaultBackend {
            inner: inner.clone(),
            fault: Fault::RefuseSession,
        });
        let (digest, frames) = store.try_write_capture(NAMESPACE, 48_000, &[0.25]).unwrap();
        assert!(store.try_save("replacement manifest").is_err());
        assert_eq!(store.load().as_deref(), Some("previous authored session"));
        assert_eq!(
            store
                .try_read_capture(NAMESPACE, &capture_ref(digest.clone(), frames))
                .unwrap(),
            vec![0.25]
        );
        assert_eq!(
            pollster::block_on(inner.list("loop-pcm/")).unwrap(),
            vec![pcm_key(NAMESPACE, &digest).unwrap()]
        );
    }

    #[test]
    fn legacy_session_defaults_authored_musical_context_and_roundtrips_it() {
        let mut value = serde_json::to_value(PersistedSession::default()).unwrap();
        value.as_object_mut().unwrap().remove("musical_context");
        let mut session: PersistedSession = serde_json::from_value(value).unwrap();
        assert!(session.musical_context.items().is_empty());
        let subject = crate::harmony::KeyedCatalogRef {
            formula_id: "scale:Major".into(),
            root: woodshedding::pitch::PitchClass::new(2),
        };
        let id = session
            .musical_context
            .keep(crate::working_sets::WorkingSetId(1), subject.clone())
            .unwrap();
        let reopened: PersistedSession =
            serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(reopened.musical_context.get(id).unwrap().subject, subject);
    }

    #[test]
    fn session_round_trips_through_json() {
        let mut stage = StageState::new();
        stage.set_lens(Lens::Arpeggios);
        stage.set_tuning(3);
        stage.set_root(3);
        stage.select_arpeggio(5);
        stage.arpeggio_inversion = 2;
        stage.select_progression(1);
        let mut set = woodshedding::rehearsal::Set::default();
        set.push(stage.card_from_lens().expect("arpeggio card"));
        let mut song = crate::song::SongDoc::default();
        song.name = "My Song".into();
        song.bars.push(crate::song::SongBar::default());
        song.one_shot = true;
        let mut history = crate::history::PracticeHistory::default();
        history.record(
            Some(1_000),
            woodshed_graph::chord_id("Minor 7"),
            crate::history::EngagementKind::Staged,
            Some(woodshed_graph::scale_id("Dorian")),
            None,
        );
        let related = RelatedSettings {
            use_history: false,
            show_neighborhood: false,
            dismissed_ids: vec![woodshed_graph::chord_id("Minor 7")],
            ..Default::default()
        };
        let settings = AppSettings {
            page: crate::settings::SettingsPage::Tuning,
            appearance: crate::settings::AppearanceSettings {
                theme: "Ember".into(),
                theme_choice: None,
            },
            tuning: crate::settings::TuningSettings { tuning_idx: 3 },
            stage: crate::settings::StageSettings {
                related: related.clone(),
                ..Default::default()
            },
            fretboard: crate::settings::FretboardSettings {
                board_layout: "Hero".into(),
                marker_style: "Sharp".into(),
                ..Default::default()
            },
            metronome: crate::settings::MetronomeSettings { bpm: 96.0 },
            ..AppSettings::default()
        };
        let snap = PersistedSession::capture(&stage, AppSection::Settings, &set, &song, &history);
        let json = serde_json::to_string(&snap).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            wire.get("theme").is_none(),
            "application settings leave the session wire"
        );
        assert!(wire.get("tuning_idx").is_none());
        let settings_json = serde_json::to_string(&settings).unwrap();
        let settings_wire: serde_json::Value = serde_json::from_str(&settings_json).unwrap();
        assert_eq!(settings_wire["theme"], "Ember");
        assert_eq!(settings_wire["tuning_idx"], 3);
        assert_eq!(settings_wire["bpm"], 96.0);
        let back: PersistedSession = serde_json::from_str(&json).unwrap();
        let mut restored = StageState::new();
        back.restore(&mut restored, &settings);
        assert_eq!(restored.lens, Lens::Arpeggios);
        assert_eq!(restored.tuning_idx, 3);
        assert_eq!(restored.root_idx, 3);
        assert_eq!(restored.arpeggio_idx, 5);
        assert_eq!(restored.arpeggio_inversion, 2);
        assert_eq!(restored.progression_idx, Some(1));
        assert_eq!(back.section, AppSection::Settings);
        assert_eq!(back.set.cards.len(), 1, "the rehearsal set round-trips");
        assert_eq!(back.set.cards[0].label, "Csus4 arpeggio");
        assert_eq!(back.song.name, "My Song", "the song doc round-trips");
        assert_eq!(back.song.bars.len(), 1);
        assert!(back.song.one_shot);
        // The practice lineage round-trips through the session wire: the
        // engagement, its subject, and the traversal that produced it.
        assert_eq!(back.practice_history.len(), history.len());
        assert_eq!(
            back.practice_history.recent(1)[0].subject_id,
            history.recent(1)[0].subject_id
        );
        assert_eq!(settings.stage.related, related);
    }

    #[test]
    fn unknown_fields_and_missing_fields_tolerated() {
        let loaded = decode_session(r#"{"lens":"Chords","bpm":88.0}"#).unwrap();
        assert_eq!(loaded.session.lens, Lens::Chords);
        assert_eq!(loaded.legacy_settings.as_ref().unwrap().metronome.bpm, 88.0);
        assert_eq!(
            loaded.session.section,
            AppSection::Stage,
            "missing fields default"
        );
        // Out-of-range indices clamp through the setters.
        let huge: PersistedSession = serde_json::from_str(r#"{"scale_idx":99999}"#).unwrap();
        let mut s = StageState::new();
        huge.restore(&mut s, &AppSettings::default());
        assert!(s.scale_idx < s.scales().len());
    }

    #[test]
    fn the_legacy_sequence_edge_toggle_becomes_a_relation_set() {
        use woodshedding::rehearsal::SetGraphEdgeKind;

        // A session written while Set-graph edge visibility was one boolean.
        let mut off: AppSettings =
            serde_json::from_str(r#"{"show_set_sequence_edges":false}"#).unwrap();
        assert!(
            off.stage.shows_relation(SetGraphEdgeKind::Next),
            "the default set is unchanged until the legacy value is adopted"
        );

        off.stage.adopt_legacy_relation_visibility();
        assert!(!off.stage.shows_relation(SetGraphEdgeKind::Next));

        // The legacy key is consumed, so the next save writes only the set.
        let json = serde_json::to_string(&off).unwrap();
        assert!(!json.contains("show_set_sequence_edges"));
        assert!(json.contains("visible_set_relations"));

        // A session written after the migration is untouched by it.
        let mut on: AppSettings = serde_json::from_str(&json).unwrap();
        on.stage.adopt_legacy_relation_visibility();
        assert!(!on.stage.shows_relation(SetGraphEdgeKind::Next));
    }

    #[test]
    fn multiple_owner_banks_preserve_legacy_active_content_and_round_trip() {
        let mut session = decode_session("{}").unwrap().session;
        assert_eq!(
            session.working_sets.active_id,
            crate::working_sets::WorkingSetId(1)
        );
        assert!(session.working_sets.inactive.is_empty());
        assert!(session.active_exploration.is_none());
        session
            .set
            .push(StageState::new().card_from_lens().unwrap());
        let original_id = session.set.cards[0].id;
        let original_owner = session.working_sets.active_id;
        let second = session.working_sets.create(
            &mut session.set,
            "Other",
            woodshedding::rehearsal::Set::default(),
        );
        let mut context = crate::catalog_explorations::CatalogExplorationState::default();
        context.search_query = "minor".into();
        session.catalog_explorations.create(
            &mut context,
            "Other exploration",
            crate::catalog_explorations::CatalogExplorationState::default(),
        );
        session.active_exploration = Some(context);
        let mut reopened = decode_session(&serde_json::to_string(&session).unwrap())
            .unwrap()
            .session;
        assert_eq!(reopened.working_sets.active_id, second);
        assert!(
            reopened
                .working_sets
                .activate(original_owner, &mut reopened.set)
        );
        assert_eq!(reopened.set.cards[0].id, original_id);
        assert_eq!(
            reopened.catalog_explorations.inactive[0].state.search_query,
            "minor"
        );
        let wire = serde_json::to_value(&reopened.working_sets).unwrap();
        assert!(wire.get("set").is_none());
    }

    #[test]
    fn retained_sets_default_for_legacy_and_round_trip_separately_from_working_set() {
        let legacy = decode_session("{}").unwrap();
        assert!(legacy.session.retained_sets.entries.is_empty());
        let mut session = PersistedSession::default();
        session
            .set
            .push(StageState::new().card_from_lens().unwrap());
        let saved_id = session
            .retained_sets
            .save_snapshot(&session.set, "My study");
        session.set.cards[0].label = "Current edited instruction".into();
        let saved_before =
            serde_json::to_value(session.retained_sets.get(saved_id).unwrap()).unwrap();
        let mut loaded = decode_session(&serde_json::to_string(&session).unwrap())
            .unwrap()
            .session;
        assert_eq!(loaded.set.cards[0].label, "Current edited instruction");
        assert_eq!(
            serde_json::to_value(loaded.retained_sets.get(saved_id).unwrap()).unwrap(),
            saved_before
        );
        let old_id = loaded.set.cards[0].id;
        assert!(loaded.retained_sets.restore_into(saved_id, &mut loaded.set));
        assert_ne!(loaded.set.cards[0].id, old_id);
        assert_eq!(
            serde_json::to_value(loaded.retained_sets.get(saved_id).unwrap()).unwrap(),
            saved_before
        );
    }

    #[test]
    fn legacy_tabs_migrate_to_current_sections() {
        let practice: PersistedSession = serde_json::from_str(r#"{"tab":"Practice"}"#).unwrap();
        assert_eq!(practice.section, AppSection::Stage);
        let song: PersistedSession = serde_json::from_str(r#"{"tab":"Song"}"#).unwrap();
        assert_eq!(song.section, AppSection::Looper);
    }
}
