use super::*;

impl WavExport {
    pub(crate) fn is_finishing(&self) -> bool {
        self.job.is_some()
    }

    /// Serialize revocation with the worker's final link/rename. A commit
    /// already holding the gate completes before the host changes session.
    pub(crate) fn revoke_session(&mut self) {
        self.pending = None;
        if let Some(job) = &self.job {
            *job.cancelled.lock().expect("export commit gate") = true;
        }
    }
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }

    pub(super) fn spawn(
        &mut self,
        session: Arc<()>,
        task: impl FnOnce(Arc<Mutex<bool>>) -> Result<Outcome, String> + Send + 'static,
    ) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        let wake = self.wake.clone();
        let cancelled = Arc::new(Mutex::new(false));
        let worker_cancelled = cancelled.clone();
        std::thread::Builder::new()
            .name("woodshed-wav-export".into())
            .spawn(move || {
                let _ = sender.send(task(worker_cancelled));
                if let Some(wake) = wake {
                    wake();
                }
            })
            .map_err(|e| format!("Could not start WAV export: {e}"))?;
        self.job = Some(Job {
            session,
            receiver,
            cancelled,
        });
        Ok(())
    }

    pub(super) fn take_completion(&mut self, session: &Arc<()>) -> Option<Result<Outcome, String>> {
        if self
            .pending
            .as_ref()
            .is_some_and(|(old, _)| !Arc::ptr_eq(old, session))
        {
            self.pending = None;
        }
        let job = self.job.as_ref()?;
        if !Arc::ptr_eq(&job.session, session) {
            *job.cancelled.lock().expect("export commit gate") = true;
        }
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("WAV export worker stopped before reporting completion.".into())
            },
        };
        let job = self.job.take().expect("observed job");
        Arc::ptr_eq(&job.session, session).then_some(result)
    }

    pub(super) fn complete(&mut self, ui: &mut UiState, result: Result<Outcome, String>) {
        ui.loop_export_busy = false;
        match result {
            Ok(Outcome::Cancelled) => {
                ui.loop_export_notice = Some("WAV export cancelled; no file was written.".into());
            },
            Ok(Outcome::Saved(path, report)) => {
                ui.loop_export_notice = Some(format!(
                    "Exported {} · {:.3} s · {} Hz · backing + completed loops.",
                    display_name(&path),
                    report.frames as f64 / report.sample_rate as f64,
                    report.sample_rate
                ));
            },
            Ok(Outcome::Collision(artifact, fingerprint)) => {
                self.next_token = self.next_token.wrapping_add(1);
                let token = self.next_token;
                let name = display_name(&artifact.path);
                ui.loop_export_notice = Some(format!(
                    "{name} already exists. Replace it with this rendered pass?"
                ));
                ui.loop_export_replace = Some((token, name));
                self.pending = Some((
                    ui.audio_session.clone(),
                    Collision {
                        artifact,
                        fingerprint,
                        token,
                    },
                ));
            },
            Err(error) => ui.loop_export_notice = Some(format!("WAV export failed: {error}")),
        }
    }
}
