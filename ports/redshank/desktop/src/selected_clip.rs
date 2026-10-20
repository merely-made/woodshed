// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! Owner-side selected-note disclosure. No recording acquisition or model writes.
use fleece::portable_clip::{CaptureContext, ClipLimits};
use redshank_model::{AnnotationId, NoteBody, RedshankModel};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

const MAX_BODY: usize = 512 * 1024;
pub(super) struct SelectedNote {
    export: serde_json::Value,
    voice: Option<(PathBuf, String)>,
    context: CaptureContext,
}

impl SelectedNote {
    pub(super) fn prepare(
        root: &Path,
        model: &RedshankModel,
        id: &AnnotationId,
    ) -> Result<Self, String> {
        let export = model
            .export_selected_note(id)
            .map_err(|e| format!("Selected-note export refused: {e:?}"))?;
        let note = model
            .annotations
            .get(id)
            .ok_or("The selected note is unavailable")?;
        let voice = match &note.body {
            NoteBody::Text { .. } => None,
            NoteBody::Audio {
                blob_id,
                media_type,
                ..
            } => {
                if media_type != "audio/wav" {
                    return Err("This note needs an exporter for its audio format".into());
                }
                let path = crate::voice::LocalVoiceCapture::blob_path(root, blob_id)?;
                let digest = blob_id
                    .strip_prefix("voice:")
                    .ok_or("Voice body identity is unavailable")?
                    .to_ascii_lowercase();
                Some((path, digest))
            },
        };
        let wire = serde_json::to_vec(&export).map_err(|e| e.to_string())?;
        let context = CaptureContext {
            source: eidetic::clip_profile::ClipSourceV1 {
                owner: None,
                entity: id.0.clone(),
                revision: None,
            },
            capture_id: format!("redshank:{}", blake3::hash(&wire).to_hex()),
            provenance: eidetic::ProvenanceRecord::self_imported(
                "explicit Redshank selected-note export",
            ),
            trust: eidetic::TrustEnvelope::self_asserted(),
            captured_at: eidetic::Timestamp(crate::now_ms()),
        };
        Ok(Self {
            export,
            voice,
            context,
        })
    }

    pub(super) fn write(self, path: &Path) -> Result<(), String> {
        let body = self
            .voice
            .map(|(path, expected)| {
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .map_err(|e| e.to_string())?
                    .take((MAX_BODY + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > MAX_BODY || blake3::hash(&bytes).to_hex().as_str() != expected {
                    return Err(
                        "The selected voice body is changed or exceeds the clip budget".to_string(),
                    );
                }
                Ok(bytes)
            })
            .transpose()?;
        let limits = ClipLimits {
            max_parts: 16,
            max_bytes: MAX_BODY as u64,
            max_record_bytes: 64 * 1024,
        };
        let package = pollster::block_on(async {
            let mut memory = eidetic::MemoryBackend::new();
            let id = fleece::portable_clip::retain_redshank_export(
                &mut memory,
                None,
                &self.export,
                body,
                &self.context,
                limits,
            )
            .await?;
            fleece::portable_clip::export_selected_clip(
                &mut memory,
                None,
                id,
                &["body".into()],
                limits,
            )
            .await
        })
        .map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("Choose a clip destination")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer(temporary.as_file_mut(), &package).map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist_noclobber(path)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl crate::Desktop {
    pub(super) fn export_note_clip(
        &mut self,
        state: &mut redshank_surfaces::RedshankSurfaceState,
        id: &AnnotationId,
    ) -> Result<(), String> {
        if self.export_pending {
            return Err("A note export is still running".into());
        }
        let prepared = SelectedNote::prepare(&self.data_root, &self.session.model, id)?;
        let path = self.data_root.join("exports").join(format!(
            "note-{}-{}.mereclip",
            blake3::hash(id.0.as_bytes()).to_hex(),
            crate::now_ms()
        ));
        let sender = self.persistence.reply_sender.clone();
        std::thread::Builder::new()
            .name("redshank-clip-export".into())
            .spawn(move || {
                let result = prepared.write(&path);
                let _ = sender.send(crate::IoReply::Exported { path, result });
            })
            .map_err(|e| e.to_string())?;
        self.export_pending = true;
        state.notice = Some("Exporting selected note…".into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redshank_model::*;

    fn voice_note(root: &Path) -> (RedshankModel, AnnotationId) {
        let body = crate::voice::write_blob(&root.join("voice"), 48_000, &[0.1; 4_800]).unwrap();
        let mut model = RedshankModel::default();
        let item = ItemId("source-recording".into());
        model
            .add_item(LibraryItem::DirectAudio {
                id: item.clone(),
                title: "Source recording".into(),
                source: MediaSource::Enclosure {
                    url: "https://unavailable.invalid/full-recording.wav".into(),
                },
            })
            .unwrap();
        let id = AnnotationId("selected-voice".into());
        model
            .add_annotation(Annotation {
                id: id.clone(),
                target: TimedTarget {
                    item_id: item,
                    offset_ms: 1250,
                    end_offset_ms: Some(4500),
                    pressed_offset_ms: Some(1400),
                    fingerprint: None,
                    representation: RepresentationReceipt::default(),
                },
                body,
                created_at_ms: 7,
                privacy: NotePrivacy::Shareable,
            })
            .unwrap();
        (model, id)
    }

    #[test]
    fn selected_voice_uses_owned_blob_and_preserves_millisecond_target() {
        let root = tempfile::tempdir().unwrap();
        let (model, id) = voice_note(root.path());
        let path = root.path().join("exports/selected.mereclip");
        SelectedNote::prepare(root.path(), &model, &id)
            .unwrap()
            .write(&path)
            .unwrap();
        let package: fleece::portable_clip::ClipPackageV1 =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(package.payloads.len(), 1);
        assert_eq!(package.clip.parts.len(), 1);
        assert_eq!(package.clip.parts[0].media_type, "audio/wav");
        assert_eq!(package.clip.anchors[0].units, "milliseconds");
        assert_eq!(package.clip.anchors[0].selector["offset_ms"], 1250);
        assert_eq!(package.clip.anchors[0].selector["pressed_offset_ms"], 1400);
        let NoteBody::Audio { blob_id, .. } = &model.annotations.get(&id).unwrap().body else {
            panic!("audio")
        };
        assert_eq!(
            package.payloads[0].bytes,
            std::fs::read(
                crate::voice::LocalVoiceCapture::blob_path(root.path(), blob_id).unwrap()
            )
            .unwrap()
        );
        let before = std::fs::read(&path).unwrap();
        assert!(
            SelectedNote::prepare(root.path(), &model, &id)
                .unwrap()
                .write(&path)
                .is_err()
        );
        assert_eq!(
            std::fs::read(path).unwrap(),
            before,
            "an existing export is never replaced"
        );
    }

    #[test]
    fn private_missing_and_changed_voice_are_refused_without_output() {
        let root = tempfile::tempdir().unwrap();
        let (mut model, id) = voice_note(root.path());
        model.annotations.get_mut(&id).unwrap().privacy = NotePrivacy::Private;
        assert!(SelectedNote::prepare(root.path(), &model, &id).is_err());
        model.annotations.get_mut(&id).unwrap().privacy = NotePrivacy::Shareable;
        let NoteBody::Audio { blob_id, .. } = &model.annotations.get(&id).unwrap().body else {
            panic!("audio")
        };
        let body_path = crate::voice::LocalVoiceCapture::blob_path(root.path(), blob_id).unwrap();
        std::fs::write(&body_path, b"changed").unwrap();
        let output = root.path().join("exports/refused.mereclip");
        assert!(
            SelectedNote::prepare(root.path(), &model, &id)
                .unwrap()
                .write(&output)
                .is_err()
        );
        assert!(!output.parent().unwrap().exists());
        std::fs::remove_file(body_path).unwrap();
        assert!(
            SelectedNote::prepare(root.path(), &model, &id)
                .unwrap()
                .write(&output)
                .is_err()
        );
        assert!(!output.parent().unwrap().exists());
    }

    /// The process receipt consumes this exact owner-generated package.
    #[test]
    #[ignore]
    fn selected_voice_process_fixture() {
        let root =
            PathBuf::from(std::env::var_os("EIDETIC_PRODUCER_FIXTURE_ROOT").expect("fixture root"));
        assert_eq!(
            std::fs::read_to_string(root.join("fixture-marker")).unwrap(),
            "eidetic-retention-20261019\n"
        );
        let (model, id) = voice_note(&root);
        SelectedNote::prepare(&root, &model, &id)
            .unwrap()
            .write(&root.join("voice.mereclip"))
            .unwrap();
        std::fs::remove_dir_all(root.join("voice")).unwrap();
    }
}
