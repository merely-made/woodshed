//! Disposable, score-derived practice transport state.
//!
//! The saved MusicXML and its editor draft remain the musical source. This
//! module captures a score at Run, lowers that immutable copy to the shared
//! integer-tick schedule, and accepts only observations from the bound Set,
//! audio session, and run epoch.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use woodshed_core::{
    music_playback::{ArrangementSchedule, PlaybackEvent},
    working_sets::WorkingSetId,
};
use woodshedding::notation::Score;

static NEXT_RUN_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeStatus {
    Stopped,
    Playing,
    Paused,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PracticePlayhead {
    pub bar_index: usize,
    pub event_index: usize,
    pub is_rest: bool,
    /// Tick within the selected bar, in the shared quarter-note tick scale.
    pub bar_beat_ticks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PracticeObservation {
    pub owner: WorkingSetId,
    pub session: Arc<()>,
    pub run_epoch: u64,
    pub applied_revision: u64,
    /// Device-reported position local to the active bar, so Set loops and
    /// mixed meters do not require reconstructing a global tick position.
    pub bar_index: usize,
    pub bar_beat_ticks: u64,
    pub status: PracticeStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MusicPractice {
    owner: Option<WorkingSetId>,
    session: Option<Arc<()>>,
    run_epoch: u64,
    applied_score: Option<Score>,
    applied_schedule: Option<ArrangementSchedule>,
    applied_revision: u64,
    desired_score: Option<Score>,
    desired_schedule: Option<ArrangementSchedule>,
    desired_revision: u64,
    tempo_bpm: Option<f64>,
    status: PracticeStatus,
    playhead: Option<PracticePlayhead>,
}

impl Default for MusicPractice {
    fn default() -> Self {
        Self {
            owner: None,
            session: None,
            run_epoch: 0,
            applied_score: None,
            applied_schedule: None,
            applied_revision: 0,
            desired_score: None,
            desired_schedule: None,
            desired_revision: 0,
            tempo_bpm: None,
            status: PracticeStatus::Stopped,
            playhead: None,
        }
    }
}

impl MusicPractice {
    /// Capture one immutable score reading and prepare its bounded schedule.
    /// This does not write the draft, MusicXML, Set, or editor history.
    pub fn start(
        &mut self,
        owner: WorkingSetId,
        score: &Score,
        tempo_bpm: f64,
        session: Arc<()>,
    ) -> Result<u64, String> {
        let captured_score = score.clone();
        let schedule = ArrangementSchedule::from_score(&captured_score, tempo_bpm)?;
        let epoch = NEXT_RUN_EPOCH
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| "Practice run epoch exhausted.".to_owned())?;
        self.owner = Some(owner);
        self.session = Some(session);
        self.run_epoch = epoch;
        self.applied_score = Some(captured_score.clone());
        self.applied_schedule = Some(schedule.clone());
        self.applied_revision = 0;
        self.desired_score = Some(captured_score);
        self.desired_schedule = Some(schedule);
        self.desired_revision = 0;
        self.tempo_bpm = Some(tempo_bpm);
        self.status = PracticeStatus::Playing;
        self.playhead = None;
        Ok(epoch)
    }

    pub fn stop(&mut self) {
        self.status = PracticeStatus::Stopped;
        self.owner = None;
        self.session = None;
        self.applied_score = None;
        self.applied_schedule = None;
        self.applied_revision = 0;
        self.desired_score = None;
        self.desired_schedule = None;
        self.desired_revision = 0;
        self.tempo_bpm = None;
        self.playhead = None;
    }

    pub fn pause(&mut self) {
        if self.status == PracticeStatus::Playing {
            self.status = PracticeStatus::Paused;
        }
    }

    pub fn resume(&mut self) -> Result<(), String> {
        if self.status != PracticeStatus::Paused
            || self.owner.is_none()
            || self.session.is_none()
            || self.applied_schedule.is_none()
        {
            return Err("No paused music practice run is available to resume.".into());
        }
        self.status = PracticeStatus::Playing;
        Ok(())
    }

    /// Apply a temporary tempo to both practice views. The host updates its
    /// tempo command immediately; score-content revisions remain unchanged.
    pub fn set_tempo(&mut self, tempo_bpm: f64) -> Result<(), String> {
        if !tempo_bpm.is_finite() || !(1.0..=400.0).contains(&tempo_bpm) {
            return Err("Practice tempo must be finite and within 1..=400 BPM.".into());
        }
        let applied_score = self
            .applied_score
            .as_ref()
            .ok_or_else(|| "Start music practice before changing its tempo.".to_owned())?;
        let desired_score = self
            .desired_score
            .as_ref()
            .ok_or_else(|| "Start music practice before changing its tempo.".to_owned())?;
        let applied_schedule = ArrangementSchedule::from_score(applied_score, tempo_bpm)?;
        let desired_schedule = ArrangementSchedule::from_score(desired_score, tempo_bpm)?;
        self.applied_schedule = Some(applied_schedule);
        self.desired_schedule = Some(desired_schedule);
        self.tempo_bpm = Some(tempo_bpm);
        Ok(())
    }

    /// Capture the latest edited score for application at the next bar
    /// boundary. Until the processor acknowledges this revision, playhead
    /// projection continues to use the applied score and schedule.
    pub fn request_update(&mut self, score: &Score) -> Result<u64, String> {
        let tempo = self
            .tempo_bpm
            .ok_or_else(|| "Start music practice before updating its score.".to_owned())?;
        let next_revision = self
            .desired_revision
            .checked_add(1)
            .ok_or_else(|| "Practice score revision exhausted.".to_owned())?;
        let desired_score = score.clone();
        let desired_schedule = ArrangementSchedule::from_score(&desired_score, tempo)?;
        self.desired_score = Some(desired_score);
        self.desired_schedule = Some(desired_schedule);
        self.desired_revision = next_revision;
        Ok(next_revision)
    }

    /// Promote a processor-confirmed version to the applied schedule. The
    /// host supplies the exact submitted score for this revision, including
    /// when an older queued revision lands just before a newer edit.
    pub fn acknowledge_revision(
        &mut self,
        revision: u64,
        submitted_score: &Score,
        tempo_bpm: f64,
    ) -> Result<(), String> {
        if revision < self.applied_revision || revision > self.desired_revision {
            return Err(
                "Practice score acknowledgement is outside the submitted revision window.".into(),
            );
        }
        if !tempo_bpm.is_finite() || !(1.0..=400.0).contains(&tempo_bpm) {
            return Err("Practice tempo must be finite and within 1..=400 BPM.".into());
        }
        if revision == self.desired_revision && self.desired_score.as_ref() != Some(submitted_score)
        {
            return Err("Acknowledged score does not match the current submitted revision.".into());
        }
        if revision == self.applied_revision && self.applied_score.as_ref() != Some(submitted_score)
        {
            return Err("Acknowledged score does not match the applied revision.".into());
        }
        let applied_score = submitted_score.clone();
        let applied_schedule = ArrangementSchedule::from_score(&applied_score, tempo_bpm)?;
        self.applied_score = Some(applied_score);
        self.applied_schedule = Some(applied_schedule);
        self.applied_revision = revision;
        self.tempo_bpm = Some(tempo_bpm);
        if revision == self.desired_revision {
            self.desired_score = self.applied_score.clone();
            self.desired_schedule = self.applied_schedule.clone();
        }
        Ok(())
    }

    /// Accept a device clock snapshot only for the current owner/session/run
    /// and the revision confirmed by the native processor. Position is local
    /// to its reported bar; authored rests remain visible as events.
    pub fn observe(&mut self, observation: &PracticeObservation) -> bool {
        if !matches!(
            self.status,
            PracticeStatus::Playing | PracticeStatus::Paused
        ) || self.owner != Some(observation.owner)
            || self.run_epoch != observation.run_epoch
            || self.applied_revision != observation.applied_revision
            || (self.status == PracticeStatus::Paused
                && observation.status == PracticeStatus::Playing)
            || (self.status == PracticeStatus::Playing
                && observation.status == PracticeStatus::Paused)
            || !self
                .session
                .as_ref()
                .is_some_and(|session| Arc::ptr_eq(session, &observation.session))
        {
            return false;
        }
        let Some(schedule) = self.applied_schedule.as_ref() else {
            return false;
        };
        let playhead = locate(schedule, observation.bar_index, observation.bar_beat_ticks);
        self.playhead = playhead;
        self.status = match observation.status {
            PracticeStatus::Complete => PracticeStatus::Complete,
            PracticeStatus::Stopped => PracticeStatus::Stopped,
            PracticeStatus::Paused => PracticeStatus::Paused,
            PracticeStatus::Playing => PracticeStatus::Playing,
        };
        true
    }

    pub fn owner(&self) -> Option<WorkingSetId> {
        self.owner
    }
    pub fn session(&self) -> Option<&Arc<()>> {
        self.session.as_ref()
    }
    pub fn run_epoch(&self) -> u64 {
        self.run_epoch
    }
    pub fn applied_score(&self) -> Option<&Score> {
        self.applied_score.as_ref()
    }
    pub fn applied_schedule(&self) -> Option<&ArrangementSchedule> {
        self.applied_schedule.as_ref()
    }
    pub fn applied_revision(&self) -> u64 {
        self.applied_revision
    }
    pub fn desired_score(&self) -> Option<&Score> {
        self.desired_score.as_ref()
    }
    pub fn desired_schedule(&self) -> Option<&ArrangementSchedule> {
        self.desired_schedule.as_ref()
    }
    pub fn desired_revision(&self) -> u64 {
        self.desired_revision
    }
    pub fn pending_update(&self) -> bool {
        self.applied_revision != self.desired_revision
    }
    pub fn tempo_bpm(&self) -> Option<f64> {
        self.tempo_bpm
    }
    pub fn status(&self) -> PracticeStatus {
        self.status
    }
    pub fn playhead(&self) -> Option<PracticePlayhead> {
        self.playhead
    }
}

fn locate(
    schedule: &ArrangementSchedule,
    bar_index: usize,
    bar_ticks: u64,
) -> Option<PracticePlayhead> {
    let measure = schedule.measures.get(bar_index)?;
    let event: &PlaybackEvent = schedule.event_at(bar_index, bar_ticks)?;
    Some(PracticePlayhead {
        bar_index: measure.measure_index,
        event_index: event.event_index,
        is_rest: event.is_rest(),
        bar_beat_ticks: bar_ticks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> WorkingSetId {
        WorkingSetId(19)
    }

    #[test]
    fn run_captures_score_and_tempo_changes_remain_ephemeral() {
        let score = Score::progression_example();
        let before = score.clone();
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let first_epoch = practice.start(owner(), &score, 90.0, session).unwrap();
        assert_eq!(practice.applied_score(), Some(&before));
        practice.set_tempo(112.5).unwrap();
        assert_eq!(practice.tempo_bpm(), Some(112.5));
        assert_eq!(practice.applied_schedule().unwrap().tempo_bpm, 112.5);
        assert_eq!(practice.desired_schedule().unwrap().tempo_bpm, 112.5);
        assert_eq!(score, before);
        practice.stop();
        let next_epoch = practice.start(owner(), &score, 90.0, Arc::new(())).unwrap();
        assert!(next_epoch > first_epoch);
        assert_eq!(practice.status(), PracticeStatus::Playing);
        assert_eq!(practice.playhead(), None);
    }

    #[test]
    fn rejects_stale_owner_session_and_run_epoch() {
        let score = Score::rhythm_example();
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let epoch = practice
            .start(owner(), &score, 96.0, session.clone())
            .unwrap();
        let stale = |owner, session, run_epoch| PracticeObservation {
            owner,
            session,
            run_epoch,
            applied_revision: 0,
            bar_index: 0,
            bar_beat_ticks: 0,
            status: PracticeStatus::Playing,
        };
        assert!(!practice.observe(&stale(WorkingSetId(owner().0 + 1), session.clone(), epoch)));
        assert!(!practice.observe(&stale(owner(), Arc::new(()), epoch)));
        assert!(!practice.observe(&stale(owner(), session.clone(), epoch.saturating_sub(1))));
        assert!(practice.observe(&stale(owner(), session, epoch)));
    }

    #[test]
    fn playhead_uses_bar_local_device_position_and_keeps_authored_rests() {
        let score = Score::rhythm_example();
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let epoch = practice
            .start(owner(), &score, 96.0, session.clone())
            .unwrap();
        let observation = |bar_index, bar_beat_ticks| PracticeObservation {
            owner: owner(),
            session: session.clone(),
            run_epoch: epoch,
            applied_revision: 0,
            bar_index,
            bar_beat_ticks,
            status: PracticeStatus::Playing,
        };
        assert!(practice.observe(&observation(0, 2_880)));
        assert_eq!(
            practice.playhead(),
            Some(PracticePlayhead {
                bar_index: 0,
                event_index: 4,
                is_rest: true,
                bar_beat_ticks: 2_880,
            })
        );
        assert!(practice.observe(&observation(1, 0)));
        assert_eq!(
            practice.playhead(),
            Some(PracticePlayhead {
                bar_index: 1,
                event_index: 0,
                is_rest: false,
                bar_beat_ticks: 0,
            })
        );
    }

    #[test]
    fn unmappable_bar_end_does_not_complete_until_engine_reports_complete() {
        let score = Score::rhythm_example();
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let epoch = practice
            .start(owner(), &score, 96.0, session.clone())
            .unwrap();
        let bar_end = practice.applied_schedule().unwrap().measures[0].duration_ticks;
        let at_bar_end = PracticeObservation {
            owner: owner(),
            session: session.clone(),
            run_epoch: epoch,
            applied_revision: 0,
            bar_index: 0,
            bar_beat_ticks: bar_end,
            status: PracticeStatus::Playing,
        };

        assert!(practice.observe(&at_bar_end));
        assert_eq!(practice.status(), PracticeStatus::Playing);
        assert_eq!(practice.playhead(), None);

        let complete = PracticeObservation {
            status: PracticeStatus::Complete,
            ..at_bar_end
        };
        assert!(practice.observe(&complete));
        assert_eq!(practice.status(), PracticeStatus::Complete);
    }

    #[test]
    fn paused_observations_are_accepted_and_stop_clears_position() {
        let score = Score::rhythm_example();
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let epoch = practice
            .start(owner(), &score, 96.0, session.clone())
            .unwrap();
        let observation = PracticeObservation {
            owner: owner(),
            session,
            run_epoch: epoch,
            applied_revision: 0,
            bar_index: 0,
            bar_beat_ticks: 960,
            status: PracticeStatus::Playing,
        };
        assert!(practice.observe(&observation));
        practice.pause();
        assert_eq!(practice.status(), PracticeStatus::Paused);
        assert!(
            !practice.observe(&observation),
            "a stale playing snapshot cannot undo a pause intent"
        );
        let paused_observation = PracticeObservation {
            status: PracticeStatus::Paused,
            ..observation.clone()
        };
        assert!(practice.observe(&paused_observation));
        practice.resume().unwrap();
        assert!(
            !practice.observe(&paused_observation),
            "a stale paused snapshot cannot undo a resume intent"
        );
        assert_eq!(practice.status(), PracticeStatus::Playing);
        let resumed = PracticeObservation {
            status: PracticeStatus::Playing,
            ..observation
        };
        assert!(practice.observe(&resumed));
        practice.stop();
        assert_eq!(practice.status(), PracticeStatus::Stopped);
        assert_eq!(practice.playhead(), None);
        assert_eq!(
            practice.resume().unwrap_err(),
            "No paused music practice run is available to resume."
        );
    }

    #[test]
    fn tempo_override_rejects_invalid_values_without_changing_schedule() {
        let score = Score::progression_example();
        let mut practice = MusicPractice::default();
        practice.start(owner(), &score, 90.0, Arc::new(())).unwrap();
        let before = practice.applied_schedule().unwrap().clone();
        for invalid in [f64::NAN, f64::INFINITY, 0.0, 401.0] {
            assert!(practice.set_tempo(invalid).is_err());
            assert_eq!(practice.applied_schedule(), Some(&before));
            assert_eq!(practice.desired_schedule(), Some(&before));
        }
    }

    #[test]
    fn score_edits_wait_for_processor_receipt_and_keep_old_playhead_valid() {
        let initial = Score::rhythm_example();
        let mut first_edit = initial.clone();
        first_edit.measures[0].events[0].notes.clear();
        first_edit.measures[0].events[0].rest = true;
        let mut latest_edit = first_edit.clone();
        latest_edit.measures[0].events[1].notes.clear();
        latest_edit.measures[0].events[1].rest = true;
        let session = Arc::new(());
        let mut practice = MusicPractice::default();
        let run = practice
            .start(owner(), &initial, 96.0, session.clone())
            .unwrap();
        let applied_before = practice.applied_schedule().unwrap().clone();

        let rev1 = practice.request_update(&first_edit).unwrap();
        let rev2 = practice.request_update(&latest_edit).unwrap();
        assert_eq!((rev1, rev2), (1, 2));
        assert!(practice.pending_update());
        assert_eq!(practice.applied_schedule(), Some(&applied_before));
        assert_eq!(practice.desired_score(), Some(&latest_edit));

        // The processor may confirm an already submitted revision immediately
        // before the latest edit supersedes it. The host passes that exact
        // score snapshot; views continue projecting against its applied plan.
        practice
            .acknowledge_revision(rev1, &first_edit, 96.0)
            .unwrap();
        assert_eq!(practice.applied_revision(), rev1);
        assert!(practice.pending_update());
        let first_receipt = PracticeObservation {
            owner: owner(),
            session: session.clone(),
            run_epoch: run,
            applied_revision: rev1,
            bar_index: 0,
            bar_beat_ticks: 1_000,
            status: PracticeStatus::Playing,
        };
        assert!(practice.observe(&first_receipt));
        assert_eq!(practice.playhead().unwrap().event_index, 1);

        practice
            .acknowledge_revision(rev2, &latest_edit, 96.0)
            .unwrap();
        assert!(!practice.pending_update());
        assert_eq!(practice.applied_score(), Some(&latest_edit));
        assert!(
            !practice.observe(&first_receipt),
            "older applied revision is stale after promotion"
        );
        let latest_receipt = PracticeObservation {
            applied_revision: rev2,
            ..first_receipt
        };
        assert!(practice.observe(&latest_receipt));
        assert!(practice.playhead().unwrap().is_rest);
        assert_eq!(
            initial,
            Score::rhythm_example(),
            "practice updates do not mutate the input score"
        );
    }
}
