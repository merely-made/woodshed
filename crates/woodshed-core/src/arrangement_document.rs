// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! A progression's portable scene document in an inert attributed Djot block.
//! The shared codec retains its existing format, including linked voicings.
use scenomise::{backdrops::SceneFacts, document::SceneDocument};

pub fn to_djot(document: &SceneDocument) -> Result<String, String> {
    if !matches!(document.primary.facts, SceneFacts::TemplatedSequence(_)) {
        return Err("A progression block requires a templated sequence".into());
    }
    document.to_djot("progression")
}

/// Reading an independent block does not edit a live Set or choose playback.
pub fn from_djot(source: &str) -> Result<SceneDocument, String> {
    let document = SceneDocument::from_djot(source)?;
    if !matches!(document.primary.facts, SceneFacts::TemplatedSequence(_)) {
        return Err("Progression block did not contain a templated sequence".into());
    }
    Ok(document)
}
