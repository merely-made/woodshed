// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use crate::{ItemId, NotePrivacy, RedshankModel};
pub use mere_ontology::Profile;
use mere_ontology::TermKind;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

const SPECIFIC_RESOURCE: &str = "http://www.w3.org/ns/oa#SpecificResource";
const AUDIO_FILE: &str = "http://purl.org/ontology/mo/AudioFile";

/// Load the local export lens and its selected subject packs.
pub fn load(directory: Option<&Path>) -> Result<Profile, String> {
    mere_ontology::load(
        include_str!("../ontology/lens.toml"),
        &[
            mere_ontology::WEB_ANNOTATION,
            include_str!("../ontology/music-ontology.toml"),
        ],
        directory,
    )
}

impl RedshankModel {
    /// Add the active lens's explicit types to the standard Web Annotation export.
    /// Only already disclosed, source-qualified targets contribute descriptions.
    pub fn export_annotations_with_profile(&self, item: &ItemId, profile: &Profile) -> Value {
        self.annotation_page(item, Some(profile))
    }

    pub(super) fn annotation_page(&self, item: &ItemId, profile: Option<&Profile>) -> Value {
        let has_class = |iri| {
            profile
                .and_then(|p| p.terms.entry(iri))
                .is_some_and(|entry| entry.kind == TermKind::Class)
        };
        let specific_resources = has_class(SPECIFIC_RESOURCE);
        let audio_files = has_class(AUDIO_FILE);
        let mut items = Vec::new();
        let mut files = BTreeSet::new();
        for note in self.annotations_for_item(item) {
            if note.privacy != NotePrivacy::Shareable {
                continue;
            }
            let source = self.annotation_source(note);
            let mut annotation = self.export_annotation(note, &source);
            if specific_resources {
                annotation["target"]["type"] = json!("SpecificResource");
            }
            let receipt = &note.target.representation;
            if audio_files
                && (source.starts_with("file:")
                    || (receipt.byte_length.is_some()
                        && receipt
                            .media_type
                            .as_deref()
                            .is_some_and(is_audio_media_type)))
            {
                files.insert(source);
            }
            items.push(annotation);
        }
        let mut page = json!({
            "@context": "http://www.w3.org/ns/anno.jsonld",
            "type": "AnnotationPage",
            "items": items,
        });
        if !files.is_empty() {
            page["@included"] = Value::Array(
                files
                    .into_iter()
                    .map(|id| json!({"@id": id, "@type": AUDIO_FILE}))
                    .collect(),
            );
        }
        page
    }
}

fn is_audio_media_type(value: &str) -> bool {
    let media_type = value.split(';').next().unwrap_or_default().trim();
    media_type
        .split_once('/')
        .is_some_and(|(top, sub)| top.eq_ignore_ascii_case("audio") && !sub.is_empty())
}
