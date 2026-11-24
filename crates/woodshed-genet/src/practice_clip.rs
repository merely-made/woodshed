//! Import a selected guitar tab or a timed listening reference into a working Set.
use std::fs::File;
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use url::Url;
use woodshed_views::{persona::PracticeSeal, workspace::WorkspacePanel};
use woodshedding::notation::{
    FrettedNote, Measure, MusicDocument, MusicSourceSelection, NoteEvent, NoteValue, Score,
    TimeSignature,
};
use woodshedding::rehearsal::{ListeningPhrase, LoopMode, Set};

use crate::{Shared, sync::Ctx};

pub(super) const MAX_REQUEST_BYTES: u64 = 4096;
const RECEIPT_OVERHEAD_BYTES: u64 = 64;
const SLOTS: usize = 8;

pub(super) enum OpenRequest {
    Listening(PathBuf),
    Wav(PathBuf),
}

impl OpenRequest {
    pub(super) fn to_handoff(&self) -> crate::handoff::Command {
        let absolute = |path: &Path| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        match self {
            Self::Listening(path) => crate::handoff::Command::OpenListening { path: absolute(path) },
            Self::Wav(path) => crate::handoff::Command::OpenWav { path: absolute(path) },
        }
    }

    pub(super) fn open(self, shared: &mut Shared, ctx: &mut Ctx<'_>) -> Result<(), String> {
        match self {
            Self::Listening(path) => open_listening(&path, shared, ctx),
            Self::Wav(path) => crate::practice_wav::open(&path, shared, ctx),
        }
    }
}

pub(super) fn import(
    directory: &Path,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    require_sealed_store(shared)?;
    let bytes = read_request(
        &directory.join("practice-clip-request.json"),
        MAX_REQUEST_BYTES,
    )?;
    let source: MusicSourceSelection = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let score = parse_score(&source)?;
    let doc = MusicDocument::from_score(&score)?;
    let mut set = Set::default();
    set.music = Some(doc);
    set.loop_mode = LoopMode::All;
    let title = source.title.clone();
    set.music_source = Some(source);
    ctx.runner.update(|ui| {
        ui.create_working_set_from(title, set);
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.sync_music_editor();
        ui.match_music_tempo();
    });
    crate::sync::after_dispatch(shared, ctx);
    inspect(directory, shared, ctx)
}

pub(super) fn read_request(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err(format!("Practice clip file exceeds {limit} bytes"));
    }
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ListeningRequest {
    Phrase(ListeningPhrase),
    Receipt { source: ListeningPhrase },
}

pub(super) fn open_listening(
    path: &Path,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    require_sealed_store(shared)?;
    let bytes = read_request(path, MAX_REQUEST_BYTES + RECEIPT_OVERHEAD_BYTES)?;
    let request = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let phrase = match request {
        ListeningRequest::Phrase(_) if bytes.len() as u64 > MAX_REQUEST_BYTES => {
            return Err(format!(
                "Practice clip request exceeds {MAX_REQUEST_BYTES} bytes"
            ));
        },
        ListeningRequest::Phrase(phrase) | ListeningRequest::Receipt { source: phrase } => phrase,
    };
    validate_listening(&phrase)?;
    ctx.runner.update(|ui| {
        let existing = ui
            .working_sets
            .inactive
            .iter()
            .find(|entry| entry.set.listening_phrase.as_ref() == Some(&phrase))
            .map(|entry| entry.id);
        if ui.set.listening_phrase.as_ref() != Some(&phrase) {
            if let Some(id) = existing {
                ui.activate_working_set(id);
            } else {
                let mut set = Set::default();
                let title = phrase.title.clone();
                set.listening_phrase = Some(phrase);
                ui.create_working_set_from(title, set);
            }
        }
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.music_editor.notice = None;
    });
    crate::sync::after_dispatch(shared, ctx);
    confirmed_listening(shared, ctx)?;
    Ok(())
}

pub(super) fn import_listening(
    directory: &Path,
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Result<(), String> {
    open_listening(&directory.join("practice-clip-request.json"), shared, ctx)?;
    inspect_listening(directory, shared, ctx)
}

pub(super) fn inspect_listening(
    directory: &Path,
    shared: &Shared,
    ctx: &Ctx<'_>,
) -> Result<(), String> {
    let source = confirmed_listening(shared, ctx)?;
    write_receipt(
        directory,
        &serde_json::json!({
            "owner": ctx.runner.state().working_sets.active_id.0,
            "source": source,
        }),
    )
}

fn confirmed_listening<'a>(
    shared: &Shared,
    ctx: &'a Ctx<'_>,
) -> Result<&'a ListeningPhrase, String> {
    let store = require_sealed_store(shared)?;
    let text = store.load().ok_or("Could not read saved practice")?;
    let stored = woodshed_core::storage::decode_session(&text)
        .map_err(|error| error.to_string())?
        .session;
    let ui = ctx.runner.state();
    let source = ui
        .set
        .listening_phrase
        .as_ref()
        .ok_or("Open a listening phrase Set")?;
    if stored.working_sets.active_id != ui.working_sets.active_id
        || stored.set.listening_phrase.as_ref() != Some(source)
        || stored.set.music.is_some()
    {
        return Err("The listening phrase is unconfirmed in session storage".into());
    }
    Ok(source)
}

pub(super) fn inspect(directory: &Path, shared: &Shared, ctx: &Ctx<'_>) -> Result<(), String> {
    let store = require_sealed_store(shared)?;
    let text = store.load().ok_or("Could not read saved practice")?;
    let stored = woodshed_core::storage::decode_session(&text)
        .map_err(|error| error.to_string())?
        .session;
    let ui = ctx.runner.state();
    let source = ui
        .set
        .music_source
        .as_ref()
        .ok_or("Open an imported practice Set")?;
    let doc = ui
        .set
        .music
        .as_ref()
        .ok_or("Imported practice Set has no score")?;
    if stored.working_sets.active_id != ui.working_sets.active_id
        || stored.set.music_source.as_ref() != Some(source)
        || stored.set.music.as_ref() != Some(doc)
    {
        return Err("The current practice clip is unconfirmed in session storage".into());
    }
    let score = doc.score()?;
    let receipt = serde_json::json!({
        "owner": ui.working_sets.active_id.0,
        "title": score.title,
        "source": source,
        "musicxml": doc.musicxml,
        "musicxml_blake3": blake3::hash(doc.musicxml.as_bytes()).to_hex().to_string(),
        "measure_count": score.measures.len(),
        "events_per_measure": score.measures.iter().map(|bar| bar.events.len()).collect::<Vec<_>>(),
    });
    write_receipt(directory, &receipt)
}

pub(super) fn write_receipt(directory: &Path, receipt: &serde_json::Value) -> Result<(), String> {
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut temporary, &receipt).map_err(|e| e.to_string())?;
    temporary.flush().map_err(|e| e.to_string())?;
    temporary
        .persist(directory.join("practice-clip-receipt.json"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn http_url(location: &str) -> Result<Url, String> {
    let authority = location
        .strip_prefix("https://")
        .or_else(|| location.strip_prefix("http://"))
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .filter(|authority| !authority.is_empty() && !authority.contains('\\'));
    Url::parse(location)
        .ok()
        .filter(|url| {
            authority.is_some()
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && !location
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control())
        })
        .ok_or_else(|| "Listening phrases require HTTP(S) source and recording URLs".into())
}

fn validate_listening(phrase: &ListeningPhrase) -> Result<(), String> {
    if phrase.title.trim().is_empty()
        || phrase.title.len() > 160
        || phrase.title.chars().any(char::is_control)
    {
        return Err("Listening phrase requires a title of at most 160 bytes".into());
    }
    if phrase.start_ms >= phrase.end_ms {
        return Err("Listening phrase requires an end time after its start time".into());
    }
    if phrase.end_ms > (1 << 53) - 1 {
        return Err("Listening phrase time exceeds the supported integer range".into());
    }
    http_url(&phrase.source_page_url)?;
    http_url(&phrase.listening_url)?;
    Ok(())
}

fn recording_url(phrase: &ListeningPhrase) -> Result<Url, String> {
    let mut url = http_url(&phrase.listening_url)?;
    if matches!(
        url.host_str(),
        Some("youtube.com" | "www.youtube.com" | "m.youtube.com" | "youtu.be")
    ) {
        let params: Vec<_> = url
            .query_pairs()
            .filter(|(name, _)| name != "t" && name != "start")
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        url.query_pairs_mut()
            .clear()
            .extend_pairs(params)
            .append_pair("t", &format!("{}s", phrase.start_ms / 1_000));
    } else if url.path().rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "wav" | "mp3" | "ogg" | "opus" | "m4a" | "flac" | "mp4" | "webm"
        )
    }) {
        let seconds = |ms: u64| format!("{}.{:03}", ms / 1_000, ms % 1_000);
        url.set_fragment(Some(&format!(
            "t={},{}",
            seconds(phrase.start_ms),
            seconds(phrase.end_ms)
        )));
    }
    Ok(url)
}

pub(super) fn after_dispatch(ctx: &mut Ctx<'_>) {
    if !ctx.runner.state().music_editor.listening_open_requested {
        return;
    }
    ctx.runner.update(|ui| {
        ui.music_editor.listening_open_requested = false;
        ui.music_editor.notice = ui.set.listening_phrase.as_ref().and_then(|phrase| {
            recording_url(phrase)
                .and_then(|url| open_external(url.as_str()))
                .err()
        });
    });
}

#[cfg(windows)]
fn open_external(url: &str) -> Result<(), String> {
    use std::ptr::null;
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let url: Vec<_> = url.encode_utf16().chain([0]).collect();
    // SAFETY: The validated URL is NUL-terminated and lives through the call.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            null(),
            url.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err("Could not open the original recording".into())
    }
}

#[cfg(not(windows))]
fn open_external(url: &str) -> Result<(), String> {
    let launcher = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(launcher)
        .arg(url)
        .status()
        .map_err(|error| format!("Could not open the original recording: {error}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "Could not open the original recording".into())
}

pub(super) fn play(shared: &mut Shared, ctx: &mut Ctx<'_>) -> Result<(), String> {
    if ctx.runner.state().set.music_source.is_none() {
        return Err("Open an imported practice Set".into());
    }
    ctx.runner.update(|ui| {
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.sync_music_editor();
        ui.practice_music();
    });
    crate::sync::after_dispatch(shared, ctx);
    if !ctx.runner.state().music_practice_requested() {
        return Err("Arrangement did not start practice playback".into());
    }
    Ok(())
}

pub(super) fn require_sealed_store(
    shared: &Shared,
) -> Result<&woodshed_core::storage::SessionStore<crate::storage::HostBackend>, String> {
    shared
        .storage
        .as_ref()
        .filter(|_| matches!(shared.seal, Some(PracticeSeal::Sealed { .. })))
        .ok_or_else(|| "Open a sealed Woodshed practice session first".into())
}

fn parse_score(source: &MusicSourceSelection) -> Result<Score, String> {
    if source.title.trim().is_empty()
        || source.title.len() > 160
        || source.title.chars().any(char::is_control)
    {
        return Err("Practice clip requires a title of at most 160 bytes".into());
    }
    for location in [&source.source_page_url, &source.listening_url] {
        if !location
            .strip_prefix("https://")
            .or_else(|| location.strip_prefix("http://"))
            .is_some_and(|suffix| !suffix.is_empty())
            || location
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err("Practice clip requires HTTP(S) source and listening URLs".into());
        }
    }
    if source.grid != "eighths-4/4" || source.tuning != "standard" {
        return Err("Practice clip requires eighths-4/4 and standard guitar tuning".into());
    }

    let mut measures = vec![
        Measure {
            meter: TimeSignature {
                beats: 4,
                beat_type: 4
            },
            harmony: None,
            events: vec![
                NoteEvent {
                    notes: vec![],
                    value: NoteValue::Eighth,
                    rest: true
                };
                SLOTS
            ],
        };
        2
    ];
    let mut lines = source.selected_text.trim_matches(['\r', '\n']).lines();
    for (string, name) in ["e", "B", "G", "D", "A", "E"].into_iter().enumerate() {
        let line = lines
            .next()
            .ok_or("Practice tab requires six string lines")?;
        let mut fields = line.split('|');
        if fields.next().map(str::trim) != Some(name) {
            return Err(format!("Expected practice tab string {name}"));
        }
        for measure in &mut measures {
            let mut slots = fields
                .next()
                .ok_or("Expected two bars per string")?
                .split_ascii_whitespace();
            for event in &mut measure.events {
                let token = slots.next().ok_or("Expected eight slots per bar")?;
                if token == "-" {
                    continue;
                }
                let fret = token
                    .parse::<u8>()
                    .ok()
                    .filter(|fret| *fret <= 24 && token.bytes().all(|byte| byte.is_ascii_digit()))
                    .ok_or("Expected '-' or a fret from 0 to 24")?;
                event.notes.push(FrettedNote {
                    string: string as u8 + 1,
                    fret,
                });
                event.rest = false;
            }
            if slots.next().is_some() {
                return Err("Expected eight slots per bar".into());
            }
        }
        if fields.next().map(str::trim) != Some("") || fields.next().is_some() {
            return Err("Expected two bars per string and a final pipe".into());
        }
    }
    if lines.next().is_some() {
        return Err("Practice tab requires six string lines".into());
    }
    let score = Score {
        title: source.title.clone(),
        instrument: "Guitar".into(),
        tuning: vec![64, 59, 55, 50, 45, 40],
        tempo: source.tempo_bpm,
        measures,
    };
    score.validate()?;
    Ok(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phrase() -> ListeningPhrase {
        ListeningPhrase {
            title: "Pickup into the chorus".into(),
            source_page_url: "http://127.0.0.1/lesson".into(),
            listening_url: "https://youtu.be/example?si=retain&t=4s".into(),
            start_ms: 12_345,
            end_ms: 18_900,
            note: "  Listen for the pickup.\nSing back twice.  ".into(),
        }
    }

    #[test]
    fn listening_phrase_reopens_without_notation_and_keeps_exact_note() {
        let phrase = phrase();
        validate_listening(&phrase).unwrap();
        let mut set = Set::default();
        set.listening_phrase = Some(phrase.clone());
        let set: Set = serde_json::from_slice(&serde_json::to_vec(&set).unwrap()).unwrap();
        assert_eq!(set.listening_phrase, Some(phrase.clone()));
        assert!(set.music.is_none());
        let receipt = serde_json::json!({ "owner": 2, "source": phrase });
        assert!(matches!(
            serde_json::from_value::<ListeningRequest>(receipt).unwrap(),
            ListeningRequest::Receipt { source } if source == phrase
        ));
        assert_eq!(
            recording_url(&phrase).unwrap().as_str(),
            "https://youtu.be/example?si=retain&t=12s"
        );
        let mut direct = phrase;
        direct.listening_url = "https://example.com/test.wav".into();
        assert_eq!(
            recording_url(&direct).unwrap().as_str(),
            "https://example.com/test.wav#t=12.345,18.900"
        );
    }

    #[test]
    fn listening_phrase_rejects_invalid_ranges_and_locations() {
        let mut request = phrase();
        request.start_ms = request.end_ms;
        assert!(validate_listening(&request).is_err());
        request.start_ms = 0;
        request.end_ms = u64::MAX;
        assert!(validate_listening(&request).is_err());
        for url in [
            "file:///recording",
            "https:///recording",
            "https://",
            "https://user:pass@example.com/",
        ] {
            request = phrase();
            request.listening_url = url.into();
            assert!(validate_listening(&request).is_err(), "{url}");
        }
    }

    #[test]
    fn practice_clip_keeps_eighth_slots_and_round_trips_musicxml() {
        let mut source = MusicSourceSelection {
            title: "Two bars".into(),
            selected_text: ["e", "B", "G", "D", "A", "E"]
                .map(|string| format!("{string} |12 - - - - - - -|- - 3 - - - - 0|"))
                .join("\r\n"),
            source_page_url: "http://127.0.0.1/lesson".into(),
            listening_url: "https://example.org/listen".into(),
            grid: "eighths-4/4".into(),
            tuning: "standard".into(),
            tempo_bpm: 88,
        };
        let score = parse_score(&source).unwrap();
        assert_eq!(score.measures[0].events[0].notes.len(), 6);
        assert_eq!(score.measures[0].events[0].notes[0].fret, 12);
        assert!(score.measures[0].events[1].rest);
        assert_eq!(score.measures[1].events[7].value, NoteValue::Eighth);
        assert_eq!(
            MusicDocument::from_score(&score).unwrap().score().unwrap(),
            score
        );

        source.selected_text = source.selected_text.replacen("12 -", "12h14 -", 1);
        assert!(parse_score(&source).is_err());
    }
}
