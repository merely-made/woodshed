//! The persisted session document: what survives a restart, and its decoder.

use serde::{Deserialize, Serialize};

use crate::arpeggio::ArpeggioDirection;
use crate::loop_capture::LoopCaptureManifest;
use crate::settings::AppSettings;
use crate::{Lens, StageState};

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
    /// View-owned Overview camera, arrangement and presentation roles. This payload
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
