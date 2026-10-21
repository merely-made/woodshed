// SPDX-License-Identifier: MPL-2.0
//! Shared kit controls routed through the existing project actor and recorder.
use super::{AppState, ProjectCommand};
use std::path::PathBuf;

impl AppState {
    /// Shared kit controls mutate only this project's local draft. A rendered
    /// contribution enters the recorder's normal AppendLayer history path.
    pub fn pack_action(&mut self, action: audio_pack_views::PackAction) {
        if self.is_recording() || self.is_project_io_active() {
            self.pack_notice =
                "Stop capture and wait for project work before editing a kit.".into();
            return;
        }
        if let Err(error) = self.try_pack_action(action) {
            self.pack_notice = error;
        }
    }
    fn try_pack_action(&mut self, action: audio_pack_views::PackAction) -> Result<(), String> {
        use audio_pack_views::PackAction;
        match action {
            PackAction::Bundled | PackAction::Import => {
                let path = if matches!(action, PackAction::Bundled) {
                    PathBuf::from("bundled")
                } else {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose an audio pack folder")
                        .pick_folder()
                    else {
                        return Ok(());
                    };
                    path
                };
                let before = self
                    .recorder
                    .audio_pack
                    .as_ref()
                    .map(audio_packs::PackSelection::revision);
                self.submit_project(ProjectCommand::LoadPack { path, before });
                self.pack_notice = "Preparing kit files…".into();
            },
            PackAction::Clear => {
                self.recorder.audio_pack = None;
                self.pack_notice = "Kit draft cleared; existing drum layers are retained.".into();
            },
            PackAction::Pattern { revision, id } => {
                let current = self
                    .recorder
                    .audio_pack
                    .as_ref()
                    .ok_or("Load a kit first.")?;
                if current.revision() != revision {
                    return Err("Kit changed; retry the selection.".into());
                }
                self.recorder.audio_pack =
                    Some(current.select_pattern(&id).map_err(|e| e.to_string())?);
            },
            PackAction::Edit { revision, edit } => {
                self.recorder.audio_pack = Some(
                    self.recorder
                        .audio_pack
                        .as_ref()
                        .ok_or("Load a kit first.")?
                        .edit(&revision, edit)
                        .map_err(|e| e.to_string())?,
                );
                self.pack_notice =
                    "Kit draft updated. Add a loop to audition it through the recorder."
                        .into();
            },
            PackAction::ExportMidi => {
                let selection = self
                    .recorder
                    .audio_pack
                    .as_ref()
                    .ok_or("Load a kit first.")?
                    .at_tempo(self.recorder.session.bpm)
                    .map_err(|e| e.to_string())?;
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Export rhythm MIDI to a new file")
                    .set_file_name("rhythm.mid")
                    .add_filter("MIDI", &["mid"])
                    .save_file()
                {
                    self.submit_project(ProjectCommand::ExportMidi { path, selection });
                }
            },
            PackAction::AddLoop => {
                let selection = self
                    .recorder
                    .audio_pack
                    .as_ref()
                    .ok_or("Load a kit first.")?
                    .clone();
                let meter = self.recorder.session.time_signature;
                if selection.pattern().time_signature.numerator != meter.numerator
                    || selection.pattern().time_signature.denominator != meter.denominator
                {
                    return Err(
                        "Choose a kit pattern with the project's meter before adding a loop."
                            .into(),
                    );
                }
                let index = self
                    .recorder
                    .armed_index()
                    .ok_or("Arm a track before adding a drum loop.")?;
                let track = self.recorder.session.tracks[index].id;
                let revision = selection.revision();
                self.submit_project(ProjectCommand::RenderPack {
                    selection,
                    revision,
                    track,
                    bpm: self.recorder.session.bpm,
                    meter,
                    bars: self.recorder.session.bars_per_phrase,
                });
                self.pack_notice = "Rendering the current rhythm at project tempo…".into();
            },
        }
        Ok(())
    }
}
