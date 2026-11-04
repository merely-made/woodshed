//! Reading the practice session back out of whichever store opened.
//!
//! Lifted out of `boot_state` when the persona gate arrived: a machine that
//! asks which persona to practise as cannot restore at boot, because the key
//! that decrypts the session is the answer to the question still on screen. So
//! the restore has two callers and one run, and it lives here rather than in
//! either of them.

use woodshed_core::personal_relations::{PersonalRelationSource, PersonalRelationSourceId};
use woodshed_core::settings::AppSettings;
use woodshed_core::storage::SessionStore;
use woodshed_views::stage::UiState;

use crate::storage::HostBackend;

/// Read the persona settings slot without applying it.
pub fn load_settings(storage: &SessionStore<HostBackend>) -> Option<AppSettings> {
    storage
        .load_settings()
        .and_then(|json| match serde_json::from_str(&json) {
            Ok(settings) => Some(settings),
            Err(error) => {
                storage.refuse_writes(format!(
                    "Application settings could not be decoded: {error}"
                ));
                None
            },
        })
}

/// Apply the stored session and application settings to a fresh [`UiState`].
///
/// A corrupt file does not stop practice opening, but quarantines all writes
/// on this store handle and its clones until a deliberate reopen. A legacy flat session migrates its settings when no split
/// settings file exists yet.
///
/// Settings ride in through [`UiState::apply_persisted`] rather than being
/// assigned, because several pieces of state are derived from them (the
/// transport's bpm, the tuning and root dropdowns, the legacy relation-set
/// migration). A settings-only store applies them over a default session, so a
/// preference file remains authoritative even when the practice artifact has
/// not been written yet.
pub fn restore(storage: &SessionStore<HostBackend>, ui: &mut UiState) {
    let stored_settings = load_settings(storage);
    let mut app_settings = stored_settings.clone().unwrap_or_default();
    let loaded = storage.try_load();
    refresh_save_status(storage, ui);
    let Some(json) = loaded.ok().flatten() else {
        if stored_settings.is_some() {
            if let Some(error) = ui.apply_persisted(
                &woodshed_core::storage::PersistedSession::default(),
                app_settings,
            ) {
                eprintln!("[woodshed-genet] ignoring invalid workspace snapshot: {error}");
            }
        }
        return;
    };
    match woodshed_core::storage::decode_session(&json) {
        Ok(loaded) => {
            if stored_settings.is_none() {
                if let Some(legacy) = loaded.legacy_settings {
                    app_settings = legacy;
                }
            }
            if let Some(error) = ui.apply_persisted(&loaded.session, app_settings) {
                storage.refuse_writes(format!(
                    "The retained workspace could not be admitted: {error}"
                ));
            }
        },
        Err(error) => {
            storage.refuse_writes(format!(
                "The saved practice session could not be decoded: {error}"
            ));
        },
    }
    refresh_save_status(storage, ui);
}

/// Fully admit an incoming persona before the host retires outgoing state.
///
/// Unlike the tolerant boot restore, a switch requires every retained slot and
/// capture companion to be usable. This prepares only a fresh UI value: it does
/// not mint a personal relation source, save data, or create an audio backend.
pub fn admit(storage: &SessionStore<HostBackend>) -> Result<UiState, String> {
    let mut ui = UiState::new();
    restore(storage, &mut ui);
    if let Some(reason) = storage.write_refusal().or_else(|| storage.save_problem()) {
        return Err(reason);
    }
    let manifest = &ui.loop_captures;
    manifest
        .validate()
        .map_err(|error| format!("The retained loop catalog could not be admitted: {error}"))?;
    for reference in &manifest.takes {
        storage
            .try_read_capture(
                manifest
                    .namespace
                    .as_deref()
                    .expect("validated capture namespace"),
                reference,
            )
            .map_err(|error| {
                format!(
                    "Captured loop {:?} could not be admitted: {error}",
                    reference.id
                )
            })?;
        let frames = woodshed_audio::Bar {
            legacy_clock: reference.legacy_clock(),
            bpm: reference.bpm,
            time_signature: woodshed_audio::TimeSignature::new(reference.beats, 4),
            length: if reference.seconds.is_some() {
                1
            } else {
                reference.bars
            },
            duration_seconds: reference.seconds,
            ..Default::default()
        }
        .checked_duration_frames(reference.sample_rate)
        .map_err(|error| {
            format!(
                "Captured loop {:?} has an invalid clock: {error}",
                reference.id
            )
        })?;
        if frames != reference.frames as u64 {
            return Err(format!(
                "Captured loop {:?} PCM length differs from its recorded clock.",
                reference.id
            ));
        }
    }
    Ok(ui)
}

pub fn refresh_save_status(storage: &SessionStore<HostBackend>, ui: &mut UiState) {
    ui.practice_save_notice = if let Some(woodshed_views::persona::PracticeSeal::Pending {
        reason,
    }) = ui.seal.as_ref()
    {
        Some(format!(
            "Practice is not being saved: {reason}. Changes stay in this window. Choose or reconnect the persona before closing if you want to keep them."
        ))
    } else if let Some(reason) = storage.write_refusal() {
        Some(format!(
            "Practice is not being saved: {reason}. The existing files will not be overwritten; current changes remain in this window."
        ))
    } else {
        storage.save_problem().map(|reason| format!(
            "Practice save is unconfirmed: {reason}. A write may already be present, but could not be verified; current changes remain in this window."
        ))
    };
    ui.practice_saved = ui.practice_save_notice.is_none()
        && !matches!(
            ui.seal,
            Some(woodshed_views::persona::PracticeSeal::Pending { .. })
        );
}

/// Ensure the session's stable personal assertion source exists and is saved
/// before a view can offer authored relation actions. Legacy sessions receive
/// one random source ID; ordinary reopen never remints it.
pub fn ensure_personal_relation_source(storage: &SessionStore<HostBackend>, ui: &mut UiState) {
    if ui.personal_relations.is_some()
        || !ui.practice_saved
        || !matches!(
            ui.seal.as_ref(),
            Some(woodshed_views::persona::PracticeSeal::Sealed { persona, .. })
                if !persona.trim().is_empty() && persona == persona.trim()
        )
    {
        return;
    }

    let mut bytes = [0_u8; 16];
    if let Err(error) = getrandom::fill(&mut bytes) {
        ui.personal_relation_notice = Some(format!(
            "Personal relations are unavailable because a source identity could not be created: {error}"
        ));
        return;
    }
    ui.personal_relations = Some(PersonalRelationSource::empty(
        PersonalRelationSourceId::from_random_uuid_bytes(bytes),
    ));
    let serialized = serde_json::to_string(&ui.to_persisted());
    match serialized {
        Ok(serialized) => match storage.try_save(&serialized) {
            Ok(()) => ui.personal_relation_notice = None,
            Err(error) => {
                ui.personal_relations = None;
                ui.personal_relation_notice = Some(format!(
                    "Personal relations are unavailable because their source identity could not be saved: {error}"
                ));
            },
        },
        Err(error) => {
            ui.personal_relations = None;
            ui.personal_relation_notice = Some(format!(
                "Personal relations are unavailable because their source identity could not be encoded: {error}"
            ));
        },
    }
    refresh_save_status(storage, ui);
}

#[cfg(test)]
#[path = "session/tests.rs"]
mod tests;
