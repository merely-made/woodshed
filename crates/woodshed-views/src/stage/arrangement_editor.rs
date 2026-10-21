//! Musical alignment authoring and shared portable scene presentation.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{
    BackdropSceneEvent, FocusEvent, FocusPhase, History, Key, KeyEvent, TextInput, text_field_typed,
    backdrop_scene_reading, clickable, custom_leaf, el, map_state, on_focus, on_key, on_pointer,
    text,
};
use pictograph::canvas::backdrop_surface::{BackdropSurface, SurfaceTheme};
use scenomise::document::SceneDocument;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
use woodshed_core::{
    arrangement_scene::saved_arrangement_scene, saved_arrangements::save_progression_arrangement,
    working_sets::WorkingSetId,
};
use woodshedding::rehearsal::{CardId, Set, arrangement::*};

mod observation;
#[cfg(test)]
mod observation_tests;
use observation::{ArrangementObservation, DraftSnapshot, PreviewSnapshot};

pub const PRIMARY_KEY: u64 = 0x5753_4152;
pub const RELATED_KEY: u64 = 0x5753_4153;
#[derive(Default)]
pub struct ArrangementSurfaces {
    pub document: Option<SceneDocument>,
    pub primary: Option<BackdropSurface>,
    pub related: Option<BackdropSurface>,
    pub revision: u64,
    pub theme: SurfaceTheme,
    pub error: Option<String>,
}
fn surface(input: &scenomise::backdrops::SceneInput) -> Result<BackdropSurface, String> {
    let mut s = BackdropSurface::new(input).map_err(|e| e.to_string())?;
    s.register_font(
        include_bytes!("../../../../assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf").to_vec(),
    );
    Ok(s)
}
impl ArrangementSurfaces {
    fn install(&mut self, doc: SceneDocument, preserve: bool) -> Result<(), String> {
        doc.validate()?;
        let selection = preserve
            .then(|| {
                self.primary
                    .as_ref()
                    .and_then(|s| s.selected())
                    .map(str::to_owned)
            })
            .flatten();
        let mut primary = surface(&doc.primary)?;
        primary.show_span_handles = true;
        if let Some(key) = selection {
            let _ = primary.select(Some(&key));
        }
        self.primary = Some(primary);
        self.document = Some(doc);
        self.error = None;
        self.sync_related()?;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
    fn sync_related(&mut self) -> Result<(), String> {
        self.related = self
            .document
            .as_ref()
            .and_then(|d| d.for_occurrence(self.primary.as_ref().and_then(|s| s.selected())))
            .map(|v| surface(&v.scene))
            .transpose()?;
        Ok(())
    }
    fn select(&mut self, event: BackdropSceneEvent) {
        if let Some(s) = &mut self.primary {
            let result = match event {
                BackdropSceneEvent::Select(key) => s.select(Some(&key)),
                BackdropSceneEvent::Clear => s.select(None),
                BackdropSceneEvent::DismissRetained => {
                    s.dismiss_retained();
                    Ok(())
                },
            };
            self.error = result.err();
            if let Err(e) = self.sync_related() {
                self.error = Some(e);
            }
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

pub struct Draft {
    context: SavedProgressionArrangement,
    base: Option<SavedProgressionArrangement>,
    pub fields: BTreeMap<String, TextInput>,
    cards: Vec<CardId>,
    placed: BTreeSet<CardId>,
    rests: Vec<String>,
    original: Option<DraftSnapshot>,
    history: History<ArrangementSnapshot, EditKey>,
    history_snapshot: ArrangementSnapshot,
    focused_field: Rc<RefCell<FocusTracker>>,
    bar_counts: TextInput,
    selected_bars: Vec<usize>,
    grouping_preview: Option<GroupingPreview>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ArrangementSnapshot {
    fields: BTreeMap<String, String>,
    cards: Vec<CardId>,
    placed: BTreeSet<CardId>,
    rests: Vec<String>,
    time_signatures: Vec<TimeSignatureChange>,
    bar_groups: Vec<BarGroup>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum EditKey {
    Field(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct GroupingPreview {
    source: DraftSnapshot,
    selected: Vec<usize>,
    counts: Vec<u32>,
    before: Vec<BarGroup>,
    after: Vec<BarGroup>,
    conflict: bool,
    use_update: bool,
}
#[derive(Default)]
struct FocusTracker {
    field: Option<String>,
    break_run: bool,
}
impl Draft {
    fn from_saved(
        context: SavedProgressionArrangement,
        base: Option<SavedProgressionArrangement>,
        set: &Set,
    ) -> Self {
        let mut d = Self {
            context,
            base,
            fields: BTreeMap::new(),
            cards: vec![],
            placed: BTreeSet::new(),
            rests: vec![],
            original: None,
            history: History::new(),
            history_snapshot: ArrangementSnapshot {
                fields: BTreeMap::new(),
                cards: vec![],
                placed: BTreeSet::new(),
                rests: vec![],
                time_signatures: vec![],
                bar_groups: vec![],
            },
            focused_field: Rc::new(RefCell::new(FocusTracker::default())),
            bar_counts: TextInput::new("2 + 2"),
            selected_bars: vec![],
            grouping_preview: None,
        };
        let a = d.context.alignment.clone();
        d.put("name", a.name);
        d.put("extent", a.extent);
        for (i, s) in a.role_spans.iter().enumerate() {
            d.span_fields(&format!("role-{i}"), *s);
        }
        for b in a.cards {
            d.card_fields(b.card, b.placement, b.role);
        }
        for r in a.rests {
            d.span_fields(&format!("rest-{}", r.key), r.placement.span);
            d.put(
                &format!("rest-{}-role", r.key),
                r.role.map(|i| (i + 1).to_string()).unwrap_or_default(),
            );
            d.rests.push(r.key);
        }
        d.add_current_cards(set);
        d.original = Some(d.snapshot());
        d.history_snapshot = d.history_snapshot();
        d
    }
    fn put(&mut self, key: &str, value: impl ToString) {
        self.fields
            .insert(key.into(), TextInput::new(value.to_string()));
    }
    fn span_fields(&mut self, key: &str, s: Span) {
        self.put(&format!("{key}-start"), s.start);
        self.put(&format!("{key}-length"), s.length);
    }
    fn card_fields(&mut self, id: CardId, p: Option<Placement>, role: Option<usize>) {
        if self.cards.contains(&id) {
            return;
        }
        let prefix = format!("card-{}", id.0);
        let index = self.cards.len() as u32;
        if p.is_some() {
            self.placed.insert(id);
        }
        self.span_fields(
            &prefix,
            p.map(|p| p.span).unwrap_or(Span {
                start: index * 16,
                length: 16,
            }),
        );
        self.put(
            &format!("{prefix}-role"),
            role.map(|i| (i + 1).to_string()).unwrap_or_default(),
        );
        self.cards.push(id);
    }
    fn add_current_cards(&mut self, set: &Set) {
        for card in &set.cards {
            self.card_fields(card.id, None, None);
        }
    }
    fn value(&self, key: &str) -> &str {
        self.fields.get(key).map(|f| f.text()).unwrap_or("")
    }
    fn number(&self, key: &str) -> Result<u32, String> {
        self.value(key)
            .trim()
            .parse()
            .map_err(|_| format!("{key}: enter a whole number of ticks"))
    }
    fn span(&self, key: &str) -> Result<Span, String> {
        Ok(Span {
            start: self.number(&format!("{key}-start"))?,
            length: self.number(&format!("{key}-length"))?,
        })
    }
    fn role(&self, key: &str) -> Result<Option<usize>, String> {
        let v = self.value(key).trim();
        if v.is_empty() {
            return Ok(None);
        }
        let n: usize = v.parse().map_err(|_| "Role must be a number, or empty")?;
        if n == 0 || n > self.context.roles.len() {
            return Err("Role number is outside the saved template".into());
        }
        Ok(Some(n - 1))
    }
    fn alignment(&self) -> Result<ProgressionArrangement, String> {
        Ok(ProgressionArrangement {
            name: self.value("name").into(),
            extent: self.number("extent")?,
            role_spans: (0..self.context.roles.len())
                .map(|i| self.span(&format!("role-{i}")))
                .collect::<Result<_, _>>()?,
            cards: self
                .cards
                .iter()
                .map(|id| {
                    Ok(CardAlignment {
                        card: *id,
                        placement: if self.placed.contains(id) {
                            Some(Placement {
                                lane: 0,
                                span: self.span(&format!("card-{}", id.0))?,
                            })
                        } else {
                            None
                        },
                        role: self.role(&format!("card-{}-role", id.0))?,
                    })
                })
                .collect::<Result<_, String>>()?,
            rests: self
                .rests
                .iter()
                .map(|key| {
                    Ok(ArrangementRest {
                        key: key.clone(),
                        placement: Placement {
                            lane: 0,
                            span: self.span(&format!("rest-{key}"))?,
                        },
                        role: self.role(&format!("rest-{key}-role"))?,
                    })
                })
                .collect::<Result<_, String>>()?,
            time_signatures: self.context.alignment.time_signatures.clone(),
            bar_groups: self.context.alignment.bar_groups.clone(),
        })
    }
    fn snapshot(&self) -> DraftSnapshot {
        DraftSnapshot::capture(self)
    }
    fn history_snapshot(&self) -> ArrangementSnapshot {
        ArrangementSnapshot {
            fields: self
                .fields
                .iter()
                .map(|(key, value)| (key.clone(), value.text().to_owned()))
                .collect(),
            cards: self.cards.clone(),
            placed: self.placed.clone(),
            rests: self.rests.clone(),
            time_signatures: self.context.alignment.time_signatures.clone(),
            bar_groups: self.context.alignment.bar_groups.clone(),
        }
    }
    fn observe_edit(&mut self) {
        let current = self.history_snapshot();
        if current == self.history_snapshot {
            return;
        }
        let mut focus = self.focused_field.borrow_mut();
        if focus.break_run {
            self.history.break_run();
            focus.break_run = false;
        }
        let coalesce_field = focus.field.as_ref().filter(|focused| {
            current.cards == self.history_snapshot.cards
                && current.placed == self.history_snapshot.placed
                && current.rests == self.history_snapshot.rests
                && current.time_signatures == self.history_snapshot.time_signatures
                && current.bar_groups == self.history_snapshot.bar_groups
                && current.fields.iter().all(|(key, value)| {
                    key == *focused || self.history_snapshot.fields.get(key) == Some(value)
                })
                && current.fields.len() == self.history_snapshot.fields.len()
        });
        let key = coalesce_field.map(|field| EditKey::Field(field.clone()));
        if key.is_none() {
            self.history.break_run();
        }
        self.history.record(self.history_snapshot.clone(), key, 0);
        self.history_snapshot = current;
    }
    fn restore(&mut self, snapshot: ArrangementSnapshot) {
        self.fields = snapshot
            .fields
            .iter()
            .map(|(key, value)| (key.clone(), TextInput::new(value)))
            .collect();
        self.cards = snapshot.cards.clone();
        self.placed = snapshot.placed.clone();
        self.rests = snapshot.rests.clone();
        self.context.alignment.time_signatures = snapshot.time_signatures.clone();
        self.context.alignment.bar_groups = snapshot.bar_groups.clone();
        self.grouping_preview = None;
        self.history_snapshot = snapshot;
    }
    fn parse_group_counts(&self) -> Result<Vec<u32>, String> {
        let pattern = self.bar_counts.text();
        if pattern.len() > 1024 {
            return Err("Bar grouping is too long".into());
        }
        let counts: Vec<u32> = pattern
            .split(|c: char| c.is_whitespace() || c == '+')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().map_err(|_| "Use counts such as 2 + 2".to_string()))
            .collect::<Result<_, _>>()?;
        if counts.is_empty() || counts.contains(&0) {
            return Err("Use positive bar counts such as 2 + 2".into());
        }
        Ok(counts)
    }
    fn preview_grouping(&mut self) -> Result<(), String> {
        let alignment = self.alignment()?;
        let counts = self.parse_group_counts()?;
        let after = woodshedding::rehearsal::arrangement::bars::regroup(
            &alignment,
            &self.selected_bars,
            &counts,
        )?;
        let first = *self.selected_bars.first().ok_or("Select bars to group")? as u32;
        let end = (*self.selected_bars.last().unwrap() + 1) as u32;
        let before: Vec<_> = alignment
            .bar_groups
            .iter()
            .copied()
            .filter(|group| group.first >= first && group.first + group.count <= end)
            .collect();
        let after_selected: Vec<_> = after
            .iter()
            .copied()
            .filter(|group| group.first >= first && group.first + group.count <= end)
            .collect();
        let conflict = !before.is_empty() && before != after_selected;
        self.grouping_preview = Some(GroupingPreview {
            source: self.snapshot(),
            selected: self.selected_bars.clone(),
            counts,
            before,
            after,
            conflict,
            use_update: !conflict,
        });
        Ok(())
    }
    fn apply_grouping(&mut self) -> Result<(), String> {
        let preview = self
            .grouping_preview
            .clone()
            .ok_or("Preview a bar grouping first")?;
        if self.snapshot() != preview.source {
            return Err("Draft changed since this grouping preview. Preview again.".into());
        }
        if self.selected_bars != preview.selected || self.parse_group_counts()? != preview.counts {
            return Err("Grouping selection or counts changed since preview. Preview again.".into());
        }
        if preview.conflict && !preview.use_update {
            self.grouping_preview = None;
            return Ok(());
        }
        self.context.alignment.bar_groups = preview.after;
        self.grouping_preview = None;
        Ok(())
    }
    pub fn dirty(&self) -> bool {
        self.base.as_ref() != Some(&self.context)
            || self
                .original
                .as_ref()
                .is_none_or(|original| !original.matches(self))
    }
    fn candidate(&self, set: &Set) -> Result<Set, String> {
        if set.progression_arrangement != self.base {
            return Err(
                "The saved arrangement changed. Reload it before saving this draft.".into(),
            );
        }
        let mut next = set.clone();
        let mut saved = self.context.clone();
        saved.alignment = self.alignment()?;
        next.progression_arrangement = Some(saved);
        Ok(next)
    }
}
pub use scenomise::span_edit::SpanEdit as SpanAction;
use scenomise::span_edit::SpanEditIntent;
#[derive(Clone, Debug)]
struct SpanGesture {
    observation: ArrangementObservation,
    key: String,
    action: SpanAction,
    original: Span,
    proposed: Span,
    intent: SpanEditIntent,
    origin_x: f32,
    ticks_per_pixel: f32,
    size: (f32, f32),
    pointer: i32,
    extent: u32,
    viewport_width: f32,
}
impl Draft {
    fn occurrence_span(&self, key: &str) -> Option<Span> {
        let card = self
            .cards
            .iter()
            .any(|id| self.placed.contains(id) && key == format!("card-{}", id.0));
        let rest = self.rests.iter().any(|k| key == format!("rest-{k}"));
        (card || rest).then(|| self.span(key).ok()).flatten()
    }
    fn candidate_intent(
        &self,
        set: &Set,
        intent: &SpanEditIntent,
        reading_id: &str,
    ) -> Result<Set, String> {
        let mut next = self.candidate(set)?;
        intent.validate(&saved_arrangement_scene(&next, reading_id)?.primary)?;
        let key = intent.occurrence.as_str();
        let span = Span {
            start: intent.after.start,
            length: intent.after.length,
        };
        self.occurrence_span(key)
            .ok_or("Select a placed Card or rest to edit")?;
        let alignment = &mut next.progression_arrangement.as_mut().unwrap().alignment;
        if let Some(card) = alignment
            .cards
            .iter_mut()
            .find(|c| key == format!("card-{}", c.card.0))
        {
            card.placement.as_mut().ok_or("Card is unplaced")?.span = span;
        } else if let Some(rest) = alignment
            .rests
            .iter_mut()
            .find(|r| key == format!("rest-{}", r.key))
        {
            rest.placement.span = span;
        } else {
            return Err("Occurrence is no longer in the draft".into());
        }
        saved_arrangement_scene(&next, "native-span-validation")?;
        Ok(next)
    }
}
#[derive(Default)]
pub struct ArrangementEditor {
    pub drafts: BTreeMap<WorkingSetId, Draft>,
    pub surfaces: Rc<RefCell<ArrangementSurfaces>>,
    preview: Option<(PreviewSnapshot, Option<GroupingPreview>, String)>,
    presentation_owner: Option<WorkingSetId>,
    pub notice: Option<String>,
    pub close_pending: bool,
    gesture: Option<SpanGesture>,
}
impl ArrangementEditor {
    pub fn preview_active(&self) -> bool {
        self.gesture.is_some()
    }
    pub fn dirty(&self) -> bool {
        self.drafts.values().any(Draft::dirty)
            || self
                .gesture
                .as_ref()
                .is_some_and(|g| g.original != g.proposed)
    }
    pub fn field(&self, owner: WorkingSetId, key: &str) -> Option<&TextInput> {
        let draft = self.drafts.get(&owner)?;
        if key == "bar-counts" {
            Some(&draft.bar_counts)
        } else {
            draft.fields.get(key)
        }
    }
    pub fn field_mut(&mut self, owner: WorkingSetId, key: &str) -> Option<&mut TextInput> {
        // map_state also borrows fields while rebuilding an unchanged view.
        // Actual text changes invalidate the captured observation in sync.
        let draft = self.drafts.get_mut(&owner)?;
        if key == "bar-counts" {
            Some(&mut draft.bar_counts)
        } else {
            draft.fields.get_mut(key)
        }
    }
    pub fn group_preview_active(&self, owner: WorkingSetId) -> bool {
        self.drafts.get(&owner).is_some_and(|draft| draft.grouping_preview.is_some())
    }
    pub fn grouping_text(&self, owner: WorkingSetId) -> Option<String> {
        let draft = self.drafts.get(&owner)?;
        let groups = &draft.context.alignment.bar_groups;
        Some(groups.iter().map(|group| group.count.to_string()).collect::<Vec<_>>().join(" + "))
    }
    pub fn can_undo(&self, owner: WorkingSetId) -> bool {
        self.drafts.get(&owner).is_some_and(|draft| draft.history.can_undo())
    }
    pub fn can_redo(&self, owner: WorkingSetId) -> bool {
        self.drafts.get(&owner).is_some_and(|draft| draft.history.can_redo())
    }
}
impl UiState {
    fn capture_arrangement_history(&mut self, owner: WorkingSetId) {
        if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
            draft.observe_edit();
            draft.history.break_run();
        }
    }
    pub fn start_arrangement(&mut self) {
        self.music_editor.legacy = true;
        let owner = self.working_sets.active_id;
        if self
            .arrangement_editor
            .drafts
            .get(&owner)
            .is_some_and(Draft::dirty)
        {
            self.arrangement_editor.notice =
                Some("Save or reload this draft before starting another template.".into());
            return;
        }
        let result = (|| {
            let progression = self
                .stage
                .progression_idx
                .and_then(|i| self.stage.progressions().get(i))
                .ok_or("Choose a progression in Practice first")?;
            let count = progression.roles.len() as u32;
            let a = ProgressionArrangement {
                name: format!("{} arrangement", self.working_sets.active_name),
                extent: count * 16,
                role_spans: (0..count)
                    .map(|i| Span {
                        start: i * 16,
                        length: 16,
                    })
                    .collect(),
                cards: vec![],
                rests: vec![],
                time_signatures: vec![],
                bar_groups: vec![],
            };
            let mut copy = self.set.clone();
            save_progression_arrangement(&self.stage, &mut copy, a)?;
            Ok::<_, String>(Draft::from_saved(
                copy.progression_arrangement.unwrap(),
                self.set.progression_arrangement.clone(),
                &self.set,
            ))
        })();
        match result {
            Ok(d) => {
                self.arrangement_editor.drafts.insert(owner, d);
                self.arrangement_editor.notice = None;
            },
            Err(e) => self.arrangement_editor.notice = Some(e),
        }
        self.sync_arrangement_scene();
    }
    pub fn reload_arrangement(&mut self) {
        self.arrangement_editor.gesture = None;
        let owner = self.working_sets.active_id;
        self.arrangement_editor.drafts.remove(&owner);
        self.arrangement_editor.preview = None;
        self.arrangement_editor.notice = None;
        self.sync_arrangement_scene();
    }
    pub fn undo_arrangement_edit(&mut self) {
        let owner = self.working_sets.active_id;
        if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
            draft.observe_edit();
        }
        let restored = self
            .arrangement_editor
            .drafts
            .get_mut(&owner)
            .and_then(|draft| draft.history.undo(draft.history_snapshot()));
        if let Some(snapshot) = restored {
            if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
                draft.restore(snapshot);
                let mut focus = draft.focused_field.borrow_mut();
                focus.field = None;
                focus.break_run = true;
            }
            self.arrangement_editor.notice = Some("Arrangement edit undone.".into());
            self.sync_arrangement_scene();
        }
    }
    pub fn redo_arrangement_edit(&mut self) {
        let owner = self.working_sets.active_id;
        if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
            draft.observe_edit();
        }
        let restored = self
            .arrangement_editor
            .drafts
            .get_mut(&owner)
            .and_then(|draft| draft.history.redo(draft.history_snapshot()));
        if let Some(snapshot) = restored {
            if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
                draft.restore(snapshot);
                let mut focus = draft.focused_field.borrow_mut();
                focus.field = None;
                focus.break_run = true;
            }
            self.arrangement_editor.notice = Some("Arrangement edit redone.".into());
            self.sync_arrangement_scene();
        }
    }
    pub fn select_arrangement_bar(&mut self, index: usize) {
        let owner = self.working_sets.active_id;
        if let Some(draft) = self.arrangement_editor.drafts.get_mut(&owner) {
            if draft.selected_bars.contains(&index) {
                draft.selected_bars.retain(|selected| *selected != index);
            } else {
                draft.selected_bars.push(index);
                draft.selected_bars.sort_unstable();
            }
            draft.grouping_preview = None;
        }
        self.sync_arrangement_scene();
    }
    pub fn preview_arrangement_grouping(&mut self) {
        let owner = self.working_sets.active_id;
        let result = self
            .arrangement_editor
            .drafts
            .get_mut(&owner)
            .ok_or("Start an arrangement first".to_string())
            .and_then(Draft::preview_grouping);
        match result {
            Ok(()) => {
                self.arrangement_editor.notice = None;
                self.sync_arrangement_scene();
            },
            Err(error) => self.arrangement_editor.notice = Some(error),
        }
    }
    pub fn choose_arrangement_grouping(&mut self, use_update: bool) {
        let owner = self.working_sets.active_id;
        if let Some(preview) = self
            .arrangement_editor
            .drafts
            .get_mut(&owner)
            .and_then(|draft| draft.grouping_preview.as_mut())
        {
            preview.use_update = use_update;
        }
        self.sync_arrangement_scene();
    }
    pub fn apply_arrangement_grouping(&mut self) {
        let owner = self.working_sets.active_id;
        let kept_local = self
            .arrangement_editor
            .drafts
            .get(&owner)
            .and_then(|draft| draft.grouping_preview.as_ref())
            .is_some_and(|preview| preview.conflict && !preview.use_update);
        let result = self
            .arrangement_editor
            .drafts
            .get_mut(&owner)
            .ok_or("Start an arrangement first".to_string())
            .and_then(|draft| {
                draft.candidate(&self.set)?;
                draft.observe_edit();
                draft.history.break_run();
                draft.apply_grouping()
            });
        match result {
            Ok(()) => {
                if kept_local {
                    self.arrangement_editor.notice = Some("Existing bar grouping kept.".into());
                    self.sync_arrangement_scene();
                } else {
                    self.arrangement_editor.notice = Some("Bar grouping choice applied to the draft.".into());
                    self.sync_arrangement_scene();
                }
            },
            Err(error) => self.arrangement_editor.notice = Some(error),
        }
    }
    pub fn cancel_arrangement_grouping(&mut self) {
        if let Some(draft) = self
            .arrangement_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            draft.grouping_preview = None;
        }
        self.arrangement_editor.notice = Some("Bar grouping preview cancelled.".into());
        self.sync_arrangement_scene();
    }
    pub fn save_arrangement(&mut self) {
        self.commit_arrangement_gesture();
        self.capture_arrangement_history(self.working_sets.active_id);
        let result = (|| {
            let d = self
                .arrangement_editor
                .drafts
                .get(&self.working_sets.active_id)
                .ok_or("Start an arrangement first")?;
            let next = d.candidate(&self.set)?;
            saved_arrangement_scene(&next, "native-validation")?;
            Ok::<_, String>(next.progression_arrangement)
        })();
        match result {
            Ok(saved) => {
                self.set.progression_arrangement = saved.clone();
                if let Some(draft) = self
                    .arrangement_editor
                    .drafts
                    .get_mut(&self.working_sets.active_id)
                {
                    if let Some(saved) = saved {
                        draft.context = saved;
                    }
                    draft.base = self.set.progression_arrangement.clone();
                    draft.original = Some(draft.snapshot());
                    draft.history_snapshot = draft.history_snapshot();
                    draft.history.mark_saved();
                    draft.grouping_preview = None;
                }
                self.arrangement_editor.notice =
                    Some("Arrangement saved to this working Set.".into());
            },
            Err(e) => self.arrangement_editor.notice = Some(e),
        }
    }
    pub fn sync_arrangement_scene(&mut self) {
        if self.workspace.active_panel() != Some(WorkspacePanel::Arrangement) {
            self.arrangement_editor.gesture = None;
            return;
        }
        let owner = self.working_sets.active_id;
        if !self.arrangement_editor.drafts.contains_key(&owner) {
            if let Some(saved) = &self.set.progression_arrangement {
                self.arrangement_editor.drafts.insert(
                    owner,
                    Draft::from_saved(saved.clone(), Some(saved.clone()), &self.set),
                );
            }
        }
        if let Some(d) = self.arrangement_editor.drafts.get_mut(&owner) {
            // Capture edits to the current draft before Set membership adds
            // unplaced, source-owned Card rows. Those rows are view inputs,
            // not arrangement history edits.
            d.observe_edit();
            d.add_current_cards(&self.set);
            d.history_snapshot = d.history_snapshot();
        }
        let observation = self.arrangement_observation();
        if self
            .arrangement_editor
            .gesture
            .as_ref()
            .is_some_and(|g| !self.arrangement_gesture_matches(g, &observation))
        {
            self.arrangement_editor.gesture = None;
        }
        let palette = self
            .appearance
            .resolve(&self.app_settings.appearance)
            .palette;
        let rgba = |c| {
            let h = tinct::color_to_hex(c);
            [
                u8::from_str_radix(&h[1..3], 16).unwrap() as f32 / 255.0,
                u8::from_str_radix(&h[3..5], 16).unwrap() as f32 / 255.0,
                u8::from_str_radix(&h[5..7], 16).unwrap() as f32 / 255.0,
                1.0,
            ]
        };
        let mut guide = rgba(palette.text_dim);
        guide[3] = 0.28;
        let theme = SurfaceTheme {
            ground: rgba(palette.surface),
            text: rgba(palette.text),
            guide,
            category: rgba(palette.secondary),
            root: rgba(palette.tertiary),
            fifth: rgba(palette.primary),
            warning: rgba(palette.danger),
        };
        let preview = PreviewSnapshot::capture(
            observation,
            &theme,
            self.arrangement_editor
                .gesture
                .as_ref()
                .filter(|g| g.proposed != g.original)
                .map(|g| (g.key.clone(), g.proposed)),
        );
        let draft = self.arrangement_editor.drafts.get(&owner);
        let preview = (
            preview,
            draft.and_then(|draft| draft.grouping_preview.clone()),
            draft
                .map(|draft| draft.bar_counts.text().to_owned())
                .unwrap_or_default(),
        );
        if self.arrangement_editor.preview.as_ref() == Some(&preview) {
            return;
        }
        self.arrangement_editor.notice = None;
        let preserve = self.arrangement_editor.presentation_owner == Some(owner);
        self.arrangement_editor.presentation_owner = Some(owner);
        self.arrangement_editor.preview = Some(preview);
        let result = self
            .arrangement_editor
            .drafts
            .get(&owner)
            .ok_or("Start an arrangement to preview it".to_string())
            .and_then(|d| {
                if let Some(g) = &self.arrangement_editor.gesture {
                    d.candidate_intent(
                        &self.set,
                        &g.intent,
                        &format!("woodshed/working-set/{}", g.observation.owner.0),
                    )
                } else {
                    let mut candidate = d.candidate(&self.set)?;
                    if let Some(preview) = &d.grouping_preview {
                        let preview_is_current = d.snapshot() == preview.source
                            && d.selected_bars == preview.selected
                            && d.parse_group_counts().ok().as_ref() == Some(&preview.counts)
                            && (!preview.conflict || preview.use_update);
                        if preview_is_current {
                            if let Some(arrangement) = candidate.progression_arrangement.as_mut() {
                                arrangement.alignment.bar_groups = preview.after.clone();
                            }
                        }
                    }
                    Ok(candidate)
                }
            })
            .and_then(|set| {
                saved_arrangement_scene(&set, &format!("woodshed/working-set/{}", owner.0))
            });
        let mut surfaces = self.arrangement_editor.surfaces.borrow_mut();
        surfaces.theme = theme;
        match result {
            Ok(doc) => {
                if let Err(e) = surfaces.install(doc, preserve) {
                    surfaces.primary = None;
                    surfaces.related = None;
                    surfaces.document = None;
                    surfaces.error = Some(e);
                    surfaces.revision = surfaces.revision.wrapping_add(1);
                }
            },
            Err(e) => {
                surfaces.primary = None;
                surfaces.related = None;
                surfaces.document = None;
                surfaces.error = Some(e);
                surfaces.revision = surfaces.revision.wrapping_add(1);
            },
        }
    }
    fn arrangement_observation(&self) -> ArrangementObservation {
        let owner = self.working_sets.active_id;
        ArrangementObservation::capture(
            owner,
            &self.set,
            self.arrangement_editor.drafts.get(&owner),
        )
    }
    fn arrangement_gesture_current(&self, g: &SpanGesture) -> bool {
        self.arrangement_gesture_matches(g, &self.arrangement_observation())
    }
    fn arrangement_gesture_matches(
        &self,
        g: &SpanGesture,
        observation: &ArrangementObservation,
    ) -> bool {
        self.workspace.active_panel() == Some(WorkspacePanel::Arrangement)
            && self.viewport_width == g.viewport_width
            && self.persona.is_none()
            && !self.appearance.workshop_open
            && g.observation == *observation
    }
    fn commit_arrangement_gesture(&mut self) {
        if let Some(g) = self.arrangement_editor.gesture.take() {
            if self.arrangement_gesture_current(&g) && g.proposed != g.original {
                let d = self
                    .arrangement_editor
                    .drafts
                    .get_mut(&g.observation.owner)
                    .unwrap();
                d.observe_edit();
                d.history.break_run();
                match d.candidate_intent(
                    &self.set,
                    &g.intent,
                    &format!("woodshed/working-set/{}", g.observation.owner.0),
                ) {
                    Ok(_) => d.span_fields(&g.key, g.proposed),
                    Err(e) => self.arrangement_editor.notice = Some(e),
                }
            }
        }
        self.sync_arrangement_scene();
    }
    pub fn cancel_arrangement_gesture(&mut self) {
        self.arrangement_editor.gesture = None;
        self.sync_arrangement_scene();
    }
    /// Captured pointer route. A preview never writes draft fields or saved data.
    pub fn arrangement_pointer(&mut self, event: cambium::PointerEvent) {
        use cambium::{PointerButton, PointerPhase};
        if event.button != PointerButton::Primary {
            return;
        }
        if event.phase == PointerPhase::Cancel {
            if self
                .arrangement_editor
                .gesture
                .as_ref()
                .is_some_and(|g| g.pointer == event.pointer_id)
            {
                self.cancel_arrangement_gesture();
            }
            return;
        }
        if !event.local.0.is_finite()
            || !event.local.1.is_finite()
            || !event.size.0.is_finite()
            || !event.size.1.is_finite()
        {
            return;
        }
        if event.phase == PointerPhase::Down {
            self.cancel_arrangement_gesture();
            let (key, geometry) = {
                let surfaces = self.arrangement_editor.surfaces.borrow();
                let Some(s) = &surfaces.primary else {
                    return;
                };
                let (w, h) = (event.size.0.round() as u32, event.size.1.round() as u32);
                let key = s
                    .pick(event.local.0, event.local.1, w, h)
                    .map(str::to_owned);
                let geometry = key
                    .as_ref()
                    .and_then(|k| s.occurrence_rect(k, w, h))
                    .and_then(|rect| {
                        let a = s.scene_point(0.0, 0.0, w, h)?;
                        let b = s.scene_point(1.0, 0.0, w, h)?;
                        Some((rect, (b.x - a.x) * 4.0 / 47.0))
                    });
                (key, geometry)
            };
            self.arrangement_select(
                key.clone()
                    .map(BackdropSceneEvent::Select)
                    .unwrap_or(BackdropSceneEvent::Clear),
            );
            if let (Some(key), Some((rect, ticks_per_pixel))) = (key, geometry) {
                let owner = self.working_sets.active_id;
                let Some(d) = self.arrangement_editor.drafts.get(&owner) else {
                    return;
                };
                if let (Some(original), Ok(extent)) = (d.occurrence_span(&key), d.number("extent"))
                {
                    let edge = 8.0_f32.min(rect.width / 3.0);
                    let action = if event.local.0 <= rect.x + edge {
                        SpanAction::Start
                    } else if event.local.0 >= rect.x + rect.width - edge {
                        SpanAction::End
                    } else {
                        SpanAction::Move
                    };
                    let input = self
                        .arrangement_editor
                        .surfaces
                        .borrow()
                        .document
                        .as_ref()
                        .unwrap()
                        .primary
                        .clone();
                    let Ok(intent) = SpanEditIntent::propose(&input, &key, action, 0.0, 4) else {
                        return;
                    };
                    self.arrangement_editor.gesture = Some(SpanGesture {
                        observation: self.arrangement_observation(),
                        key,
                        action,
                        original,
                        proposed: original,
                        intent,
                        origin_x: event.local.0,
                        ticks_per_pixel,
                        size: event.size,
                        pointer: event.pointer_id,
                        extent,
                        viewport_width: self.viewport_width,
                    });
                }
            }
        } else if let Some(mut g) = self.arrangement_editor.gesture.take() {
            if event.pointer_id != g.pointer {
                self.arrangement_editor.gesture = Some(g);
                return;
            }
            if event.phase != PointerPhase::Cancel
                && self.arrangement_gesture_current(&g)
                && event.size == g.size
            {
                let dx = event.local.0 - g.origin_x;
                g.proposed = if dx.abs() < 3.0 {
                    g.original
                } else {
                    let Ok(span) = g.action.apply(
                        scenomise::template::Span {
                            start: g.original.start,
                            length: g.original.length,
                        },
                        dx * g.ticks_per_pixel,
                        g.extent,
                        4,
                    ) else {
                        return;
                    };
                    Span {
                        start: span.start,
                        length: span.length,
                    }
                };
                g.intent.after = scenomise::template::Span {
                    start: g.proposed.start,
                    length: g.proposed.length,
                };
                self.arrangement_editor.gesture = Some(g);
                if event.phase == PointerPhase::Up {
                    self.commit_arrangement_gesture();
                }
            }
        }
        event.prop.prevent_default();
        self.sync_arrangement_scene();
    }
    pub fn edit_arrangement_span(&mut self, action: SpanAction, beats: i32) {
        self.cancel_arrangement_gesture();
        let key = self
            .arrangement_editor
            .surfaces
            .borrow()
            .primary
            .as_ref()
            .and_then(|s| s.selected())
            .map(str::to_owned);
        let Some(key) = key else {
            return;
        };
        let Some(d) = self
            .arrangement_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        else {
            return;
        };
        d.observe_edit();
        d.history.break_run();
        if let (Some(original), Ok(extent)) = (d.occurrence_span(&key), d.number("extent")) {
            if original.length == 0
                || original
                    .start
                    .checked_add(original.length)
                    .is_none_or(|end| end > extent)
            {
                return;
            }
            let reading_id = format!("woodshed/working-set/{}", self.working_sets.active_id.0);
            let result = d
                .candidate(&self.set)
                .and_then(|set| saved_arrangement_scene(&set, &reading_id))
                .and_then(|doc| {
                    SpanEditIntent::propose(&doc.primary, &key, action, beats as f32 * 4.0, 4)
                });
            let Ok(intent) = result else {
                return;
            };
            let proposed = Span {
                start: intent.after.start,
                length: intent.after.length,
            };
            match d.candidate_intent(&self.set, &intent, &reading_id) {
                Ok(_) => d.span_fields(&key, proposed),
                Err(e) => self.arrangement_editor.notice = Some(e),
            }
            self.sync_arrangement_scene();
        }
    }
    pub fn arrangement_key(&mut self, event: cambium::KeyEvent) {
        use cambium::{Key, NamedKey};
        match event.key {
            Key::Named(NamedKey::Escape) if self.arrangement_editor.gesture.is_some() => {
                self.cancel_arrangement_gesture()
            },
            Key::Named(NamedKey::Enter) if self.arrangement_editor.gesture.is_some() => {
                self.commit_arrangement_gesture()
            },
            Key::Named(key @ (NamedKey::ArrowLeft | NamedKey::ArrowRight))
                if !event.mods.ctrl && !event.mods.meta =>
            {
                let action = if event.mods.alt {
                    SpanAction::Start
                } else if event.mods.shift {
                    SpanAction::End
                } else {
                    SpanAction::Move
                };
                self.edit_arrangement_span(action, if key == NamedKey::ArrowLeft { -1 } else { 1 });
            },
            _ => return,
        }
        event.prevent_default();
        event.stop_propagation();
    }
    pub fn arrangement_select(&mut self, event: BackdropSceneEvent) {
        self.cancel_arrangement_gesture();
        self.arrangement_editor.surfaces.borrow_mut().select(event);
    }
    pub fn place_arrangement_cards_in_order(&mut self) {
        self.capture_arrangement_history(self.working_sets.active_id);
        if let Some(d) = self
            .arrangement_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            for (i, c) in self.set.cards.iter().enumerate() {
                d.placed.insert(c.id);
                d.span_fields(
                    &format!("card-{}", c.id.0),
                    Span {
                        start: i as u32 * 16,
                        length: 16,
                    },
                );
                d.put(
                    &format!("card-{}-role", c.id.0),
                    if i < d.context.roles.len() {
                        (i + 1).to_string()
                    } else {
                        String::new()
                    },
                );
            }
            if self.set.cards.len() > d.context.roles.len() {
                d.put("extent", self.set.cards.len() * 16);
            }
        }
        self.sync_arrangement_scene();
    }
    pub fn add_arrangement_rest(&mut self) {
        self.capture_arrangement_history(self.working_sets.active_id);
        if let Some(d) = self
            .arrangement_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            if d.rests.len() >= 512 {
                self.arrangement_editor.notice =
                    Some("This arrangement has reached its rest budget.".into());
                return;
            }
            let key = (1..=513)
                .map(|i| format!("rest-{i}"))
                .find(|k| !d.rests.contains(k))
                .unwrap();
            d.span_fields(
                &format!("rest-{key}"),
                Span {
                    start: 0,
                    length: 4,
                },
            );
            d.put(&format!("rest-{key}-role"), "");
            d.rests.push(key);
        }
        self.sync_arrangement_scene();
    }
}

fn button(
    label: impl Into<String>,
    class: &str,
    action: impl Fn(&mut UiState) + 'static,
) -> UiChild {
    Box::new(clickable(
        el("button", text(label.into())).attr("class", format!("t-btn {class}")),
        move |ui: &mut UiState, _| action(ui),
    ))
}
fn action_button(
    label: impl Into<String>,
    class: &str,
    action_name: &str,
    action: impl Fn(&mut UiState) + 'static,
) -> UiChild {
    Box::new(clickable(
        el("button", text(label.into()))
            .attr("class", format!("t-btn {class}"))
            .attr("data-arrangement-action", action_name),
        move |ui: &mut UiState, _| action(ui),
    ))
}
fn grouping_label(groups: &[BarGroup]) -> String {
    if groups.is_empty() {
        return "No existing groups".into();
    }
    groups
        .iter()
        .map(|group| format!("Bars {}–{}", group.first + 1, group.first + group.count))
        .collect::<Vec<_>>()
        .join(", ")
}
fn quarter_note_position(ticks: u32) -> String {
    let whole = ticks / 4 + 1;
    let remainder = ticks % 4;
    if remainder == 0 {
        whole.to_string()
    } else {
        format!("{whole} + {remainder}/4")
    }
}
fn field(ui: &UiState, key: &str, label: &str) -> UiChild {
    let owner = ui.working_sets.active_id;
    let name = key.to_string();
    let focus_name = name.clone();
    let focus_tracker = ui
        .arrangement_editor
        .drafts
        .get(&owner)
        .expect("draft field")
        .focused_field
        .clone();
    let input = ui
        .arrangement_editor
        .field(owner, key)
        .expect("draft field");
    Box::new(
        el(
            "label",
            (
                el("span", text(label)),
                map_state(
                    on_focus(
                        text_field_typed(input),
                        move |_input: &mut TextInput, event: FocusEvent| {
                            let mut tracker = focus_tracker.borrow_mut();
                            tracker.break_run = true;
                            match event.phase {
                                FocusPhase::Gained => tracker.field = Some(focus_name.clone()),
                                FocusPhase::Lost => {
                                    if tracker.field.as_deref() == Some(focus_name.as_str()) {
                                        tracker.field = None;
                                    }
                                }
                            }
                        },
                    ),
                    move |ui: &mut UiState| {
                        ui.arrangement_editor
                            .field_mut(owner, &name)
                            .expect("mounted draft field")
                    },
                ),
            ),
        )
        .attr("class", "arrangement-field")
        .attr("aria-label", label.to_string())
        .attr("data-arrangement-field", key.to_string()),
    )
}

fn count_field(ui: &UiState, owner: WorkingSetId) -> UiChild {
    let input = ui
        .arrangement_editor
        .drafts
        .get(&owner)
        .map(|draft| &draft.bar_counts)
        .expect("arrangement draft");
    Box::new(
        el(
            "label",
            (
                el("span", text("Bar counts (for example 2 + 2)")),
                map_state(
                    text_field_typed(input)
                        .attr("class", "arrangement-group-count-input")
                        .attr("aria-label", "Bar counts")
                        .attr("data-arrangement-field", "bar-counts"),
                    move |ui: &mut UiState| {
                        &mut ui.arrangement_editor.drafts.get_mut(&owner).expect("mounted draft").bar_counts
                    },
                ),
            ),
        )
        .attr("class", "arrangement-field")
        .attr("style", "width:max-content;align-items:flex-start")
        .attr("data-arrangement-field", "bar-counts"),
    )
}

fn arrangement_history_key(ui: &mut UiState, event: KeyEvent) {
    let command = event.mods.ctrl || event.mods.meta;
    if !command {
        return;
    }
    let Key::Character(character) = &event.key else {
        return;
    };
    let key = character.to_ascii_lowercase();
    match (key.as_str(), event.mods.shift) {
        ("z", true) | ("y", _) => ui.redo_arrangement_edit(),
        ("z", false) => ui.undo_arrangement_edit(),
        _ => return,
    }
    event.prevent_default();
    event.stop_propagation();
}
fn spans(ui: &UiState, prefix: &str) -> Vec<UiChild> {
    vec![
        field(ui, &format!("{prefix}-start"), "Start (ticks)"),
        field(ui, &format!("{prefix}-length"), "Length (ticks)"),
    ]
}
pub fn screen(ui: &UiState) -> UiChild {
    let mut form: Option<UiChild> = None;
    let mut body: Vec<UiChild> = vec![
        Box::new(el(
            "h2",
            text(format!("Arrangement · {}", ui.working_sets.active_name)),
        )),
        Box::new(el(
            "p",
            text(
            "Expected roles and supplied occurrences. Four ticks per quarter note. Editing updates the arrangement; Card order and practice playback remain separate.",
            ),
        )),
    ];
    let owner = ui.working_sets.active_id;
    // Keep a stable slot: clearing a notice must not retire the captured scene
    // by shifting its ancestor's positional children during a gesture.
    body.push(Box::new(
        el(
            "p",
            text(ui.arrangement_editor.notice.clone().unwrap_or_default()),
        )
        .attr("role", "status")
        .attr(
            "style",
            if ui.arrangement_editor.notice.is_some() {
                ""
            } else {
                "display:none"
            },
        ),
    ));
    body.push(Box::new(
        el(
            "div",
            vec![
                button(
                    "Start from selected progression",
                    "arrangement-start",
                    UiState::start_arrangement,
                ),
                button(
                    "Reload saved / discard draft",
                    "arrangement-reload",
                    UiState::reload_arrangement,
                ),
            ],
        )
        .attr("class", "arrangement-actions"),
    ));
    if let Some(d) = ui.arrangement_editor.drafts.get(&owner) {
        body.push(Box::new(
            el(
                "p",
                text(format!(
                    "{} · saved key {} · {} / {} · {}",
                    d.context.progression,
                    crate::stage::ROOT_NAMES[usize::from((d.context.root_pc + 3) % 12).min(11)],
                    d.context.instrument,
                    d.context.tuning,
                    if ui
                        .arrangement_editor
                        .gesture
                        .as_ref()
                        .is_some_and(|g| g.proposed != g.original)
                    {
                        "Drag preview — release to update draft"
                    } else if d.dirty() {
                        "Draft — not saved"
                    } else {
                        "Saved arrangement"
                    }
                )),
            )
            .attr("class", "arrangement-context"),
        ));
        body.push(Box::new(
            el(
                "div",
                vec![
                    action_button("Undo", "arrangement-undo", "undo", UiState::undo_arrangement_edit),
                    action_button("Redo", "arrangement-redo", "redo", UiState::redo_arrangement_edit),
                    field(ui, "name", "Arrangement name"),
                    field(ui, "extent", "Extent (ticks)"),
                    button(
                        "Place cards in Set order",
                        "arrangement-place-order",
                        UiState::place_arrangement_cards_in_order,
                    ),
                    button(
                        "Add rest",
                        "arrangement-add-rest",
                        UiState::add_arrangement_rest,
                    ),
                    button(
                        "Save arrangement",
                        "arrangement-save",
                        UiState::save_arrangement,
                    ),
                ],
            )
            .attr("class", "arrangement-actions"),
        ));
        if !d.context.alignment.time_signatures.is_empty() {
            let mut grouping: Vec<UiChild> = vec![
                Box::new(el("h3", text("Bar grouping"))),
                Box::new(el("p", text("Select consecutive bars, then preview a partition. Grouping changes only the authored bar groups."))),
                count_field(ui, owner),
            ];
            match d.alignment().and_then(|alignment| woodshedding::rehearsal::arrangement::bars::bars(&alignment).map(|bars| (alignment, bars))) {
                Ok((_, bars)) => {
                    for (index, bar) in bars.iter().enumerate() {
                        let selected = d.selected_bars.contains(&index);
                    grouping.push(Box::new(clickable(
                            el("button", text(format!(
                                "Bar {} · quarter-note positions {}–{}{}{}",
                                index + 1,
                                quarter_note_position(bar.start),
                                quarter_note_position(bar.start + bar.length),
                                if !bar.complete { " · incomplete" } else { "" },
                                if selected { " · selected" } else { "" }
                            )))
                            .attr("class", "t-btn arrangement-bar-select")
                            .attr("data-arrangement-bar", index.to_string())
                            .attr("aria-pressed", selected.to_string()),
                            move |ui: &mut UiState, _| ui.select_arrangement_bar(index),
                        )));
                    }
                }
                Err(error) => grouping.push(Box::new(el("p", text(error)).attr("role", "status"))),
            }
            grouping.push(action_button("Preview grouping", "arrangement-group-preview", "group-preview", UiState::preview_arrangement_grouping));
            if let Some(preview) = &d.grouping_preview {
                let stale = d.snapshot() != preview.source
                    || d.selected_bars != preview.selected
                    || d.parse_group_counts().ok().as_ref() != Some(&preview.counts);
                grouping.push(Box::new(el("p", text(format!(
                    "{} bars · {} · {}",
                    preview.selected.len(),
                    if preview.conflict { "existing groups conflict" } else { "no conflicting groups" },
                    if stale { "Draft changed; preview again before applying." } else { "Preview is current." }
                ))).attr("role", "status")));
                grouping.push(Box::new(el(
                    "p",
                    text(format!(
                        "Existing: {} · Proposed: {}",
                        grouping_label(&preview.before),
                        grouping_label(&preview.after)
                    )),
                )));
                if preview.conflict {
                    grouping.push(action_button(
                        if preview.use_update { "Keep local" } else { "Keep local · selected" },
                        "arrangement-group-choice",
                        "group-choice-local",
                        |ui| ui.choose_arrangement_grouping(false),
                    ));
                    grouping.push(action_button(
                        if preview.use_update { "Use update · selected" } else { "Use update" },
                        "arrangement-group-choice",
                        "group-choice-update",
                        |ui| ui.choose_arrangement_grouping(true),
                    ));
                }
                grouping.push(action_button("Apply grouping", "arrangement-group-apply", "group-apply", UiState::apply_arrangement_grouping));
                grouping.push(action_button("Cancel preview", "arrangement-group-cancel", "group-cancel", UiState::cancel_arrangement_grouping));
            }
            body.push(Box::new(el("div", grouping).attr("class", "arrangement-grouping")));
        } else {
            body.push(Box::new(el("p", text("Author a time signature before grouping bars."))));
        }
        let mut rows: Vec<UiChild> = vec![Box::new(el("h3", text("Expected roles")))];
        for (i, role) in d.context.roles.iter().enumerate() {
            let mut row = vec![Box::new(el(
                "strong",
                text(format!("{} · {}", i + 1, woodshed_core::format_role(role))),
            )) as UiChild];
            row.extend(spans(ui, &format!("role-{i}")));
            rows.push(Box::new(el("div", row).attr("class", "arrangement-row")));
        }
        rows.push(Box::new(el("h3", text("Card occurrences"))));
        for id in &d.cards {
            let card = ui.set.card(*id);
            let id = *id;
            let label = card
                .map(|c| c.label.clone())
                .unwrap_or_else(|| format!("Missing Card {}", id.0));
            let mut row: Vec<UiChild> = vec![
                button(
                    format!("Inspect Card {} · {label}", id.0),
                    "arrangement-inspect",
                    move |ui| {
                        ui.arrangement_select(BackdropSceneEvent::Select(format!("card-{}", id.0)))
                    },
                ),
                button(
                    if d.placed.contains(&id) {
                        "Placed"
                    } else {
                        "Unplaced"
                    },
                    "arrangement-place",
                    move |ui| {
                        ui.capture_arrangement_history(owner);
                        if let Some(d) = ui.arrangement_editor.drafts.get_mut(&owner) {
                            if !d.placed.remove(&id) {
                                d.placed.insert(id);
                            }
                        }
                        ui.sync_arrangement_scene();
                    },
                ),
            ];
            row.extend(spans(ui, &format!("card-{}", id.0)));
            row.push(field(
                ui,
                &format!("card-{}-role", id.0),
                "Role number (empty = none)",
            ));
            if card.is_none() {
                row.push(button(
                    "Forget missing binding",
                    "arrangement-forget",
                    move |ui| {
                        ui.capture_arrangement_history(owner);
                        if let Some(d) = ui.arrangement_editor.drafts.get_mut(&owner) {
                            d.cards.retain(|c| *c != id);
                            d.placed.remove(&id);
                        }
                        ui.sync_arrangement_scene();
                    },
                ));
            }
            rows.push(Box::new(
                el("div", row)
                    .attr("class", "arrangement-row")
                    .attr("data-card", id.0.to_string()),
            ));
        }
        rows.push(Box::new(el("h3", text("Authored rests"))));
        for key in &d.rests {
            let k = key.clone();
            let mut row: Vec<UiChild> = vec![button(
                format!("Inspect rest {key}"),
                "arrangement-inspect",
                {
                    let k = k.clone();
                    move |ui| ui.arrangement_select(BackdropSceneEvent::Select(format!("rest-{k}")))
                },
            )];
            row.extend(spans(ui, &format!("rest-{key}")));
            row.push(field(
                ui,
                &format!("rest-{key}-role"),
                "Role number (empty = none)",
            ));
            row.push(button(
                "Remove rest",
                "arrangement-remove-rest",
                move |ui| {
                    ui.capture_arrangement_history(owner);
                    if let Some(d) = ui.arrangement_editor.drafts.get_mut(&owner) {
                        d.rests.retain(|r| r != &k);
                    }
                    ui.sync_arrangement_scene();
                },
            ));
            rows.push(Box::new(el("div", row).attr("class", "arrangement-row")));
        }
        form = Some(Box::new(
            el("div", rows).attr("class", "arrangement-form").attr(
                "style",
                if ui.viewport_width < 1000.0 {
                    "width:100%;flex:none"
                } else {
                    "width:460px;flex:none"
                },
            ),
        ));
    }
    let surfaces = ui.arrangement_editor.surfaces.borrow();
    let mut preview: Vec<UiChild> = vec![Box::new(el("h3", text("Scene preview")))];
    if let Some(e) = &surfaces.error {
        preview.push(Box::new(
            el("p", text(e.clone()))
                .attr("role", "status")
                .attr("class", "arrangement-error"),
        ));
    }
    if let Some(primary) = &surfaces.primary {
        let width = (ui.viewport_width
            - if ui.viewport_width < 1000.0 {
                100.0
            } else {
                600.0
            })
        .clamp(280.0, 1050.0) as u32;
        let primary_width = width.max(580);
        preview.push(Box::new(
            el(
                "div",
                on_key(
                    on_pointer(
                        custom_leaf::<UiState, ()>(PRIMARY_KEY, primary_width, 300)
                            .attr("class", "arrangement-scene")
                            .attr("aria-label", "Progression arrangement scene. Left and right move one beat; Shift resizes end; Alt resizes start; Escape cancels a drag."),
                        UiState::arrangement_pointer,
                    ),
                    UiState::arrangement_key,
                ),
            )
            .attr("class", "arrangement-scene-scroll"),
        ));
        if ui.viewport_width < 1000.0 {
            preview.push(Box::new(el(
                "p",
                text("Scroll the scene sideways to inspect its full span."),
            )));
        }
        if let Some(key) = primary.selected() {
            if let Some(span) = ui
                .arrangement_editor
                .drafts
                .get(&owner)
                .and_then(|d| d.occurrence_span(key))
            {
                let proposed = ui
                    .arrangement_editor
                    .gesture
                    .as_ref()
                    .filter(|g| g.key == key)
                    .map(|g| g.proposed)
                    .unwrap_or(span);
                let mut controls: Vec<UiChild> = vec![Box::new(
                    el(
                        "p",
                        text(format!(
                            "{} · start {} / length {} ticks{}",
                            key,
                            proposed.start,
                            proposed.length,
                            if proposed != span {
                                " · drag preview"
                            } else {
                                ""
                            }
                        )),
                    )
                    .attr("role", "status")
                    .attr("class", "arrangement-span-status"),
                )];
                for (action, label) in [
                    (SpanAction::Move, "Move"),
                    (SpanAction::Start, "Start"),
                    (SpanAction::End, "End"),
                ] {
                    for (beats, direction) in [(-1, "earlier"), (1, "later")] {
                        controls.push(button(
                            format!("{label} {direction} 1 beat"),
                            "arrangement-span-edit",
                            move |ui| ui.edit_arrangement_span(action, beats),
                        ));
                    }
                }
                preview.push(Box::new(on_key(
                    el("div", controls).attr("class", "arrangement-span-controls"),
                    UiState::arrangement_key,
                )));
            }
        }
        preview.push(Box::new(el("p", text("Drag a span to move it; drag its edges to resize. Snap: 1 beat. Focus the scene: ← / → move; Shift changes the end; Alt changes the start. Escape cancels a drag."))));
        if let Some(related) = &surfaces.related {
            preview.push(Box::new(el("h3", text(related.scene().title.clone()))));
            preview.push(Box::new(
                custom_leaf::<UiState, ()>(RELATED_KEY, width, 270)
                    .attr("aria-label", "Selected saved voicing scene"),
            ));
        } else {
            preview.push(Box::new(el(
                "p",
                text("Select a Card with a saved voicing to inspect its fretboard."),
            )));
        }
        preview.push(Box::new(backdrop_scene_reading(
            primary.scene(),
            primary.selected(),
            primary.retained(),
            |ui: &mut UiState, event| ui.arrangement_select(event),
        )));
    }
    let preview: UiChild = Box::new(
        el("div", preview)
            .attr("class", "arrangement-preview")
            .attr(
                "style",
                if ui.viewport_width < 1000.0 {
                    "width:100%;flex:none"
                } else {
                    "flex:1 1 0"
                },
            ),
    );
    let mut columns: Vec<UiChild> = vec![];
    if ui.viewport_width < 1000.0 {
        columns.push(preview);
        if let Some(form) = form {
            columns.push(form);
        }
    } else {
        if let Some(form) = form {
            columns.push(form);
        }
        columns.push(preview);
    }
    body.push(Box::new(
        el("div", columns)
            .attr("class", "arrangement-columns")
            .attr(
                "style",
                if ui.viewport_width < 1000.0 {
                    "flex-direction:column"
                } else {
                    "flex-direction:row"
                },
            ),
    ));
    Box::new(
        on_key(
            el("div", body).attr("class", "arrangement-workspace"),
            arrangement_history_key,
        )
        .capture(true)
        .focusable(false),
    )
}

pub const CSS:&str="
.arrangement-workspace { display:flex; flex-direction:column; gap:12px; padding:12px; min-width:0; }
.arrangement-actions,.arrangement-row { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.arrangement-field { display:flex; flex-direction:column; gap:3px; font-size:12px; min-width:100px; }
.arrangement-field [role=textbox] { min-width:80px; max-width:220px; min-height:26px; padding:4px; border:1px solid currentColor; }
.arrangement-row { padding:8px 0; border-bottom:1px solid #606878; }
.arrangement-columns {display:flex;gap:16px;align-items:flex-start;min-width:0;}
.arrangement-preview { flex:1 1 auto; display:flex; flex-direction:column; gap:8px; min-width:0; }
.arrangement-scene-scroll { width:100%; height:300px; overflow-x:auto; overflow-y:hidden; flex:none; }
.arrangement-preview .backdrop-scene-reading { display:flex; flex-direction:column; gap:6px; padding:8px; }
.arrangement-preview .backdrop-scene-reading button { min-height:28px; text-align:left; }
.arrangement-preview .backdrop-scene-occurrence { padding:5px; }
.arrangement-preview .backdrop-scene-occurrence[aria-pressed=true] { outline:2px solid currentColor; }
.arrangement-span-controls { display:flex; flex-wrap:wrap; gap:6px; align-items:center; }
.arrangement-span-status { width:100%; }
.arrangement-scene:focus { outline:2px solid currentColor; }
.arrangement-form { display:flex; flex-direction:column; gap:6px; }
";

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> UiState {
        let mut ui = UiState::new();
        ui.stage.lens = woodshed_core::Lens::Progressions;
        ui.stage.set_root(
            woodshed_core::ROOT_NAMES
                .iter()
                .position(|s| *s == "C")
                .unwrap(),
        );
        ui.root_dd.selected = ui.stage.root_idx;
        ui.stage.select_progression(
            ui.stage
                .progressions()
                .iter()
                .position(|p| p.name == "ii-V-I (Jazz)")
                .unwrap(),
        );
        for i in 0..3 {
            ui.stage.progression_expand(i);
            let mut c = ui.stage.card_from_lens().unwrap();
            ui.stage.select_card_shape_index(&mut c, 0).unwrap();
            ui.set.push(c);
        }
        ui.set.duplicate(2);
        ui.stage
            .select_card_shape_index(&mut ui.set.cards[3], 1)
            .unwrap();
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.start_arrangement();
        ui
    }
    fn edit(ui: &mut UiState, key: &str, value: &str) {
        let owner = ui.working_sets.active_id;
        *ui.arrangement_editor.field_mut(owner, key).unwrap() = TextInput::new(value);
        ui.sync_arrangement_scene();
    }
    fn value(v: &impl serde::Serialize) -> serde_json::Value {
        serde_json::to_value(v).unwrap()
    }
    fn enable_test_meter(ui: &mut UiState) {
        let owner = ui.working_sets.active_id;
        let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
        draft.context.alignment.time_signatures = vec![TimeSignatureChange {
            start: 0,
            numerator: 4,
            denominator: 4,
        }];
        draft.history_snapshot = draft.history_snapshot();
    }
    #[test]
    fn grouping_preview_cancel_and_conflict_choices_do_not_edit_until_apply() {
        let mut ui = fixture();
        enable_test_meter(&mut ui);
        let owner = ui.working_sets.active_id;
        {
            let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
            draft.context.alignment.bar_groups = vec![BarGroup { first: 0, count: 2 }];
            draft.history_snapshot = draft.history_snapshot();
            draft.selected_bars = vec![0, 1];
            draft.bar_counts = TextInput::new("1 + 1");
        }
        let initial = ui.arrangement_editor.drafts[&owner].history_snapshot();
        ui.preview_arrangement_grouping();
        let draft = &ui.arrangement_editor.drafts[&owner];
        assert_eq!(draft.history_snapshot(), initial);
        assert!(draft.grouping_preview.as_ref().unwrap().conflict);
        assert!(!draft.grouping_preview.as_ref().unwrap().use_update);
        let scene = ui.arrangement_editor.surfaces.borrow();
        let scenomise::backdrops::SceneFacts::TemplatedSequence(sequence) =
            &scene.document.as_ref().unwrap().primary.facts
        else {
            panic!("arrangement scene should be a templated sequence")
        };
        assert!(sequence.template.groups.iter().any(|group| {
            group.key == "bar-group-0" && group.label == "Bars 1–2"
        }));
        drop(scene);
        ui.cancel_arrangement_grouping();
        assert_eq!(ui.arrangement_editor.drafts[&owner].history_snapshot(), initial);
        assert!(!ui.arrangement_editor.group_preview_active(owner));

        ui.preview_arrangement_grouping();
        ui.apply_arrangement_grouping();
        assert_eq!(ui.arrangement_editor.drafts[&owner].history_snapshot(), initial);
        ui.preview_arrangement_grouping();
        ui.choose_arrangement_grouping(true);
        ui.apply_arrangement_grouping();
        assert_eq!(
            ui.arrangement_editor.drafts[&owner].context.alignment.bar_groups,
            vec![BarGroup { first: 0, count: 1 }, BarGroup { first: 1, count: 1 }]
        );
        ui.undo_arrangement_edit();
        assert_eq!(
            ui.arrangement_editor.drafts[&owner].context.alignment.bar_groups,
            vec![BarGroup { first: 0, count: 2 }]
        );
        ui.redo_arrangement_edit();
        assert_eq!(
            ui.arrangement_editor.drafts[&owner].context.alignment.bar_groups,
            vec![BarGroup { first: 0, count: 1 }, BarGroup { first: 1, count: 1 }]
        );
    }
    #[test]
    fn preview_draws_the_proposal_without_changing_the_authored_candidate_or_history() {
        let mut ui = fixture();
        enable_test_meter(&mut ui);
        let owner = ui.working_sets.active_id;
        {
            let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
            draft.selected_bars = vec![0, 1];
            draft.bar_counts = TextInput::new("2");
        }
        let before = ui.arrangement_editor.surfaces.borrow().document.clone().unwrap();
        ui.preview_arrangement_grouping();

        let candidate = ui.arrangement_editor.drafts[&owner].candidate(&ui.set).unwrap();
        assert!(candidate.progression_arrangement.unwrap().alignment.bar_groups.is_empty());
        assert!(!ui.arrangement_editor.can_undo(owner));
        let after = ui.arrangement_editor.surfaces.borrow().document.clone().unwrap();
        assert_ne!(before, after);
        let scenomise::backdrops::SceneFacts::TemplatedSequence(sequence) = &after.primary.facts
        else {
            panic!("arrangement scene should be a templated sequence")
        };
        assert!(sequence.template.groups.iter().any(|group| {
            group.key == "bar-group-0" && group.label == "Bars 1–2"
        }));
    }

    #[test]
    fn bar_count_native_field_is_borrowed_but_remains_outside_authored_history() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let dirty_before = ui.arrangement_editor.drafts[&owner].dirty();
        assert_eq!(ui.arrangement_editor.field(owner, "bar-counts").unwrap().text(), "2 + 2");
        *ui.arrangement_editor.field_mut(owner, "bar-counts").unwrap() = TextInput::new("1 + 3");
        ui.sync_arrangement_scene();
        assert_eq!(ui.arrangement_editor.drafts[&owner].bar_counts.text(), "1 + 3");
        assert!(!ui.arrangement_editor.can_undo(owner));
        assert_eq!(ui.arrangement_editor.drafts[&owner].dirty(), dirty_before);
    }

    #[test]
    fn grouping_apply_refuses_a_changed_saved_set() {
        let mut ui = fixture();
        enable_test_meter(&mut ui);
        ui.save_arrangement();
        let owner = ui.working_sets.active_id;
        {
            let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
            draft.selected_bars = vec![0, 1];
            draft.bar_counts = TextInput::new("2");
        }
        ui.preview_arrangement_grouping();
        ui.set.progression_arrangement.as_mut().unwrap().alignment.name.push_str(" external");
        ui.apply_arrangement_grouping();
        assert_eq!(
            ui.arrangement_editor.drafts[&owner].context.alignment.bar_groups,
            Vec::<BarGroup>::new()
        );
        assert!(ui.arrangement_editor.notice.as_deref().unwrap().contains("changed"));
    }
    #[test]
    fn grouping_preview_refuses_a_changed_source_and_ordinary_edit_is_earlier_in_history() {
        let mut ui = fixture();
        enable_test_meter(&mut ui);
        let owner = ui.working_sets.active_id;
        let original_name = ui.arrangement_editor.drafts[&owner].value("name").to_string();
        let original_extent = ui.arrangement_editor.drafts[&owner].value("extent").to_string();
        edit(&mut ui, "name", "Earlier title");
        {
            let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
            draft.span_fields("role-0", Span { start: 8, length: 16 });
        }
        ui.sync_arrangement_scene();
        {
            let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
            draft.selected_bars = vec![0, 1];
            draft.bar_counts = TextInput::new("2");
        }
        ui.preview_arrangement_grouping();
        edit(&mut ui, "extent", "128");
        assert!(ui.arrangement_editor.drafts.get_mut(&owner).unwrap().apply_grouping().is_err());
        ui.undo_arrangement_edit();
        assert_eq!(ui.arrangement_editor.drafts[&owner].value("extent"), original_extent);
        ui.undo_arrangement_edit();
        assert_eq!(ui.arrangement_editor.drafts[&owner].span("role-0").unwrap().start, 0);
        ui.undo_arrangement_edit();
        assert_eq!(ui.arrangement_editor.drafts[&owner].value("name"), original_name);
    }
    #[test]
    fn save_keeps_history_and_undo_redo_tracks_the_saved_marker() {
        let mut ui = fixture();
        let original_name = ui.arrangement_editor.drafts[&ui.working_sets.active_id]
            .value("name")
            .to_string();
        edit(&mut ui, "name", "Saved name");
        ui.save_arrangement();
        assert!(!ui.arrangement_editor.drafts[&ui.working_sets.active_id].dirty());
        ui.undo_arrangement_edit();
        assert!(ui.arrangement_editor.drafts[&ui.working_sets.active_id].dirty());
        assert_eq!(ui.arrangement_editor.drafts[&ui.working_sets.active_id].value("name"), original_name);
        ui.redo_arrangement_edit();
        assert!(!ui.arrangement_editor.drafts[&ui.working_sets.active_id].dirty());
        assert_eq!(ui.arrangement_editor.drafts[&ui.working_sets.active_id].value("name"), "Saved name");
    }
    #[test]
    fn typing_in_one_focused_field_is_one_arrangement_history_step() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let original_name = ui.arrangement_editor.drafts[&owner].value("name").to_string();
        ui.arrangement_editor.drafts.get_mut(&owner).unwrap().focused_field.borrow_mut().field = Some("name".into());
        edit(&mut ui, "name", "First");
        edit(&mut ui, "name", "Second");
        ui.arrangement_editor.drafts.get_mut(&owner).unwrap().focused_field.borrow_mut().field = None;
        ui.arrangement_editor.drafts.get_mut(&owner).unwrap().history.break_run();
        ui.undo_arrangement_edit();
        assert_eq!(ui.arrangement_editor.drafts[&owner].value("name"), original_name);
    }
    #[test]
    fn refreshing_set_membership_does_not_record_or_undo_an_arrangement_edit() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let original_name = ui.arrangement_editor.drafts[&owner].value("name").to_string();
        edit(&mut ui, "name", "Authored title");
        let history = ui.arrangement_editor.drafts[&owner].history.clone();

        let mut incoming = ui.set.cards[0].clone();
        let next_id = ui.set.cards.iter().map(|card| card.id.0).max().unwrap() + 1;
        incoming.id = CardId(next_id);
        ui.set.cards.push(incoming);
        ui.sync_arrangement_scene();
        assert_eq!(ui.arrangement_editor.drafts[&owner].history, history);
        assert!(ui.arrangement_editor.drafts[&owner].cards.contains(&CardId(next_id)));

        ui.undo_arrangement_edit();
        assert_eq!(ui.arrangement_editor.drafts[&owner].value("name"), original_name);
        assert!(ui.arrangement_editor.drafts[&owner].cards.contains(&CardId(next_id)));
        ui.sync_arrangement_scene();
        assert!(!ui.arrangement_editor.can_undo(owner));
    }
    #[test]
    fn span_editor_rebuild_preserves_authored_meter_and_bar_groups() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let draft = ui.arrangement_editor.drafts.get_mut(&owner).unwrap();
        draft.context.alignment.time_signatures = vec![
            woodshedding::rehearsal::arrangement::TimeSignatureChange {
                start: 0,
                numerator: 4,
                denominator: 4,
            },
        ];
        draft.context.alignment.bar_groups = vec![
            woodshedding::rehearsal::arrangement::BarGroup { first: 0, count: 2 },
        ];
        draft.put("role-0-start", 2);
        let rebuilt = draft.alignment().unwrap();
        assert_eq!(rebuilt.time_signatures, draft.context.alignment.time_signatures);
        assert_eq!(rebuilt.bar_groups, draft.context.alignment.bar_groups);
        assert_eq!(rebuilt.role_spans[0].start, 2);
    }
    fn card_span(ui: &UiState, key: &str) -> Span {
        ui.arrangement_editor.drafts[&ui.working_sets.active_id]
            .occurrence_span(key)
            .unwrap()
    }
    fn gesture_point(ui: &UiState, key: &str, action: SpanAction) -> ((f32, f32), f32) {
        let surfaces = ui.arrangement_editor.surfaces.borrow();
        let surface = surfaces.primary.as_ref().unwrap();
        let r = surface.occurrence_rect(key, 580, 300).unwrap();
        let x = match action {
            SpanAction::Move => r.x + r.width / 2.0,
            SpanAction::Start => r.x + 1.0,
            SpanAction::End => r.x + r.width - 1.0,
        };
        let a = surface
            .viewport_point(sceno::Vec2::new(48.0, 50.0), 580, 300)
            .unwrap();
        let b = surface
            .viewport_point(sceno::Vec2::new(95.0, 50.0), 580, 300)
            .unwrap();
        ((x, r.y + r.height / 2.0), b.x - a.x)
    }
    fn pointer(ui: &mut UiState, phase: cambium::PointerPhase, point: (f32, f32)) {
        ui.arrangement_pointer(cambium::PointerEvent::new(phase, point, (580.0, 300.0)));
    }
    #[test]
    fn shared_intents_recheck_the_working_set_reading_before_native_acceptance() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.save_arrangement();
        let before = value(&ui.set);
        let owner = ui.working_sets.active_id;
        let reading = format!("woodshed/working-set/{}", owner.0);
        let draft = &ui.arrangement_editor.drafts[&owner];
        let doc = saved_arrangement_scene(&draft.candidate(&ui.set).unwrap(), &reading).unwrap();
        let intent =
            SpanEditIntent::propose(&doc.primary, "card-2", SpanAction::Move, 4.0, 4).unwrap();
        let accepted = draft.candidate_intent(&ui.set, &intent, &reading).unwrap();
        assert_ne!(value(&accepted), before);
        assert_eq!(value(&ui.set), before);
        assert!(
            draft
                .candidate_intent(&ui.set, &intent, "other-set")
                .is_err()
        );
        for cause in 0..6 {
            let mut bad = intent.clone();
            match cause {
                0 => bad.source.id.push('x'),
                1 => bad.revision.push('x'),
                2 => bad.occurrence = "card-999".into(),
                3 => bad.lane += 1,
                4 => bad.before.start += 1,
                _ => bad.after.length = u32::MAX,
            }
            assert!(draft.candidate_intent(&ui.set, &bad, &reading).is_err());
            assert_eq!(value(&ui.set), before);
        }
    }
    #[test]
    fn span_gestures_preview_cancel_and_commit_without_mutating_cards_or_saved_source() {
        use cambium::PointerPhase::*;
        for (action, beats, expected) in [
            (
                SpanAction::Move,
                1.0,
                Span {
                    start: 20,
                    length: 16,
                },
            ),
            (
                SpanAction::Start,
                -1.0,
                Span {
                    start: 12,
                    length: 20,
                },
            ),
            (
                SpanAction::End,
                1.0,
                Span {
                    start: 16,
                    length: 20,
                },
            ),
        ] {
            let mut ui = fixture();
            ui.place_arrangement_cards_in_order();
            ui.save_arrangement();
            let saved = value(&ui.set);
            let original_doc = ui
                .arrangement_editor
                .surfaces
                .borrow()
                .document
                .clone()
                .unwrap();
            let original = card_span(&ui, "card-2");
            let (at, beat_width) = gesture_point(&ui, "card-2", action);
            let target = (at.0 + beat_width * beats, at.1);
            pointer(&mut ui, Down, at);
            pointer(&mut ui, Move, target);
            assert_eq!(
                ui.arrangement_editor.gesture.as_ref().unwrap().proposed,
                expected
            );
            assert_eq!(
                card_span(&ui, "card-2"),
                original,
                "preview does not mutate fields"
            );
            assert_eq!(value(&ui.set), saved);
            assert!(ui.arrangement_editor.dirty());
            assert_ne!(
                ui.arrangement_editor
                    .surfaces
                    .borrow()
                    .document
                    .as_ref()
                    .unwrap(),
                &original_doc
            );
            ui.arrangement_key(cambium::KeyEvent::new(cambium::Key::Named(
                cambium::NamedKey::Escape,
            )));
            pointer(&mut ui, Up, target);
            assert!(!ui.arrangement_editor.dirty());
            assert_eq!(
                ui.arrangement_editor
                    .surfaces
                    .borrow()
                    .document
                    .as_ref()
                    .unwrap(),
                &original_doc
            );
            pointer(&mut ui, Down, at);
            pointer(&mut ui, Move, target);
            pointer(&mut ui, Up, target);
            assert_eq!(card_span(&ui, "card-2"), expected);
            assert_eq!(value(&ui.set), saved);
            ui.save_arrangement();
            assert!(!ui.arrangement_editor.dirty());
            assert_eq!(value(&ui.set.cards), saved["cards"]);
            let binding = &ui
                .set
                .progression_arrangement
                .as_ref()
                .unwrap()
                .alignment
                .cards[1];
            assert_eq!(binding.placement.as_ref().unwrap().span, expected);
            assert_eq!(binding.role, Some(1));
        }
    }
    #[test]
    fn stale_cancelled_or_resized_gestures_cannot_write_drafts() {
        use cambium::PointerPhase::*;
        for cause in ["cancel", "field", "source", "owner", "panel", "resize"] {
            let mut ui = fixture();
            ui.place_arrangement_cards_in_order();
            let owner = ui.working_sets.active_id;
            let before = ui.arrangement_editor.drafts[&owner].history_snapshot();
            let (at, beat_width) = gesture_point(&ui, "card-2", SpanAction::Move);
            let target = (at.0 + beat_width, at.1);
            pointer(&mut ui, Down, at);
            pointer(&mut ui, Move, target);
            match cause {
                "cancel" => pointer(&mut ui, Cancel, target),
                "field" => edit(&mut ui, "card-2-start", "8"),
                "source" => {
                    ui.set.cards[0].label.push_str(" changed");
                    ui.sync_arrangement_scene();
                },
                "owner" => {
                    ui.working_sets.active_id = WorkingSetId(99);
                    ui.sync_arrangement_scene();
                },
                "panel" => {
                    ui.activate_workspace_panel(WorkspacePanel::Practice);
                    ui.sync_arrangement_scene();
                },
                "resize" => {
                    ui.arrangement_pointer(cambium::PointerEvent::new(Move, target, (900.0, 300.0)))
                },
                _ => unreachable!(),
            }
            pointer(&mut ui, Up, target);
            assert!(ui.arrangement_editor.gesture.is_none(), "{cause}");
            if cause == "field" {
                assert_eq!(
                    ui.arrangement_editor.drafts[&owner].value("card-2-start"),
                    "8"
                );
            } else {
                assert_eq!(
                    ui.arrangement_editor.drafts[&owner].history_snapshot(),
                    before,
                    "{cause}"
                );
            }
        }
    }
    #[test]
    fn keyboard_and_rest_edits_snap_clamp_and_preserve_bindings() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.add_arrangement_rest();
        ui.arrangement_select(BackdropSceneEvent::Select("rest-rest-1".into()));
        let key = |ui: &mut UiState, mods| {
            ui.arrangement_key(cambium::KeyEvent::with_mods(
                cambium::Key::Named(cambium::NamedKey::ArrowRight),
                mods,
            ))
        };
        key(&mut ui, cambium::Modifiers::default());
        assert_eq!(
            card_span(&ui, "rest-rest-1"),
            Span {
                start: 4,
                length: 4
            }
        );
        key(
            &mut ui,
            cambium::Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(
            card_span(&ui, "rest-rest-1"),
            Span {
                start: 4,
                length: 8
            }
        );
        key(
            &mut ui,
            cambium::Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            card_span(&ui, "rest-rest-1"),
            Span {
                start: 8,
                length: 4
            }
        );
        let before = card_span(&ui, "rest-rest-1");
        key(
            &mut ui,
            cambium::Modifiers {
                meta: true,
                ..Default::default()
            },
        );
        assert_eq!(card_span(&ui, "rest-rest-1"), before);
        ui.edit_arrangement_span(SpanAction::Move, -100);
        assert_eq!(card_span(&ui, "rest-rest-1").start, 0);
        ui.edit_arrangement_span(SpanAction::Move, 100);
        assert_eq!(card_span(&ui, "rest-rest-1").start, 60);
        ui.edit_arrangement_span(SpanAction::Start, 100);
        assert_eq!(
            card_span(&ui, "rest-rest-1"),
            Span {
                start: 63,
                length: 1
            }
        );
        ui.edit_arrangement_span(SpanAction::End, -100);
        assert_eq!(card_span(&ui, "rest-rest-1").length, 1);
        // Expected roles and unplaced occurrences are selection only.
        ui.arrangement_select(BackdropSceneEvent::Select("role-0".into()));
        let before = ui.arrangement_editor.drafts[&ui.working_sets.active_id].history_snapshot();
        ui.edit_arrangement_span(SpanAction::Move, 1);
        assert_eq!(
            ui.arrangement_editor.drafts[&ui.working_sets.active_id].history_snapshot(),
            before
        );
    }
    #[test]
    fn draft_preview_selection_and_save_preserve_instructions_and_voicings() {
        let mut ui = fixture();
        let cards = value(&ui.set.cards);
        let before = value(&ui.set);
        let audio = ui.audio_requests.len();
        assert!(ui.arrangement_editor.dirty());
        assert!(ui.set.progression_arrangement.is_none());
        ui.place_arrangement_cards_in_order();
        edit(&mut ui, "role-2-length", "32");
        edit(&mut ui, "card-2-length", "8");
        ui.add_arrangement_rest();
        edit(&mut ui, "rest-rest-1-start", "24");
        edit(&mut ui, "rest-rest-1-length", "8");
        assert_eq!(value(&ui.set), before);
        ui.arrangement_select(BackdropSceneEvent::Select("card-3".into()));
        let first = ui
            .arrangement_editor
            .surfaces
            .borrow()
            .related
            .as_ref()
            .unwrap()
            .scene()
            .revision
            .clone();
        ui.arrangement_select(BackdropSceneEvent::Select("card-4".into()));
        let second = ui
            .arrangement_editor
            .surfaces
            .borrow()
            .related
            .as_ref()
            .unwrap()
            .scene()
            .revision
            .clone();
        assert_ne!(first, second);
        ui.arrangement_select(BackdropSceneEvent::Select("rest-rest-1".into()));
        assert!(ui.arrangement_editor.surfaces.borrow().related.is_none());
        assert_eq!(value(&ui.set), before);
        assert_eq!(ui.audio_requests.len(), audio);
        ui.save_arrangement();
        assert!(!ui.arrangement_editor.dirty());
        assert_eq!(value(&ui.set.cards), cards);
        let saved = ui.set.progression_arrangement.as_ref().unwrap();
        assert_eq!(saved.alignment.extent, 64);
        assert_eq!(saved.alignment.rests.len(), 1);
        let session = ui.to_persisted();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, ui.app_settings.clone());
        reopened.activate_workspace_panel(WorkspacePanel::Arrangement);
        assert_eq!(value(&reopened.set), value(&ui.set));
        assert!(
            reopened
                .arrangement_editor
                .surfaces
                .borrow()
                .primary
                .is_some()
        );
    }
    #[test]
    fn invalid_draft_never_replaces_saved_arrangement_or_leaves_stale_preview() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.save_arrangement();
        let before = value(&ui.set);
        edit(&mut ui, "card-1-length", "-8");
        assert!(ui.arrangement_editor.surfaces.borrow().primary.is_none());
        ui.save_arrangement();
        assert_eq!(value(&ui.set), before);
        assert!(
            ui.arrangement_editor
                .notice
                .as_deref()
                .unwrap()
                .contains("whole number")
        );
        ui.reload_arrangement();
        assert!(!ui.arrangement_editor.dirty());
        assert!(ui.arrangement_editor.surfaces.borrow().primary.is_some());
    }
    #[test]
    fn drafts_are_owner_scoped_and_external_changes_require_reload() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.save_arrangement();
        let first = ui.working_sets.active_id;
        edit(&mut ui, "name", "First draft");
        let mut other = ui.set.clone();
        other.progression_arrangement = None;
        let second = ui.working_sets.create(&mut ui.set, "Other", other);
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.start_arrangement();
        edit(&mut ui, "name", "Second draft");
        assert_ne!(first, second);
        ui.working_sets.activate(first, &mut ui.set);
        ui.sync_arrangement_scene();
        assert_eq!(
            ui.arrangement_editor.field(first, "name").unwrap().text(),
            "First draft"
        );
        ui.set
            .progression_arrangement
            .as_mut()
            .unwrap()
            .alignment
            .name = "Changed outside editor".into();
        ui.save_arrangement();
        assert!(
            ui.arrangement_editor
                .notice
                .as_deref()
                .unwrap()
                .contains("Reload")
        );
        assert_eq!(
            ui.set
                .progression_arrangement
                .as_ref()
                .unwrap()
                .alignment
                .name,
            "Changed outside editor"
        );
    }
    #[test]
    fn missing_references_are_retained_until_explicit_repair_and_rebinding_is_dirty() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.save_arrangement();
        ui.set.remove(0);
        ui.sync_arrangement_scene();
        assert!(
            ui.arrangement_editor
                .surfaces
                .borrow()
                .error
                .as_deref()
                .unwrap()
                .contains("missing")
        );
        let d = ui
            .arrangement_editor
            .drafts
            .get_mut(&ui.working_sets.active_id)
            .unwrap();
        d.cards.retain(|id| id.0 != 1);
        ui.sync_arrangement_scene();
        assert!(ui.arrangement_editor.surfaces.borrow().primary.is_some());
        ui.save_arrangement();
        ui.stage.set_root(7);
        ui.start_arrangement();
        assert!(ui.arrangement_editor.dirty());
    }
    #[test]
    fn saved_context_is_independent_of_live_navigation_and_new_panel_restores() {
        let mut ui = fixture();
        ui.place_arrangement_cards_in_order();
        ui.save_arrangement();
        let before = value(&ui.set);
        ui.stage.set_root(7);
        ui.stage.select_progression(0);
        ui.activate_workspace_panel(WorkspacePanel::Practice);
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        assert_eq!(value(&ui.set), before);
        assert!(
            ui.arrangement_editor
                .surfaces
                .borrow()
                .document
                .as_ref()
                .unwrap()
                .primary
                .title
                .contains("in C")
        );
        let snapshot = ui.workspace.to_snapshot_json().unwrap();
        let restored = crate::workspace::WoodshedWorkspace::from_snapshot_json(&snapshot).unwrap();
        assert_eq!(restored.active_panel(), Some(WorkspacePanel::Arrangement));
    }
}
