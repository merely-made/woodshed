use super::*;
use crate::persona::PracticeSeal;

#[test]
fn the_persona_line_names_who_is_practising_and_what_holds_the_key() {
    // The gap this closes: Settings offered to switch a persona it could
    // not name.
    let mut ui = UiState::new();
    ui.seal = Some(PracticeSeal::Sealed {
        persona: "work".into(),
        protection: "OS auto-unlock sealed records".into(),
    });
    let line = persona_line(&ui);
    assert!(line.contains("work"), "{line}");
    assert!(
        line.contains("OS auto-unlock"),
        "the backend's own words: {line}"
    );
}

#[test]
fn a_pending_session_says_so_and_says_why() {
    let mut ui = UiState::new();
    ui.seal = Some(PracticeSeal::Pending {
        reason: "djinn is not running".into(),
    });
    let line = persona_line(&ui);
    assert!(line.contains("Identity pending"), "{line}");
    assert!(line.contains("not saved"), "never in the clear: {line}");
    assert!(
        line.contains("djinn is not running"),
        "a reason, because 'pending' alone is not actionable: {line}"
    );
}

#[test]
fn a_declined_gate_outranks_any_seal_left_on_the_state() {
    // Declining opens no store, so a seal from before the decline would be
    // a stale claim that this session is being kept.
    let mut ui = UiState::new();
    ui.seal = Some(PracticeSeal::Sealed {
        persona: "work".into(),
        protection: "DPAPI".into(),
    });
    crate::persona::practise_unsaved(&mut ui);
    let line = persona_line(&ui);
    assert!(line.contains("No persona chosen"), "{line}");
    assert!(
        !line.contains("work"),
        "the stale persona must not survive: {line}"
    );
}

#[test]
fn a_store_that_has_not_reported_yet_says_that_rather_than_nothing() {
    let ui = UiState::new();
    assert!(persona_line(&ui).contains("not reported"));
}
