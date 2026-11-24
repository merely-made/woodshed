//! Saves a marked recording and binds its Set to the native output lane.

mod page;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use woodshed_core::{storage::MAX_PRACTICE_WAV_BYTES, working_sets::WorkingSetId};
use woodshed_views::workspace::WorkspacePanel;
use woodshedding::rehearsal::{Set, WavPhrase, WavPhraseSource};

use crate::{
    Shared,
    audio::CpalBackend,
    practice_clip::{
        MAX_REQUEST_BYTES, http_url, read_request, require_sealed_store, write_receipt,
    },
    sync::Ctx,
};

const REQUEST_NAME: &str = "practice-clip-request.json";
const WAV_NAME: &str = "practice-clip.wav";
const RECEIPT_OVERHEAD: u64 = 160;

#[derive(Deserialize)]
#[serde(untagged)]
enum Request {
    Source(WavPhraseSource),
    Receipt(WavPhrase),
}

#[derive(Default)]
pub(crate) struct Playback {
    loaded: Option<(Arc<()>, WorkingSetId)>,
}

impl Playback {
    pub(super) fn is_loaded(&self) -> bool {
        self.loaded.is_some()
    }

    fn owns(&self, ui: &woodshed_views::stage::UiState) -> bool {
        self.loaded.as_ref().is_some_and(|(session, owner)| {
            Arc::ptr_eq(session, &ui.audio_session)
                && *owner == ui.working_sets.active_id
                && ui.set.wav_phrase.is_some()
        })
    }
}

pub(super) fn open(path: &Path, shared: &mut Shared, ctx: &mut Ctx<'_>) -> Result<(), String> {
    let request = read_request(path, MAX_REQUEST_BYTES + RECEIPT_OVERHEAD)?;
    let document = serde_json::from_slice(&request).map_err(|error| error.to_string())?;
    let (source, expected) = match document {
        Request::Source(source) if request.len() as u64 <= MAX_REQUEST_BYTES => (source, None),
        Request::Source(_) => return Err("Practice WAV request exceeds 4096 bytes.".into()),
        Request::Receipt(phrase) => (phrase.source, Some((phrase.wav_blake3, phrase.wav_bytes))),
    };
    import_file(
        &path.with_file_name(WAV_NAME),
        source,
        expected,
        shared,
        ctx,
    )
}

pub(super) fn import_file(
    path: &Path,
    source: WavPhraseSource,
    expected: Option<(String, usize)>,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    let store = require_sealed_store(shared)?;
    validate_marks(&source)?;
    http_url(&source.source_page_url)
        .map_err(|_| "Practice WAV requires an HTTP(S) source URL.")?;
    let bytes = read_request(path, MAX_PRACTICE_WAV_BYTES as u64)
        .map_err(|error| format!("Could not read practice recording: {error}"))?;
    if let Some((digest, length)) = expected {
        if bytes.len() != length || blake3::hash(&bytes).to_hex().as_str() != digest {
            return Err("Practice WAV does not match its supplied receipt.".into());
        }
    }
    CpalBackend::validate_listening_wav(&bytes, source.start_ms, source.end_ms)?;
    let phrase = WavPhrase {
        source,
        wav_blake3: store.try_write_practice_wav(&bytes)?,
        wav_bytes: bytes.len(),
        region_only: false,
    };
    activate_phrase(phrase, shared, ctx)
}

pub(super) enum Region<'a> {
    Recording {
        path: &'a Path,
        preview: Option<&'a Path>,
    },
    PagePhrase(&'a Path),
}

pub(super) fn import_region(
    region: Region<'_>,
    source: WavPhraseSource,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    let store = require_sealed_store(shared)?;
    validate_marks(&source)?;
    let bytes = match region {
        Region::Recording { path, preview } => extract_local_region(path, preview, &source)?,
        Region::PagePhrase(path) => page::read_phrase(path, &source)?,
    };
    let phrase = WavPhrase {
        source,
        wav_blake3: store.try_write_wav_region(&bytes)?,
        wav_bytes: bytes.len(),
        region_only: true,
    };
    activate_phrase(phrase, shared, ctx)
}

fn canonical_local_file(path: &Path) -> Result<PathBuf, String> {
    if !crate::handoff::local_file_path(path) {
        return Err("Choose a recording on this computer.".into());
    }
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    if !crate::handoff::local_file_path(&canonical) {
        return Err("Choose a recording on this computer.".into());
    }
    Ok(canonical)
}

fn extract_local_region(
    path: &Path,
    preview_path: Option<&Path>,
    source: &WavPhraseSource,
) -> Result<Vec<u8>, String> {
    let recording = canonical_local_file(path)?;
    let source_file = url::Url::parse(&source.source_page_url)
        .ok()
        .filter(|url| url.scheme() == "file" && url.query().is_none() && url.fragment().is_none())
        .and_then(|url| url.to_file_path().ok())
        .ok_or("Practice requires the selected recording's file URL.")?;
    if canonical_local_file(&source_file)? != recording {
        return Err("The source file URL does not match the selected recording.".into());
    }
    let extension = source_file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let audio = match (extension.to_ascii_lowercase().as_str(), preview_path) {
        ("wav", None) => recording,
        ("mp3" | "mp4", Some(preview)) => {
            // The PCM used by the Turnstone preview is the clock the musician marked.
            // Read the selected range from that file; retain the original URL as provenance.
            let preview = canonical_local_file(preview)?;
            let reader = hound::WavReader::open(&preview)
                .map_err(|error| format!("Could not read the prepared audio preview: {error}"))?;
            let spec = reader.spec();
            if spec.sample_format != hound::SampleFormat::Int
                || spec.bits_per_sample != 16
                || !(1..=2).contains(&spec.channels)
            {
                return Err("The prepared preview must contain mono or stereo 16-bit PCM.".into());
            }
            preview
        },
        ("mp3" | "mp4", None) => return Err("Prepare the audio preview in Turnstone first.".into()),
        ("wav", Some(_)) => {
            return Err("WAV practice reads the selected original recording directly.".into());
        },
        _ => return Err("Choose a local WAV, MP3, or MP4 recording.".into()),
    };
    crate::wav_file::extract(&audio, source.start_ms, source.end_ms)
}

fn activate_phrase(
    phrase: WavPhrase,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    ctx.runner.update(|ui| {
        if ui.set.wav_phrase.as_ref() != Some(&phrase) {
            let existing = ui
                .working_sets
                .inactive
                .iter()
                .find(|entry| entry.set.wav_phrase.as_ref() == Some(&phrase))
                .map(|entry| entry.id);
            if let Some(id) = existing {
                ui.activate_working_set(id);
            } else {
                let mut set = Set::default();
                let title = phrase.source.title.clone();
                set.wav_phrase = Some(phrase);
                ui.create_working_set_from(title, set);
            }
        }
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.music_editor.notice = None;
    });
    crate::sync::after_dispatch(shared, ctx);
    confirmed(shared, ctx).map(|_| ())
}

fn validate_marks(source: &WavPhraseSource) -> Result<(), String> {
    if source.title.trim().is_empty()
        || source.title.len() > 160
        || source.title.chars().any(char::is_control)
    {
        return Err("The recording title must be between 1 and 160 bytes.".into());
    }
    if source.start_ms >= source.end_ms {
        return Err("Mark the end of the phrase after its start.".into());
    }
    if source.end_ms > (1 << 53) - 1 {
        return Err("The phrase time exceeds the supported range.".into());
    }
    Ok(())
}

pub(super) fn import(
    directory: &Path,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    open(&directory.join(REQUEST_NAME), shared, ctx)?;
    publish_receipt(directory, ctx)
}

pub(super) fn inspect(directory: &Path, shared: &Shared, ctx: &Ctx<'_>) -> Result<(), String> {
    confirmed(shared, ctx)?;
    publish_receipt(directory, ctx)
}

fn publish_receipt(directory: &Path, ctx: &Ctx<'_>) -> Result<(), String> {
    let ui = ctx.runner.state();
    let phrase = ui
        .set
        .wav_phrase
        .as_ref()
        .ok_or("Open a practice recording.")?;
    write_receipt(
        directory,
        &serde_json::json!({
            "owner": ui.working_sets.active_id.0,
            "source": phrase.source,
            "wav_blake3": phrase.wav_blake3,
            "wav_bytes": phrase.wav_bytes,
        }),
    )
}

fn confirmed(shared: &Shared, ctx: &Ctx<'_>) -> Result<Vec<u8>, String> {
    let store = require_sealed_store(shared)?;
    let text = store.load().ok_or("Could not read saved practice.")?;
    let stored = woodshed_core::storage::decode_session(&text)
        .map_err(|error| error.to_string())?
        .session;
    let ui = ctx.runner.state();
    let phrase = ui
        .set
        .wav_phrase
        .as_ref()
        .ok_or("Open a practice recording.")?;
    if stored.working_sets.active_id != ui.working_sets.active_id
        || stored.set.wav_phrase.as_ref() != Some(phrase)
        || stored.set.music.is_some()
    {
        return Err("Could not confirm the saved practice recording.".into());
    }
    if phrase.region_only {
        store.try_read_wav_region(&phrase.wav_blake3, phrase.wav_bytes)
    } else {
        store.try_read_practice_wav(&phrase.wav_blake3, phrase.wav_bytes)
    }
}

pub(super) fn set_playing(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    playing: bool,
) -> Result<(), String> {
    let ui = ctx.runner.state();
    let phrase = ui
        .set
        .wav_phrase
        .as_ref()
        .ok_or("Open a practice recording.")?;
    require_sealed_store(shared)?;
    let needs_load = !shared.wav_practice.owns(ui);
    let initial = if needs_load && playing {
        Some(confirmed(shared, ctx)?)
    } else {
        None
    };
    let backend = shared
        .backend
        .as_mut()
        .filter(|backend| backend.output_stream_count() == 1)
        .ok_or("Playing a practice recording requires a native audio output stream.")?;
    if let Some(bytes) = initial {
        if phrase.region_only {
            backend.set_extracted_wav(&bytes)?;
        } else {
            backend.set_listening_wav(&bytes, phrase.source.start_ms, phrase.source.end_ms)?;
        }
        shared.wav_practice.loaded = Some((ui.audio_session.clone(), ui.working_sets.active_id));
    }
    if shared.wav_practice.owns(ui) {
        backend.set_listening_wav_playing(playing)?;
        backend.service();
    }
    ctx.runner.update(|ui| {
        ui.music_editor.wav_playing = playing;
        ui.music_editor.notice = None;
    });
    Ok(())
}

pub(super) fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let ui = ctx.runner.state();
    if shared.wav_practice.loaded.is_some() && !shared.wav_practice.owns(ui) {
        let result = shared
            .backend
            .as_mut()
            .map(CpalBackend::clear_listening_wav);
        if let Some(Err(error)) = result {
            eprintln!("[woodshed] Could not clear practice WAV: {error}");
        } else {
            shared.wav_practice.loaded = None;
        }
        ctx.runner.update(|ui| ui.music_editor.wav_playing = false);
    }
    if let Some(playing) = ctx.runner.state().music_editor.wav_play_requested {
        ctx.runner
            .update(|ui| ui.music_editor.wav_play_requested = None);
        if let Err(error) = set_playing(shared, ctx, playing) {
            ctx.runner.update(|ui| ui.music_editor.notice = Some(error));
        }
    }
}

#[cfg(test)]
mod local_region_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn compressed_recordings_import_only_marked_preview_frames_with_original_provenance() {
        let temp = tempfile::tempdir().unwrap();
        let preview = temp.path().join("prepared.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 1_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&preview, spec).unwrap();
        for i in 0..10_000i16 {
            writer.write_sample(i).unwrap();
            writer.write_sample(-i).unwrap();
        }
        writer.finalize().unwrap();
        for extension in ["MP3", "mP4"] {
            // Compressed decoding belongs to Turnstone. Woodshed uses its exact preview clock.
            let original = temp.path().join(format!("song Ω #2.{extension}"));
            std::fs::write(&original, b"original container").unwrap();
            let mut source = WavPhraseSource {
                title: "Second take".into(),
                source_page_url: url::Url::from_file_path(&original).unwrap().to_string(),
                start_ms: 9_888,
                end_ms: 9_890,
                note: "Accent the second beat".into(),
            };
            let region = extract_local_region(&original, Some(&preview), &source).unwrap();
            let mut reader = hound::WavReader::new(Cursor::new(region)).unwrap();
            assert_eq!(reader.duration(), 2);
            let samples: Vec<_> = reader.samples::<f32>().map(Result::unwrap).collect();
            assert_eq!(
                samples,
                [9_888., -9_888., 9_889., -9_889.].map(|s| s / 32_768.)
            );

            assert!(extract_local_region(&original, None, &source).is_err());
            assert!(extract_local_region(&original, Some(&original), &source).is_err());
            source.end_ms = 10_001;
            assert!(extract_local_region(&original, Some(&preview), &source).is_err());
            source.end_ms = 9_890;
            source.source_page_url = url::Url::from_file_path(&preview).unwrap().to_string();
            assert!(extract_local_region(&original, Some(&preview), &source).is_err());
        }
    }
}
