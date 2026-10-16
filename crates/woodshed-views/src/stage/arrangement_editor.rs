//! Musical alignment authoring and shared portable scene presentation.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{
    BackdropSceneEvent, TextInput, backdrop_scene_reading, clickable, custom_leaf, el, map_state,
    on_pointer, text, text_field,
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
    base: Option<serde_json::Value>,
    pub fields: BTreeMap<String, TextInput>,
    cards: Vec<CardId>,
    placed: BTreeSet<CardId>,
    rests: Vec<String>,
    original: String,
}
impl Draft {
    fn from_saved(
        context: SavedProgressionArrangement,
        base: Option<serde_json::Value>,
        set: &Set,
    ) -> Self {
        let mut d = Self {
            context,
            base,
            fields: BTreeMap::new(),
            cards: vec![],
            placed: BTreeSet::new(),
            rests: vec![],
            original: String::new(),
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
        d.original = d.signature();
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
        })
    }
    fn signature(&self) -> String {
        format!(
            "{}|{:?}|{:?}|{:?}|{:?}",
            serde_json::to_string(&self.context).expect("serializable arrangement context"),
            self.fields
                .iter()
                .map(|(k, v)| (k, v.text()))
                .collect::<Vec<_>>(),
            self.cards,
            self.placed,
            self.rests
        )
    }
    pub fn dirty(&self) -> bool {
        self.base.as_ref() != serde_json::to_value(&self.context).ok().as_ref()
            || self.signature() != self.original
    }
    fn candidate(&self, set: &Set) -> Result<Set, String> {
        if serde_json::to_value(&set.progression_arrangement).ok()
            != Some(self.base.clone().unwrap_or(serde_json::Value::Null))
        {
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
#[derive(Default)]
pub struct ArrangementEditor {
    pub drafts: BTreeMap<WorkingSetId, Draft>,
    pub surfaces: Rc<RefCell<ArrangementSurfaces>>,
    owner: Option<WorkingSetId>,
    signature: String,
    pub notice: Option<String>,
    pub close_pending: bool,
}
impl ArrangementEditor {
    pub fn dirty(&self) -> bool {
        self.drafts.values().any(Draft::dirty)
    }
    pub fn field(&self, owner: WorkingSetId, key: &str) -> Option<&TextInput> {
        self.drafts.get(&owner)?.fields.get(key)
    }
    pub fn field_mut(&mut self, owner: WorkingSetId, key: &str) -> Option<&mut TextInput> {
        self.drafts.get_mut(&owner)?.fields.get_mut(key)
    }
}
impl UiState {
    pub fn start_arrangement(&mut self) {
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
            };
            let mut copy = self.set.clone();
            save_progression_arrangement(&self.stage, &mut copy, a)?;
            Ok::<_, String>(Draft::from_saved(
                copy.progression_arrangement.unwrap(),
                serde_json::to_value(&self.set.progression_arrangement)
                    .ok()
                    .filter(|v| !v.is_null()),
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
        let owner = self.working_sets.active_id;
        self.arrangement_editor.drafts.remove(&owner);
        self.arrangement_editor.signature.clear();
        self.arrangement_editor.notice = None;
        self.sync_arrangement_scene();
    }
    pub fn save_arrangement(&mut self) {
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
                self.set.progression_arrangement = saved;
                self.reload_arrangement();
                self.arrangement_editor.notice =
                    Some("Arrangement saved to this working Set.".into());
            },
            Err(e) => self.arrangement_editor.notice = Some(e),
        }
    }
    pub fn sync_arrangement_scene(&mut self) {
        if self.workspace.active_panel() != Some(WorkspacePanel::Arrangement) {
            return;
        }
        let owner = self.working_sets.active_id;
        if !self.arrangement_editor.drafts.contains_key(&owner) {
            if let Some(saved) = &self.set.progression_arrangement {
                self.arrangement_editor.drafts.insert(
                    owner,
                    Draft::from_saved(saved.clone(), serde_json::to_value(saved).ok(), &self.set),
                );
            }
        }
        if let Some(d) = self.arrangement_editor.drafts.get_mut(&owner) {
            d.add_current_cards(&self.set);
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
        let signature = format!(
            "{:?}|{}|{}",
            theme,
            serde_json::to_string(&self.set).unwrap_or_default(),
            self.arrangement_editor
                .drafts
                .get(&owner)
                .map(Draft::signature)
                .unwrap_or_default()
        );
        if self.arrangement_editor.owner == Some(owner)
            && self.arrangement_editor.signature == signature
        {
            return;
        }
        let preserve = self.arrangement_editor.owner == Some(owner);
        self.arrangement_editor.owner = Some(owner);
        self.arrangement_editor.signature = signature;
        let result = self
            .arrangement_editor
            .drafts
            .get(&owner)
            .ok_or("Start an arrangement to preview it".to_string())
            .and_then(|d| d.candidate(&self.set))
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
    pub fn arrangement_select(&mut self, event: BackdropSceneEvent) {
        self.arrangement_editor.surfaces.borrow_mut().select(event);
    }
    pub fn place_arrangement_cards_in_order(&mut self) {
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
fn field(ui: &UiState, key: &str, label: &str) -> UiChild {
    let owner = ui.working_sets.active_id;
    let name = key.to_string();
    let input = ui
        .arrangement_editor
        .field(owner, key)
        .expect("draft field");
    Box::new(
        el(
            "label",
            (
                el("span", text(label)),
                map_state(text_field(input), move |ui: &mut UiState| {
                    ui.arrangement_editor
                        .field_mut(owner, &name)
                        .expect("mounted draft field")
                }),
            ),
        )
        .attr("class", "arrangement-field")
        .attr("aria-label", label.to_string())
        .attr("data-arrangement-field", key.to_string()),
    )
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
                "Expected roles and supplied occurrences. Four ticks per quarter-note beat; editing does not change Card order, timing or playback.",
            ),
        )),
    ];
    let owner = ui.working_sets.active_id;
    if let Some(notice) = &ui.arrangement_editor.notice {
        body.push(Box::new(
            el("p", text(notice.clone())).attr("role", "status"),
        ));
    }
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
                    if d.dirty() {
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
        preview.push(Box::new(on_pointer(
            custom_leaf::<UiState, ()>(PRIMARY_KEY, width, 230)
                .attr("aria-label", "Progression arrangement scene"),
            |ui: &mut UiState, event: cambium::PointerEvent| {
                if event.phase == cambium::PointerPhase::Up
                    && event.button == cambium::PointerButton::Primary
                {
                    let key = ui
                        .arrangement_editor
                        .surfaces
                        .borrow()
                        .primary
                        .as_ref()
                        .and_then(|s| {
                            s.pick(
                                event.local.0,
                                event.local.1,
                                event.size.0.round() as u32,
                                event.size.1.round() as u32,
                            )
                        })
                        .map(str::to_owned);
                    ui.arrangement_select(
                        key.map(BackdropSceneEvent::Select)
                            .unwrap_or(BackdropSceneEvent::Clear),
                    );
                }
            },
        )));
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
    let preview: UiChild = Box::new(el("div", preview).attr("class", "arrangement-preview"));
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
    Box::new(el("div", body).attr("class", "arrangement-workspace"))
}

pub const CSS:&str="
.arrangement-workspace { display:flex; flex-direction:column; gap:12px; padding:12px; min-width:0; }
.arrangement-actions,.arrangement-row { display:flex; flex-wrap:wrap; gap:8px; align-items:center; }
.arrangement-field { display:flex; flex-direction:column; gap:3px; font-size:12px; min-width:100px; }
.arrangement-field [role=textbox] { min-width:80px; max-width:220px; min-height:26px; padding:4px; border:1px solid currentColor; }
.arrangement-row { padding:8px 0; border-bottom:1px solid #606878; }
.arrangement-columns {display:flex;gap:16px;align-items:flex-start;min-width:0;}
.arrangement-preview { flex:1 1 auto; display:flex; flex-direction:column; gap:8px; min-width:0; overflow-x:auto; }
.arrangement-preview .backdrop-scene-reading { display:flex; flex-direction:column; gap:6px; padding:8px; }
.arrangement-preview .backdrop-scene-reading button { min-height:28px; text-align:left; }
.arrangement-preview .backdrop-scene-occurrence { padding:5px; }
.arrangement-preview .backdrop-scene-occurrence[aria-pressed=true] { outline:2px solid currentColor; }
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
