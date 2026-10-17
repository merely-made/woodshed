//! Explicit Looper snapshot, native save choice and atomic WAV publication.
//! Rendering and filesystem work run on one bounded worker. Collision consent
//! retains that rendered snapshot; edits cannot redirect a later Replace.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use cambium_genet_winit_host::CloseDisposition;
use muniment::blob::Hash;
use tempfile::NamedTempFile;
use woodshed_audio::Song;
use woodshed_audio::offline::{SongExportReport, export_song_wav_to};
use woodshed_core::loop_plan::LooperSource;
use woodshed_views::stage::{LoopExportRequest, UiState};

use crate::shared::Shared;
use crate::sync::Ctx;

const MAX_REPLACE_BYTES: u64 = 64 * 1024 * 1024 + 4096;

#[derive(Clone, Default)]
struct Protection {
    paths: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

fn normalized(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let mut lexical = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::ParentDir => {
                lexical.pop();
            },
            Component::CurDir => {},
            other => lexical.push(other.as_os_str()),
        }
    }
    let mut ancestor = lexical.as_path();
    let mut missing = Vec::new();
    while !ancestor.try_exists().map_err(|e| e.to_string())? {
        missing.push(
            ancestor
                .file_name()
                .ok_or("Invalid export destination.")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().ok_or("Invalid export destination.")?;
    }
    let mut result = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for part in missing.into_iter().rev() {
        result.push(part);
    }
    Ok(result)
}

impl Protection {
    fn current() -> Self {
        let storage = crate::storage::FsBackend::new();
        let mut paths = storage.protected_export_paths();
        if let Some(path) = crate::appearance::configured_library_path() {
            paths.push(path);
        }
        Self {
            paths,
            directories: storage.protected_export_directories(),
        }
    }

    fn check(&self, path: &Path) -> Result<PathBuf, String> {
        if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("Choose a regular WAV destination rather than a symbolic link.".into());
        }
        let filename = path.file_name().ok_or("Invalid WAV destination.")?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(filename);
        for protected in &self.paths {
            let protected = normalized(protected)?;
            if reserved_match(&path, &protected, false) || same_file(&path, &protected) {
                return Err(
                    "WAV export cannot replace application storage. Choose another destination."
                        .into(),
                );
            }
        }
        for protected in &self.directories {
            if reserved_match(&path, &normalized(protected)?, true) {
                return Err("WAV export cannot write inside captured-loop storage. Choose another destination.".into());
            }
        }
        Ok(path)
    }
}

// Reserve case aliases even for files/directories that do not exist yet.
// Canonicalizing an existing ancestor alone cannot detect these on macOS.
fn reserved_match(path: &Path, reserved: &Path, subtree: bool) -> bool {
    let path: Vec<_> = path.components().collect();
    let reserved: Vec<_> = reserved.components().collect();
    (if subtree {
        path.len() >= reserved.len()
    } else {
        path.len() == reserved.len()
    }) && path.iter().zip(reserved.iter()).all(|(a, b)| {
        match (a.as_os_str().to_str(), b.as_os_str().to_str()) {
            (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
            _ => a
                .as_os_str()
                .as_encoded_bytes()
                .eq_ignore_ascii_case(b.as_os_str().as_encoded_bytes()),
        }
    })
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(a)
        .ok()
        .zip(fs::metadata(b).ok())
        .is_some_and(|(a, b)| a.dev() == b.dev() && a.ino() == b.ino())
}
#[cfg(not(unix))]
fn same_file(_: &Path, _: &Path) -> bool {
    false
}

fn fingerprint(path: &Path) -> Result<Hash, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_REPLACE_BYTES {
        return Err("Choose another destination; only regular files within the WAV export size limit can be replaced.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_REPLACE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_REPLACE_BYTES {
        return Err("Destination exceeds the replacement size limit.".into());
    }
    Ok(Hash::of(&bytes))
}

struct Artifact {
    file: NamedTempFile,
    path: PathBuf,
    report: SongExportReport,
    protection: Protection,
}
struct Collision {
    artifact: Artifact,
    fingerprint: Hash,
    token: u64,
}
enum Outcome {
    Saved(PathBuf, SongExportReport),
    Collision(Artifact, Hash),
    Cancelled,
}
struct Job {
    session: Arc<()>,
    receiver: mpsc::Receiver<Result<Outcome, String>>,
    cancelled: Arc<Mutex<bool>>,
}

#[derive(Default)]
pub struct WavExport {
    job: Option<Job>,
    pending: Option<(Arc<()>, Collision)>,
    next_token: u64,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    close_when_finished: bool,
}

impl WavExport {
    pub(crate) fn is_finishing(&self) -> bool { self.job.is_some() }

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

    fn spawn(
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

    fn take_completion(&mut self, session: &Arc<()>) -> Option<Result<Outcome, String>> {
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

    fn complete(&mut self, ui: &mut UiState, result: Result<Outcome, String>) {
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

fn display_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| "WAV".into(), |name| name.to_string_lossy().into_owned())
}

fn publish(
    artifact: Artifact,
    replacement: Option<Hash>,
    cancelled: &Mutex<bool>,
) -> Result<Outcome, String> {
    let path = artifact.protection.check(&artifact.path)?;
    if let Some(expected) = replacement {
        if fingerprint(&path)? != expected {
            return Err("The destination changed after the replacement prompt. Export again to review the current file.".into());
        }
        let _commit = commit_permit(cancelled)?;
        artifact
            .file
            .persist(&path)
            .map_err(|e| e.error.to_string())?;
        return Ok(Outcome::Saved(path, artifact.report));
    }
    // link/no-clobber is the commit point. A file appearing during rendering
    // is treated as a collision, never truncated by a check-then-create race.
    let _commit = commit_permit(cancelled)?;
    match artifact.file.persist_noclobber(&path) {
        Ok(_) => Ok(Outcome::Saved(path, artifact.report)),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            let digest = fingerprint(&path)?;
            Ok(Outcome::Collision(
                Artifact {
                    file: error.file,
                    ..artifact
                },
                digest,
            ))
        },
        Err(error) => Err(error.error.to_string()),
    }
}

fn render(
    song: Song,
    rate: u32,
    path: PathBuf,
    protection: Protection,
    cancelled: &Mutex<bool>,
) -> Result<Outcome, String> {
    let path = protection.check(&path)?;
    let parent = path.parent().ok_or("WAV destination has no directory.")?;
    let mut file = tempfile::Builder::new()
        .prefix(".woodshed-wav-")
        .tempfile_in(parent)
        .map_err(|e| e.to_string())?;
    let report = export_song_wav_to(&song, rate, file.as_file_mut()).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    publish(
        Artifact {
            file,
            path,
            report,
            protection,
        },
        None,
        cancelled,
    )
}

fn snapshot(shared: &mut Shared, ui: &UiState) -> Result<(Song, u32), String> {
    if !crate::capture_persistence::session_current(shared, ui) {
        return Err(
            "Wait for the current practice session to restore before exporting WAV.".into(),
        );
    }
    match ui.looper.source {
        LooperSource::SavedSong if shared.last_loop_plan.is_none() && shared.last_song.as_ref() == Some(&ui.song) => {},
        LooperSource::Set(_) if ui.requested_loop_plan().ok().as_ref() == shared.last_loop_plan.as_ref() && shared.last_loop_plan.is_some() => {},
        _ => return Err("The requested backing has not been admitted. Resolve the Looper notice before exporting WAV.".into()),
    }
    shared
        .capture_persistence
        .permit_export(shared.last_loop_plan.as_ref())?;
    let (song, rate) = shared
        .backend
        .as_mut()
        .ok_or("Audio output is unavailable.")?
        .export_backing()?;
    if song.bars.is_empty() {
        return Err("Load backing before exporting WAV.".into());
    }
    Ok((song, rate))
}

pub fn poll(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let session = ctx.runner.state().audio_session.clone();
    if let Some(result) = shared.wav_export.take_completion(&session) {
        ctx.runner
            .update(|ui| shared.wav_export.complete(ui, result));
    }
    if shared.wav_export.close_when_finished
        && shared.wav_export.job.is_none()
        && !shared.capture_persistence.preparation_busy()
    {
        shared.wav_export.close_when_finished = false;
        shared.wav_export.pending = None;
        ctx.runner.update(|ui| ui.loop_export_replace = None);
        if crate::prepare_close(shared, ctx) == CloseDisposition::Exit {
            *ctx.close = true;
        }
    }
}

pub fn close_request(shared: &mut Shared, ctx: &mut Ctx<'_>) -> Option<CloseDisposition> {
    if shared.wav_export.job.is_some() {
        shared.wav_export.close_when_finished = true;
        ctx.runner.update(|ui| {
            ui.loop_export_notice = Some("Woodshed will close when the WAV export finishes.".into())
        });
        Some(CloseDisposition::KeepVisible)
    } else {
        None
    }
}

fn commit_permit(cancelled: &Mutex<bool>) -> Result<MutexGuard<'_, bool>, String> {
    let permit = cancelled
        .lock()
        .map_err(|_| "WAV export commit gate failed.".to_string())?;
    if *permit {
        Err("WAV export cancelled because the practice session changed.".into())
    } else {
        Ok(permit)
    }
}

pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    after_dispatch_inner(shared, ctx, None::<fn(&str) -> Option<PathBuf>>);
}

#[cfg(target_os = "macos")]
fn choose_wav_destination(name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export Looper WAV")
        .set_file_name(name)
        .add_filter("Wave audio", &["wav"])
        .save_file()
}

#[cfg(not(target_os = "macos"))]
fn choose_wav_destination(name: &str) -> Option<PathBuf> {
    cambium_genet_winit_host::choose_save_path("Export Looper WAV", name, &["wav"])
}

pub(crate) fn after_dispatch_with_chooser(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    choose: impl FnMut(&str) -> Option<PathBuf>,
) {
    after_dispatch_inner(shared, ctx, Some(choose));
}

fn after_dispatch_inner<F: FnMut(&str) -> Option<PathBuf>>(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    mut choose: Option<F>,
) {
    poll(shared, ctx);
    if ctx.runner.state().loop_export_requests.is_empty() {
        return;
    }
    let mut requests = Vec::new();
    ctx.runner
        .update(|ui| requests = std::mem::take(&mut ui.loop_export_requests));
    for request in requests {
        let result = match request {
            LoopExportRequest::ChooseDestination => {
                if shared.wav_export.job.is_some() {
                    Err("A WAV export is already finishing.".into())
                } else {
                    choose_and_start(shared, ctx, choose.as_mut())
                }
            },
            LoopExportRequest::Replace(token) => replace(shared, ctx, token),
            LoopExportRequest::CancelReplace(token) => {
                if shared
                    .wav_export
                    .pending
                    .as_ref()
                    .is_some_and(|(_, pending)| pending.token == token)
                {
                    shared.wav_export.pending = None;
                    ctx.runner.update(|ui| {
                        ui.loop_export_replace = None;
                        ui.loop_export_notice = Some(
                            "WAV replacement cancelled; the existing file is unchanged.".into(),
                        );
                    });
                }
                Ok(())
            },
        };
        if let Err(error) = result {
            ctx.runner.update(|ui| {
                ui.loop_export_busy = false;
                ui.loop_export_notice = Some(error);
            });
        }
    }
}

fn choose_and_start<F: FnMut(&str) -> Option<PathBuf>>(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    choose: Option<&mut F>,
) -> Result<(), String> {
    let (song, rate) = snapshot(shared, ctx.runner.state())?;
    let session = ctx.runner.state().audio_session.clone();
    let name = format!(
        "{}.wav",
        song.name
            .chars()
            .filter(|c| !c.is_control() && !"/\\:".contains(*c))
            .take(80)
            .collect::<String>()
    );
    shared.wav_export.pending = None;
    ctx.runner.update(|ui| ui.loop_export_replace = None);
    // RFD marshals AppKit work to the main queue. Schedule it only after the
    // dispatch returns, releasing Shared and the host's input-handler borrows
    // before AppKit enters its nested modal event loop.
    let Some(choose) = choose else {
        start_dialog(
            &mut shared.wav_export,
            session,
            song,
            rate,
            name,
            Protection::current(),
            choose_wav_destination,
        )?;
        ctx.runner.update(|ui| {
            ui.loop_export_busy = true;
            ui.loop_export_notice = Some("Choose a WAV destination…".into());
        });
        return Ok(());
    };
    let Some(mut path) = choose(&name) else {
        ctx.runner.update(|ui| {
            ui.loop_export_notice = Some("WAV export cancelled; no file was written.".into())
        });
        return Ok(());
    };
    if !Arc::ptr_eq(&session, &ctx.runner.state().audio_session) {
        return Err(
            "The practice session changed while choosing a WAV destination. Export again.".into(),
        );
    }
    path = wav_path(path)?;
    let protection = Protection::current();
    let path = protection.check(&path)?;
    shared.wav_export.spawn(session, move |cancelled| {
        render(song, rate, path, protection, &cancelled)
    })?;
    ctx.runner.update(|ui| {
        ui.loop_export_busy = true;
        ui.loop_export_notice = Some("Rendering backing + completed loops…".into());
    });
    Ok(())
}

fn wav_path(mut path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_none() {
        path.set_extension("wav");
    }
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wav"))
    {
        return Err("Choose a .wav destination.".into());
    }
    Ok(path)
}

fn start_dialog(
    exporter: &mut WavExport,
    session: Arc<()>,
    song: Song,
    rate: u32,
    name: String,
    protection: Protection,
    choose: impl FnOnce(&str) -> Option<PathBuf> + Send + 'static,
) -> Result<(), String> {
    exporter.spawn(session, move |cancelled| {
        drop(commit_permit(&cancelled)?);
        let Some(path) = choose(&name) else {
            return Ok(Outcome::Cancelled);
        };
        // Recheck after the modal choice; a profile change can revoke this job
        // while the native panel is open. Final publication checks the gate too.
        drop(commit_permit(&cancelled)?);
        render(song, rate, wav_path(path)?, protection, &cancelled)
    })
}

fn replace(shared: &mut Shared, ctx: &mut Ctx<'_>, token: u64) -> Result<(), String> {
    let pending = shared
        .wav_export
        .pending
        .as_ref()
        .ok_or("There is no WAV replacement awaiting confirmation.")?;
    if !Arc::ptr_eq(&pending.0, &ctx.runner.state().audio_session) || pending.1.token != token {
        return Err("This WAV replacement belongs to an earlier export. Export again.".into());
    }
    let (session, pending) = shared
        .wav_export
        .pending
        .take()
        .expect("validated pending export");
    ctx.runner.update(|ui| ui.loop_export_replace = None);
    shared.wav_export.spawn(session, move |cancelled| {
        publish(pending.artifact, Some(pending.fingerprint), &cancelled)
    })?;
    ctx.runner.update(|ui| {
        ui.loop_export_busy = true;
        ui.loop_export_notice = Some("Saving rendered WAV…".into());
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::{Harness, HostHooks, Init};
    use std::cell::Cell;
    use std::rc::Rc;
    use taproot::Selector;

    #[test]
    fn deferred_dialog_cancel_and_epoch_revocation_preserve_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chosen.wav");
        let mut exporter = WavExport::default();
        let mut ui = UiState::new();
        start_dialog(
            &mut exporter,
            ui.audio_session.clone(),
            song(0.1),
            48_000,
            "take.wav".into(),
            Protection::default(),
            |_| None,
        )
        .unwrap();
        let job = exporter.job.take().unwrap();
        exporter.complete(&mut ui, job.receiver.recv().unwrap());
        assert!(
            ui.loop_export_notice
                .as_ref()
                .unwrap()
                .contains("cancelled")
        );
        assert!(!path.exists());

        fs::write(&path, b"keep").unwrap();
        let target = path.clone();
        let (entered, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        start_dialog(
            &mut exporter,
            ui.audio_session.clone(),
            song(0.1),
            48_000,
            "take.wav".into(),
            Protection::default(),
            move |_| {
                entered.send(()).unwrap();
                blocked.recv().unwrap();
                Some(target)
            },
        )
        .unwrap();
        ready.recv().unwrap();
        exporter.revoke_session();
        let saved = ui.to_persisted();
        ui.apply_persisted(&saved, ui.app_settings.clone());
        release.send(()).unwrap();
        let job = exporter.job.take().unwrap();
        assert!(
            job.receiver
                .recv()
                .unwrap()
                .err()
                .unwrap()
                .contains("cancelled")
        );
        assert!(ui.loop_export_notice.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn deferred_export_close_preserves_the_dirty_appearance_gate() {
        let dir = tempfile::tempdir().unwrap();
        let store = woodshed_core::storage::SessionStore::new(
            Box::new(muniment::MemoryBackend::default()) as crate::storage::HostBackend,
        );
        let shared = Shared::with_store(Some(store), None, None);
        let mut ui = UiState::new();
        crate::capture_persistence::begin_session(&mut shared.borrow_mut(), &ui).unwrap();
        ui.appearance =
            woodshed_views::appearance::AppearanceState::load(dir.path().join("themes.json"))
                .unwrap();
        ui.appearance
            .begin_edit(&ui.app_settings.appearance)
            .unwrap();
        ui.appearance.workshop.new_copy();
        assert!(ui.appearance.workshop.has_changes());
        let (sender, receiver) = mpsc::channel();
        shared.borrow_mut().wav_export.job = Some(Job {
            session: ui.audio_session.clone(),
            receiver,
            cancelled: Arc::new(Mutex::new(false)),
        });
        let close_shared = shared.clone();
        let wake_shared = shared.clone();
        let mut hooks: HostHooks<UiState, crate::sync::Logic, woodshed_views::stage::UiChild> =
            HostHooks::inert();
        hooks.close_request = Box::new(move |ctx, _| {
            close_request(&mut close_shared.borrow_mut(), ctx)
                .unwrap_or_else(|| crate::appearance::close_request(ctx))
        });
        hooks.after_wake = Box::new(move |ctx| poll(&mut wake_shared.borrow_mut(), ctx));
        let logic: crate::sync::Logic = Box::new(woodshed_views::appearance::workshop_screen);
        let mut host = Harness::with_hooks(
            Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic,
                fonts: Vec::new(),
                images: Vec::new(),
            },
            hooks,
        );
        host.layout_at(1280.0, 960.0);
        host.request_close(cambium_genet_winit_host::CloseRequest::Native);
        assert!(!host.close_requested());
        sender
            .send(Err("fixture worker completion".into()))
            .unwrap();
        (host.wake().callback())();
        assert!(host.process_wake());
        assert!(!host.close_requested());
        assert!(host.state().appearance.workshop.has_changes());
        assert!(host.state().appearance.workshop.close_requested());
        assert!(host.state().appearance_close_app);
        assert!(!shared.borrow().wav_export.close_when_finished);
    }

    #[test]
    fn export_completion_rechecks_later_save_failure_before_closing() {
        let refusing = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let backend = crate::tests::SaveFailureBackend {
            inner: Default::default(),
            refusing: refusing.clone(),
        };
        let store = woodshed_core::storage::SessionStore::new(
            Box::new(backend) as crate::storage::HostBackend
        );
        let (shared, mut host) = crate::tests::save_close_harness(store);
        let (sender, receiver) = mpsc::channel();
        shared.borrow_mut().wav_export.job = Some(Job {
            session: host.state().audio_session.clone(),
            receiver,
            cancelled: Arc::new(Mutex::new(false)),
        });
        host.request_close(cambium_genet_winit_host::CloseRequest::Native);
        assert!(!host.close_requested());
        refusing.store(true, std::sync::atomic::Ordering::Relaxed);
        host.update(|ui| ui.stage_current(None));
        host.after_dispatch();
        sender.send(Err("fixture completion".into())).unwrap();
        (host.wake().callback())();
        assert!(host.process_wake());
        assert!(!host.close_requested());
        assert!(host.state().practice_close_pending);
        assert_eq!(host.state().set.cards.len(), 2);
        let persisted = shared.borrow().storage.as_ref().unwrap().load().unwrap();
        assert_eq!(
            woodshed_core::storage::decode_session(&persisted)
                .unwrap()
                .session
                .set
                .cards
                .len(),
            1
        );
    }

    #[test]
    fn explicit_discard_survives_export_deferral_without_later_autosave() {
        let refusing = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let backend = crate::tests::SaveFailureBackend {
            inner: Default::default(),
            refusing: refusing.clone(),
        };
        let store = woodshed_core::storage::SessionStore::new(
            Box::new(backend) as crate::storage::HostBackend
        );
        let (shared, mut host) = crate::tests::save_close_harness(store);
        let (sender, receiver) = mpsc::channel();
        shared.borrow_mut().wav_export.job = Some(Job {
            session: host.state().audio_session.clone(),
            receiver,
            cancelled: Arc::new(Mutex::new(false)),
        });
        host.request_close(cambium_genet_winit_host::CloseRequest::Native);
        host.relayout();
        assert!(host.click_on(&taproot::Selector::class("practice-close-discard")));
        assert!(host.state().practice_discard_close);
        assert!(!host.close_requested());
        refusing.store(false, std::sync::atomic::Ordering::Relaxed);
        host.after_dispatch();
        assert!(shared.borrow().storage.as_ref().unwrap().load().is_none());
        sender.send(Err("fixture completion".into())).unwrap();
        (host.wake().callback())();
        assert!(host.process_wake());
        assert!(host.close_requested());
        assert!(shared.borrow().storage.as_ref().unwrap().load().is_none());
        assert!(
            shared
                .borrow()
                .storage
                .as_ref()
                .unwrap()
                .load_settings()
                .is_none()
        );
    }
    use woodshed_audio::{Bar, SampleBuffer};
    use woodshed_core::audio::AudioBackend;

    #[test]
    fn native_chooser_cancellation_is_one_request_and_writes_nothing() {
        let shared = Shared::with_store(None, None, None);
        let mut ui = UiState::new();
        ui.song.bars.push(woodshed_core::song::SongBar::default());
        ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        {
            let mut shared = shared.borrow_mut();
            let mut backend = crate::audio::CpalBackend::offline_for_test();
            backend.try_set_song(&ui.song).unwrap();
            shared.backend = Some(backend);
            shared.last_song = Some(ui.song.clone());
            shared.last_audio_session = Some(ui.audio_session.clone());
        }
        let chosen = Rc::new(Cell::new(0));
        let calls = chosen.clone();
        let binding = shared.clone();
        let mut hooks: HostHooks<UiState, crate::sync::Logic, woodshed_views::stage::UiChild> =
            HostHooks::inert();
        hooks.after_dispatch = Box::new(move |ctx| {
            after_dispatch_with_chooser(&mut binding.borrow_mut(), ctx, |_| {
                calls.set(calls.get() + 1);
                None
            });
        });
        let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
        let mut host = Harness::with_hooks(
            Init {
                sheet: crate::appearance::stylesheet(&ui),
                state: ui,
                logic,
                fonts: Vec::new(),
                images: Vec::new(),
            },
            hooks,
        );
        host.layout_at(1280.0, 960.0);
        assert!(host.click_on(&Selector::class("t-btn").containing("Export WAV")));
        assert_eq!(chosen.get(), 1);
        assert!(
            host.state()
                .loop_export_notice
                .as_ref()
                .unwrap()
                .contains("cancelled")
        );
        assert!(!host.state().loop_export_busy);
        assert!(host.state().loop_export_requests.is_empty());
        assert!(shared.borrow().wav_export.job.is_none());
        assert!(shared.borrow().wav_export.pending.is_none());
    }

    #[test]
    #[ignore = "native receipt helper; requires an isolated sealed profile and owned reference directory"]
    fn sealed_profile_export_reference() {
        let directory = PathBuf::from(
            std::env::var_os("WOODSHED_EXPORT_REFERENCE_DIR").expect("owned reference directory"),
        );
        assert!(
            std::env::var_os("WOODSHED_STATE").is_some(),
            "isolated state override required"
        );
        let (storage, seal) = crate::storage::open_store_as(None);
        let mut ui = UiState::new();
        crate::session::restore(&storage, &mut ui);
        let shared = Shared::with_store(Some(storage), Some(seal), None);
        let mut shared = shared.borrow_mut();
        shared.backend = Some(crate::audio::CpalBackend::offline_for_test());
        crate::capture_persistence::begin_session(&mut shared, &ui).unwrap();
        let plan = ui.requested_loop_plan().unwrap();
        assert!(
            !ui.loop_captures.is_empty(),
            "reference profile requires a completed take"
        );
        shared
            .capture_persistence
            .permit_export(Some(&plan))
            .unwrap();
        let backend = shared.backend.as_mut().unwrap();
        backend.set_loop_plan(&plan).unwrap();
        let (mut frozen, rate) = backend.export_backing().unwrap();
        fs::create_dir_all(&directory).unwrap();
        let full = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("loop-reference.wav"))
            .unwrap();
        let report = export_song_wav_to(&frozen, rate, full).unwrap();
        for bar in &mut frozen.bars {
            bar.audio_buffer = None;
        }
        let backing = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("backing-reference.wav"))
            .unwrap();
        assert_eq!(export_song_wav_to(&frozen, rate, backing).unwrap(), report);
        fs::write(directory.join("reference.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "frames":report.frames,"rate":rate,"channels":report.channels,"click":plan.click,"segments":plan.segments.len(),"captured_takes":ui.loop_captures.takes.len(),
            "capture_manifest":ui.loop_captures
        })).unwrap()).unwrap();
        assert!(backend.capture_source_rate().is_none());
    }

    fn song(value: f32) -> Song {
        Song {
            bars: vec![Bar {
                bpm: 120.0,
                audio_buffer: Some(SampleBuffer::new(vec![value; 96_000], 48_000)),
                ..Default::default()
            }],
            click_enabled: false,
            ..Default::default()
        }
    }

    #[test]
    fn cancelled_worker_cannot_publish_new_or_replacement_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        let gate = Mutex::new(true);
        assert!(
            render(
                song(0.1),
                48_000,
                path.clone(),
                Protection::default(),
                &gate
            )
            .is_err()
        );
        assert!(!path.exists());
        fs::write(&path, b"keep").unwrap();
        let (artifact, hash) = collision(
            render(
                song(0.1),
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false),
            )
            .unwrap(),
        );
        let session = Arc::new(());
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let mut exporter = WavExport::default();
        exporter
            .spawn(session, move |cancelled| {
                started.send(()).unwrap();
                blocked.recv().unwrap();
                publish(artifact, Some(hash), &cancelled)
            })
            .unwrap();
        ready.recv().unwrap();
        let mut ui = UiState::new();
        assert!(exporter.take_completion(&ui.audio_session).is_none());
        release.send(()).unwrap();
        let job = exporter.job.take().unwrap();
        assert!(
            job.receiver
                .recv()
                .unwrap()
                .err()
                .unwrap()
                .contains("cancelled")
        );
        assert!(ui.loop_export_notice.take().is_none());
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    fn collision(outcome: Outcome) -> (Artifact, Hash) {
        match outcome {
            Outcome::Collision(artifact, hash) => (artifact, hash),
            _ => panic!("expected collision"),
        }
    }

    #[test]
    fn publication_collision_replace_and_changed_destination_preserve_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("take.wav");
        fs::write(&path, b"original").unwrap();
        let (artifact, hash) = collision(
            render(
                song(0.25),
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false),
            )
            .unwrap(),
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let rendered = fs::read(artifact.file.path()).unwrap();
        // A later backing edit does not redirect the rendered replacement.
        let changed_backing = song(0.9);
        assert_ne!(
            changed_backing.bars[0].audio_buffer.as_ref().unwrap().data[0],
            0.25
        );
        assert!(matches!(
            publish(artifact, Some(hash), &Mutex::new(false)).unwrap(),
            Outcome::Saved(_, SongExportReport { frames: 96_000, .. })
        ));
        assert_eq!(fs::read(&path).unwrap(), rendered);

        let (artifact, hash) = collision(
            render(
                song(0.5),
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false),
            )
            .unwrap(),
        );
        fs::write(&path, b"concurrent edit").unwrap();
        assert!(
            publish(artifact, Some(hash), &Mutex::new(false))
                .err()
                .unwrap()
                .contains("changed")
        );
        assert_eq!(fs::read(&path).unwrap(), b"concurrent edit");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn collision_cancellation_and_invalid_render_leave_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cancel.wav");
        fs::write(&path, b"keep").unwrap();
        let (artifact, _) = collision(
            render(
                song(0.1),
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false),
            )
            .unwrap(),
        );
        drop(artifact);
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        let mut bad = song(0.1);
        bad.bars[0].audio_buffer = Some(SampleBuffer::new(vec![f32::NAN; 96_000], 48_000));
        assert!(
            render(
                bad,
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false)
            )
            .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(
            render(
                song(0.1),
                48_000,
                dir.path().join("absent").join("take.wav"),
                Protection::default(),
                &Mutex::new(false)
            )
            .is_err()
        );
    }

    #[test]
    fn protects_existing_missing_storage_and_future_capture_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state.wav");
        let settings = dir.path().join("settings.wav");
        let capture = dir.path().join("state.loop-pcm");
        fs::write(&state, b"sealed session").unwrap();
        let protection = Protection {
            paths: vec![state.clone(), settings.clone()],
            directories: vec![capture.clone()],
        };
        for path in [
            state.clone(),
            settings,
            capture.join("namespace").join("future.wav"),
            dir.path().join("SETTINGS.WAV"),
            dir.path().join("STATE.LOOP-PCM").join("future.wav"),
            dir.path().join("sub").join("..").join("state.wav"),
        ] {
            assert!(protection.check(&path).is_err(), "{}", path.display());
        }
        assert!(protection.check(&dir.path().join("external.wav")).is_ok());
        #[cfg(unix)]
        {
            let alias = dir.path().join("alias.wav");
            std::os::unix::fs::symlink(&state, &alias).unwrap();
            assert!(protection.check(&alias).is_err());
            fs::remove_file(&alias).unwrap();
            fs::hard_link(&state, &alias).unwrap();
            assert!(protection.check(&alias).is_err());
            std::os::unix::fs::symlink(dir.path(), dir.path().join("folder")).unwrap();
            assert!(
                protection
                    .check(&dir.path().join("folder").join("state.wav"))
                    .is_err()
            );
        }
        assert_eq!(fs::read(&state).unwrap(), b"sealed session");
    }

    #[test]
    fn prior_session_completion_and_replacement_do_not_cross_personas() {
        let mut ui = UiState::new();
        let session = ui.audio_session.clone();
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(Mutex::new(false));
        let mut exporter = WavExport {
            job: Some(Job {
                session,
                receiver,
                cancelled,
            }),
            ..Default::default()
        };
        let persisted = ui.to_persisted();
        ui.apply_persisted(&persisted, ui.app_settings.clone());
        sender.send(Err("old session failure".into())).unwrap();
        if let Some(result) = exporter.take_completion(&ui.audio_session) {
            exporter.complete(&mut ui, result);
        }
        assert!(ui.loop_export_notice.is_none());
        assert!(!ui.loop_export_busy);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.wav");
        fs::write(&path, b"keep").unwrap();
        let (artifact, fingerprint) = collision(
            render(
                song(0.1),
                48_000,
                path.clone(),
                Protection::default(),
                &Mutex::new(false),
            )
            .unwrap(),
        );
        exporter.pending = Some((
            Arc::new(()),
            Collision {
                artifact,
                fingerprint,
                token: 1,
            },
        ));
        if let Some(result) = exporter.take_completion(&ui.audio_session) {
            exporter.complete(&mut ui, result);
        }
        assert!(exporter.pending.is_none());
        assert_eq!(fs::read(&path).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn snapshot_requires_current_admitted_backing_and_keeps_complete_pcm() {
        let shared = Shared::with_store(None, None, None);
        let mut shared = shared.borrow_mut();
        let mut ui = UiState::new();
        ui.song.bars.push(woodshed_core::song::SongBar::default());
        let mut backend = crate::audio::CpalBackend::offline_for_test();
        backend.try_set_song(&ui.song).unwrap();
        shared.backend = Some(backend);
        shared.last_song = Some(ui.song.clone());
        assert!(snapshot(&mut shared, &ui).unwrap_err().contains("restore"));
        shared.last_audio_session = Some(ui.audio_session.clone());
        assert!(snapshot(&mut shared, &ui).is_ok());
        ui.song.bars[0].bpm += 1.0;
        assert!(snapshot(&mut shared, &ui).unwrap_err().contains("admitted"));
        ui.song = shared.last_song.clone().unwrap();
        for _ in 0..64 {
            shared.backend.as_mut().unwrap().song_rewind();
        }
        shared.backend.as_mut().unwrap().clear_loop_session();
        assert!(snapshot(&mut shared, &ui).is_err());
    }
}
