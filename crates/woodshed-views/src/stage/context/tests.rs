use super::*;
use woodshed_core::audio::AudioRequest;
use woodshed_core::history::EngagementKind;
use woodshedding::pitch::PitchClass;

#[test]
fn generic_catalog_audition_is_attributed_at_acceptance_to_click_time_focus() {
    let mut ui = UiState::new();
    let original = KeyedCatalogRef {
        formula_id: "chord:Major 7".into(),
        root: PitchClass::new(0),
    };
    assert!(ui.focus_context_catalog(original.clone()));
    ui.audition_context_focus();
    let request = match ui.audio_requests.last().unwrap() {
        AudioRequest::Audition(request) => request.clone(),
        other => panic!("unexpected audio request: {other:?}"),
    };
    let observation = request.observation.as_ref().unwrap();
    assert_eq!(observation.subject_id, original.formula_id);
    assert!(observation.from_id.is_none());
    assert!(observation.provenance.is_some());
    assert!(
        ui.practice_history
            .recent(20)
            .iter()
            .all(|event| { event.kind != EngagementKind::Previewed })
    );

    assert!(ui.focus_context_catalog(KeyedCatalogRef {
        formula_id: "chord:Minor".into(),
        root: PitchClass::new(2),
    }));
    ui.finish_audition_admission(&request, Ok(()));

    let accepted = ui
        .practice_history
        .recent(20)
        .into_iter()
        .find(|event| event.kind == EngagementKind::Previewed)
        .unwrap();
    assert_eq!(accepted.subject_id, original.formula_id);
    assert_eq!(accepted.from_id, None);
    assert!(accepted.provenance.is_some());
}
