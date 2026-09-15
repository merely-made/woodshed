//! Tab bodies. Lane S2 owns this directory.

pub mod controls;
pub mod library;
pub mod listen;
pub mod notes;
pub mod rows;
pub mod settings;

/// Rules for the tab-local classes the bodies emit. `theme::sheet()` must
/// append this to `redshank.css`; it uses only `--t-*`, `--font-*`,
/// `--space-*`, and `--text-*` tokens, no hex, no `outline`, and no
/// `text-overflow`.
pub const TABS_CSS: &str = r#"
/* Track lists are plain `auto | <length> | <percent> | <fr>` words: Livery's
   grid parser splits on whitespace and knows no minmax() or repeat(), and a
   track list it cannot parse leaves the grid with one implicit column. */

/* --- shared row furniture ------------------------------------------- */
.rs-section-head { display: flex; justify-content: space-between; align-items: baseline; }
.rs-empty { border: 1px dashed var(--t-surface-2); color: var(--t-text-dim);
  background: transparent; padding: var(--space-10); font-size: var(--text-ui-13); }
.rs-row-actions { display: flex; gap: var(--space-4); align-items: center; flex: none; }
/* Overflow menu: the trigger always shows, the strip only for the open row. */
.rs-row-menu { position: relative; display: flex; align-items: center; flex: none; }
.rs-row-tail { display: flex; align-items: center; gap: var(--space-6); flex: none; }
.rs-row-more { width: 24px; height: 24px; background: transparent; border: none;
  border-radius: 2px; padding: 0; flex: none; display: grid; place-items: center; }
.rs-more-dots { display: grid; gap: 3px; justify-items: center; }
.rs-more-dot { display: block; width: 3px; height: 3px; border-radius: 50%;
  background: var(--t-text-dim); }
.rs-row-more[aria-expanded="true"] { box-shadow: inset 0 0 0 1px var(--t-text); }
.rs-row-more[aria-expanded="true"] .rs-more-dot { background: var(--t-text); }
/* The panel hangs off the trigger rather than sharing the row: an inline
   strip pushed the title and its listened bar out of the row entirely. */
.rs-row-menu .rs-row-actions { position: absolute; top: 28px; right: 0; z-index: 5;
  white-space: nowrap; background: var(--t-surface-2); border: 1px solid var(--t-text);
  border-radius: 2px; padding: var(--space-4) var(--space-6); }
.rs-row-menu .rs-row-action { color: var(--t-text); }
/* Pinned: a small filled square before the title, never colour alone. */
.rs-pin-mark { display: block; width: 8px; height: 8px; background: var(--t-text);
  flex: none; margin-right: var(--space-4); }
.rs-row-action { background: transparent; border: 1px solid transparent;
  color: var(--t-text-dim); font-size: var(--text-ui-12); padding: 2px 6px; border-radius: 2px; }
.rs-row-action:hover { color: var(--t-text); box-shadow: inset 0 0 0 1px var(--t-surface-2); }
.rs-bar-fill { display: block; height: 3px; background: var(--t-secondary); }
.rs-bar-done { background: var(--t-text-dim); }

/* --- notes rows (Listen and Notes share them) ------------------------ */
.rs-note-row { display: grid; grid-template-columns: auto 1fr auto;
  align-items: center; gap: var(--space-10); }
.rs-note-anchor { display: flex; flex-direction: column; align-items: center; width: 44px; }
.rs-note-time { font-family: var(--font-mono); font-size: var(--text-ui-13); }
.rs-note-text { font-size: var(--text-ui-13); overflow: hidden; }
.rs-note-voice { display: flex; align-items: center; gap: var(--space-6); }
.rs-note-voice-time { font-family: var(--font-mono); font-size: var(--text-ui-12);
  color: var(--t-text-dim); }
.rs-voice-bars { display: flex; gap: 2px; align-items: flex-end; height: 16px; }
.rs-voice-bar { display: block; width: 3px; background: var(--t-text-dim); }
.rs-note-preview { width: 24px; height: 24px; background: transparent;
  box-shadow: inset 0 0 0 1px var(--t-surface-2); border: none; color: var(--t-text);
  font-size: var(--text-ui-12); border-radius: 2px; }
.rs-note-open { background: transparent; border: none; color: var(--t-text);
  box-shadow: inset 0 0 0 1px var(--t-text); border-radius: 2px;
  padding: 4px 10px; font-size: var(--text-ui-12); }

/* --- listen ----------------------------------------------------------- */
.rs-listen { display: grid; grid-template-columns: 38fr 62fr;
  gap: var(--space-20); min-height: 0; }
.rs-listen-queue, .rs-listen-notes { display: flex; flex-direction: column;
  gap: var(--space-6); min-width: 0; }
.rs-listen-row { display: flex; align-items: center; gap: var(--space-6); }
.rs-listen-face { width: 32px; height: 32px; flex: none; }
.rs-listen-cell { flex: 1; min-width: 0; }
.rs-listen-title { display: block; width: 100%; text-align: left; background: transparent;
  border: none; color: var(--t-text); font-size: var(--text-ui-13);
  overflow: hidden; white-space: nowrap; }
.rs-listen-bar { height: 3px; background: var(--t-surface-2); margin-top: 6px; }
.rs-listen-editor { display: flex; flex-direction: column; gap: var(--space-6); }
/* The phone Listen segment: hidden where both sections fit side by side. It
   is a grid item, so it says where it sits: the shared .rs-segment carries
   margin-left:auto for the header, and a stretched grid row makes the strip
   as tall as the panel. */
.rs-listen-pane { display: none; align-self: start; justify-self: start; }
.rs-listen-pane .rs-segment { margin-left: 0; align-self: flex-start; }
.rs-listen-editor-field textarea { width: 100%; min-height: 72px; background: var(--t-bg);
  color: var(--t-text); border: 1px solid var(--t-surface-2); font-family: var(--font-ui); }
.rs-listen-save { background: var(--t-text); color: var(--t-bg); border: none;
  border-radius: 2px; padding: 5px 12px; font-size: var(--text-ui-12); }
@media (max-width: 900px) {
  /* One column, rows sized by their content: without align-content the
     segment's row takes a share of the whole panel height. */
  .rs-listen { grid-template-columns: 1fr; align-content: start; }
  .rs-listen-pane { display: flex; }
  .rs-listen .rs-pane-off { display: none; }
}

/* --- library ---------------------------------------------------------- */
.rs-library { display: grid; grid-template-columns: 40fr 60fr;
  gap: var(--space-20); min-height: 0; }
.rs-library-feeds, .rs-library-episodes { display: flex; flex-direction: column;
  gap: var(--space-6); min-width: 0; }
.rs-library-row { display: flex; align-items: center; gap: var(--space-6); }
.rs-library-face { width: 34px; height: 34px; flex: none; position: relative; }
.rs-library-cell { flex: 1; min-width: 0; }
.rs-library-title { display: block; width: 100%; text-align: left; background: transparent;
  border: none; color: var(--t-text); font-size: var(--text-ui-13);
  overflow: hidden; white-space: nowrap; }
.rs-library-detail { font-size: var(--text-ui-12); color: var(--t-text-dim);
  overflow: hidden; white-space: nowrap; }
.rs-library-field { display: flex; gap: var(--space-6); align-items: center;
  margin-top: var(--space-10); }
.rs-library-field textarea { flex: 1; min-height: 28px; background: var(--t-bg);
  color: var(--t-text); border: 1px dashed var(--t-surface-2); font-family: var(--font-mono);
  font-size: var(--text-ui-12); }
.rs-library-open { background: transparent; border: none; color: var(--t-text-dim);
  font-size: var(--text-ui-12); display: inline-flex; gap: var(--space-6); align-items: center; }
.rs-library-episode { display: flex; align-items: center; gap: var(--space-6); }
.rs-library-progress { width: 60px; height: 3px; background: var(--t-surface-2); flex: none; }
.rs-library-primary { background: var(--t-text); color: var(--t-bg); border: none;
  border-radius: 2px; padding: 5px 12px; font-size: var(--text-ui-12); flex: none; }
.rs-library-primary[aria-disabled="true"] { background: var(--t-surface-2);
  color: var(--t-text-dim); }
.rs-library-head { display: flex; align-items: center; gap: var(--space-6); }
.rs-library-scene { display: none; }
@media (max-width: 600px) {
  .rs-library { grid-template-columns: 1fr; }
  .rs-library-scene { display: block; }
}

/* --- notes ------------------------------------------------------------ */
.rs-notes { display: flex; flex-direction: column; gap: var(--space-10); min-height: 0; }
.rs-notes-head { display: flex; align-items: center; gap: var(--space-10); }
.rs-notes-strip { position: relative; height: 56px; flex: none; margin: var(--space-10) 0; }
.rs-notes-track { display: block; position: absolute; left: 0; width: 100%; top: 28px; height: 4px;
  background: var(--t-surface-2); }
.rs-notes-tick { display: block; position: absolute; top: 22px; width: 2px; height: 16px;
  background: var(--t-text); }
.rs-notes-chip { position: absolute; top: 0; }
.rs-notes-wash { display: block; position: absolute; top: 24px; height: 12px;
  background: var(--t-secondary); opacity: .35; }
.rs-notes-wash-label { position: absolute; top: 40px; font-family: var(--font-mono);
  font-size: var(--text-ui-12); color: var(--t-text-dim); }
.rs-notes-end { position: absolute; top: 44px; font-family: var(--font-mono);
  font-size: var(--text-ui-12); color: var(--t-text-dim); }
.rs-notes-grid { display: grid; grid-template-columns: 55fr 45fr;
  gap: var(--space-20); min-height: 0; }
.rs-notes-list, .rs-notes-side { display: flex; flex-direction: column; gap: var(--space-6);
  min-width: 0; }
.rs-notes-more { font-size: var(--text-ui-12); color: var(--t-text-dim); }
.rs-notes-cluster { display: flex; flex-direction: column; gap: var(--space-6); }
.rs-notes-cluster-head { display: flex; align-items: baseline; gap: var(--space-6); }
.rs-notes-collapse { margin-left: 0; background: transparent; border: none;
  color: var(--t-text-dim); font-size: var(--text-ui-12); padding: 2px 4px;
  border-bottom: 1px solid var(--t-surface-2); }
.rs-notes-representation-body { font-size: var(--text-ui-12); line-height: 1.5; }
.rs-notes-add { align-self: flex-start; background: transparent; color: var(--t-text);
  border: none; box-shadow: inset 0 0 0 1px var(--t-text); border-radius: 2px;
  padding: 8px 12px; font-size: var(--text-ui-13); }
.rs-notes-chip .rs-badge { background: var(--t-text); color: var(--t-bg); border-radius: 2px;
  padding: 0 4px; }
.rs-notes-span-head { display: flex; justify-content: space-between; align-items: baseline; }
.rs-notes-span-body { font-size: var(--text-ui-13); line-height: 1.45; }
@media (max-width: 900px) {
  .rs-notes-grid { grid-template-columns: 1fr; }
}

/* --- settings --------------------------------------------------------- */
.rs-settings { display: grid; grid-template-columns: 1fr 1fr;
  gap: var(--space-20); align-content: start; }
.rs-settings-group { display: flex; flex-direction: column; gap: var(--space-6);
  min-width: 0; }
.rs-settings-row { display: flex; align-items: center; justify-content: space-between;
  gap: var(--space-10); }
.rs-settings-labels { display: flex; flex-direction: column; min-width: 0; flex: 1; }
/* A row control never floats: margin-left:auto pushes an inline-flex segment past the row edge. */
.rs-settings-row .rs-segment { margin-left: 0; display: flex; flex: none; }
.rs-settings-label { font-size: var(--text-ui-13); overflow: hidden; white-space: nowrap; }
.rs-settings-hint { font-size: var(--text-ui-12); color: var(--t-text-dim); }
.rs-settings-swatches { display: flex; gap: var(--space-4); }
.rs-settings-swatch { width: 18px; height: 18px; border-radius: 2px; }
.rs-stepper { display: inline-flex; align-items: center; gap: var(--space-4); flex: none; }
.rs-readout { display: inline-flex; align-items: baseline; flex: none; }
.rs-stepper-step { width: 24px; height: 24px; background: transparent; border: none;
  color: var(--t-text); box-shadow: inset 0 0 0 1px var(--t-surface-2); border-radius: 2px;
  padding: 0; line-height: 24px; text-align: center; }
.rs-stepper-value { display: inline-block; font-family: var(--font-mono); font-size: var(--text-ui-12);
  min-width: 56px; text-align: center; }
.rs-segment-option { background: var(--t-surface); color: var(--t-text-dim); border: none;
  box-shadow: inset 0 0 0 1px var(--t-surface-2); font-size: var(--text-ui-12);
  padding: 4px 10px; }
.rs-segment-on { background: var(--t-text); color: var(--t-bg); }
.rs-readout-value { font-family: var(--font-mono); font-size: var(--text-ui-16); }
.rs-readout-unit { font-family: var(--font-mono); font-size: var(--text-ui-12);
  color: var(--t-text-dim); margin-left: var(--space-4); }
@media (max-width: 900px) {
  .rs-settings { grid-template-columns: 1fr; }
}
/* Phone: action strips wrap under the title instead of squeezing it. */
@media (max-width: 599px) {
  .rs-row { flex-wrap: wrap; }
  .rs-row-actions { width: 100%; justify-content: flex-end; }
}
"#;

#[cfg(test)]
pub mod tests_support {
    //! The headless harness the tab tests share.

    use crate::{
        CompactCommand, CompactPlayerState, Face, FullView, NowPlaying, RedshankSurfaceState,
        SourceKind, TransportState,
    };
    use cambium::{DomHandle, GenetAppRunner, PointerClick};
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::ItemId;
    use std::cell::RefCell;
    use std::rc::Rc;

    pub type Logic = fn(&RedshankSurfaceState) -> FullView;
    pub type Runner = GenetAppRunner<RedshankSurfaceState, Logic, FullView, ()>;

    pub fn playing() -> CompactPlayerState {
        CompactPlayerState {
            transport: TransportState::Playing,
            now_playing: Some(NowPlaying {
                item_id: ItemId("episode-42".into()),
                title: "Wetland".into(),
                feed_title: None,
                face: Face::Tag("m4a".into()),
                source: SourceKind::Local,
                position_ms: 80_400,
                duration_ms: Some(120_000),
                resumed_from_ms: None,
                buffered_percent: 100,
                markers: Vec::new(),
            }),
            ..CompactPlayerState::default()
        }
    }

    pub fn runner(state: RedshankSurfaceState, logic: Logic) -> Runner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        Runner::new(dom, logic, state)
    }

    pub fn markup(state: RedshankSurfaceState, logic: Logic) -> String {
        let runner = runner(state, logic);
        let dom = runner.dom();
        let html = dom.borrow().outer_html(runner.root());
        drop(runner);
        html
    }

    pub fn node_with_label(dom: &ScriptedDom, root: NodeId, label: &str) -> NodeId {
        let aria = LocalName::from("aria-label");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &aria)
                .is_some_and(|value| value == label)
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing control {label}");
    }

    pub fn click(runner: &mut Runner, label: &str) {
        let node = {
            let handle = runner.dom();
            let dom = handle.borrow();
            node_with_label(&dom, runner.root(), label)
        };
        runner.dispatch_click(node, PointerClick::at((1.0, 1.0)));
    }

    /// Click a control and apply the presentation command a host would, so a
    /// view test can reach a state that needs one round trip.
    pub fn act(runner: &mut Runner, label: &str) -> Vec<CompactCommand> {
        click(runner, label);
        let drained = commands(runner);
        runner.update(|state| {
            for command in &drained {
                state.apply_presentation(command);
            }
        });
        drained
    }

    /// Open one row's overflow menu the way a pointer would.
    pub fn open_menu(runner: &mut Runner, subject: &str) {
        act(runner, &format!("More actions for {subject}"));
    }

    pub fn commands(runner: &mut Runner) -> Vec<CompactCommand> {
        let mut drained = Vec::new();
        runner.update(|state| drained.extend(state.drain_commands()));
        drained
    }
}
