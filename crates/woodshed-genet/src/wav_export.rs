//! Explicit Looper snapshot, native save choice and atomic WAV publication.
//! Rendering and filesystem work run on one bounded worker. Collision consent
//! retains that rendered snapshot; edits cannot redirect a later Replace.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};

use cambium_genet_winit_host::CloseDisposition;
use muniment::blob::Hash;
use tempfile::NamedTempFile;
use woodshed_audio::Song;
#[cfg(test)]
use woodshed_audio::offline::export_song_wav_to;
use woodshed_audio::offline::{SongExportReport, export_song_wav_with_pattern_to};
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

pub(crate) fn normalized(path: &Path) -> Result<PathBuf, String> {
    crate::paths::absolute(path)
}

impl Protection {
    fn current(shared: &Shared) -> Self {
        Self {
            paths: shared.theme_library.iter().cloned().collect(),
            directories: shared.data_root.iter().cloned().collect(),
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
pub(crate) fn reserved_match(path: &Path, reserved: &Path, subtree: bool) -> bool {
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
    pack: Option<audio_packs::PackSelection>,
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
    let pattern = pack
        .map(|p| p.hydrated_pattern())
        .transpose()
        .map_err(|e| e.to_string())?;
    let report = export_song_wav_with_pattern_to(&song, rate, pattern.as_ref(), file.as_file_mut())
        .map_err(|e| e.to_string())?;
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
    let pack = ctx.runner.state().audio_pack.clone();
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
    let protection = Protection::current(shared);
    let Some(choose) = choose else {
        start_dialog(
            pack,
            &mut shared.wav_export,
            session,
            song,
            rate,
            name,
            protection,
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
    let protection = Protection::current(shared);
    let path = protection.check(&path)?;
    shared.wav_export.spawn(session, move |cancelled| {
        render(pack, song, rate, path, protection, &cancelled)
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
    pack: Option<audio_packs::PackSelection>,
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
        render(pack, song, rate, wav_path(path)?, protection, &cancelled)
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
#[path = "wav_export/tests.rs"]
mod tests;

#[path = "wav_export/request.rs"]
mod request;
