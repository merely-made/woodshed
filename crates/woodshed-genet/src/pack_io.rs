// SPDX-License-Identifier: MPL-2.0
//! File preparation off the dispatch thread; authority stays in UiState/session.
use crate::{shared::Shared, sync::Ctx};
use audio_pack_views::PackAction;
use audio_packs::PackSelection;
use std::sync::{Arc, mpsc};
use woodshed_views::stage::UiState;

enum Outcome {
    Loaded(PackSelection),
    Exported,
    Cancelled,
}
struct Job {
    session: Arc<()>,
    revision: Option<String>,
    receiver: mpsc::Receiver<Result<Outcome, String>>,
}
#[derive(Default)]
pub struct PackIo {
    close_after: bool,
    job: Option<Job>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl PackIo {
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }
    fn start(
        &mut self,
        ui: &mut UiState,
        task: impl FnOnce() -> Result<Outcome, String> + Send + 'static,
    ) {
        if self.job.is_some() {
            ui.pack_notice = "A kit operation is still finishing.".into();
            return;
        }
        let (tx, receiver) = mpsc::channel();
        let wake = self.wake.clone();
        match std::thread::Builder::new()
            .name("woodshed-pack-files".into())
            .spawn(move || {
                let result = task();
                let _ = tx.send(result);
                if let Some(wake) = wake {
                    wake();
                }
            }) {
            Ok(_) => {
                self.job = Some(Job {
                    session: ui.audio_session.clone(),
                    revision: ui.audio_pack.as_ref().map(PackSelection::revision),
                    receiver,
                });
                ui.pack_busy = true;
                ui.pack_notice = "Preparing kit files…".into();
            },
            Err(e) => ui.pack_notice = e.to_string(),
        }
    }
    fn poll(&mut self, ui: &mut UiState) {
        let Some(job) = &self.job else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(value) => value,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("Kit worker stopped before completing.".into()),
        };
        let job = self.job.take().expect("present above");
        if !Arc::ptr_eq(&job.session, &ui.audio_session) {
            return;
        }
        ui.pack_busy = false;
        if job.revision != ui.audio_pack.as_ref().map(PackSelection::revision) {
            ui.pack_notice =
                "Kit changed while preparing files; the current rhythm was retained.".into();
            return;
        }
        match result {
            Ok(Outcome::Loaded(selection)) => {
                if ui.song_recording || ui.calib_active {
                    ui.pack_notice = "Stop recording or calibration and load the kit again.".into();
                    return;
                }
                ui.audio_pack = Some(selection);
                ui.pack_notice = "Kit loaded. Play uses the current tempo; your rhythm and assets are saved with practice.".into();
            },
            Ok(Outcome::Exported) => {
                ui.pack_notice = "MIDI exported from the requested rhythm and tempo.".into()
            },
            Ok(Outcome::Cancelled) => ui.pack_notice = "File choice cancelled.".into(),
            Err(e) => ui.pack_notice = e,
        }
    }
    fn action(&mut self, ui: &mut UiState, action: PackAction) -> Result<(), String> {
        if ui.pack_busy {
            return Err("Wait for kit file preparation.".into());
        }
        if ui.song_recording || ui.calib_active {
            return Err("Stop recording or calibration before changing the drum kit.".into());
        }
        match action {
            PackAction::Bundled => self.start(ui, || {
                audio_packs::files::load(std::path::Path::new("bundled"), None)
                    .map(Outcome::Loaded)
                    .map_err(|e| e.to_string())
            }),
            PackAction::Import => self.start(ui, || {
                let Some(path) = rfd::FileDialog::new()
                    .set_title("Choose an audio pack folder")
                    .pick_folder()
                else {
                    return Ok(Outcome::Cancelled);
                };
                audio_packs::files::load(&path, None)
                    .map(Outcome::Loaded)
                    .map_err(|e| e.to_string())
            }),
            PackAction::Clear => {
                ui.audio_pack = None;
                ui.pack_notice = "Using the metronome click.".into();
            },
            PackAction::ExportMidi => {
                let selection = ui
                    .audio_pack
                    .as_ref()
                    .ok_or("Load a kit first.")?
                    .at_tempo(ui.transport.bpm)
                    .map_err(|e| e.to_string())?;
                self.start(ui, move || {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Export rhythm MIDI to a new file")
                        .set_file_name("rhythm.mid")
                        .add_filter("MIDI", &["mid"])
                        .save_file()
                    else {
                        return Ok(Outcome::Cancelled);
                    };
                    audio_packs::files::export_midi(&selection, &path)
                        .map_err(|e| e.to_string())?;
                    Ok(Outcome::Exported)
                });
            },
            PackAction::Pattern { revision, id } => {
                let current = ui.audio_pack.as_ref().ok_or("Load a kit first.")?;
                if current.revision() != revision {
                    return Err("Kit changed; retry the selection.".into());
                }
                ui.audio_pack = Some(current.select_pattern(&id).map_err(|e| e.to_string())?);
            },
            PackAction::Edit { revision, edit } => {
                ui.audio_pack = Some(
                    ui.audio_pack
                        .as_ref()
                        .ok_or("Load a kit first.")?
                        .edit(&revision, edit)
                        .map_err(|e| e.to_string())?,
                );
                ui.pack_notice = "Rhythm updated; Play and MIDI export use this pattern.".into();
            },
            PackAction::AddLoop => {
                return Err("Use the recorder's material import for a drum layer.".into());
            },
        }
        Ok(())
    }
}
pub fn poll(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let finishing = shared.pack_io.job.is_some();
    ctx.runner.update(|ui| shared.pack_io.poll(ui));
    if finishing && shared.pack_io.job.is_none() {
        crate::sync::after_dispatch(shared, ctx);
        if std::mem::take(&mut shared.pack_io.close_after) {
            *ctx.close = crate::prepare_close(shared, ctx)
                == cambium_genet_winit_host::CloseDisposition::Exit;
        }
    }
}
pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    ctx.runner.update(|ui| {
        shared.pack_io.poll(ui);
        for action in std::mem::take(&mut ui.pack_actions) {
            if let Err(e) = shared.pack_io.action(ui, action) {
                ui.pack_notice = e;
            }
        }
    });
}

pub fn close_request(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Option<cambium_genet_winit_host::CloseDisposition> {
    if shared.pack_io.job.is_none() {
        return None;
    }
    shared.pack_io.close_after = true;
    ctx.runner
        .update(|ui| ui.pack_notice = "Finishing kit files before closing…".into());
    Some(cambium_genet_winit_host::CloseDisposition::KeepVisible)
}
