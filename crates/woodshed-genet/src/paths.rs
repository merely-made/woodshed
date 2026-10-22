//! Resolve host paths once, following symlinks before interpreting `..`.
use std::path::{Component, Path, PathBuf};

pub(crate) fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Choose a nonempty filesystem path.".into());
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {},
            Component::ParentDir => {
                resolved.pop();
            },
            other => {
                resolved.push(other.as_os_str());
                match std::fs::symlink_metadata(&resolved) {
                    Ok(_) => {
                        resolved = resolved.canonicalize().map_err(|error| error.to_string())?
                    },
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                    Err(error) => return Err(error.to_string()),
                }
            },
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_children_are_absolute_without_creating_directories() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("future/../private/session");
        assert_eq!(
            absolute(&path).unwrap(),
            directory
                .path()
                .canonicalize()
                .unwrap()
                .join("private/session")
        );
        assert!(!directory.path().join("future").exists());
        assert!(absolute(Path::new("")).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn symlink_is_resolved_before_parent_for_live_and_future_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let other = directory.path().join("other");
        std::fs::create_dir_all(other.join("sub")).unwrap();
        std::os::unix::fs::symlink(other.join("sub"), directory.path().join("link")).unwrap();
        let path = directory.path().join("link/../private/session");
        assert_eq!(
            absolute(&path).unwrap(),
            other.canonicalize().unwrap().join("private/session")
        );
        std::fs::create_dir_all(other.join("private")).unwrap();
        assert_eq!(
            absolute(&path).unwrap(),
            other.canonicalize().unwrap().join("private/session")
        );
    }
}
