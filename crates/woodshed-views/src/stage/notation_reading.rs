//! Host-local reading and attention over the Set's saved MusicXML.
use super::{UiChild, UiState, WorkspacePanel};
use cambium::{clickable, custom_leaf, el, text};
use woodshed_notation::{ColorF, NOTATION_LEAF_KEY, NotationLeaf, NotationView};

#[derive(Default)]
pub struct Reading {
    pub view: Option<NotationView>,
    selected: Option<(
        woodshed_core::working_sets::WorkingSetId,
        String,
        (usize, usize),
    )>,
}
fn width(ui: &UiState) -> f32 {
    (ui.viewport_width - 72.0).clamp(240.0, 1160.0)
}
fn selection(ui: &UiState) -> (usize, usize) {
    ui.notation_reading
        .selected
        .as_ref()
        .filter(|(owner, source, _)| {
            *owner == ui.working_sets.active_id
                && ui.set.music.as_ref().is_some_and(|d| &d.musicxml == source)
        })
        .map_or((usize::MAX, usize::MAX), |(_, _, event)| *event)
}
pub fn signature(ui: &UiState) -> Option<String> {
    if ui.workspace.active_panel() != Some(WorkspacePanel::Arrangement)
        || ui.notation_reading.view.is_none()
    {
        return None;
    }
    let doc = ui.set.music.as_ref()?;
    Some(format!(
        "{:?}:{:?}:{}:{:?}:{}",
        ui.notation_reading.view,
        selection(ui),
        width(ui),
        ui.appearance.resolve(&ui.app_settings.appearance).palette,
        doc.musicxml
    ))
}
pub fn leaf(ui: &UiState) -> Option<NotationLeaf> {
    let score = ui.set.music.as_ref()?.score().ok()?;
    let palette = ui.appearance.resolve(&ui.app_settings.appearance).palette;
    let color = |c: tinct::Srgb| ColorF {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    };
    Some(NotationLeaf::new(
        score,
        ui.notation_reading.view?,
        selection(ui),
        width(ui),
        color(palette.text),
        color(palette.primary),
    ))
}
pub fn screen(ui: &UiState) -> UiChild {
    let mut children: Vec<UiChild> = Vec::new();
    if let Some(document) = &ui.set.music {
        let mut controls: Vec<UiChild> = Vec::new();
        for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
            controls.push(Box::new(clickable(
                el("button", text(view.label()))
                    .attr("class", "t-btn")
                    .attr(
                        "aria-pressed",
                        (ui.notation_reading.view == Some(view)).to_string(),
                    ),
                move |ui: &mut UiState, _| ui.notation_reading.view = Some(view),
            )));
        }
        children.push(Box::new(
            el("div", controls)
                .attr("class", "arrangement-actions")
                .attr("aria-label", "Musical reading"),
        ));
        match document.score() {
            Err(error) => children.push(Box::new(el(
                "p",
                text(format!("Notation unavailable: {error}. MusicXML retained.")),
            ))),
            Ok(score) => {
                if let Some(leaf) = leaf(ui) {
                    let size = leaf.intrinsic_size();
                    let mut overlay: Vec<UiChild> = vec![Box::new(
                        custom_leaf::<UiState, ()>(
                            NOTATION_LEAF_KEY,
                            size.width as u32,
                            size.height.ceil() as u32,
                        )
                        .attr(
                            "aria-label",
                            format!("{} notation", ui.notation_reading.view.unwrap().label()),
                        ),
                    )];
                    for label in leaf.labels() {
                        overlay.push(Box::new(
                            el("span", text(label.text))
                                .attr("aria-hidden", "true")
                                .attr(
                                    "style",
                                    format!(
                                        "position:absolute;left:{}px;top:{}px;pointer-events:none",
                                        label.x, label.y
                                    ),
                                ),
                        ));
                    }
                    for (bar, event, (x, y, width, height)) in leaf.event_rects() {
                        let n = &score.measures[bar].events[event];
                        let label = format!(
                            "Bar {}, event {} · {}{}",
                            bar + 1,
                            event + 1,
                            n.value.label(),
                            if n.rest { " rest" } else { "" }
                        );
                        let source = document.musicxml.clone();
                        let owner = ui.working_sets.active_id;
                        overlay.push(Box::new(clickable(el("button",text(label.clone())).attr("aria-label",label).attr("aria-pressed",(selection(ui)==(bar,event)).to_string()).attr("style",format!("position:absolute;left:{x}px;top:{y}px;width:{width}px;height:{height}px;padding:0;border:0;background:transparent;color:transparent")),move |ui: &mut UiState,_| {
                        if ui.working_sets.active_id == owner && ui.set.music.as_ref().is_some_and(|d| d.musicxml==source) { ui.notation_reading.selected=Some((owner,source.clone(),(bar,event))); }
                    })));
                    }
                    children.push(Box::new(el("div", overlay).attr(
                        "style",
                        format!(
                            "position:relative;width:{}px;height:{}px;flex:none",
                            size.width, size.height
                        ),
                    )));
                }
            },
        }
    }
    Box::new(
        el("div", children)
            .attr("class", "notation-reading")
            .attr("style", "overflow-x:auto;flex:none"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::notation::{MusicDocument, Score};
    #[test]
    fn readings_and_attention_do_not_author_and_corrupt_music_is_retained() {
        let mut ui = UiState::new();
        ui.set.music = Some(MusicDocument::from_score(&Score::rhythm_example()).unwrap());
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        let original = serde_json::to_vec(&ui.set).unwrap();
        let source = ui.set.music.as_ref().unwrap().musicxml.clone();
        ui.notation_reading.selected = Some((ui.working_sets.active_id, source, (0, 1)));
        for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
            ui.notation_reading.view = Some(view);
            assert!(leaf(&ui).is_some());
            assert_eq!(selection(&ui), (0, 1));
            assert_eq!(serde_json::to_vec(&ui.set).unwrap(), original);
        }
        ui.set.music.as_mut().unwrap().musicxml = "<broken>".into();
        assert!(leaf(&ui).is_none());
        assert_eq!(selection(&ui), (usize::MAX, usize::MAX));
        assert_eq!(ui.set.music.as_ref().unwrap().musicxml, "<broken>");
    }
}
