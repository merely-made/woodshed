// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Native note export retains time units and representation evidence. Hosts
//! supply voice-body bytes separately; this model neither reads nor fetches.

use crate::{AnnotationId, ModelError, NotePrivacy, RedshankModel};

pub const SCHEMA: &str = "redshank.selected-note/v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectedNoteExportError {
    Model(ModelError),
    Private(AnnotationId),
}

impl RedshankModel {
    /// Export one explicitly selected shareable note. This does not grant
    /// disclosure of its recording or access to a saved playback handle.
    pub fn export_selected_note(
        &self,
        id: &AnnotationId,
    ) -> Result<serde_json::Value, SelectedNoteExportError> {
        let note = self.annotations.get(id).ok_or_else(|| {
            SelectedNoteExportError::Model(ModelError::MissingAnnotation(id.clone()))
        })?;
        if note.privacy != NotePrivacy::Shareable {
            return Err(SelectedNoteExportError::Private(id.clone()));
        }
        Ok(
            serde_json::json!({"schema": SCHEMA, "annotation": note, "source": self.annotation_source(note)}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Annotation, ItemId, LibraryItem, MediaSource, NoteBody, RepresentationReceipt, TimedTarget,
    };

    #[test]
    fn voice_export_preserves_target_evidence_and_requires_shareable_note() {
        let id = AnnotationId("voice-1".into());
        let mut model = RedshankModel::default();
        model
            .add_item(LibraryItem::DirectAudio {
                id: ItemId("recording".into()),
                title: "Recording".into(),
                source: MediaSource::Enclosure {
                    url: "https://example.test/recording.wav".into(),
                },
            })
            .unwrap();
        model
            .add_annotation(Annotation {
                id: id.clone(),
                target: TimedTarget {
                    item_id: ItemId("recording".into()),
                    offset_ms: 1250,
                    end_offset_ms: Some(4500),
                    pressed_offset_ms: Some(1400),
                    fingerprint: None,
                    representation: RepresentationReceipt {
                        final_url: Some("https://example.test/recording.wav".into()),
                        etag: Some("\"original\"".into()),
                        ..Default::default()
                    },
                },
                body: NoteBody::Audio {
                    blob_id: "voice-blob".into(),
                    media_type: "audio/wav".into(),
                    duration_ms: 500,
                },
                created_at_ms: 7,
                privacy: NotePrivacy::Private,
            })
            .unwrap();
        assert_eq!(
            model.export_selected_note(&id),
            Err(SelectedNoteExportError::Private(id.clone()))
        );
        model
            .set_annotation_privacy(&id, NotePrivacy::Shareable)
            .unwrap();
        let export = model.export_selected_note(&id).unwrap();
        assert_eq!(export["annotation"]["target"]["offset_ms"], 1250);
        assert_eq!(export["annotation"]["target"]["pressed_offset_ms"], 1400);
        assert_eq!(
            export["annotation"]["target"]["representation"]["etag"],
            "\"original\""
        );
        assert_eq!(export["annotation"]["body"]["blob_id"], "voice-blob");
        assert!(export.get("historical_author").is_none());
    }
}
