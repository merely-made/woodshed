//! Native destination selection for the frozen MusicXML draft export.
use crate::sync::Ctx;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
fn destination(name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export MusicXML")
        .set_file_name(name)
        .add_filter("MusicXML", &["musicxml"])
        .save_file()
}
#[cfg(not(target_os = "macos"))]
fn destination(name: &str) -> Option<PathBuf> {
    cambium_genet_winit_host::choose_save_path("Export MusicXML", name, &["musicxml"])
}
pub fn after_dispatch(ctx: &mut Ctx<'_>) {
    after_dispatch_with(ctx, |name| {
        if std::env::var_os("WOODSHED_SCENARIO").is_some()
            && std::env::var_os("WOODSHED_MUSICXML_RECEIPT_EXPORT").is_some()
        {
            return std::env::var_os("WOODSHED_CAPTURE_DIR")
                .map(|dir| PathBuf::from(dir).join("exported.musicxml"));
        }
        destination(name)
    });
}
pub(crate) fn after_dispatch_with(ctx: &mut Ctx<'_>, choose: impl FnOnce(&str) -> Option<PathBuf>) {
    let request = ctx.runner.state().music_editor.export_request.clone();
    let Some((name, xml)) = request else {
        return;
    };
    ctx.runner
        .update(|ui| ui.music_editor.export_request = None);
    let Some(path) = choose(&name) else {
        ctx.runner
            .update(|ui| ui.music_editor.notice = Some("MusicXML export cancelled.".into()));
        return;
    };
    let result = (|| {
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Choose a regular MusicXML file, rather than a symbolic link.".to_owned());
        }
        let parent = path
            .parent()
            .ok_or("Choose a destination folder")?
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let target = if path.exists() {
            path.canonicalize().map_err(|e| e.to_string())?
        } else {
            parent.join(path.file_name().ok_or("Choose a file name")?)
        };
        let storage = crate::storage::FsBackend::new();
        for protected in storage.protected_export_paths() {
            if let Ok(existing) = protected.canonicalize() {
                if target == existing {
                    return Err(
                        "Choose an export outside Woodshed's saved session and settings.".into(),
                    );
                }
            }
        }
        for protected in storage.protected_export_directories() {
            if let Ok(existing) = protected.canonicalize() {
                if target.starts_with(existing) {
                    return Err("Choose an export outside Woodshed's private storage.".into());
                }
            }
        }
        use std::io::Write;
        let mut temporary = tempfile::NamedTempFile::new_in(&parent).map_err(|e| e.to_string())?;
        temporary
            .write_all(xml.as_bytes())
            .map_err(|e| e.to_string())?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary.persist(&target).map_err(|e| e.to_string())?;
        Ok::<_, String>(target)
    })();
    ctx.runner.update(|ui| {
        ui.music_editor.notice = Some(match result {
            Ok(path) => format!("MusicXML exported to {}", path.display()),
            Err(e) => format!("MusicXML export failed: {e}"),
        })
    });
}

#[cfg(test)]
mod tests {
    use taproot::Selector;
    use woodshed_views::{
        notation_leaf::NotationView, stage::music_editor, workspace::WorkspacePanel,
    };
    #[test]
    fn native_music_controls_capture_fret_drags_and_save_without_changing_cards() {
        let store = woodshed_core::storage::SessionStore::new(Box::new(
            muniment::MemoryBackend::default(),
        )
            as crate::storage::HostBackend);
        let (_, mut h) = crate::tests::save_close_harness(store);
        h.update(|ui| {
            ui.activate_workspace_panel(WorkspacePanel::Arrangement);
            ui.music_example(true);
            ui.set_music_view(NotationView::Tab);
            ui.set_viewport_width(1180.0);
        });
        h.after_dispatch();
        h.layout_at(1180.0, 1100.0);
        let cell = |s: &str, f: &str| {
            Selector::class("music-fret").with_attr("data-music-position", format!("s{s}/f{f}/"))
        };
        let cards_before = serde_json::to_value(&h.state().set.cards).unwrap();
        let before = h.state().music_draft().unwrap().score.clone();
        let from = h.resolve(&cell("2", "1")).expect("C4 fret button");
        let to = h.resolve(&cell("2", "3")).expect("D4 fret button");
        h.press_at(from.0, from.1);
        assert!(h.state().music_drag_active());
        h.move_to(to.0, to.1);
        h.relayout();
        assert_eq!(
            h.state().music_draft().unwrap().score,
            before,
            "preview leaves musical authority intact"
        );
        assert_eq!(
            h.state().music_drag_position(),
            Some((2, 3)),
            "capture={:?} notice={:?} active={} origin={:?} viewport={}",
            h.pointer_capture(),
            h.state().music_editor.notice,
            h.state().music_drag_active(),
            h.state().music_editor.active_tone,
            h.state().viewport_width
        );
        h.key_named(winit::keyboard::NamedKey::Escape);
        h.release_at(to.0, to.1);
        h.relayout();
        assert_eq!(
            h.state().music_draft().unwrap().score,
            before,
            "Escape cancels the captured move"
        );
        let from = h.resolve(&cell("2", "1")).unwrap();
        let to = h.resolve(&cell("2", "3")).unwrap();
        h.press_at(from.0, from.1);
        h.move_to(to.0, to.1);
        h.release_at(to.0, to.1);
        h.relayout();
        assert_eq!(
            h.state().music_draft().unwrap().score.measures[0].events[0].notes[0].fret,
            3
        );
        assert!(h.click_on(&Selector::class("music-undo")));
        assert_eq!(h.state().music_draft().unwrap().score, before);
        assert!(h.click_on(&cell("1", "0")));
        assert_eq!(
            h.state().music_draft().unwrap().score.measures[0].events[0]
                .notes
                .len(),
            2,
            "click adds a simultaneous tone"
        );
        // Undo in place retains the empty fret's native focus for keyboard entry.
        h.update(|ui| ui.music_undo());
        h.relayout();
        h.key_named(winit::keyboard::NamedKey::Space);
        h.relayout();
        assert_eq!(
            h.state().music_draft().unwrap().score.measures[0].events[0]
                .notes
                .len(),
            2,
            "Space adds the focused fret"
        );
        h.update(|ui| ui.music_undo());
        h.relayout();
        h.key_named(winit::keyboard::NamedKey::Enter);
        h.relayout();
        assert_eq!(
            h.state().music_draft().unwrap().score.measures[0].events[0]
                .notes
                .len(),
            2,
            "Enter adds the focused fret"
        );
        assert!(h.click_on(&Selector::class("music-save")));
        assert!(h.state().set.music.is_some());
        assert!(!h.state().music_draft().unwrap().dirty());
        assert_eq!(
            serde_json::to_value(&h.state().set.cards).unwrap(),
            cards_before
        );
        assert!(h.click_on(&Selector::class("music-view").with_attr("data-music-view", "Score")));
        assert_eq!(h.state().music_draft().unwrap().view, NotationView::Score);
        h.update(|ui| {
            ui.set_viewport_width(420.0);
        });
        h.layout_at(420.0, 1500.0);
        let leaf = music_editor::notation_leaf(h.state()).unwrap();
        assert!(leaf.intrinsic_size().width <= 420.0);
        h.update(|ui| ui.music_fret(3, 2));
        h.request_close(cambium_genet_winit_host::CloseRequest::Native);
        h.relayout();
        assert!(!h.close_requested());
        assert!(h.state().arrangement_editor.close_pending);
    }
}
