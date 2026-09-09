#![forbid(unsafe_code)]

use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use redshank_model::{MediaSource, RepresentationReceipt};
use ureq::ResponseExt;

static PENDING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum CacheError {
    Io(io::Error),
    Http(ureq::Error),
    Invalid(String),
    Budget { needed: u64, available: u64 },
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache storage failed: {error}"),
            Self::Http(error) => write!(formatter, "episode download failed: {error}"),
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

impl From<ureq::Error> for CacheError {
    fn from(error: ureq::Error) -> Self {
        Self::Http(error)
    }
}

#[derive(Clone, Debug)]
pub struct EpisodeCache {
    root: PathBuf,
    budget_bytes: u64,
}

impl EpisodeCache {
    pub fn new(root: impl Into<PathBuf>, budget_bytes: u64) -> Self {
        Self {
            root: root.into(),
            budget_bytes,
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

    pub fn cache_url(&self, url: &str) -> Result<MediaSource, CacheError> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(CacheError::Invalid(
                "offline downloads require an HTTP or HTTPS enclosure URL".into(),
            ));
        }
        let used = self.used_bytes()?;
        let available = self.budget_bytes.saturating_sub(used);
        let mut response = ureq::get(url)
            .header("Accept-Encoding", "identity")
            .call()?;
        let final_url = response.get_uri().to_string();
        let declared_length = optional_header(&response, "Content-Length")
            .and_then(|value| value.parse::<u64>().ok());
        if let Some(needed) = declared_length
            && needed > self.budget_bytes
        {
            return Err(CacheError::Budget {
                needed,
                available: self.budget_bytes,
            });
        }
        let media_type = optional_header(&response, "Content-Type")
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_owned());
        let etag = optional_header(&response, "ETag").map(str::to_owned);
        let last_modified = optional_header(&response, "Last-Modified").map(str::to_owned);
        let reader = response.body_mut().as_reader();
        self.publish(
            url,
            final_url,
            media_type,
            etag,
            last_modified,
            declared_length,
            available,
            reader,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        &self,
        requested_url: &str,
        final_url: String,
        media_type: Option<String>,
        etag: Option<String>,
        last_modified: Option<String>,
        declared_length: Option<u64>,
        available: u64,
        mut reader: impl Read,
    ) -> Result<MediaSource, CacheError> {
        fs::create_dir_all(&self.root)?;
        let pending_path = self.pending_path();
        let mut pending = PendingFile::create(pending_path)?;
        let mut digest = blake3::Hasher::new();
        let mut length = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            length = length.saturating_add(count as u64);
            if length > self.budget_bytes {
                return Err(CacheError::Budget {
                    needed: length,
                    available: self.budget_bytes,
                });
            }
            digest.update(&buffer[..count]);
            pending.file.write_all(&buffer[..count])?;
        }
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

fn optional_header<'a>(response: &'a http::Response<ureq::Body>, name: &str) -> Option<&'a str> {
    response.headers().get(name)?.to_str().ok()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, Read as _},
        net::TcpListener,
        thread,
    };

    use super::*;

    #[test]
    fn complete_object_is_content_addressed_and_reusable() {
        let directory = tempfile::tempdir().unwrap();
        let cache = EpisodeCache::new(directory.path(), 6);
        let first = cache
            .publish(
                "https://example.test/episode.mp3",
                "https://cdn.example.test/episode.mp3".into(),
                Some("audio/mpeg".into()),
                Some("\"fixed\"".into()),
                None,
                Some(6),
                6,
                Cursor::new(b"abcdef"),
            )
            .unwrap();
        let second = cache
            .publish(
                "https://example.test/episode.mp3",
                "https://cdn.example.test/episode.mp3".into(),
                Some("audio/mpeg".into()),
                Some("\"fixed\"".into()),
                None,
                Some(6),
                0,
                Cursor::new(b"abcdef"),
            )
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
        let cache = EpisodeCache::new(directory.path(), 4);
        let exhausted = cache
            .publish(
                "https://example.test/a.mp3",
                "https://example.test/a.mp3".into(),
                None,
                None,
                None,
                None,
                4,
                Cursor::new(b"12345"),
            )
            .unwrap_err();
        assert!(matches!(exhausted, CacheError::Budget { .. }));
        let truncated = cache
            .publish(
                "https://example.test/a.mp3",
                "https://example.test/a.mp3".into(),
                None,
                None,
                None,
                Some(5),
                10,
                Cursor::new(b"1234"),
            )
            .unwrap_err();
        assert!(matches!(truncated, CacheError::Invalid(_)));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
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
        let cache = EpisodeCache::new(directory.path(), 1024);
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
