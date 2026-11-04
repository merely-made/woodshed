use super::*;

impl FsBackend {
    pub fn at(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
        }
    }

    pub(super) fn path(&self, key: &str) -> Result<Option<PathBuf>, StoreError> {
        match key {
            "owner" | "session" | "settings" => Ok(Some(self.directory.join(key))),
            _ => {
                let (namespace, digest) = key
                    .strip_prefix("loop-pcm/")
                    .and_then(|key| key.split_once('/'))
                    .ok_or_else(|| StoreError::Backend("Unknown Woodshed storage key.".into()))?;
                pcm_key(namespace, digest).map_err(StoreError::Backend)?;
                Ok(self
                    .capture_dir()
                    .map(|directory| directory.join(namespace).join(format!("{digest}.bin"))))
            },
        }
    }

    pub(super) fn capture_dir(&self) -> Option<PathBuf> {
        Some(self.directory.join("loop-pcm"))
    }

    pub fn protected_export_paths(&self) -> Vec<PathBuf> {
        ["owner", "session", "settings"]
            .map(|name| self.directory.join(name))
            .into()
    }

    /// Protect the entire store, including unknown future slots and temporaries.
    pub fn protected_export_directories(&self) -> Vec<PathBuf> {
        vec![self.directory.clone()]
    }
}

#[async_trait]
impl Backend for FsBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(path) = self.path(key)? else {
            return Ok(None);
        };
        if key.starts_with("loop-pcm/") || key == "owner" {
            let maximum = if key == "owner" {
                MAX_OWNER_FILE_BYTES
            } else {
                MAX_CAPTURE_FILE_BYTES
            };
            let file = match std::fs::File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(StoreError::Backend(error.to_string())),
            };
            let length = file
                .metadata()
                .map_err(|error| StoreError::Backend(error.to_string()))?
                .len();
            if length > maximum as u64 {
                return Err(StoreError::Backend(format!(
                    "{} file exceeds its bounded byte budget.",
                    if key == "owner" {
                        "Owner record"
                    } else {
                        "Loop PCM"
                    }
                )));
            }
            let mut bytes = Vec::with_capacity(length as usize);
            file.take(maximum as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            if bytes.len() > maximum {
                return Err(StoreError::Backend(format!(
                    "{} file grew beyond its bounded byte budget.",
                    if key == "owner" {
                        "Owner record"
                    } else {
                        "Loop PCM"
                    }
                )));
            }
            return Ok(Some(bytes));
        }
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(StoreError::Backend(error.to_string())),
        }
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let Some(path) = self.path(key)? else {
            return Ok(());
        };
        if key.starts_with("loop-pcm/") && bytes.len() > MAX_CAPTURE_FILE_BYTES {
            return Err(StoreError::Backend(
                "Loop PCM write exceeds its bounded byte budget.".into(),
            ));
        }
        if key == "owner" && bytes.len() > MAX_OWNER_FILE_BYTES {
            return Err(StoreError::Backend(
                "Owner record exceeds its bounded byte budget.".into(),
            ));
        }
        atomic_write(&path, bytes, key.starts_with("loop-pcm/") || key == "owner")
    }

    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        if key == "owner" {
            return Err(StoreError::Backend(
                "The persona-store owner record is immutable.".into(),
            ));
        }
        let Some(path) = self.path(key)? else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StoreError::Backend(error.to_string())),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        let mut keys = Vec::new();
        for key in ["owner", "session", "settings"] {
            if key.starts_with(prefix) && self.path(key)?.is_some_and(|path| path.exists()) {
                keys.push(key.to_string());
            }
        }
        if let Some(directory) = self.capture_dir() {
            let entries = match std::fs::read_dir(directory) {
                Ok(entries) => Some(entries),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(StoreError::Backend(error.to_string())),
            };
            if let Some(entries) = entries {
                for namespace in entries {
                    let namespace =
                        namespace.map_err(|error| StoreError::Backend(error.to_string()))?;
                    if !namespace
                        .file_type()
                        .map_err(|error| StoreError::Backend(error.to_string()))?
                        .is_dir()
                    {
                        continue;
                    }
                    let name = namespace.file_name();
                    let Some(name) = name.to_str() else {
                        continue;
                    };
                    if validate_namespace(name).is_err() {
                        continue;
                    }
                    for entry in std::fs::read_dir(namespace.path())
                        .map_err(|error| StoreError::Backend(error.to_string()))?
                    {
                        let entry =
                            entry.map_err(|error| StoreError::Backend(error.to_string()))?;
                        if !entry
                            .file_type()
                            .map_err(|error| StoreError::Backend(error.to_string()))?
                            .is_file()
                        {
                            continue;
                        }
                        let file = entry.file_name();
                        let Some(digest) = file.to_str().and_then(|file| file.strip_suffix(".bin"))
                        else {
                            continue;
                        };
                        if let Ok(key) = pcm_key(name, digest) {
                            if key.starts_with(prefix) {
                                keys.push(key);
                            }
                        }
                    }
                }
            }
        }
        Ok(keys)
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        let mut keys = self.list("").await?;
        keys.retain(|key| key.as_str() >= start && key.as_str() < end);
        keys.sort();
        Ok(keys)
    }

    /// Files written in order, each individually replaced atomically.
    ///
    /// This is not a multi-file transaction. Capture persistence writes and
    /// verifies blobs before committing its session reference separately.
    async fn apply(&self, ops: &[WriteOp]) -> Result<(), StoreError> {
        for op in ops {
            match op {
                WriteOp::Put { key, value } => self.put(key, value).await?,
                WriteOp::Delete { key } => self.delete(key).await?,
            }
        }
        Ok(())
    }
}

/// Commit one file through a synced sibling: rename for mutable slots, a
/// no-clobber hard link for immutable blobs. Failed publication retains old data.
/// This does not make a set of companion/session files one atomic transaction.
pub(super) fn atomic_write(path: &Path, bytes: &[u8], immutable: bool) -> Result<(), StoreError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| StoreError::Backend(error.to_string()))?;
    let name = path
        .file_name()
        .ok_or_else(|| StoreError::Backend("Storage destination has no filename.".into()))?;
    for _ in 0..32 {
        let mut temporary_name = std::ffi::OsString::from(".");
        temporary_name.push(name);
        temporary_name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(temporary_name);
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(StoreError::Backend(error.to_string())),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            if immutable {
                std::fs::hard_link(&temporary, path)
            } else {
                std::fs::rename(&temporary, path)
            }
        })();
        let _ = std::fs::remove_file(&temporary);
        if result.is_ok() {
            // A publication already committed cannot be rolled back by a
            // directory-sync error. Match muniment's zip backend's best effort.
            #[cfg(unix)]
            if let Ok(directory) = std::fs::File::open(parent) {
                let _ = directory.sync_all();
            }
        }
        return result.map_err(|error| StoreError::Backend(error.to_string()));
    }
    Err(StoreError::Backend(
        "Could not reserve a sibling storage temporary file.".into(),
    ))
}
