//! Device-local desktop preferences, independent of the selected persona.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use woodshed_core::settings::WindowSettings;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct DesktopSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowSettings>,
}

fn path(root: &Path) -> PathBuf {
    root.join("desktop.json")
}

fn validate(settings: &DesktopSettings) -> Result<(), String> {
    if let Some(window) = settings.window {
        if !window.x.is_finite()
            || !window.y.is_finite()
            || !window.width.is_finite()
            || !window.height.is_finite()
            || window.width <= 0.0
            || window.height <= 0.0
        {
            return Err("Desktop window geometry must be finite with positive dimensions.".into());
        }
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<DesktopSettings, String> {
    let settings = serde_json::from_slice(bytes)
        .map_err(|error| format!("Could not read desktop preferences: {error}"))?;
    validate(&settings)?;
    Ok(settings)
}

/// Load only `root/desktop.json`. An absent file has no persisted preferences;
/// an unreadable or invalid file remains untouched and is reported to the host.
pub(crate) fn load(root: &Path) -> Result<DesktopSettings, String> {
    let path = path(root);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DesktopSettings::default());
        },
        Err(error) => return Err(format!("Could not inspect desktop preferences: {error}")),
    };
    if !metadata.file_type().is_file() {
        return Err("Desktop preferences must be a regular file.".into());
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("Could not read desktop preferences: {error}"))?;
    decode(&bytes)
}

/// Replace valid preferences through a synced sibling file. Existing corrupt,
/// unreadable, or nonregular destinations are retained for explicit recovery.
/// Both the prepared bytes and the published bytes are verified before success.
pub(crate) fn save(root: &Path, settings: &DesktopSettings) -> Result<(), String> {
    validate(settings)?;
    // Do not turn the host's default after a failed load into an implicit reset.
    load(root)?;
    let bytes = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Could not encode desktop preferences: {error}"))?;
    std::fs::create_dir_all(root)
        .map_err(|error| format!("Could not create the desktop preferences directory: {error}"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".desktop-")
        .suffix(".tmp")
        .tempfile_in(root)
        .map_err(|error| format!("Could not prepare desktop preferences: {error}"))?;
    temporary
        .write_all(&bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not write desktop preferences: {error}"))?;
    let prepared = std::fs::read(temporary.path())
        .map_err(|error| format!("Could not verify prepared desktop preferences: {error}"))?;
    if prepared != bytes || decode(&prepared)? != *settings {
        return Err("Prepared desktop preferences did not match the requested settings.".into());
    }
    temporary
        .persist(path(root))
        .map_err(|error| format!("Could not publish desktop preferences: {error}"))?;
    // Atomic publication has committed. Directory syncing is best effort, as in
    // the practice backend; a failed sync cannot roll back that publication.
    #[cfg(unix)]
    if let Ok(directory) = std::fs::File::open(root) {
        let _ = directory.sync_all();
    }
    let published = std::fs::read(path(root))
        .map_err(|error| format!("Could not verify saved desktop preferences: {error}"))?;
    if published != bytes || load(root)? != *settings {
        return Err("Saved desktop preferences did not match the requested settings.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> DesktopSettings {
        DesktopSettings {
            window: Some(WindowSettings {
                x: -120.0,
                y: 80.0,
                width: 900.0,
                height: 640.0,
                maximized: true,
            }),
        }
    }

    #[test]
    fn missing_preferences_do_not_create_a_directory() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("absent");
        assert_eq!(load(&root).unwrap(), DesktopSettings::default());
        assert!(!root.exists());
    }

    #[test]
    fn preferences_round_trip_only_within_the_explicit_root() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        std::fs::create_dir(&first).unwrap();
        let practice = first.join("settings.json");
        std::fs::write(&practice, b"persona settings remain separate").unwrap();
        save(&first, &settings()).unwrap();
        assert_eq!(load(&first).unwrap(), settings());
        assert_eq!(load(&second).unwrap(), DesktopSettings::default());
        assert_eq!(
            std::fs::read(practice).unwrap(),
            b"persona settings remain separate"
        );
        assert_eq!(std::fs::read_dir(&first).unwrap().count(), 2);
        let reset = DesktopSettings::default();
        save(&first, &reset).unwrap();
        assert_eq!(load(&first).unwrap(), reset);
    }

    #[test]
    fn invalid_existing_preferences_are_retained_and_refuse_writes() {
        for bytes in [
            &b"broken json"[..],
            &b"{\"window\":{\"x\":0,\"y\":0,\"width\":0,\"height\":640,\"maximized\":false}}"[..],
            &b"{\"unrecognized\":true}"[..],
        ] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::write(path(directory.path()), bytes).unwrap();
            assert!(load(directory.path()).is_err());
            assert!(save(directory.path(), &settings()).is_err());
            assert_eq!(std::fs::read(path(directory.path())).unwrap(), bytes);
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn invalid_new_geometry_retains_the_previous_preferences() {
        let directory = tempfile::tempdir().unwrap();
        save(directory.path(), &settings()).unwrap();
        let original = std::fs::read(path(directory.path())).unwrap();
        for value in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let mut invalid = settings();
            invalid.window.as_mut().unwrap().width = value;
            assert!(save(directory.path(), &invalid).is_err());
            assert_eq!(std::fs::read(path(directory.path())).unwrap(), original);
        }
    }

    #[test]
    fn nonregular_destination_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let destination = path(directory.path());
        std::fs::create_dir(&destination).unwrap();
        let child = destination.join("retained");
        std::fs::write(&child, b"original").unwrap();
        assert!(load(directory.path()).is_err());
        assert!(save(directory.path(), &settings()).is_err());
        assert_eq!(std::fs::read(child).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    #[cfg(unix)]
    fn symbolic_link_is_neither_read_nor_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let other = directory.path().join("other.json");
        std::fs::write(&other, serde_json::to_vec(&settings()).unwrap()).unwrap();
        let original = std::fs::read(&other).unwrap();
        std::os::unix::fs::symlink(&other, path(directory.path())).unwrap();
        assert!(load(directory.path()).is_err());
        assert!(save(directory.path(), &settings()).is_err());
        assert!(
            std::fs::symlink_metadata(path(directory.path()))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(other).unwrap(), original);
    }
}
