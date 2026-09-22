#![forbid(unsafe_code)]

use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use fetch::{Fetch, FetchError};
use redshank_model::{MediaSource, RepresentationReceipt};

static PENDING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum CacheError {
    Io(io::Error),
    Fetch(FetchError),
    Invalid(String),
    Budget { needed: u64, available: u64 },
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache storage failed: {error}"),
            Self::Fetch(error) => write!(formatter, "episode download failed: {error}"),
            Self::Invalid(message) => formatter.write_str(message),
            Self::Budget { needed, available } => write!(
                formatter,
                "offline cache is full: episode needs {needed} bytes but {available} bytes remain"
            ),
        }
    }
}

impl Error for CacheError {}

impl From<io::Error> for CacheError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FetchError> for CacheError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

#[derive(Clone)]
pub struct EpisodeCache {
    root: PathBuf,
    budget_bytes: u64,
    fetch: Arc<dyn Fetch>,
}

impl fmt::Debug for EpisodeCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpisodeCache")
            .field("root", &self.root)
            .field("budget_bytes", &self.budget_bytes)
            .finish_non_exhaustive()
    }
}

impl EpisodeCache {
    pub fn new(root: impl Into<PathBuf>, budget_bytes: u64, fetch: Arc<dyn Fetch>) -> Self {
        Self {
            root: root.into(),
            budget_bytes,
            fetch,
        }
    }

    pub fn used_bytes(&self) -> Result<u64, CacheError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error.into()),
        };
        let mut used = 0_u64;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("audio") {
                used = used.saturating_add(entry.metadata()?.len());
            }
        }
        Ok(used)
    }

    /// Remove one published cache object. Stored paths are admitted only when
    /// they name a direct `.audio` child of this cache root.
    pub fn remove_cached_path(&self, path: &Path) -> Result<bool, CacheError> {
        if path.parent() != Some(self.root.as_path())
            || path.extension().and_then(|value| value.to_str()) != Some("audio")
        {
            return Err(CacheError::Invalid(
                "refusing to remove a file outside the Redshank cache".into(),
            ));
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn cache_url(&self, url: &str) -> Result<MediaSource, CacheError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(CacheError::Invalid(
                "offline downloads require an HTTP or HTTPS enclosure URL".into(),
            ));
        }
        let used = self.used_bytes()?;
        let available = self.budget_bytes.saturating_sub(used);
        let budget = self.budget_bytes;
        self.publish(url, available, |sink| {
            // The handle refuses on the declared length before any byte lands.
            let facts =
                self.fetch
                    .read_into(url, sink, Some(budget))
                    .map_err(|error| match error {
                        FetchError::TooLarge { limit } => CacheError::Invalid(format!(
                            "episode is larger than the {limit}-byte offline cache budget"
                        )),
                        other => CacheError::Fetch(other),
                    })?;
            Ok(Meta {
                final_url: facts.final_url,
                media_type: facts.content_type,
                etag: facts.etag,
                last_modified: facts.last_modified,
                declared_length: facts.content_length,
            })
        })
    }

    /// Stream one representation into a pending file and publish it by digest.
    /// `body` writes the bytes and returns what the origin said about them.
    fn publish(
        &self,
        requested_url: &str,
        available: u64,
        body: impl FnOnce(&mut dyn Write) -> Result<Meta, CacheError>,
    ) -> Result<MediaSource, CacheError> {
        fs::create_dir_all(&self.root)?;
        let pending_path = self.pending_path();
        let pending = PendingFile::create(pending_path)?;
        let mut sink = Publishing {
            pending,
            digest: blake3::Hasher::new(),
            length: 0,
            budget: self.budget_bytes,
            over_budget: false,
        };
        let meta = match body(&mut sink) {
            Ok(meta) => meta,
            Err(_) if sink.over_budget => {
                return Err(CacheError::Budget {
                    needed: sink.length,
                    available: self.budget_bytes,
                });
            },
            Err(error) => return Err(error),
        };
        let Meta {
            final_url,
            media_type,
            etag,
            last_modified,
            declared_length,
        } = meta;
        let Publishing {
            mut pending,
            digest,
            length,
            ..
        } = sink;
        if let Some(expected) = declared_length
            && expected != length
        {
            return Err(CacheError::Invalid(format!(
                "episode download ended at {length} bytes; the server declared {expected}"
            )));
        }
        pending.file.sync_all()?;
        let digest = digest.finalize();
        let digest_text = digest.to_hex().to_string();
        let final_path = self.root.join(format!("{digest_text}.audio"));
        if !final_path.exists() && length > available {
            return Err(CacheError::Budget {
                needed: length,
                available,
            });
        }
        pending.publish(&final_path)?;
        let path = final_path
            .to_str()
            .ok_or_else(|| CacheError::Invalid("cache path cannot be stored as Unicode".into()))?
            .to_owned();
        Ok(MediaSource::Cached {
            path,
            origin_url: requested_url.to_owned(),
            representation: Box::new(RepresentationReceipt {
                requested_url: Some(requested_url.to_owned()),
                final_url: Some(final_url),
                media_type,
                byte_length: Some(length),
                etag,
                last_modified,
                retrieved_at_ms: Some(now_ms()),
                complete_digest: Some(format!("blake3:{digest_text}")),
            }),
        })
    }

    fn pending_path(&self) -> PathBuf {
        let sequence = PENDING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        self.root.join(format!(
            ".download-{}-{sequence}.pending",
            std::process::id()
        ))
    }
}

struct PendingFile {
    path: PathBuf,
    file: File,
    published: bool,
}

impl PendingFile {
    fn create(path: PathBuf) -> Result<Self, CacheError> {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        Ok(Self {
            path,
            file,
            published: false,
        })
    }

    fn publish(&mut self, final_path: &Path) -> Result<(), CacheError> {
        if final_path.exists() {
            fs::remove_file(&self.path)?;
        } else {
            fs::rename(&self.path, final_path)?;
        }
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// What the origin said about a representation, learned while its body streamed.
struct Meta {
    final_url: String,
    media_type: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
    declared_length: Option<u64>,
}

/// The pending file as a sink: hashes and counts every byte, and refuses the
/// first byte past the budget so a runaway body never fills the disk.
struct Publishing {
    pending: PendingFile,
    digest: blake3::Hasher,
    length: u64,
    budget: u64,
    over_budget: bool,
}

impl Write for Publishing {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self.length.saturating_add(bytes.len() as u64);
        if length > self.budget {
            self.over_budget = true;
            self.length = length;
            return Err(io::Error::other("offline cache budget exceeded"));
        }
        self.digest.update(bytes);
        self.pending.file.write_all(bytes)?;
        self.length = length;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.pending.file.flush()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::{io::Read as _, net::TcpListener, thread};

    use fetch::{NetFetch, Stores};

    use super::*;

    fn test_fetch() -> Arc<dyn Fetch> {
        Arc::new(NetFetch::new(&Stores::in_memory()).unwrap())
    }

    fn meta(
        final_url: &str,
        media_type: Option<&str>,
        etag: Option<&str>,
        declared: Option<u64>,
    ) -> Meta {
        Meta {
            final_url: final_url.into(),
            media_type: media_type.map(str::to_owned),
            etag: etag.map(str::to_owned),
            last_modified: None,
            declared_length: declared,
        }
    }

    #[test]
    fn complete_object_is_content_addressed_and_reusable() {
        let directory = tempfile::tempdir().unwrap();
        let cache = EpisodeCache::new(directory.path(), 6, test_fetch());
        let first = cache
            .publish("https://example.test/episode.mp3", 6, |sink| {
                sink.write_all(b"abcdef")?;
                Ok(meta(
                    "https://cdn.example.test/episode.mp3",
                    Some("audio/mpeg"),
                    Some("\"fixed\""),
                    Some(6),
                ))
            })
            .unwrap();
        let second = cache
            .publish("https://example.test/episode.mp3", 0, |sink| {
                sink.write_all(b"abcdef")?;
                Ok(meta(
                    "https://cdn.example.test/episode.mp3",
                    Some("audio/mpeg"),
                    Some("\"fixed\""),
                    Some(6),
                ))
            })
            .unwrap();
        let (first_path, first_digest) = match first {
            MediaSource::Cached {
                path,
                representation,
                ..
            } => (path, representation.complete_digest),
            _ => unreachable!(),
        };
        let (second_path, second_digest) = match second {
            MediaSource::Cached {
                path,
                representation,
                ..
            } => (path, representation.complete_digest),
            _ => unreachable!(),
        };
        assert_eq!(first_path, second_path);
        assert_eq!(first_digest, second_digest);
        assert_eq!(cache.used_bytes().unwrap(), 6);
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            1,
            "deduplication must not retain a pending file"
        );
    }

    #[test]
    fn exhausted_and_truncated_downloads_publish_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let cache = EpisodeCache::new(directory.path(), 4, test_fetch());
        let exhausted = cache
            .publish("https://example.test/a.mp3", 4, |sink| {
                sink.write_all(b"12345")?;
                Ok(meta("https://example.test/a.mp3", None, None, None))
            })
            .unwrap_err();
        assert!(matches!(exhausted, CacheError::Budget { .. }));
        let truncated = cache
            .publish("https://example.test/a.mp3", 10, |sink| {
                sink.write_all(b"1234")?;
                Ok(meta("https://example.test/a.mp3", None, None, Some(5)))
            })
            .unwrap_err();
        assert!(matches!(truncated, CacheError::Invalid(_)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn removal_is_idempotent_and_confined_to_the_cache_root() {
        let directory = tempfile::tempdir().unwrap();
        let cache = EpisodeCache::new(directory.path(), 10, test_fetch());
        let path = directory.path().join("object.audio");
        fs::write(&path, b"abc").unwrap();
        assert!(cache.remove_cached_path(&path).unwrap());
        assert!(!cache.remove_cached_path(&path).unwrap());

        let outside = directory.path().join("..").join("outside.audio");
        assert!(matches!(
            cache.remove_cached_path(&outside),
            Err(CacheError::Invalid(_))
        ));
        let wrong_extension = directory.path().join("object.json");
        assert!(matches!(
            cache.remove_cached_path(&wrong_extension),
            Err(CacheError::Invalid(_))
        ));
    }

    #[test]
    fn http_download_retains_origin_headers_and_offline_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nContent-Type: audio/mpeg\r\nETag: \"episode-1\"\r\nConnection: close\r\n\r\nabcdef",
                )
                .unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let cache = EpisodeCache::new(directory.path(), 1024, test_fetch());
        let url = format!("http://{address}/episode.mp3");
        let source = cache.cache_url(&url).unwrap();
        server.join().unwrap();
        let MediaSource::Cached {
            path,
            origin_url,
            representation,
        } = source
        else {
            unreachable!();
        };
        assert_eq!(origin_url, url);
        assert_eq!(representation.requested_url.as_deref(), Some(url.as_str()));
        assert_eq!(representation.media_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(representation.etag.as_deref(), Some("\"episode-1\""));
        assert_eq!(representation.byte_length, Some(6));
        assert!(representation.complete_digest.is_some());
        assert_eq!(fs::read(path).unwrap(), b"abcdef");
    }
}
