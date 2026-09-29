//! Product session authority. Playback snapshots only belong to a selection
//! when their load token matches; queue order is independent of that selection.
use redshank_model::{
    CaptureAnchor, FeedEpisodeFacts, ItemId, LibraryItem, ListeningSession, MediaSource, NoteBody,
    NotePrivacy, Progress, RedshankModel, RepresentationIdentity, RepresentationReceipt,
    ResumeCompleted, ThemeMode, ThemeSeed,
};
use redshank_playback::{PlaybackCommand, PlaybackSnapshot, PlaybackState, PreviewState};
use redshank_surfaces::{
    Face, FeedRow, ItemRow, ListeningSessionRow, Mode, NoteSummary, NoteSummaryBody, NotesFilter,
    NowPlaying, Recording, RedshankSurfaceState, RepresentationSummary, Seed, SourceKind,
    TransportState, VoiceNotePreview, short_digest,
};

/// Host facts the projection cannot derive from the model or the snapshot.
#[derive(Clone, Debug, Default)]
pub struct HostFacts {
    /// Wall clock in epoch milliseconds; the trail's day labels need it.
    pub now_ms: u64,
    pub recording: Option<Recording>,
    pub microphone_label: Option<String>,
}

/// A listening stretch that has not ended yet.
#[derive(Clone, Debug)]
pub struct OpenListening {
    pub item_id: ItemId,
    pub start_ms: u64,
    pub started_at_ms: u64,
    pub stop_ms: u64,
}

pub struct Session {
    pub model: RedshankModel,
    pub selected: Option<ItemId>,
    pub token: u64,
    /// Saved progress the last selection resumed from, for the dock's mark.
    pub resumed_from_ms: Option<u64>,
    pub open_listening: Option<OpenListening>,
    /// A note is checking a newly loaded copy at zero. Keep saved progress
    /// until its seek is acknowledged or the user explicitly uses transport.
    pub hold_progress: bool,
}

impl Session {
    pub fn new(model: RedshankModel) -> Self {
        Self {
            model,
            selected: None,
            token: 0,
            resumed_from_ms: None,
            open_listening: None,
            hold_progress: false,
        }
    }

    pub fn matches(&self, snapshot: &PlaybackSnapshot) -> bool {
        self.selected.is_some() && snapshot.load_token == Some(self.token)
    }

    /// Start a listening stretch, or extend the open one.
    fn note_listening(&mut self, position_ms: u64, now: u64) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => return,
        };
        match &mut self.open_listening {
            Some(open) if open.item_id == id => open.stop_ms = position_ms.max(open.stop_ms),
            _ => {
                self.open_listening = Some(OpenListening {
                    item_id: id,
                    start_ms: position_ms,
                    started_at_ms: now,
                    stop_ms: position_ms,
                });
            },
        }
    }

    /// Close the open stretch, recording it when it covered any source time.
    pub fn close_listening(&mut self, now: u64, completed: bool) -> bool {
        let Some(open) = self.open_listening.take() else {
            return false;
        };
        if open.stop_ms <= open.start_ms {
            return false;
        }
        self.model.record_listening_session(ListeningSession {
            item_id: open.item_id,
            start_ms: open.start_ms,
            stop_ms: open.stop_ms,
            started_at_ms: open.started_at_ms,
            ended_at_ms: now,
            completed,
        });
        true
    }

    pub fn record_progress(&mut self, snapshot: &PlaybackSnapshot, now: u64) -> bool {
        if self.hold_progress
            || !self.matches(snapshot)
            || !matches!(
                snapshot.state,
                PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Ended
            )
        {
            return false;
        }
        let completed = snapshot.state == PlaybackState::Ended;
        let mut changed = match snapshot.state {
            PlaybackState::Playing => {
                self.note_listening(snapshot.position_ms, now);
                false
            },
            _ => {
                if let Some(open) = &mut self.open_listening {
                    open.stop_ms = snapshot.position_ms.max(open.stop_ms);
                }
                self.close_listening(now, completed)
            },
        };
        let id = self.selected.as_ref().expect("matched selection");
        let existing = self.model.progress.get(id);
        if existing
            .is_some_and(|p| p.position_ms == snapshot.position_ms && p.completed == completed)
        {
            return changed;
        }
        // A completed item reopened at zero stays completed (Replay is its
        // primary action) until playback actually moves.
        if existing.is_some_and(|p| p.completed)
            && snapshot.state == PlaybackState::Paused
            && snapshot.position_ms == 0
        {
            return changed;
        }
        let id = id.clone();
        self.model.progress.insert(
            id,
            Progress {
                position_ms: snapshot.position_ms,
                completed,
                updated_at_ms: now,
            },
        );
        changed = true;
        changed
    }

    pub fn select(
        &mut self,
        id: ItemId,
        snapshot: &PlaybackSnapshot,
        now: u64,
    ) -> Result<Vec<PlaybackCommand>, String> {
        let source = self
            .model
            .library
            .get(&id)
            .ok_or_else(|| "That library item no longer exists".to_owned())?
            .source()
            .clone();
        self.record_progress(snapshot, now);
        self.close_listening(now, false);
        self.hold_progress = false;
        self.token = self
            .token
            .checked_add(1)
            .ok_or("Playback identity exhausted")?;
        self.selected = Some(id.clone());
        self.model.selected_item = Some(id.clone());
        // A completed item reopens at zero, or where the listener stopped,
        // as `resume_completed_from` says.
        let progress = self.model.progress.get(&id);
        let resume = match self.model.settings.resume_completed_from {
            ResumeCompleted::Saved => progress.map_or(0, |progress| progress.position_ms),
            ResumeCompleted::Start => progress
                .filter(|progress| !progress.completed)
                .map_or(0, |progress| progress.position_ms),
        };
        self.resumed_from_ms = (resume > 0).then_some(resume);
        Ok(vec![PlaybackCommand::Load {
            token: self.token,
            source,
            resume_ms: resume,
        }])
    }

    pub fn capture(&self, snapshot: &PlaybackSnapshot) -> Result<CaptureAnchor, String> {
        if !self.matches(snapshot)
            || !matches!(
                snapshot.state,
                PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Ended
            )
        {
            return Err("Wait until the selected recording has loaded before adding a note".into());
        }
        let pressed = snapshot.position_ms;
        let offset = pressed.saturating_sub(self.model.settings.reaction_offset_ms);
        Ok(CaptureAnchor {
            fingerprint: snapshot
                .fingerprint_context
                .as_ref()
                .and_then(|context| context.at(offset, self.model.settings.alignment_window_ms)),
            item_id: self.selected.clone().expect("matched selection"),
            offset_ms: offset,
            end_offset_ms: None,
            pressed_offset_ms: (offset != pressed).then_some(pressed),
            representation: snapshot
                .representation
                .clone()
                .ok_or("The loaded recording has no representation receipt")?,
        })
    }

    pub fn note_identity(
        &self,
        target: &CaptureAnchor,
        snapshot: &PlaybackSnapshot,
    ) -> RepresentationIdentity {
        if self.selected.as_ref() != Some(&target.item_id)
            || !self.matches(snapshot)
            || !matches!(
                snapshot.state,
                PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Ended
            )
        {
            return RepresentationIdentity::Unproven;
        }
        snapshot
            .representation
            .as_ref()
            .map_or(RepresentationIdentity::Unproven, |receipt| {
                target.representation.compare(receipt)
            })
    }

    /// An alignment is a separate point estimate for one known destination.
    /// Never infer a span end or alter the annotation's original target.
    pub fn aligned_target(
        &self,
        id: &redshank_model::AnnotationId,
        snapshot: &PlaybackSnapshot,
    ) -> Result<CaptureAnchor, String> {
        let note = self
            .model
            .annotations
            .get(id)
            .ok_or("That note no longer exists")?;
        let derived = self
            .model
            .derived_positions
            .get(id)
            .ok_or("Realign this note first")?;
        self.model
            .validate_derived_position(id, derived)
            .map_err(|e| format!("Saved alignment evidence is invalid: {e:?}"))?;
        if snapshot
            .representation
            .as_ref()
            .and_then(|receipt| receipt.complete_digest.as_ref())
            != derived.destination.complete_digest.as_ref()
        {
            return Err(
                "The loaded copy has not verified the alignment's destination digest".into(),
            );
        }
        if derived.original_target != note.target
            || derived.confidence_per_mille < self.model.settings.alignment_min_confidence_per_mille
            || derived.destination.complete_digest.is_none()
        {
            return Err(
                "The saved alignment does not meet this note's current requirements".into(),
            );
        }
        let mut target = note.target.clone();
        target.offset_ms = derived.offset_ms;
        target.end_offset_ms = None;
        target.pressed_offset_ms = None;
        target.fingerprint = None;
        target.representation = derived.destination.clone();
        self.note_seek(&target, snapshot, false)?;
        Ok(target)
    }

    /// Called only against the loaded selection, including for explicit
    /// approximate opens. A stale load must never receive a note's seek.
    pub fn note_seek(
        &self,
        target: &CaptureAnchor,
        snapshot: &PlaybackSnapshot,
        approximate: bool,
    ) -> Result<PlaybackCommand, String> {
        if self.selected.as_ref() != Some(&target.item_id)
            || !self.matches(snapshot)
            || !matches!(
                snapshot.state,
                PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Ended
            )
        {
            return Err("Wait until the note's recording has loaded".into());
        }
        if !approximate {
            match self.note_identity(target, snapshot) {
                RepresentationIdentity::Same => {},
                RepresentationIdentity::Different => return Err("This copy differs. Use Open at approximate time to use the original timestamp.".into()),
                RepresentationIdentity::Unproven => return Err("Couldn't verify this copy. Use Open at approximate time to use the original timestamp.".into()),
            }
        }
        Ok(PlaybackCommand::Seek(target.offset_ms))
    }

    /// Update the projection without replacing its editor, focus, or notices.
    pub fn project(
        &self,
        state: &mut RedshankSurfaceState,
        snapshot: &PlaybackSnapshot,
        host: &HostFacts,
    ) {
        let saved = self
            .selected
            .as_ref()
            .and_then(|id| self.model.transcripts.get(id).map(|saved| (id, saved)));
        match saved {
            Some((id, saved))
                if state
                    .transcript
                    .as_ref()
                    .is_some_and(|(old_id, old, _)| old_id == id && old == saved) => {},
            Some((id, saved)) => {
                state.transcript = timed_text::parse(&saved.source)
                    .ok()
                    .map(|parsed| (id.clone(), saved.clone(), parsed))
            },
            None => state.transcript = None,
        }
        state.library = self.model.library.values().cloned().collect();
        state.subscriptions = self.model.subscriptions.values().cloned().collect();
        state.queue = self.model.queue.clone();
        state.settings = self.model.settings.clone();
        state.seed = match self.model.settings.seed {
            ThemeSeed::Wetland => Seed::Wetland,
            ThemeSeed::BrandShell => Seed::BrandShell,
        };
        state.mode = match self.model.settings.mode {
            ThemeMode::Dark => Mode::Dark,
            ThemeMode::Light => Mode::Light,
            ThemeMode::HcDark => Mode::HcDark,
            ThemeMode::HcLight => Mode::HcLight,
        };
        state.compact.skip_backward_ms = self.model.settings.skip_backward_ms;
        state.compact.skip_forward_ms = self.model.settings.skip_forward_ms;
        state.compact.rate_percent = snapshot.rate_percent;
        state.compact.volume_percent = snapshot.volume_percent;
        state.compact.recording = host.recording.clone();
        state.microphone_label = host.microphone_label.clone();
        state.cache_used_bytes = self.cache_used_bytes();

        let item = self
            .selected
            .as_ref()
            .and_then(|id| self.model.library.get(id));
        let matched = self.matches(snapshot);
        let completed_progress = self
            .selected
            .as_ref()
            .and_then(|id| self.model.progress.get(id))
            .is_some_and(|progress| progress.completed);
        state.compact.transport = if item.is_none() {
            TransportState::Empty
        } else if !matched {
            TransportState::Buffering
        } else {
            match &snapshot.state {
                PlaybackState::Empty | PlaybackState::Loading => TransportState::Buffering,
                PlaybackState::Playing => TransportState::Playing,
                PlaybackState::Ended => TransportState::Completed,
                PlaybackState::Paused if completed_progress => TransportState::Completed,
                PlaybackState::Paused => TransportState::Paused,
                PlaybackState::Unavailable(error) => TransportState::Unavailable(error.clone()),
            }
        };

        state.items = self
            .model
            .library
            .values()
            .map(|item| self.item_row(item, snapshot, matched))
            .collect();
        state.feeds = self.feed_rows();
        state.notes = self.note_rows(state.notes_filter, snapshot);
        state.note_identities = state
            .notes
            .iter()
            .filter_map(|row| {
                self.model
                    .annotations
                    .get(&row.id)
                    .map(|note| (row.id.clone(), self.note_identity(&note.target, snapshot)))
            })
            .collect();
        state.note_alignments = state
            .notes
            .iter()
            .filter_map(|row| {
                use redshank_surfaces::NoteAlignment;
                let note = self.model.annotations.get(&row.id)?;
                if self.note_identity(&note.target, snapshot) == RepresentationIdentity::Same {
                    return None;
                }
                let alignment = if self.aligned_target(&row.id, snapshot).is_ok() {
                    let derived = &self.model.derived_positions[&row.id];
                    NoteAlignment::Aligned {
                        position_ms: derived.offset_ms,
                        score_per_mille: derived.confidence_per_mille,
                    }
                } else if note.target.fingerprint.is_none() {
                    NoteAlignment::Unavailable(
                        "No audio fingerprint was captured for this note.".into(),
                    )
                } else if self
                    .model
                    .library
                    .get(&row.item_id)
                    .is_some_and(|item| item.source().is_cached())
                {
                    NoteAlignment::Ready
                } else if self
                    .model
                    .library
                    .get(&row.item_id)
                    .and_then(|item| item.source().enclosure_url())
                    .is_some()
                {
                    NoteAlignment::Download
                } else {
                    NoteAlignment::Unavailable(
                        "Realignment requires a downloaded episode copy.".into(),
                    )
                };
                Some((row.id.clone(), alignment))
            })
            .collect();
        state.sessions = self.session_rows(host.now_ms);

        let markers = state
            .notes
            .iter()
            .filter(|note| Some(&note.item_id) == self.selected.as_ref())
            .map(NoteSummary::marker)
            .collect();
        state.compact.now_playing = item.map(|item| NowPlaying {
            item_id: item.id().clone(),
            title: item.title().to_owned(),
            feed_title: self.feed_title(item),
            face: self.face(item),
            source: SourceKind::from_item(item),
            position_ms: if matched { snapshot.position_ms } else { 0 },
            duration_ms: if matched {
                snapshot.duration_ms.or_else(|| facts_duration_ms(item))
            } else {
                facts_duration_ms(item)
            },
            resumed_from_ms: self.resumed_from_ms,
            buffered_percent: if matched {
                snapshot.buffered_percent
            } else {
                0
            },
            markers,
        });
    }

    /// Bytes the offline cache holds, counting each shared object once.
    fn cache_used_bytes(&self) -> u64 {
        self.model
            .cache_eviction_order()
            .iter()
            .filter_map(|candidate| candidate.byte_length)
            .sum()
    }

    fn feed_title(&self, item: &LibraryItem) -> Option<String> {
        let LibraryItem::FeedEpisode { feed_url, .. } = item else {
            return None;
        };
        self.model
            .subscriptions
            .get(feed_url)
            .map(|feed| feed.title.clone())
    }

    fn face(&self, item: &LibraryItem) -> Face {
        if let LibraryItem::FeedEpisode {
            feed_url, facts, ..
        } = item
        {
            if let Some(artwork) = facts.artwork.clone() {
                return Face::Artwork(artwork);
            }
            if let Some(artwork) = self
                .model
                .subscriptions
                .get(feed_url)
                .and_then(|feed| feed.artwork.clone())
            {
                return Face::Artwork(artwork);
            }
        }
        Face::Tag(format_tag(item))
    }

    fn item_row(&self, item: &LibraryItem, snapshot: &PlaybackSnapshot, matched: bool) -> ItemRow {
        let id = item.id().clone();
        let selected = matched && self.selected.as_ref() == Some(&id);
        let progress = self.model.progress.get(&id);
        ItemRow {
            id: id.clone(),
            title: item.title().to_owned(),
            feed_url: match item {
                LibraryItem::FeedEpisode { feed_url, .. } => Some(feed_url.clone()),
                _ => None,
            },
            feed_title: self.feed_title(item),
            face: self.face(item),
            source: SourceKind::from_item(item),
            duration_ms: selected
                .then_some(snapshot.duration_ms)
                .flatten()
                .or_else(|| facts_duration_ms(item)),
            position_ms: if selected {
                snapshot.position_ms
            } else {
                progress.map_or(0, |progress| progress.position_ms)
            },
            completed: progress.is_some_and(|progress| progress.completed)
                || (selected && snapshot.state == PlaybackState::Ended),
            published: match item {
                LibraryItem::FeedEpisode { facts, .. } => facts.published.clone(),
                _ => None,
            },
            cached_bytes: match item.source() {
                MediaSource::Cached { representation, .. } => representation.byte_length,
                _ => None,
            },
            note_count: self.model.annotations_for_item(&id).len(),
            unavailable: match (&snapshot.state, selected) {
                (PlaybackState::Unavailable(message), true) => Some(message.clone()),
                _ => None,
            },
            pinned: self.model.is_pinned(&id),
            representation: selected
                .then_some(snapshot.representation.as_ref())
                .flatten()
                .map(|receipt| self.representation_summary(&id, receipt)),
        }
    }

    /// What the Notes card may say about one item's bytes: when they were
    /// fetched, their short digest, and whether the notes still target them.
    fn representation_summary(
        &self,
        id: &ItemId,
        receipt: &RepresentationReceipt,
    ) -> RepresentationSummary {
        let item_digest = receipt.complete_digest.as_deref();
        let mut compared = false;
        let mut unproven = false;
        let mut matches = true;
        for note in self.model.annotations_for_item(id) {
            compared = true;
            match note.target.representation.compare(receipt) {
                RepresentationIdentity::Same => {},
                RepresentationIdentity::Different => matches = false,
                RepresentationIdentity::Unproven => unproven = true,
            }
        }
        RepresentationSummary {
            retrieved_at_ms: receipt.retrieved_at_ms,
            short_digest: item_digest.and_then(short_digest),
            matches: if compared && !matches {
                Some(false)
            } else if compared && !unproven {
                Some(true)
            } else {
                None
            },
        }
    }

    fn feed_rows(&self) -> Vec<FeedRow> {
        self.model
            .subscriptions
            .values()
            .map(|feed| {
                let episodes: Vec<_> = self
                    .model
                    .library
                    .values()
                    .filter(|item| {
                        matches!(item, LibraryItem::FeedEpisode { feed_url, .. } if feed_url == &feed.feed_url)
                    })
                    .collect();
                let unplayed = episodes
                    .iter()
                    .filter(|item| !self.model.progress.contains_key(item.id()))
                    .count();
                let mut counted = Vec::new();
                let mut offline_bytes = 0;
                for item in &episodes {
                    if let MediaSource::Cached {
                        path,
                        representation,
                        ..
                    } = item.source()
                        && !counted.contains(path)
                    {
                        counted.push(path.clone());
                        offline_bytes += representation.byte_length.unwrap_or(0);
                    }
                }
                FeedRow {
                    feed_url: feed.feed_url.clone(),
                    title: feed.title.clone(),
                    subtitle: feed.subtitle.clone(),
                    face: feed
                        .artwork
                        .clone()
                        .map_or_else(|| Face::Tag(episodes.len().to_string()), Face::Artwork),
                    episode_count: episodes.len(),
                    unplayed_count: unplayed,
                    offline_bytes,
                    last_refreshed_ms: feed.last_refreshed_ms,
                    // Parser diagnostics describe a successful import. Actual
                    // refresh failures are transient host facts, projected by Desktop.
                    failure: None,
                }
            })
            .collect()
    }

    fn note_rows(&self, filter: NotesFilter, snapshot: &PlaybackSnapshot) -> Vec<NoteSummary> {
        let notes: Vec<_> = match filter {
            NotesFilter::AllNotes => self.model.annotations.values().collect(),
            NotesFilter::ThisEpisode => match &self.selected {
                Some(id) => self.model.annotations_for_item(id),
                None => Vec::new(),
            },
        };
        notes
            .into_iter()
            .map(|note| NoteSummary {
                id: note.id.clone(),
                item_id: note.target.item_id.clone(),
                offset_ms: note.target.offset_ms,
                end_offset_ms: note.target.end_offset_ms,
                private: note.privacy == NotePrivacy::Private,
                body: match &note.body {
                    NoteBody::Text { plain_text } => NoteSummaryBody::Text(plain_text.clone()),
                    NoteBody::Audio { duration_ms, .. } => NoteSummaryBody::Voice {
                        duration_ms: *duration_ms,
                        preview: snapshot
                            .preview
                            .as_ref()
                            .filter(|preview| preview.id == note.id.0)
                            .map(|preview| match &preview.state {
                                PreviewState::Loading => VoiceNotePreview::Loading,
                                PreviewState::Playing => VoiceNotePreview::Playing {
                                    position_ms: preview.position_ms,
                                },
                                PreviewState::Ended => VoiceNotePreview::Ended,
                                PreviewState::Unavailable(error) => {
                                    VoiceNotePreview::Unavailable(error.clone())
                                },
                            })
                            .unwrap_or_default(),
                    },
                },
            })
            .collect()
    }

    /// The trail: the open stretch first, then recorded ones newest first.
    fn session_rows(&self, now_ms: u64) -> Vec<ListeningSessionRow> {
        let open = self.open_listening.iter().map(|open| ListeningSession {
            item_id: open.item_id.clone(),
            start_ms: open.start_ms,
            stop_ms: open.stop_ms,
            started_at_ms: open.started_at_ms,
            ended_at_ms: now_ms,
            completed: false,
        });
        let recorded = self.model.listening_sessions.iter().rev().cloned();
        let mut rows = Vec::new();
        for (index, session) in open.chain(recorded).enumerate() {
            let title = self
                .model
                .library
                .get(&session.item_id)
                .map_or_else(|| session.item_id.0.clone(), |item| item.title().to_owned());
            let duration_ms = self
                .model
                .library
                .get(&session.item_id)
                .and_then(facts_duration_ms);
            rows.push(ListeningSessionRow {
                note_offsets_ms: self
                    .model
                    .annotations_for_item(&session.item_id)
                    .into_iter()
                    .filter(|note| {
                        (session.start_ms..=session.stop_ms).contains(&note.target.offset_ms)
                    })
                    .map(|note| note.target.offset_ms)
                    .collect(),
                active: index == 0 && self.open_listening.is_some(),
                day_label: day_label(session.started_at_ms, now_ms),
                wall_clock: wall_clock(session.started_at_ms),
                item_id: session.item_id,
                title,
                start_ms: session.start_ms,
                stop_ms: session.stop_ms,
                duration_ms,
                completed: session.completed,
            });
        }
        rows
    }
}

/// A short lowercase format tag for an item without artwork.
fn format_tag(item: &LibraryItem) -> String {
    let media_type = match item {
        LibraryItem::FeedEpisode { facts, .. } => facts.enclosure_media_type.as_deref(),
        _ => None,
    };
    if let Some(subtype) = media_type.and_then(|value| value.rsplit('/').next()) {
        return subtype.trim_start_matches("x-").to_owned();
    }
    let path = match item.source() {
        MediaSource::Local { path } => Some(path.as_str()),
        MediaSource::Enclosure { url } => Some(url.as_str()),
        MediaSource::Cached { origin_url, .. } => Some(origin_url.as_str()),
        MediaSource::HostBlob { .. } => None,
    };
    path.and_then(|path| path.split(['?', '#']).next())
        .and_then(|path| path.rsplit('.').next())
        .filter(|extension| extension.len() <= 4 && !extension.contains('/'))
        .map_or_else(|| "audio".to_owned(), str::to_lowercase)
}

/// `1:02:03`, `3:21`, or `3600` as milliseconds.
fn parse_duration_ms(value: &str) -> Option<u64> {
    let mut total = 0_u64;
    let mut parts = 0;
    for part in value.trim().split(':') {
        total = total
            .checked_mul(60)?
            .checked_add(part.trim().parse().ok()?)?;
        parts += 1;
    }
    (parts > 0).then_some(total * 1_000)
}

fn facts_duration_ms(item: &LibraryItem) -> Option<u64> {
    let LibraryItem::FeedEpisode { facts, .. } = item else {
        return None;
    };
    let facts: &FeedEpisodeFacts = facts;
    facts.duration.as_deref().and_then(parse_duration_ms)
}

const DAY_MS: u64 = 86_400_000;

/// `TODAY`, `YESTERDAY`, or a short `12 SEP` date, against the host clock.
fn day_label(at_ms: u64, now_ms: u64) -> String {
    let day = at_ms / DAY_MS;
    let today = now_ms / DAY_MS;
    match today.checked_sub(day) {
        Some(0) => "TODAY".into(),
        Some(1) => "YESTERDAY".into(),
        _ => {
            let (_, month, date) = civil_from_days(day);
            const MONTHS: [&str; 12] = [
                "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
            ];
            format!("{date} {}", MONTHS[(month as usize - 1).min(11)])
        },
    }
}

fn wall_clock(at_ms: u64) -> String {
    let minutes = at_ms / 60_000;
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

/// Howard Hinnant's days-from-epoch to civil date, for day labels alone.
fn civil_from_days(days: u64) -> (i64, u32, u32) {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use redshank_model::{Annotation, AnnotationId, FeedSubscription, RepresentationReceipt};
    use redshank_surfaces::TextCapture;

    fn session() -> Session {
        let mut model = RedshankModel::default();
        for name in ["a", "b"] {
            let id = ItemId(name.into());
            model
                .add_item(LibraryItem::LocalAudio {
                    id: id.clone(),
                    title: name.into(),
                    source: MediaSource::Local {
                        path: format!("{name}.mp3"),
                    },
                })
                .unwrap();
            model.enqueue(&id).unwrap();
        }
        Session::new(model)
    }

    fn anchor(item: &str, offset_ms: u64) -> CaptureAnchor {
        CaptureAnchor {
            fingerprint: None,
            item_id: ItemId(item.into()),
            offset_ms,
            end_offset_ms: None,
            pressed_offset_ms: None,
            representation: Default::default(),
        }
    }

    fn snapshot(token: u64) -> PlaybackSnapshot {
        PlaybackSnapshot {
            load_token: Some(token),
            position_ms: 1_234,
            state: PlaybackState::Playing,
            representation: Some(RepresentationReceipt {
                complete_digest: Some("blake3:a".into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn note_seeking_requires_identity_unless_approximate_is_explicit() {
        use RepresentationIdentity::{Different, Same, Unproven};

        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let snap = snapshot(session.token);
        for (label, digest, expected) in [
            ("same digest", Some("blake3:a"), Same),
            ("different digest", Some("blake3:b"), Different),
            ("legacy note without evidence", None, Unproven),
        ] {
            let mut target = anchor("a", 42_000);
            target.representation.complete_digest = digest.map(str::to_owned);
            let original = target.clone();
            assert_eq!(session.note_identity(&target, &snap), expected, "{label}");
            let exact = session.note_seek(&target, &snap, false);
            if expected == Same {
                assert!(
                    matches!(exact, Ok(PlaybackCommand::Seek(42_000))),
                    "{label}"
                );
            } else {
                assert!(exact.is_err(), "{label}");
            }
            assert!(
                matches!(
                    session.note_seek(&target, &snap, true),
                    Ok(PlaybackCommand::Seek(42_000))
                ),
                "{label}"
            );
            assert_eq!(
                target, original,
                "seeking must not rewrite the original target"
            );
        }
    }

    #[test]
    fn streaming_notes_need_a_strong_validator_for_exact_seeking() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        for (tag, expected) in [
            (Some("\"episode-v1\""), RepresentationIdentity::Same),
            (Some("W/\"episode-v1\""), RepresentationIdentity::Unproven),
            (None, RepresentationIdentity::Unproven),
        ] {
            let mut target = anchor("a", 9_000);
            target.representation = RepresentationReceipt {
                final_url: Some("https://example.test/a.mp3".into()),
                etag: tag.map(str::to_owned),
                ..Default::default()
            };
            let mut snap = snapshot(session.token);
            snap.representation = Some(target.representation.clone());
            assert_eq!(session.note_identity(&target, &snap), expected);
            assert_eq!(
                session.note_seek(&target, &snap, false).is_ok(),
                expected == RepresentationIdentity::Same
            );
            assert!(matches!(
                session.note_seek(&target, &snap, true),
                Ok(PlaybackCommand::Seek(9_000))
            ));
        }
    }

    #[test]
    fn approximate_note_seeking_still_requires_a_ready_current_load() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let mut target = anchor("a", 3_000);
        target.representation = snapshot(session.token).representation.unwrap();

        for playback_state in [
            PlaybackState::Playing,
            PlaybackState::Paused,
            PlaybackState::Ended,
        ] {
            let mut snap = snapshot(session.token);
            snap.state = playback_state;
            // Missing representation evidence permits only the explicit approximation.
            snap.representation = None;
            assert_eq!(
                session.note_identity(&target, &snap),
                RepresentationIdentity::Unproven
            );
            assert!(session.note_seek(&target, &snap, false).is_err());
            assert!(matches!(
                session.note_seek(&target, &snap, true),
                Ok(PlaybackCommand::Seek(3_000))
            ));
        }

        for playback_state in [
            PlaybackState::Empty,
            PlaybackState::Loading,
            PlaybackState::Unavailable("load failed".into()),
        ] {
            let mut snap = snapshot(session.token);
            snap.state = playback_state;
            assert_eq!(
                session.note_identity(&target, &snap),
                RepresentationIdentity::Unproven
            );
            for approximate in [false, true] {
                assert!(session.note_seek(&target, &snap, approximate).is_err());
            }
        }

        for token in [None, Some(session.token + 1)] {
            let mut snap = snapshot(session.token);
            snap.load_token = token;
            assert_eq!(
                session.note_identity(&target, &snap),
                RepresentationIdentity::Unproven
            );
            for approximate in [false, true] {
                assert!(session.note_seek(&target, &snap, approximate).is_err());
            }
        }

        let snap = snapshot(session.token);
        let mut other_item = target.clone();
        other_item.item_id = ItemId("b".into());
        assert_eq!(
            session.note_identity(&other_item, &snap),
            RepresentationIdentity::Unproven
        );
        for approximate in [false, true] {
            assert!(session.note_seek(&other_item, &snap, approximate).is_err());
        }

        session.select(ItemId("b".into()), &snap, 2).unwrap();
        for approximate in [false, true] {
            assert!(session.note_seek(&target, &snap, approximate).is_err());
            assert!(session.note_seek(&other_item, &snap, approximate).is_err());
        }
    }

    #[test]
    fn projection_keeps_each_notes_identity_and_frozen_target_separate() {
        use RepresentationIdentity::{Different, Same, Unproven};

        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let cases = [
            ("same", "a", Some("blake3:a"), Same),
            ("different", "a", Some("blake3:b"), Different),
            ("unknown", "a", None, Unproven),
            ("other-item", "b", Some("blake3:a"), Unproven),
        ];
        for (name, item, digest, _) in cases {
            let mut target = anchor(item, 12_000);
            target.end_offset_ms = Some(14_000);
            target.pressed_offset_ms = Some(12_500);
            target.representation.complete_digest = digest.map(str::to_owned);
            session
                .model
                .add_text_annotation(AnnotationId(name.into()), target, "note".into(), 1)
                .unwrap();
        }
        let original_annotations = session.model.annotations.clone();
        let mut state = RedshankSurfaceState::default();
        state.notes_filter = NotesFilter::AllNotes;
        let snap = snapshot(session.token);
        session.project(&mut state, &snap, &HostFacts::default());
        assert_eq!(state.notes.len(), cases.len());
        assert_eq!(state.note_identities.len(), cases.len());
        for (name, _, _, expected) in cases {
            let id = AnnotationId(name.into());
            assert_eq!(state.note_identities.get(&id), Some(&expected), "{name}");
            let target = &session.model.annotations[&id].target;
            let _ = session.note_seek(target, &snap, false);
            let _ = session.note_seek(target, &snap, true);
        }
        assert_eq!(session.model.annotations, original_annotations);

        let mut loading = snap;
        loading.state = PlaybackState::Loading;
        session.project(&mut state, &loading, &HostFacts::default());
        assert_eq!(state.note_identities.len(), cases.len());
        assert!(
            state
                .note_identities
                .values()
                .all(|identity| *identity == Unproven)
        );
        assert_eq!(session.model.annotations, original_annotations);
    }

    #[test]
    fn refused_note_probe_preserves_saved_progress_when_selecting_away() {
        let mut session = session();
        let item = ItemId("a".into());
        let saved = Progress {
            position_ms: 81_000,
            completed: false,
            updated_at_ms: 1,
        };
        session.model.set_progress(&item, saved.clone()).unwrap();
        session
            .select(item.clone(), &PlaybackSnapshot::default(), 2)
            .unwrap();
        session.hold_progress = true;
        let mut probe = snapshot(session.token);
        probe.state = PlaybackState::Paused;
        probe.position_ms = 0;
        let target = anchor("a", 12_000);
        assert!(session.note_seek(&target, &probe, false).is_err());
        assert!(!session.record_progress(&probe, 3));
        assert_eq!(session.model.progress[&item], saved);
        assert!(session.open_listening.is_none());
        assert!(session.model.listening_sessions.is_empty());

        session.select(ItemId("b".into()), &probe, 4).unwrap();
        assert_eq!(session.model.progress[&item], saved);
        assert!(!session.hold_progress);
        assert!(session.model.listening_sessions.is_empty());

        let next = snapshot(session.token);
        assert!(session.record_progress(&next, 5));
        assert_eq!(
            session.model.progress[&ItemId("b".into())].position_ms,
            next.position_ms
        );
        assert_eq!(session.model.progress[&item], saved);
    }

    #[test]
    fn selection_change_rejects_old_clock_and_receipt() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let old = snapshot(session.token);
        session.select(ItemId("b".into()), &old, 2).unwrap();
        assert_eq!(
            session.model.progress[&ItemId("a".into())].position_ms,
            1_234
        );
        assert!(session.capture(&old).is_err());
        assert!(!session.record_progress(&old, 3));
        assert!(!session.model.progress.contains_key(&ItemId("b".into())));
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &old, &HostFacts::default());
        assert_eq!(state.compact.transport, TransportState::Buffering);
        assert_eq!(state.compact.now_playing.unwrap().position_ms, 0);
    }

    #[test]
    fn selecting_a_completed_item_restarts_instead_of_seeking_to_end() {
        let mut session = session();
        let id = ItemId("a".into());
        session.model.progress.insert(
            id.clone(),
            Progress {
                position_ms: 2_040,
                completed: true,
                updated_at_ms: 1,
            },
        );

        let commands = session.select(id, &PlaybackSnapshot::default(), 2).unwrap();
        let [PlaybackCommand::Load { resume_ms, .. }] = commands.as_slice() else {
            panic!("selection should issue exactly one load");
        };
        assert_eq!(*resume_ms, 0);
    }

    #[test]
    fn queue_reorder_and_projection_preserve_playback_and_draft_anchor() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let snap = snapshot(session.token);
        let anchor = session.capture(&snap).unwrap();
        let mut state = RedshankSurfaceState::default();
        state.text_capture = Some(TextCapture {
            anchor: anchor.clone(),
            draft: "keep".into(),
            end_offset_ms: None,
        });
        state.set_text_draft("keep");
        session.model.reorder_queue(0, 1).unwrap();
        session.project(&mut state, &snap, &HostFacts::default());
        assert_eq!(
            state.compact.now_playing.as_ref().unwrap().item_id,
            ItemId("a".into())
        );
        session.select(ItemId("b".into()), &snap, 2).unwrap();
        session.project(&mut state, &snapshot(session.token), &HostFacts::default());
        assert_eq!(state.text_capture.unwrap().anchor, anchor);
        assert_eq!(state.text_editor.text(), "keep");
    }

    #[test]
    fn projection_retains_voice_note_duration() {
        let mut session = session();
        let id = ItemId("a".into());
        session
            .select(id.clone(), &PlaybackSnapshot::default(), 1)
            .unwrap();
        session
            .model
            .add_annotation(Annotation {
                id: AnnotationId("voice".into()),
                target: anchor("a", 456),
                body: NoteBody::Audio {
                    blob_id: "voice:abc".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 1_500,
                },
                created_at_ms: 2,
                privacy: NotePrivacy::Private,
            })
            .unwrap();
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &snapshot(session.token), &HostFacts::default());
        assert_eq!(
            state.notes[0].body,
            NoteSummaryBody::Voice {
                duration_ms: 1_500,
                preview: VoiceNotePreview::Idle,
            }
        );
        assert!(state.notes[0].private);
        assert_eq!(state.notes[0].item_id, id);
    }

    #[test]
    fn projection_attaches_preview_state_only_to_the_matching_voice_note() {
        let mut session = session();
        let item_id = ItemId("a".into());
        session
            .select(item_id.clone(), &PlaybackSnapshot::default(), 1)
            .unwrap();
        for note_id in ["voice-a", "voice-b"] {
            session
                .model
                .add_annotation(Annotation {
                    id: AnnotationId(note_id.into()),
                    target: anchor("a", 456),
                    body: NoteBody::Audio {
                        blob_id: format!("voice:{note_id}"),
                        media_type: "audio/wav".into(),
                        duration_ms: 1_500,
                    },
                    created_at_ms: 2,
                    privacy: NotePrivacy::Private,
                })
                .unwrap();
        }
        let mut snapshot = snapshot(session.token);
        snapshot.preview = Some(redshank_playback::PreviewSnapshot {
            id: "voice-b".into(),
            state: PreviewState::Playing,
            position_ms: 700,
            duration_ms: Some(1_500),
        });
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &snapshot, &HostFacts::default());
        assert!(matches!(
            state.notes[0].body,
            NoteSummaryBody::Voice {
                preview: VoiceNotePreview::Idle,
                ..
            }
        ));
        assert!(matches!(
            state.notes[1].body,
            NoteSummaryBody::Voice {
                preview: VoiceNotePreview::Playing { position_ms: 700 },
                ..
            }
        ));
    }

    #[test]
    fn item_rows_and_feed_rows_carry_the_library_facts_the_design_shows() {
        let mut session = session();
        session.model.upsert_subscription(FeedSubscription {
            feed_url: "https://example.test/feed.xml".into(),
            title: "Field Notes".into(),
            subtitle: Some("weekly".into()),
            artwork: Some("https://example.test/art.png".into()),
            last_refreshed_ms: Some(1_000),
            diagnostics: vec!["HTML was stripped from 1 episode summaries".into()],
            ..Default::default()
        });
        let episode = ItemId("episode".into());
        session
            .model
            .add_item(LibraryItem::FeedEpisode {
                id: episode.clone(),
                feed_url: "https://example.test/feed.xml".into(),
                guid: "one".into(),
                title: "One".into(),
                source: MediaSource::Cached {
                    path: "cache/one.audio".into(),
                    origin_url: "https://example.test/one.mp3".into(),
                    representation: Box::new(RepresentationReceipt {
                        byte_length: Some(4_096),
                        ..Default::default()
                    }),
                },
                facts: Box::new(FeedEpisodeFacts {
                    published: Some("2026-09-10".into()),
                    duration: Some("1:02:03".into()),
                    ..Default::default()
                }),
            })
            .unwrap();
        session
            .model
            .add_text_annotation(
                AnnotationId("n".into()),
                anchor("episode", 10),
                "x".into(),
                1,
            )
            .unwrap();

        let mut state = RedshankSurfaceState::default();
        session.project(
            &mut state,
            &PlaybackSnapshot::default(),
            &HostFacts::default(),
        );

        let row = state.item(&episode).unwrap();
        assert_eq!(row.source, SourceKind::Offline);
        assert_eq!(
            row.face,
            Face::Artwork("https://example.test/art.png".into())
        );
        assert_eq!(row.feed_title.as_deref(), Some("Field Notes"));
        assert_eq!(row.duration_ms, Some(3_723_000));
        assert_eq!(row.cached_bytes, Some(4_096));
        assert_eq!(row.note_count, 1);
        assert_eq!(row.published.as_deref(), Some("2026-09-10"));

        let feed = state.feed("https://example.test/feed.xml").unwrap();
        assert_eq!(feed.episode_count, 1);
        assert_eq!(feed.unplayed_count, 1);
        assert_eq!(feed.offline_bytes, 4_096);
        assert_eq!(feed.failure, None);
        assert_eq!(
            session.model.subscriptions["https://example.test/feed.xml"]
                .diagnostics
                .len(),
            1
        );
        assert_eq!(state.cache_used_bytes, 4_096);
    }

    #[test]
    fn ending_playback_reports_completed_and_closes_the_listening_session() {
        let mut session = session();
        let id = ItemId("a".into());
        session
            .select(id.clone(), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let mut playing = snapshot(session.token);
        playing.position_ms = 0;
        assert!(session.record_progress(&playing, 10));
        playing.position_ms = 30_000;
        session.record_progress(&playing, 20);
        let mut ended = playing.clone();
        ended.state = PlaybackState::Ended;
        ended.position_ms = 60_000;
        session.record_progress(&ended, 30);
        assert_eq!(session.model.listening_sessions.len(), 1);
        let recorded = &session.model.listening_sessions[0];
        assert_eq!((recorded.start_ms, recorded.stop_ms), (0, 60_000));
        assert!(recorded.completed);

        let mut state = RedshankSurfaceState::default();
        session.project(
            &mut state,
            &ended,
            &HostFacts {
                now_ms: 30,
                ..Default::default()
            },
        );
        assert_eq!(state.compact.transport, TransportState::Completed);
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.sessions[0].day_label, "TODAY");
        assert!(!state.sessions[0].active);
    }

    #[test]
    fn the_open_listening_session_is_projected_as_active() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let mut playing = snapshot(session.token);
        playing.position_ms = 5_000;
        session.record_progress(&playing, 100);
        let mut state = RedshankSurfaceState::default();
        session.project(
            &mut state,
            &playing,
            &HostFacts {
                now_ms: 200,
                ..Default::default()
            },
        );
        assert_eq!(state.sessions.len(), 1);
        assert!(state.sessions[0].active);
    }

    #[test]
    fn the_reaction_offset_moves_the_anchor_back_and_keeps_the_press() {
        let mut session = session();
        session.model.settings.reaction_offset_ms = 4_000;
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let mut snap = snapshot(session.token);
        snap.position_ms = 10_000;
        let anchor = session.capture(&snap).unwrap();
        assert_eq!(anchor.offset_ms, 6_000);
        assert_eq!(anchor.pressed_offset_ms, Some(10_000));
    }

    #[test]
    fn all_notes_filter_lists_every_item_s_annotations() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        for (note, item) in [("n-a", "a"), ("n-b", "b")] {
            session
                .model
                .add_text_annotation(AnnotationId(note.into()), anchor(item, 1), "x".into(), 1)
                .unwrap();
        }
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &snapshot(session.token), &HostFacts::default());
        assert_eq!(state.notes.len(), 1);
        state.notes_filter = NotesFilter::AllNotes;
        session.project(&mut state, &snapshot(session.token), &HostFacts::default());
        assert_eq!(state.notes.len(), 2);
    }

    #[test]
    fn day_labels_read_from_the_host_clock() {
        let today = 20_000 * DAY_MS + 3_600_000;
        assert_eq!(day_label(today, today), "TODAY");
        assert_eq!(day_label(today - DAY_MS, today), "YESTERDAY");
        assert_eq!(day_label(1_757_000_000_000, today), "4 SEP");
        assert_eq!(wall_clock(3_600_000 + 1_920_000), "01:32");
    }

    /// `resume_completed_from` is the only thing that moves the resume offset
    /// of a completed item; `Start` keeps the pass-1 behaviour.
    #[test]
    fn resume_completed_from_saved_reopens_where_the_listener_stopped() {
        let mut session = session();
        let id = ItemId("a".into());
        session
            .model
            .set_progress(
                &id,
                Progress {
                    position_ms: 2_040,
                    completed: true,
                    updated_at_ms: 1,
                },
            )
            .unwrap();
        session.model.settings.resume_completed_from = ResumeCompleted::Saved;
        let commands = session
            .select(id.clone(), &PlaybackSnapshot::default(), 2)
            .unwrap();
        let [PlaybackCommand::Load { resume_ms, .. }] = commands.as_slice() else {
            panic!("selection should issue exactly one load");
        };
        assert_eq!(*resume_ms, 2_040);
        session.model.settings.resume_completed_from = ResumeCompleted::Start;
        let commands = session.select(id, &PlaybackSnapshot::default(), 3).unwrap();
        let [PlaybackCommand::Load { resume_ms, .. }] = commands.as_slice() else {
            panic!("selection should issue exactly one load");
        };
        assert_eq!(*resume_ms, 0);
    }

    /// A completed item reopened at zero stays Completed until playback moves,
    /// which `Saved` must not undo for an item it reopens at zero.
    #[test]
    fn a_completed_item_reopened_at_zero_stays_completed() {
        let mut session = session();
        let id = ItemId("a".into());
        session.model.settings.resume_completed_from = ResumeCompleted::Saved;
        session
            .model
            .set_progress(
                &id,
                Progress {
                    position_ms: 0,
                    completed: true,
                    updated_at_ms: 1,
                },
            )
            .unwrap();
        session
            .select(id.clone(), &PlaybackSnapshot::default(), 2)
            .unwrap();
        let paused = PlaybackSnapshot {
            load_token: Some(session.token),
            position_ms: 0,
            state: PlaybackState::Paused,
            ..Default::default()
        };
        session.record_progress(&paused, 3);
        assert!(session.model.progress[&id].completed);
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &paused, &HostFacts::default());
        assert_eq!(state.compact.transport, TransportState::Completed);
    }

    /// The representation card's facts, and the pin, come off the model.
    #[test]
    fn item_rows_carry_the_cached_receipt_and_the_pin() {
        let mut session = session();
        let id = ItemId("a".into());
        let receipt = RepresentationReceipt {
            retrieved_at_ms: Some(1_757_635_200_000),
            complete_digest: Some("blake3:c41f9ab2deadbeef".into()),
            ..Default::default()
        };
        session
            .model
            .library
            .get_mut(&id)
            .unwrap()
            .replace_source(MediaSource::Cached {
                path: "cache/a.audio".into(),
                origin_url: "https://example.test/a.mp3".into(),
                representation: Box::new(receipt.clone()),
            });
        let mut matching = anchor("a", 100);
        matching.representation = receipt.clone();
        session
            .model
            .add_text_annotation(AnnotationId("n1".into()), matching, "note".into(), 1)
            .unwrap();
        session.model.toggle_pin(&id).unwrap();

        let mut state = RedshankSurfaceState::default();
        session.project(
            &mut state,
            &PlaybackSnapshot::default(),
            &HostFacts::default(),
        );
        assert!(
            state
                .items
                .iter()
                .find(|row| row.id == id)
                .unwrap()
                .representation
                .is_none()
        );
        session
            .select(id.clone(), &PlaybackSnapshot::default(), 3)
            .unwrap();
        let loaded = PlaybackSnapshot {
            representation: Some(receipt),
            ..snapshot(session.token)
        };
        session.project(&mut state, &loaded, &HostFacts::default());
        let row = state.items.iter().find(|row| row.id == id).unwrap();
        assert!(row.pinned);
        let summary = row.representation.clone().unwrap();
        assert_eq!(summary.retrieved_at_ms, Some(1_757_635_200_000));
        assert_eq!(summary.short_digest.as_deref(), Some("c41f9ab2"));
        assert_eq!(summary.matches, Some(true));

        // A note frozen against a different object reads as drift.
        let mut drifted = anchor("a", 200);
        drifted.representation = RepresentationReceipt {
            complete_digest: Some("blake3:0000000000000000".into()),
            ..Default::default()
        };
        session
            .model
            .add_text_annotation(AnnotationId("n2".into()), drifted, "note".into(), 2)
            .unwrap();
        session.project(&mut state, &loaded, &HostFacts::default());
        let row = state.items.iter().find(|row| row.id == id).unwrap();
        assert_eq!(row.representation.clone().unwrap().matches, Some(false));
    }

    fn capture_context() -> std::sync::Arc<redshank_playback::FingerprintContext> {
        std::sync::Arc::new(redshank_playback::FingerprintContext {
            start_ms: 100_000,
            fingerprint: redshank_model::AudioFingerprint {
                version: 1,
                frame_ms: 100,
                anchor_offset_ms: 25_000,
                frames: (0..250).map(|index| [index as u8; 16]).collect(),
            },
        })
    }

    #[test]
    fn capture_fingerprint_uses_source_anchor_after_reaction_and_excludes_decode_future() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        session.model.settings.reaction_offset_ms = 2_345;
        session.model.settings.alignment_window_ms = 5_000;
        let context = capture_context();
        let before = context.clone();
        let snap = PlaybackSnapshot {
            position_ms: 122_390,
            fingerprint_context: Some(context),
            ..snapshot(session.token)
        };
        let captured = session.capture(&snap).unwrap();
        assert_eq!(captured.offset_ms, 120_045);
        assert_eq!(captured.pressed_offset_ms, Some(122_390));
        let fingerprint = captured.fingerprint.unwrap();
        assert_eq!(fingerprint.anchor_offset_ms, 5_045);
        assert_eq!(fingerprint.frames, before.fingerprint.frames[150..200]);
        assert_eq!(fingerprint.reference_duration_ms(), 5_000);
        assert!(fingerprint.validate().is_ok());
        assert_eq!(snap.fingerprint_context.as_ref().unwrap(), &before);
    }

    #[test]
    fn capture_fingerprint_is_optional_when_disabled_missing_or_insufficient() {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let context = capture_context();
        for (label, position_ms, window_ms, context) in [
            ("disabled", 120_000, 0, Some(context.clone())),
            (
                "less than three seconds",
                102_999,
                20_000,
                Some(context.clone()),
            ),
            (
                "before retained context",
                99_999,
                20_000,
                Some(context.clone()),
            ),
            (
                "past decoded context",
                125_100,
                20_000,
                Some(context.clone()),
            ),
            ("legacy or unavailable context", 120_000, 20_000, None),
        ] {
            session.model.settings.alignment_window_ms = window_ms;
            let snap = PlaybackSnapshot {
                position_ms,
                fingerprint_context: context,
                ..snapshot(session.token)
            };
            let captured = session.capture(&snap).unwrap();
            assert!(captured.fingerprint.is_none(), "{label}");
            assert_eq!(captured.offset_ms, position_ms, "{label}");
        }
    }

    fn aligned_session() -> (Session, AnnotationId, PlaybackSnapshot) {
        let mut session = session();
        session
            .select(ItemId("a".into()), &PlaybackSnapshot::default(), 1)
            .unwrap();
        let id = AnnotationId("aligned".into());
        let original = CaptureAnchor {
            item_id: ItemId("a".into()),
            offset_ms: 60_000,
            end_offset_ms: Some(70_000),
            pressed_offset_ms: Some(63_000),
            representation: RepresentationReceipt::default(),
            fingerprint: Some(redshank_model::AudioFingerprint {
                version: 1,
                frame_ms: 100,
                anchor_offset_ms: 20_000,
                frames: vec![[17; 16]; 200],
            }),
        };
        session
            .model
            .add_text_annotation(id.clone(), original.clone(), "original".into(), 1)
            .unwrap();
        let destination = RepresentationReceipt {
            final_url: Some("https://example.test/destination.mp3".into()),
            etag: Some("\"destination\"".into()),
            complete_digest: Some(format!("blake3:{}", "b".repeat(64))),
            ..Default::default()
        };
        session
            .model
            .store_derived_position(
                &id,
                redshank_model::DerivedPosition {
                    original_target: original,
                    destination: destination.clone(),
                    offset_ms: 90_000,
                    algorithm_version: 1,
                    confidence_per_mille: 980,
                    runner_up_per_mille: 700,
                    reference_duration_ms: 20_000,
                },
            )
            .unwrap();
        let snapshot = PlaybackSnapshot {
            representation: Some(destination),
            ..snapshot(session.token)
        };
        (session, id, snapshot)
    }

    #[test]
    fn aligned_open_returns_a_separate_point_without_rewriting_original_span() {
        let (session, id, snap) = aligned_session();
        let before = session.model.annotations[&id].clone();
        let derived = session.model.derived_positions[&id].clone();
        let target = session.aligned_target(&id, &snap).unwrap();
        assert_eq!(target.offset_ms, 90_000);
        assert_eq!(target.item_id, before.target.item_id);
        assert_eq!(target.representation, derived.destination);
        assert!(target.end_offset_ms.is_none());
        assert!(target.pressed_offset_ms.is_none());
        assert!(target.fingerprint.is_none());
        assert_eq!(session.model.annotations[&id], before);
        assert_eq!(session.model.derived_positions[&id], derived);
    }

    #[test]
    fn aligned_open_requires_a_ready_current_selection_and_destination_digest() {
        let (mut session, id, snap) = aligned_session();
        for state in [
            PlaybackState::Empty,
            PlaybackState::Loading,
            PlaybackState::Unavailable("failed".into()),
        ] {
            let unavailable = PlaybackSnapshot {
                state,
                ..snap.clone()
            };
            assert!(session.aligned_target(&id, &unavailable).is_err());
        }
        let old = PlaybackSnapshot {
            load_token: Some(session.token + 1),
            ..snap.clone()
        };
        assert!(session.aligned_target(&id, &old).is_err());
        for digest in [None, Some(format!("blake3:{}", "c".repeat(64)))] {
            let mut changed = snap.clone();
            changed.representation.as_mut().unwrap().complete_digest = digest;
            // Identical URL and strong ETag cannot substitute for the actual
            // destination digest when opening a downloaded-copy alignment.
            assert!(session.aligned_target(&id, &changed).is_err());
        }
        session.selected = Some(ItemId("b".into()));
        assert!(session.aligned_target(&id, &snap).is_err());
        session.selected = None;
        assert!(session.aligned_target(&id, &snap).is_err());
    }

    #[test]
    fn aligned_open_rechecks_current_threshold_and_original_target() {
        let (mut session, id, snap) = aligned_session();
        session.model.settings.alignment_min_confidence_per_mille = 990;
        assert!(session.aligned_target(&id, &snap).is_err());
        session.model.settings.alignment_min_confidence_per_mille = 940;
        session
            .model
            .annotations
            .get_mut(&id)
            .unwrap()
            .target
            .offset_ms += 1;
        assert!(session.aligned_target(&id, &snap).is_err());
        session.model.delete_annotation(&id).unwrap();
        assert!(session.aligned_target(&id, &snap).is_err());
    }

    #[test]
    fn aligned_open_revalidates_saved_evidence() {
        let (mut session, id, snap) = aligned_session();
        let valid = session.model.derived_positions[&id].clone();
        let mut invalids = Vec::new();
        let mut changed = valid.clone();
        changed.algorithm_version = 2;
        invalids.push(changed);
        let mut changed = valid.clone();
        changed.confidence_per_mille = 1_001;
        invalids.push(changed);
        let mut changed = valid.clone();
        changed.runner_up_per_mille = 979;
        invalids.push(changed);
        let mut changed = valid.clone();
        changed.reference_duration_ms = 19_999;
        invalids.push(changed);
        let mut changed = valid.clone();
        changed.offset_ms = session.model.settings.alignment_max_search_ms + 1;
        invalids.push(changed);
        for invalid in invalids {
            // Simulate a restored map: its values have not passed through the
            // producer's store_derived_position validation this session.
            session.model.derived_positions.insert(id.clone(), invalid);
            assert!(session.aligned_target(&id, &snap).is_err());
        }
    }
}
