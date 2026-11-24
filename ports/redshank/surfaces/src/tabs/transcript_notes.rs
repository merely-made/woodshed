//! Listening notes on the same recording clock as the current transcript passage.

use crate::{
    CompactCommand, FullView, NoteSummaryBody, NowPlaying, RedshankSurfaceState, format_time,
};
use cambium::{button, el, text};
use redshank_model::RepresentationIdentity;

pub(super) fn section(
    state: &RedshankSurfaceState,
    now: &NowPlaying,
    transcript: &timed_text::Transcript,
) -> Option<FullView> {
    // Active cues all contain the playhead, so their union is one interval.
    let (start, end) = transcript
        .cues
        .iter()
        .filter(|cue| cue.active_at(now.position_ms))
        .map(|cue| (cue.start_ms, cue.end_ms))
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))?;
    let mut rows: Vec<FullView> = Vec::new();
    for note in &state.notes {
        let overlaps = match note.end_offset_ms {
            Some(stop) => note.offset_ms < stop && note.offset_ms < end && start < stop,
            None => (start..end).contains(&note.offset_ms),
        };
        if !overlaps
            || note.item_id != now.item_id
            || state.note_identity(&note.id) != RepresentationIdentity::Same
        {
            continue;
        }
        let body = match &note.body {
            NoteSummaryBody::Text(value) => value.as_str(),
            NoteSummaryBody::Voice { .. } => "Voice note",
        };
        let label = format!("{} · {body}", format_time(note.offset_ms));
        let id = note.id.clone();
        rows.push(Box::new(
            button(label.clone(), move |state: &mut RedshankSurfaceState, _| {
                state.request(CompactCommand::OpenNote(id.clone()));
            })
            .attr("class", "rs-row")
            .attr("aria-label", format!("Open passage note {label}")),
        ));
    }
    if rows.is_empty() {
        return None;
    }
    Some(Box::new(
        el(
            "section",
            (el("h4", text("Notes in this transcript passage")), rows),
        )
        .attr("aria-label", "Notes in this transcript passage"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::{
        listen,
        tests_support::{click, commands, markup, playing, runner},
    };
    use crate::{NoteSummary, VoiceNotePreview};
    use redshank_model::{AnnotationId, ItemId, SavedTranscript};

    #[test]
    fn transcript_passage_opens_only_overlapping_notes_with_matching_recording_evidence() {
        let mut state = RedshankSurfaceState {
            compact: playing(),
            ..Default::default()
        };
        let now = state.compact.now_playing.as_mut().unwrap();
        now.position_ms = 2_500;
        let item = now.item_id.clone();
        let source = "WEBVTT\n\n00:01.000 --> 00:03.000\nA\n\n00:02.000 --> 00:04.000\nB";
        state.transcript = Some((
            item.clone(),
            SavedTranscript {
                resource: Default::default(),
                final_url: "https://example.test/words.vtt".into(),
                retrieved_at_ms: 0,
                source: source.into(),
            },
            timed_text::parse(source).unwrap(),
        ));
        for (id, start, end) in [
            ("point", 1_000, None),
            ("voice", 3_500, Some(5_000)),
            ("early", 50, Some(1_000)),
            ("late", 4_000, None),
            ("changed", 2_500, None),
            ("foreign", 2_500, None),
        ] {
            let note_id = AnnotationId(id.into());
            state.note_identities.insert(
                note_id.clone(),
                if id == "changed" {
                    RepresentationIdentity::Different
                } else {
                    RepresentationIdentity::Same
                },
            );
            state.notes.push(NoteSummary {
                id: note_id,
                item_id: if id == "foreign" {
                    ItemId("other".into())
                } else {
                    item.clone()
                },
                offset_ms: start,
                end_offset_ms: end,
                private: true,
                body: if id == "voice" {
                    NoteSummaryBody::Voice {
                        duration_ms: 800,
                        preview: VoiceNotePreview::Idle,
                    }
                } else {
                    NoteSummaryBody::Text(id.into())
                },
            });
        }
        let html = markup(state.clone(), listen::panel);
        assert_eq!(html.matches("aria-label=\"Open passage note ").count(), 2);
        assert!(html.contains("Open passage note 0:03 · Voice note"));
        let mut ui = runner(state, listen::panel);
        click(&mut ui, "Open passage note 0:01 · point");
        assert_eq!(
            commands(&mut ui),
            [CompactCommand::OpenNote(AnnotationId("point".into()))]
        );
    }
}
