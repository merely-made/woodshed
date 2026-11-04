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
    /// Exact kit assets and the editable sequencer pattern, not an installation path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_pack: Option<audio_packs::PackSelection>,
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
    /// Woodshed-owned contextual route history. The core preserves this opaque
    /// payload so the view layer can validate durable owner and Card ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_navigation_json: Option<String>,
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
            audio_pack: None,
            looper: crate::loop_plan::LooperConfig::default(),
            loop_captures: LoopCaptureManifest::default(),
            practice_history: practice_history.clone(),
            workspace_json: None,
            scope_navigation_json: None,
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
    let mut session: PersistedSession = serde_json::from_value(value.clone())?;
    if let Some(pack) = &session.audio_pack {
        pack.validate()
            .map_err(|e| <serde_json::Error as serde::de::Error>::custom(e.to_string()))?;
    }
    // Identity is local to each Set: retained snapshots may intentionally
    // share IDs with their source. Within one Set, ambiguity is a decode error
    // so the host retains/quarantines the original instead of adopting it.
    let admit = |set: &mut woodshedding::rehearsal::Set, owner: String| {
        set.admit_card_ids().map_err(|error| {
            <serde_json::Error as serde::de::Error>::custom(format!("{owner}: {error}"))
        })
    };
    admit(
        &mut session.set,
        format!("Active working Set {}", session.working_sets.active_id.0),
    )?;
    for entry in &mut session.working_sets.inactive {
        admit(
            &mut entry.set,
            format!("Inactive working Set {}", entry.id.0),
        )?;
    }
    for entry in &mut session.retained_sets.entries {
        admit(&mut entry.set, format!("Retained saved Set {}", entry.id.0))?;
    }
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
mod tests;
