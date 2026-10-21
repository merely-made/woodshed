//! Failed restoration never grants write authority over the original store.
//! Recovery copies are independent snapshots, not original-store save replies.

use super::*;

#[derive(Default)]
pub(crate) struct Recovery {
    pub close_requested: bool,
    pub copying: bool,
    pub notice: Option<String>,
    pub exit_requested: bool,
    snapshot: Option<Snapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn native_close_hook_and_recovery_controls_keep_the_appearance_barrier() {
        for request in [
            cambium_genet_winit_host::CloseRequest::Native,
            cambium_genet_winit_host::CloseRequest::Command,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let desktop = Rc::new(RefCell::new(crate::tests::recovery_desktop(
                directory.path(),
            )));
            desktop.borrow_mut().persistence.changed();
            let mut state = DesktopState {
                surface: Default::default(),
                appearance: appearance::AppearanceState::load(
                    directory.path().join("appearance.json"),
                    directory.path().join("themes.json"),
                    Vec::new(),
                ),
            };
            desktop.borrow().project_recovery(&mut state.surface);
            let sheet = state
                .appearance
                .take_stylesheet_change(&state.surface)
                .unwrap();
            let mut host = cambium_genet_winit_host::Harness::with_hooks(
                Init {
                    state,
                    logic: appearance::root as Logic,
                    sheet,
                    fonts: Vec::new(),
                    images: Vec::new(),
                },
                hooks(Rc::clone(&desktop), Rc::new(RefCell::new(None))),
            );
            host.layout_at(860.0, 680.0);
            host.request_close(request);
            host.relayout();
            assert!(!host.close_requested());
            assert!(
                host.state()
                    .surface
                    .recovery
                    .as_ref()
                    .unwrap()
                    .close_requested
            );
            assert!(host.click_on(
                &taproot::Selector::role("button").with_attr("aria-label", "Keep Working")
            ));
            assert!(!desktop.borrow().recovery.close_requested);
            host.request_close(request);
            host.update(|state| {
                state.appearance.begin_edit(&state.surface).unwrap();
            });
            // A queued explicit recovery decision cannot bypass a newly dirty
            // appearance editor at the final native exit boundary.
            host.update(|state| state.surface.request(CompactCommand::ExitWithoutSaving));
            host.after_dispatch();
            host.prepare_frame();
            assert!(!host.close_requested());
            assert!(host.state().appearance.editor_open);
            assert!(!desktop.borrow().closing);
            assert!(desktop.borrow().persistence.requests.is_none());
        }
    }

    #[test]
    fn invalid_span_draft_refuses_copy_without_consuming_the_editor() {
        let directory = tempfile::tempdir().unwrap();
        let mut desktop = crate::tests::recovery_desktop(directory.path());
        let mut state = RedshankSurfaceState::default();
        state.text_capture = Some(TextCapture {
            anchor: CaptureAnchor {
                item_id: ItemId("draft".into()),
                offset_ms: 100,
                end_offset_ms: None,
                pressed_offset_ms: None,
                representation: Default::default(),
                fingerprint: None,
            },
            draft: String::new(),
            end_offset_ms: Some(99),
        });
        state.set_text_draft("retain this invalid span for correction");
        let capture = state.text_capture.clone();
        desktop.recovery.copying = true;
        assert!(
            desktop
                .start_recovery_copy(&mut state, directory.path().join("copy"))
                .is_err()
        );
        assert_eq!(state.text_capture, capture);
        assert_eq!(
            state.text_editor.text(),
            "retain this invalid span for correction"
        );
        assert!(desktop.session.model.annotations.is_empty());
        assert!(!directory.path().join("copy").exists());
    }

    fn settle(desktop: &mut Desktop, state: &mut RedshankSurfaceState) {
        for _ in 0..1000 {
            desktop.poll(state);
            if !desktop.recovery.copying {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("recovery worker did not complete");
    }

    #[test]
    fn recovery_copy_contains_draft_and_newer_editor_text_prevents_exit() {
        for end in [None, Some(400)] {
            let parent = tempfile::tempdir().unwrap();
            let original = parent.path().join("original");
            fs::create_dir(&original).unwrap();
            let mut desktop = crate::tests::recovery_desktop(&original);
            let id = ItemId("episode".into());
            desktop
                .session
                .model
                .add_item(LibraryItem::DirectAudio {
                    id: id.clone(),
                    title: "Episode".into(),
                    source: MediaSource::Enclosure {
                        url: "https://example.test/episode.mp3".into(),
                    },
                })
                .unwrap();
            let anchor = CaptureAnchor {
                item_id: id,
                offset_ms: 100,
                end_offset_ms: None,
                pressed_offset_ms: None,
                representation: Default::default(),
                fingerprint: None,
            };
            let mut state = RedshankSurfaceState::default();
            state.text_capture = Some(TextCapture {
                anchor: anchor.clone(),
                draft: String::new(),
                end_offset_ms: end,
            });
            state.set_text_draft("captured draft");
            desktop.close(&mut state).unwrap();
            desktop.recovery.copying = true;
            let copy = parent.path().join("copy");
            desktop
                .start_recovery_copy(&mut state, copy.clone())
                .unwrap();
            state.set_text_draft("newer draft");
            settle(&mut desktop, &mut state);
            let saved = JsonDirectoryStore::new(copy).load().unwrap().unwrap();
            let note = saved.annotations.values().next().unwrap();
            let mut saved_anchor = anchor.clone();
            saved_anchor.end_offset_ms = end;
            assert_eq!(note.target, saved_anchor);
            assert_eq!(state.text_capture.as_ref().unwrap().anchor, anchor);
            assert!(desktop.session.model.annotations.is_empty());
            assert_eq!(
                note.body,
                NoteBody::Text {
                    plain_text: "captured draft".into()
                }
            );
            assert_eq!(state.text_editor.text(), "newer draft");
            assert!(!desktop.ready_to_close());
            assert!(desktop.recovery.close_requested);
            assert!(
                desktop
                    .recovery
                    .notice
                    .as_ref()
                    .unwrap()
                    .contains("Newer changes")
            );
            assert!(desktop.persistence.requests.is_none());
            assert_eq!(desktop.persistence.durable, 0);
        }
    }

    #[test]
    fn recovery_copy_completion_cannot_discard_a_newer_model_revision() {
        let directory = tempfile::tempdir().unwrap();
        let mut desktop = crate::tests::recovery_desktop(directory.path());
        let state = RedshankSurfaceState::default();
        desktop.persistence.changed();
        desktop.recovery.close_requested = true;
        desktop.recovery.copying = true;
        desktop.recovery.snapshot = Some(Snapshot {
            revision: 1,
            draft: None,
        });
        desktop.session.model.settings.playback_rate_percent = 125;
        desktop.persistence.changed();
        desktop.finish_recovery_copy(&state, Ok(directory.path().join("copy")));
        assert!(!desktop.ready_to_close());
        assert_eq!(desktop.session.model.settings.playback_rate_percent, 125);
        assert!(
            desktop
                .recovery
                .notice
                .as_ref()
                .unwrap()
                .contains("Newer changes")
        );
    }

    #[test]
    fn recovery_discard_does_not_admit_queued_media_removals_or_forged_save_replies() {
        let directory = tempfile::tempdir().unwrap();
        let mut desktop = crate::tests::recovery_desktop(directory.path());
        let media = directory.path().join("cached.audio");
        fs::write(&media, b"retained cached media").unwrap();
        let digest = "a".repeat(64);
        fs::create_dir(directory.path().join("voice")).unwrap();
        let voice = directory.path().join("voice").join(format!("{digest}.wav"));
        fs::write(&voice, b"retained voice media").unwrap();
        desktop.persistence.changed();
        desktop.cache_removals.push(PendingCacheRemoval {
            revision: 1,
            id: ItemId("removed".into()),
            path: media.clone(),
        });
        desktop.voice_removals.push(PendingVoiceRemoval {
            revision: 1,
            blob_id: format!("voice:{digest}"),
        });
        let mut state = RedshankSurfaceState::default();
        desktop
            .persistence
            .reply_sender
            .send(IoReply::Saved {
                revision: 1,
                result: Ok(()),
                execution: None,
            })
            .unwrap();
        desktop.poll(&mut state);
        assert_eq!(desktop.cache_removals.len(), 1);
        assert_eq!(desktop.voice_removals.len(), 1);
        assert!(!desktop.scenario_busy());
        desktop.close(&mut state).unwrap();
        desktop
            .command(&mut state, CompactCommand::ExitWithoutSaving)
            .unwrap();
        desktop.poll(&mut state);
        assert!(desktop.ready_to_close());
        assert_eq!(fs::read(media).unwrap(), b"retained cached media");
        assert_eq!(fs::read(voice).unwrap(), b"retained voice media");
        assert_eq!(desktop.persistence.durable, 0);
    }
}

struct Snapshot {
    revision: u64,
    draft: Option<(TextCapture, String)>,
}

impl Desktop {
    fn recovery_dirty(&self, state: &RedshankSurfaceState) -> bool {
        self.persistence.revision != 0
            || self.voice_session.is_some()
            || state.text_capture.is_some() && !state.text_editor.text().trim().is_empty()
    }

    fn recovery_work_pending(&self) -> bool {
        self.dialog_pending
            || self.cache_pending.is_some()
            || self.cache_removals_in_flight != 0
            || self.voice_removals_in_flight != 0
            || self.feed_pending.is_some()
            || self.transcript_pending.is_some()
            || self.alignment_pending.is_some()
            || self.export_pending
            || self.pending_note_open.is_some()
            || self.pending_note_seek.is_some()
    }

    pub(crate) fn project_recovery(&self, state: &mut RedshankSurfaceState) {
        state.recovery = self.persistence.recovery_error.as_ref().map(|error| {
            redshank_surfaces::RecoveryState {
                error: error.clone(),
                dirty: self.recovery_dirty(state),
                close_requested: self.recovery.close_requested,
                copying: self.recovery.copying,
                copy_notice: self.recovery.notice.clone(),
            }
        });
    }

    pub(crate) fn close_recovery(&mut self, state: &RedshankSurfaceState) -> Result<(), String> {
        if self.recovery.copying {
            return Err("Wait for the recovery copy to finish before closing".into());
        }
        self.recovery.close_requested = true;
        if !self.recovery_dirty(state) && !self.recovery_work_pending() {
            self.send(PlaybackCommand::Pause)?;
            self.recovery.exit_requested = true;
            self.closing = true;
        }
        Ok(())
    }

    pub(crate) fn keep_recovery(&mut self) -> Result<(), String> {
        self.require_recovery_action()?;
        self.recovery.close_requested = false;
        self.recovery.exit_requested = false;
        self.closing = false;
        Ok(())
    }

    fn require_recovery_action(&self) -> Result<(), String> {
        if self.persistence.recovery_error.is_none() {
            return Err("This session is not in listener recovery".into());
        }
        if self.recovery.copying || self.recovery.exit_requested {
            return Err("Wait for the recovery operation to finish".into());
        }
        Ok(())
    }

    pub(crate) fn discard_recovery(
        &mut self,
        state: &mut RedshankSurfaceState,
    ) -> Result<(), String> {
        self.require_recovery_action()?;
        if !self.recovery.close_requested {
            return Err("Close Redshank before choosing Exit Without Saving".into());
        }
        if self.recovery_work_pending() {
            return Err("Wait for pending listener work before exiting without saving".into());
        }
        self.send(PlaybackCommand::Pause)?;
        if self.voice_session.is_some() {
            self.voice_capture.cancel_capture()?;
            self.voice_session = None;
            state.compact.voice_capture_active = false;
        }
        // Explicit discard does not call save_note, flush, or any media removal.
        self.recovery.exit_requested = true;
        self.closing = true;
        Ok(())
    }

    pub(crate) fn choose_recovery_copy(
        &mut self,
        state: &RedshankSurfaceState,
    ) -> Result<(), String> {
        self.require_recovery_action()?;
        if self.recovery_work_pending() || self.voice_session.is_some() {
            return Err("Finish or cancel the active capture and wait for pending listener work before copying".into());
        }
        if !self.recovery_dirty(state) {
            return Err("There are no listener changes to copy".into());
        }
        self.recovery.copying = true;
        self.recovery.notice = None;
        let replies = self.persistence.reply_sender.clone();
        thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_title("Save Recovery Copy — choose a new directory name")
                .set_file_name("Redshank Recovery")
                .save_file();
            let _ = replies.send(IoReply::RecoveryPicked(path));
        });
        Ok(())
    }

    pub(crate) fn start_recovery_copy(
        &mut self,
        state: &mut RedshankSurfaceState,
        path: PathBuf,
    ) -> Result<(), String> {
        if self.persistence.recovery_error.is_none() || !self.recovery.copying {
            return Err("No recovery copy was requested".into());
        }
        if self.recovery_work_pending() || self.voice_session.is_some() {
            return Err(
                "Listener work is still pending; try the recovery copy again when it finishes"
                    .into(),
            );
        }
        self.send(PlaybackCommand::Pause)?;
        if self
            .session
            .record_progress(&self.runtime.snapshot(), now_ms())
            | self.session.close_listening(now_ms(), false)
        {
            self.persistence.changed();
        }
        let mut model = self.session.model.clone();
        if let Some(capture) = &state.text_capture {
            let text = state.text_editor.text().to_owned();
            if !text.trim().is_empty() {
                let result = if let Some(id) = &state.editing_note {
                    if model
                        .annotations
                        .get(id)
                        .is_none_or(|note| note.target != capture.anchor)
                    {
                        return Err(
                            "The edited note's target changed; its draft was retained".into()
                        );
                    }
                    model.update_text_annotation(id, text)
                } else if let Some(end) = capture.end_offset_ms {
                    model.add_span_annotation(
                        self.next_annotation_id(),
                        capture.anchor.clone(),
                        end,
                        text,
                        now_ms(),
                    )
                } else {
                    model.add_text_annotation(
                        self.next_annotation_id(),
                        capture.anchor.clone(),
                        text,
                        now_ms(),
                    )
                };
                result.map_err(|error| format!("Could not copy note draft: {error:?}"))?;
            }
        }
        self.recovery.snapshot = Some(Snapshot {
            revision: self.persistence.revision,
            draft: state
                .text_capture
                .clone()
                .map(|capture| (capture, state.text_editor.text().to_owned())),
        });
        let source = self.data_root.clone();
        let replies = self.persistence.reply_sender.clone();
        thread::spawn(move || {
            let result = JsonDirectoryStore::new(source)
                .save_recovery_copy(&path, &model)
                .map_err(|error| format!("Recovery copy failed: {error:?}. The original store is unchanged; choose a new directory to retry."));
            let _ = replies.send(IoReply::RecoveryCopied(result));
        });
        Ok(())
    }

    pub(crate) fn finish_recovery_copy(
        &mut self,
        state: &RedshankSurfaceState,
        result: Result<PathBuf, String>,
    ) {
        self.recovery.copying = false;
        let Some(snapshot) = self.recovery.snapshot.take() else {
            return;
        };
        match result {
            Err(error) => self.recovery.notice = Some(error),
            Ok(path) => {
                let draft = state
                    .text_capture
                    .clone()
                    .map(|capture| (capture, state.text_editor.text().to_owned()));
                let unchanged =
                    self.persistence.revision == snapshot.revision && draft == snapshot.draft;
                self.recovery.notice = Some(format!(
                    "Recovery copy saved to {}. Reopen it using REDSHANK_DATA_DIR.{}",
                    path.display(),
                    if unchanged {
                        " The original store remains protected."
                    } else {
                        " Newer changes remain in memory; save another copy before exiting."
                    },
                ));
                if unchanged
                    && self.recovery.close_requested
                    && !self.recovery_work_pending()
                    && self.voice_session.is_none()
                {
                    self.recovery.exit_requested = true;
                    self.closing = true;
                }
            },
        }
    }

    pub(crate) fn ready_to_close(&self) -> bool {
        if !self.closing || self.receipt.as_ref().is_some_and(HeadedReceipt::active) {
            return false;
        }
        if self.persistence.recovery_error.is_some() {
            // Queued removals were never admitted by original-store durability.
            return self.recovery.exit_requested
                && !self.recovery.copying
                && !self.recovery_work_pending()
                && self.voice_session.is_none();
        }
        self.transcript_pending.is_none()
            && self.persistence.durable == self.persistence.revision
            && self.cache_removals.is_empty()
            && self.cache_removals_in_flight == 0
            && self.voice_removals.is_empty()
            && self.voice_removals_in_flight == 0
    }
}
