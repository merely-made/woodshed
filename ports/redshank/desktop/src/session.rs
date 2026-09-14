//! Product session authority. Playback snapshots only belong to a selection
//! when their load token matches; queue order is independent of that selection.
use redshank_model::{CaptureAnchor, ItemId, NoteBody, Progress, RedshankModel};
use redshank_playback::{PlaybackCommand, PlaybackSnapshot, PlaybackState, PreviewState};
use redshank_surfaces::{
    NoteSummary, NoteSummaryBody, NowPlaying, RedshankSurfaceState, TransportState,
    VoiceNotePreview,
};

pub struct Session {
    pub model: RedshankModel,
    pub selected: Option<ItemId>,
    pub token: u64,
}

impl Session {
    pub fn new(model: RedshankModel) -> Self {
        Self {
            model,
            selected: None,
            token: 0,
        }
    }

    pub fn matches(&self, snapshot: &PlaybackSnapshot) -> bool {
        self.selected.is_some() && snapshot.load_token == Some(self.token)
    }

    pub fn record_progress(&mut self, snapshot: &PlaybackSnapshot, now: u64) -> bool {
        if !self.matches(snapshot)
            || !matches!(
                snapshot.state,
                PlaybackState::Playing | PlaybackState::Paused | PlaybackState::Ended
            )
        {
            return false;
        }
        let id = self.selected.as_ref().expect("matched selection");
        let completed = snapshot.state == PlaybackState::Ended;
        if self
            .model
            .progress
            .get(id)
            .is_some_and(|p| p.position_ms == snapshot.position_ms && p.completed == completed)
        {
            return false;
        }
        self.model.progress.insert(
            id.clone(),
            Progress {
                position_ms: snapshot.position_ms,
                completed,
                updated_at_ms: now,
            },
        );
        true
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
        self.token = self
            .token
            .checked_add(1)
            .ok_or("Playback identity exhausted")?;
        self.selected = Some(id.clone());
        self.model.selected_item = Some(id.clone());
        let resume = self
            .model
            .progress
            .get(&id)
            .filter(|progress| !progress.completed)
            .map_or(0, |progress| progress.position_ms);
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
        Ok(CaptureAnchor {
            item_id: self.selected.clone().expect("matched selection"),
            offset_ms: snapshot.position_ms,
            representation: snapshot
                .representation
                .clone()
                .ok_or("The loaded recording has no representation receipt")?,
        })
    }

    /// Update the projection without replacing its editor, focus, or notices.
    pub fn project(&self, state: &mut RedshankSurfaceState, snapshot: &PlaybackSnapshot) {
        state.library = self.model.library.values().cloned().collect();
        state.subscriptions = self.model.subscriptions.values().cloned().collect();
        state.queue = self.model.queue.clone();
        state.settings = self.model.settings.clone();
        state.compact.skip_backward_ms = self.model.settings.skip_backward_ms;
        state.compact.skip_forward_ms = self.model.settings.skip_forward_ms;
        let item = self
            .selected
            .as_ref()
            .and_then(|id| self.model.library.get(id));
        let matched = self.matches(snapshot);
        state.compact.transport = if item.is_none() {
            TransportState::Empty
        } else if !matched {
            TransportState::Buffering
        } else {
            match &snapshot.state {
                PlaybackState::Empty | PlaybackState::Loading => TransportState::Buffering,
                PlaybackState::Playing => TransportState::Playing,
                PlaybackState::Paused | PlaybackState::Ended => TransportState::Paused,
                PlaybackState::Unavailable(error) => TransportState::Unavailable(error.clone()),
            }
        };
        state.compact.now_playing = item.map(|item| NowPlaying {
            item_id: item.id().clone(),
            title: item.title().to_owned(),
            position_ms: if matched { snapshot.position_ms } else { 0 },
            duration_ms: if matched { snapshot.duration_ms } else { None },
        });
        state.notes = item
            .map(|item| {
                self.model
                    .annotations_for_item(item.id())
                    .into_iter()
                    .map(|note| NoteSummary {
                        id: note.id.clone(),
                        offset_ms: note.target.offset_ms,
                        body: match &note.body {
                            NoteBody::Text { plain_text } => {
                                NoteSummaryBody::Text(plain_text.clone())
                            },
                            NoteBody::Audio { duration_ms, .. } => {
                                let preview = snapshot
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
                                    .unwrap_or_default();
                                NoteSummaryBody::Voice {
                                    duration_ms: *duration_ms,
                                    preview,
                                }
                            },
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redshank_model::{LibraryItem, MediaSource, RepresentationReceipt};
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
        session.project(&mut state, &old);
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
        });
        state.set_text_draft("keep");
        session.model.reorder_queue(0, 1).unwrap();
        session.project(&mut state, &snap);
        assert_eq!(
            state.compact.now_playing.as_ref().unwrap().item_id,
            ItemId("a".into())
        );
        session.select(ItemId("b".into()), &snap, 2).unwrap();
        session.project(&mut state, &snapshot(session.token));
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
            .add_annotation(redshank_model::Annotation {
                id: redshank_model::AnnotationId("voice".into()),
                target: CaptureAnchor {
                    item_id: id,
                    offset_ms: 456,
                    representation: Default::default(),
                },
                body: NoteBody::Audio {
                    blob_id: "voice:abc".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 1_500,
                },
                created_at_ms: 2,
            })
            .unwrap();
        let mut state = RedshankSurfaceState::default();
        session.project(&mut state, &snapshot(session.token));
        assert_eq!(
            state.notes[0].body,
            NoteSummaryBody::Voice {
                duration_ms: 1_500,
                preview: VoiceNotePreview::Idle,
            }
        );
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
                .add_annotation(redshank_model::Annotation {
                    id: redshank_model::AnnotationId(note_id.into()),
                    target: CaptureAnchor {
                        item_id: item_id.clone(),
                        offset_ms: 456,
                        representation: Default::default(),
                    },
                    body: NoteBody::Audio {
                        blob_id: format!("voice:{note_id}"),
                        media_type: "audio/wav".into(),
                        duration_ms: 1_500,
                    },
                    created_at_ms: 2,
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
        session.project(&mut state, &snapshot);
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
}
