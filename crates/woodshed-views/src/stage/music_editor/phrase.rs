//! The recorded-phrase screens: a WAV or listening phrase in place of a score.

use super::*;

fn timestamp(ms: u64) -> String {
    format!("{}:{:02}.{:03}", ms / 60_000, ms / 1_000 % 60, ms % 1_000)
}

pub(super) fn wav_phrase(ui: &UiState, phrase: &woodshedding::rehearsal::WavPhrase) -> UiChild {
    let source = &phrase.source;
    let playing = ui.music_editor.wav_playing;
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h2", text(source.title.clone()))),
        Box::new(el("p", text(format!(
            "{} to {}",
            timestamp(source.start_ms),
            timestamp(source.end_ms),
        ))).attr("class", "listening-range")),
        Box::new(el("p", text(source.note.clone()))
            .attr("class", "listening-note")
            .attr("style", "white-space: pre-wrap")),
        action(if playing { "Pause loop" } else { "Play loop" }, "wav-loop-toggle", move |ui| {
            ui.music_editor.wav_play_requested = Some(!playing);
        }),
    ];
    if let Some(notice) = &ui.music_editor.notice {
        children.push(Box::new(el("p", text(notice.clone())).attr("role", "status")));
    }
    Box::new(el("section", children).attr("class", "music-editor listening-phrase wav-phrase"))
}

pub(super) fn listening_phrase(ui: &UiState, phrase: &woodshedding::rehearsal::ListeningPhrase) -> UiChild {
    let mut children: Vec<UiChild> = vec![
        Box::new(el("h2", text(phrase.title.clone()))),
        Box::new(el("p", text(format!(
            "{} to {}",
            timestamp(phrase.start_ms),
            timestamp(phrase.end_ms),
        ))).attr("class", "listening-range")),
        Box::new(el("p", text(phrase.note.clone()))
            .attr("class", "listening-note")
            .attr("style", "white-space: pre-wrap")),
        action("Open original recording", "listening-open", |ui| {
            ui.music_editor.listening_open_requested = true;
        }),
    ];
    if let Some(notice) = &ui.music_editor.notice {
        children.push(Box::new(el("p", text(notice.clone())).attr("role", "status")));
    }
    Box::new(el("section", children).attr("class", "music-editor listening-phrase"))
}
