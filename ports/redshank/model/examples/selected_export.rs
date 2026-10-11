// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

fn main() {
    use redshank_model::{
        Annotation, AnnotationId, ItemId, LibraryItem, MediaSource, NoteBody, NotePrivacy,
        RedshankModel, RepresentationReceipt, TimedTarget,
    };
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
    let id = AnnotationId("voice-1".into());
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
            privacy: NotePrivacy::Shareable,
        })
        .unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(&model.export_selected_note(&id).unwrap()).unwrap()
    );
}
