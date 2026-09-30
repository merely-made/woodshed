//! Named catalog configurations. Only inactive contexts live in the bank;
//! callers capture the current Stage and its scoped preferences when parking it.
use crate::{
    Lens, StageState,
    arpeggio::ArpeggioDirection,
    harmony::KeyedCatalogRef,
    settings::{AppSettings, FretboardSettings, StageSettings, TuningSettings},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CatalogExplorationId(pub u64);
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogExplorationState {
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
    pub tuning: TuningSettings,
    pub fretboard: FretboardSettings,
    pub stage_settings: StageSettings,
    pub search_query: String,
    pub context_focus: Option<KeyedCatalogRef>,
}
impl Default for CatalogExplorationState {
    fn default() -> Self {
        Self::capture(&StageState::new(), &AppSettings::default())
    }
}
impl CatalogExplorationState {
    pub fn capture(stage: &StageState, settings: &AppSettings) -> Self {
        Self {
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
            tuning: TuningSettings {
                tuning_idx: stage.tuning_idx,
            },
            fretboard: settings.fretboard.clone(),
            stage_settings: settings.stage.clone(),
            search_query: String::new(),
            context_focus: None,
        }
    }
    /// Apply configuration only. Catalog transports and drawn paths are reset;
    /// shared audio, appearance, accessibility and metronome remain untouched.
    pub fn apply(&self, stage: &mut StageState, settings: &mut AppSettings) {
        settings.tuning = self.tuning.clone();
        settings.fretboard = self.fretboard.clone();
        settings.stage = self.stage_settings.clone();
        *stage = StageState::new();
        stage.set_lens(self.lens);
        stage.set_tuning(self.tuning.tuning_idx);
        stage.set_root(self.root_idx);
        stage.select_scale(self.scale_idx);
        stage.select_chord(self.chord_idx);
        stage.select_arpeggio(self.arpeggio_idx);
        stage.arpeggio_position_idx = self.arpeggio_position_idx;
        stage.arpeggio_direction = self.arpeggio_direction;
        stage.arpeggio_inversion = self.arpeggio_inversion;
        if let Some(index) = self.progression_idx {
            stage.select_progression(index);
            stage.progression_expand(self.progression_expanded);
        }
        stage.select_exercise(self.exercise_idx);
        stage.exercise_starting_fret = self.exercise_starting_fret;
        stage.apply_neck(settings.fretboard.neck_start, settings.fretboard.neck_end);
        settings.tuning.tuning_idx = stage.tuning_idx;
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CatalogExploration {
    pub id: CatalogExplorationId,
    pub name: String,
    pub state: CatalogExplorationState,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogExplorations {
    pub active_id: CatalogExplorationId,
    pub active_name: String,
    pub inactive: Vec<CatalogExploration>,
    next_id: u64,
}
impl Default for CatalogExplorations {
    fn default() -> Self {
        Self {
            active_id: CatalogExplorationId(1),
            active_name: "Catalog exploration 1".into(),
            inactive: Vec::new(),
            next_id: 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogExplorationSummary {
    pub id: CatalogExplorationId,
    pub name: String,
    pub active: bool,
}
impl CatalogExplorations {
    pub fn create(
        &mut self,
        current: &mut CatalogExplorationState,
        name: impl Into<String>,
        state: CatalogExplorationState,
    ) -> CatalogExplorationId {
        self.next_id = self.next_id.max(self.active_id.0).max(
            self.inactive
                .iter()
                .map(|entry| entry.id.0)
                .max()
                .unwrap_or(0),
        );
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("catalog exploration identity exhausted");
        let id = CatalogExplorationId(self.next_id);
        let name = name.into();
        self.inactive.push(CatalogExploration {
            id,
            name: if name.trim().is_empty() {
                format!("Catalog exploration {}", id.0)
            } else {
                name
            },
            state,
        });
        assert!(self.activate(id, current));
        id
    }
    pub fn activate(
        &mut self,
        id: CatalogExplorationId,
        current: &mut CatalogExplorationState,
    ) -> bool {
        if id == self.active_id {
            return true;
        }
        let Some(index) = self.inactive.iter().position(|entry| entry.id == id) else {
            return false;
        };
        let target = self.inactive.remove(index);
        let previous = std::mem::replace(current, target.state);
        self.inactive.push(CatalogExploration {
            id: self.active_id,
            name: std::mem::replace(&mut self.active_name, target.name),
            state: previous,
        });
        self.active_id = id;
        true
    }
    pub fn get<'a>(
        &'a self,
        id: CatalogExplorationId,
        current: &'a CatalogExplorationState,
    ) -> Option<&'a CatalogExplorationState> {
        if id == self.active_id {
            Some(current)
        } else {
            self.inactive
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| &entry.state)
        }
    }
    pub fn get_mut<'a>(
        &'a mut self,
        id: CatalogExplorationId,
        current: &'a mut CatalogExplorationState,
    ) -> Option<&'a mut CatalogExplorationState> {
        if id == self.active_id {
            Some(current)
        } else {
            self.inactive
                .iter_mut()
                .find(|entry| entry.id == id)
                .map(|entry| &mut entry.state)
        }
    }
    pub fn summaries(&self) -> Vec<CatalogExplorationSummary> {
        let mut out = vec![CatalogExplorationSummary {
            id: self.active_id,
            name: self.active_name.clone(),
            active: true,
        }];
        out.extend(self.inactive.iter().map(|entry| CatalogExplorationSummary {
            id: entry.id,
            name: entry.name.clone(),
            active: false,
        }));
        out.sort_by_key(|entry| entry.id);
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configurations_park_independently_and_apply_does_not_start_transport_or_copy_shared_preferences()
     {
        let mut settings = AppSettings::default();
        settings.appearance.theme = "Shared theme".into();
        settings.metronome.bpm = 91.0;
        let mut stage = StageState::new();
        stage.set_root(3);
        stage.set_tuning(2);
        stage.scale_run_playing = true;
        settings.fretboard.neck_start = 4;
        settings.fretboard.neck_end = Some(12);
        let mut current = CatalogExplorationState::capture(&stage, &settings);
        current.search_query = "Dorian".into();
        let mut bank = CatalogExplorations::default();
        let first = bank.active_id;
        bank.create(&mut current, "Other", CatalogExplorationState::default());
        assert!(bank.activate(first, &mut current));
        assert_eq!(current.search_query, "Dorian");
        current.apply(&mut stage, &mut settings);
        assert_eq!(stage.root_idx, 3);
        assert_eq!(stage.tuning_idx, 2);
        assert_eq!(stage.fret_start, 4);
        assert!(!stage.scale_run_playing);
        assert!(!stage.arpeggio_playing);
        assert!(!stage.exercise_playing);
        assert_eq!(settings.appearance.theme, "Shared theme");
        assert_eq!(settings.metronome.bpm, 91.0);
        let before = (
            serde_json::to_value(&bank).unwrap(),
            serde_json::to_value(&current).unwrap(),
        );
        assert!(!bank.activate(CatalogExplorationId(999), &mut current));
        assert_eq!(
            before,
            (
                serde_json::to_value(&bank).unwrap(),
                serde_json::to_value(&current).unwrap()
            )
        );
    }
}
