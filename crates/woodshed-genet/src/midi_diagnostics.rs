//! Desktop diagnostic controls and file-dialog coordination. No MIDI writer.
use crate::{shared::Shared, sync::Ctx};
use std::{
    io::Write,
    sync::{Arc, mpsc},
};
use woodshed_core::midi::MidiBackend as _;
use woodshed_views::stage::UiState;

#[derive(Default)]
pub struct DiagnosticExport {
    job: Option<mpsc::Receiver<Result<Option<std::path::PathBuf>, String>>>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    close_after: bool,
}
impl DiagnosticExport {
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }
    fn start(&mut self, ui: &mut UiState, bytes: Vec<u8>) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let wake = self.wake.clone();
        match std::thread::Builder::new()
            .name("woodshed-midi-diagnostic-export".into())
            .spawn(move || {
                let result = (|| {
                    let Some(path) = rfd::FileDialog::new()
                        .set_title("Export MIDI diagnostics to a new file")
                        .set_file_name("midi-diagnostics.json")
                        .add_filter("JSON", &["json"])
                        .save_file()
                    else {
                        return Ok(None);
                    };
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
                    file.write_all(&bytes)
                        .and_then(|_| file.sync_all())
                        .map_err(|error| format!("Could not finish {}: {error}", path.display()))?;
                    Ok(Some(path))
                })();
                let _ = tx.send(result);
                if let Some(wake) = wake {
                    wake();
                }
            }) {
            Ok(_) => {
                self.job = Some(rx);
                ui.midi.diagnostics_export_busy = true;
                ui.midi.diagnostics_notice =
                    Some("Choose a new JSON file for the captured snapshot.".into());
            },
            Err(error) => {
                ui.midi.diagnostics_notice = Some(format!("Could not start export: {error}"))
            },
        }
    }
    fn poll(&mut self, ui: &mut UiState) {
        let Some(rx) = &self.job else { return };
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("Diagnostic export stopped before completing.".into()),
        };
        self.job = None;
        ui.midi.diagnostics_export_busy = false;
        ui.midi.diagnostics_notice = Some(match result {
            Ok(Some(path)) => format!("Diagnostic snapshot exported to {}.", path.display()),
            Ok(None) => "Diagnostic export cancelled; history retained.".into(),
            Err(error) => error,
        });
    }
}

pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    ctx.runner.update(|ui| {
        shared.midi_diagnostics.poll(ui);
        if let Some(recording) = ui.midi.diagnostics_record_request.take() {
            shared.midi.set_diagnostics_recording(recording);
            ui.midi.diagnostics_notice = Some(
                if recording {
                    "Recording MIDI diagnostics for this session."
                } else {
                    "Recording stopped; captured history is retained for export."
                }
                .into(),
            );
        }
        if std::mem::take(&mut ui.midi.diagnostics_export_requested)
            && shared.midi_diagnostics.job.is_none()
        {
            match shared.midi.diagnostics_export() {
                Ok(bytes) => shared.midi_diagnostics.start(ui, bytes),
                Err(error) => ui.midi.diagnostics_notice = Some(error),
            }
        }
        ui.midi.diagnostics = shared.midi.diagnostics_status();
    });
}

pub fn poll(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    ctx.runner.update(|ui| {
        shared.midi_diagnostics.poll(ui);
        ui.midi.diagnostics = shared.midi.diagnostics_status();
    });
    if shared.midi_diagnostics.job.is_none()
        && std::mem::take(&mut shared.midi_diagnostics.close_after)
    {
        *ctx.close =
            crate::prepare_close(shared, ctx) == cambium_genet_winit_host::CloseDisposition::Exit;
    }
}

pub fn close_request(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
) -> Option<cambium_genet_winit_host::CloseDisposition> {
    if shared.midi_diagnostics.job.is_none() {
        return None;
    }
    shared.midi_diagnostics.close_after = true;
    ctx.runner.update(|ui| {
        ui.midi.diagnostics_notice =
            Some("Finish or cancel diagnostic export before closing.".into())
    });
    Some(cambium_genet_winit_host::CloseDisposition::KeepVisible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::CloseRequest;

    #[test]
    fn pending_export_defers_native_close_until_cancel_save_or_failure_is_observed() {
        for result in [
            Ok(None),
            Ok(Some(std::path::PathBuf::from("captured.json"))),
            Err("write refused".to_string()),
        ] {
            let store = woodshed_core::storage::SessionStore::new(Box::new(
                muniment::MemoryBackend::default(),
            )
                as crate::storage::HostBackend);
            let (shared, mut host) = crate::tests::save_close_harness(store);
            let (tx, rx) = mpsc::channel();
            shared.borrow_mut().midi_diagnostics.job = Some(rx);
            host.request_close(CloseRequest::Native);
            assert!(!host.close_requested());
            assert!(shared.borrow().midi_diagnostics.close_after);
            assert_eq!(
                host.state().midi.diagnostics_notice.as_deref(),
                Some("Finish or cancel diagnostic export before closing.")
            );
            tx.send(result).unwrap();
            host.wake().wake();
            assert!(host.process_wake());
            assert!(host.close_requested());
            assert!(shared.borrow().midi_diagnostics.job.is_none());
            assert!(!host.state().midi.diagnostics_export_busy);
        }
    }

    #[test]
    fn cancelled_or_failed_job_releases_busy_state_and_keeps_ui_history() {
        for result in [Ok(None), Err("write refused".to_string())] {
            let mut ui = UiState::new();
            ui.midi.diagnostics.retained_events = 7;
            ui.midi.diagnostics.history_available = true;
            ui.midi.diagnostics_export_busy = true;
            let (tx, rx) = mpsc::channel();
            let mut export = DiagnosticExport {
                job: Some(rx),
                ..Default::default()
            };
            tx.send(result).unwrap();
            export.poll(&mut ui);
            assert!(!ui.midi.diagnostics_export_busy);
            assert_eq!(ui.midi.diagnostics.retained_events, 7);
            assert!(ui.midi.diagnostics.history_available);
            assert!(export.job.is_none());
        }
    }
}
