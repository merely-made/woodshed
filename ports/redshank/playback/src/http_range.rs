use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Seek, SeekFrom},
    sync::Arc,
};

use anyhow::{Result, bail};
use fetch::{Fetch, FetchError, Range};
use redshank_model::RepresentationReceipt;
use symphonia::core::io::MediaSource;

pub(super) const DEFAULT_CACHE_BYTES: usize = 256 * 1024;
const CHUNK_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct HttpStats {
    pub(super) requests: u64,
    pub(super) fetched_bytes: u64,
    pub(super) peak_cache_bytes: usize,
    pub(super) ranges: Vec<(u64, u64)>,
}

/// Progressive playback over HTTP byte ranges, read through the host's fetch
/// handle: the host's cookies, cache policy and wire, one connection reused
/// across chunks, and a sliding window of decoded-from bytes under a budget.
pub(super) struct HttpRangeSource {
    fetch: Arc<dyn Fetch>,
    url: String,
    len: u64,
    position: u64,
    cache: BTreeMap<u64, Vec<u8>>,
    cache_order: VecDeque<u64>,
    cache_bytes: usize,
    cache_budget: usize,
    validator: Option<String>,
    stats: HttpStats,
}

impl HttpRangeSource {
    pub(super) fn open(
        fetch: Arc<dyn Fetch>,
        url: &str,
        cache_budget: usize,
    ) -> Result<(Self, RepresentationReceipt)> {
        if cache_budget < CHUNK_BYTES as usize {
            bail!(
                "HTTP cache budget is {cache_budget} bytes; range playback requires at least {CHUNK_BYTES} bytes"
            );
        }
        let probe = match fetch.read_range(
            url,
            Range {
                start: 0,
                end: Some(0),
            },
            None,
        ) {
            Ok(reply) => reply,
            Err(FetchError::RangeIgnored) => {
                bail!("HTTP audio source does not support byte ranges: expected 206, received 200")
            },
            Err(FetchError::Status(status)) => bail!(
                "HTTP audio source does not support byte ranges: expected 206, received {status}"
            ),
            Err(error) => bail!("could not reach HTTP audio source {url}: {error}"),
        };
        if probe.start != 0 || probe.bytes.len() != 1 {
            bail!(
                "initial HTTP range returned {} bytes at offset {}, expected one byte at 0",
                probe.bytes.len(),
                probe.start
            );
        }
        let len = probe.total;
        let facts = probe.facts;
        let validator = facts.etag.clone().or_else(|| facts.last_modified.clone());
        let receipt = RepresentationReceipt {
            requested_url: Some(url.to_owned()),
            final_url: Some(facts.final_url.clone()),
            media_type: facts.content_type,
            byte_length: Some(len),
            etag: facts.etag,
            last_modified: facts.last_modified,
            retrieved_at_ms: Some(now_ms()),
            complete_digest: None,
        };
        Ok((
            Self {
                fetch,
                url: facts.final_url,
                len,
                position: 0,
                cache: BTreeMap::new(),
                cache_order: VecDeque::new(),
                cache_bytes: 0,
                cache_budget,
                validator,
                stats: HttpStats {
                    requests: 1,
                    fetched_bytes: 1,
                    peak_cache_bytes: 0,
                    ranges: vec![(0, 0)],
                },
            },
            receipt,
        ))
    }

    #[cfg(test)]
    pub(super) fn stats(&self) -> &HttpStats {
        &self.stats
    }

    fn fetch_chunk(&mut self, start: u64) -> io::Result<()> {
        if self.cache.contains_key(&start) {
            return Ok(());
        }
        let end = (start + CHUNK_BYTES - 1).min(self.len.saturating_sub(1));
        let expected = (end - start + 1) as usize;
        if expected > self.cache_budget {
            return Err(io::Error::other(format!(
                "HTTP range needs {expected} cache bytes but the budget is {}",
                self.cache_budget
            )));
        }
        let reply = self
            .fetch
            .read_range(
                &self.url,
                Range {
                    start,
                    end: Some(end),
                },
                self.validator.as_deref(),
            )
            .map_err(|error| match error {
                FetchError::Changed | FetchError::RangeIgnored => io::Error::other(format!(
                    "HTTP range bytes={start}-{end} was not honored; the representation may have changed ({error})"
                )),
                FetchError::BadRange(why) => {
                    io::Error::other(format!("HTTP range bytes={start}-{end}: {why}"))
                },
                other => io::Error::other(other.to_string()),
            })?;
        if reply.start != start || reply.total != self.len {
            return Err(io::Error::other(format!(
                "HTTP range bytes={start}-{end} returned bytes {}-{}/{}",
                reply.start,
                reply.start + reply.bytes.len() as u64 - 1,
                reply.total
            )));
        }
        let bytes = reply.bytes;
        if bytes.len() != expected {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "HTTP range bytes={start}-{end} returned {} bytes",
                    bytes.len()
                ),
            ));
        }

        while self.cache_bytes.saturating_add(bytes.len()) > self.cache_budget {
            let Some(oldest) = self.cache_order.pop_front() else {
                return Err(io::Error::other(format!(
                    "HTTP range needs {} cache bytes but only {} are available",
                    bytes.len(),
                    self.cache_budget
                )));
            };
            if let Some(removed) = self.cache.remove(&oldest) {
                self.cache_bytes -= removed.len();
            }
        }
        self.cache_bytes += bytes.len();
        self.cache.insert(start, bytes);
        self.cache_order.push_back(start);
        self.stats.requests += 1;
        self.stats.fetched_bytes += expected as u64;
        self.stats.peak_cache_bytes = self.stats.peak_cache_bytes.max(self.cache_bytes);
        self.stats.ranges.push((start, end));
        Ok(())
    }
}

impl Read for HttpRangeSource {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.position >= self.len {
            return Ok(0);
        }
        let mut written = 0;
        while written < output.len() && self.position < self.len {
            let start = (self.position / CHUNK_BYTES) * CHUNK_BYTES;
            self.fetch_chunk(start)?;
            let chunk = self
                .cache
                .get(&start)
                .ok_or_else(|| io::Error::other("fetched HTTP range missing from cache"))?;
            let offset = (self.position - start) as usize;
            let available = chunk.len().saturating_sub(offset);
            if available == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "HTTP range ended before its declared boundary",
                ));
            }
            let count = available.min(output.len() - written);
            output[written..written + count].copy_from_slice(&chunk[offset..offset + count]);
            written += count;
            self.position += count as u64;
        }
        Ok(written)
    }
}

impl Seek for HttpRangeSource {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::End(offset) => self.len as i128 + offset as i128,
            SeekFrom::Current(offset) => self.position as i128 + offset as i128,
        };
        if next < 0 || next > self.len as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside HTTP audio object",
            ));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

impl MediaSource for HttpRangeSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use fetch::{NetFetch, Stores};
    use std::{
        io::Write,
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    fn test_fetch() -> Arc<dyn Fetch> {
        Arc::new(NetFetch::new(&Stores::in_memory()).unwrap())
    }

    #[derive(Clone, Copy)]
    enum ServerMode {
        Ranges,
        /// Ranges on a connection the server leaves open between requests.
        KeepAlive,
        IgnoreRanges,
        ChangeAfterProbe,
    }

    struct TestServer {
        address: SocketAddr,
        connections: Arc<AtomicUsize>,
        stop: Arc<AtomicBool>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start(data: Vec<u8>, mode: ServerMode) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = Arc::clone(&stop);
            let connections = Arc::new(AtomicUsize::new(0));
            let data = Arc::new(data);
            let accepted = Arc::clone(&connections);
            let thread = thread::spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if thread_stop.load(Ordering::Relaxed) {
                                break;
                            }
                            accepted.fetch_add(1, Ordering::Relaxed);
                            stream.set_nonblocking(false).unwrap();
                            if matches!(mode, ServerMode::KeepAlive) {
                                // Its own thread, so a client that opens a second
                                // connection is counted rather than left waiting.
                                let data = Arc::clone(&data);
                                thread::spawn(move || serve(stream, &data, mode));
                            } else {
                                serve(stream, &data, mode);
                            }
                        },
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        },
                        Err(error) => panic!("test range server failed: {error}"),
                    }
                }
            });
            Self {
                address,
                connections,
                stop,
                thread: Some(thread),
            }
        }

        fn url(&self) -> String {
            format!("http://{}/episode.mp3", self.address)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.address);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn serve(mut stream: TcpStream, data: &[u8], mode: ServerMode) {
        if matches!(mode, ServerMode::KeepAlive) {
            // The loop ends when the client hangs up.
            while serve_kept(&mut stream, data) {}
            return;
        }
        let mut request = vec![0_u8; 4096];
        let count = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..count]);
        let range = request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find_map(|(name, value)| name.eq_ignore_ascii_case("range").then_some(value.trim()))
            .and_then(|value| value.strip_prefix("bytes="))
            .and_then(|value| value.trim().split_once('-'))
            .map(|(start, end)| {
                (
                    start.parse::<usize>().unwrap(),
                    end.parse::<usize>().unwrap(),
                )
            });
        let has_if_range = request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .any(|(name, _)| name.eq_ignore_ascii_case("if-range"));
        let honor = matches!(mode, ServerMode::Ranges)
            || matches!(mode, ServerMode::ChangeAfterProbe) && !has_if_range;
        if honor {
            let Some((start, end)) = range else {
                let _ = stream.write_all(
                    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                return;
            };
            let end = end.min(data.len() - 1);
            let body = &data[start..=end];
            write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Type: audio/mpeg; charset=binary\r\nETag: \"fixture-v1\"\r\nLast-Modified: Mon, 08 Sep 2026 00:00:00 GMT\r\nConnection: close\r\n\r\n",
                body.len(),
                data.len()
            )
            .ok();
            let _ = stream.write_all(body);
        } else {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: audio/mpeg\r\nConnection: close\r\n\r\n",
                data.len()
            )
            .ok();
            let _ = stream.write_all(data);
        }
    }

    /// Answer one ranged request and leave the connection open. False at EOF.
    fn serve_kept(stream: &mut TcpStream, data: &[u8]) -> bool {
        let mut request = vec![0_u8; 4096];
        let count = match stream.read(&mut request) {
            Ok(0) | Err(_) => return false,
            Ok(count) => count,
        };
        let request = String::from_utf8_lossy(&request[..count]);
        let Some((start, end)) = request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find_map(|(name, value)| name.eq_ignore_ascii_case("range").then_some(value.trim()))
            .and_then(|value| value.strip_prefix("bytes="))
            .and_then(|value| value.split_once('-'))
            .map(|(start, end)| {
                (
                    start.parse::<usize>().unwrap(),
                    end.parse::<usize>().unwrap(),
                )
            })
        else {
            return false;
        };
        let end = end.min(data.len() - 1);
        let body = &data[start..=end];
        write!(
            stream,
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Type: audio/mpeg\r\nETag: \"fixture-v1\"\r\n\r\n",
            body.len(),
            data.len()
        )
        .is_ok()
            && stream.write_all(body).is_ok()
    }

    #[test]
    fn range_source_seeks_within_its_budget_and_records_representation() {
        let data: Vec<_> = (0..3_000_000).map(|index| (index % 251) as u8).collect();
        let server = TestServer::start(data.clone(), ServerMode::Ranges);
        let (mut source, receipt) =
            HttpRangeSource::open(test_fetch(), &server.url(), 128 * 1024).unwrap();

        assert_eq!(
            receipt.requested_url.as_deref(),
            Some(server.url().as_str())
        );
        assert_eq!(receipt.final_url, receipt.requested_url);
        assert_eq!(receipt.media_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(receipt.byte_length, Some(data.len() as u64));
        assert_eq!(receipt.etag.as_deref(), Some("\"fixture-v1\""));
        assert!(receipt.retrieved_at_ms.is_some());
        assert!(receipt.complete_digest.is_none());

        let mut output = [0_u8; 32];
        for offset in [70_000_u64, 1_000_000, 2_500_000] {
            source.seek(SeekFrom::Start(offset)).unwrap();
            source.read_exact(&mut output).unwrap();
            assert_eq!(
                &output,
                &data[offset as usize..offset as usize + output.len()]
            );
        }
        assert_eq!(source.stats().requests, 4);
        assert!(source.stats().fetched_bytes * 10 < data.len() as u64);
        assert!(source.stats().peak_cache_bytes <= 128 * 1024);
        assert_eq!(source.stats().ranges.len(), 4);
    }

    #[test]
    fn range_source_reuses_one_connection_across_ranges() {
        let data: Vec<_> = (0..3_000_000).map(|index| (index % 251) as u8).collect();
        let server = TestServer::start(data.clone(), ServerMode::KeepAlive);
        let (mut source, _) =
            HttpRangeSource::open(test_fetch(), &server.url(), 128 * 1024).unwrap();
        let mut output = [0_u8; 32];
        for offset in [70_000_u64, 1_000_000, 2_500_000] {
            source.seek(SeekFrom::Start(offset)).unwrap();
            source.read_exact(&mut output).unwrap();
            assert_eq!(
                &output,
                &data[offset as usize..offset as usize + output.len()]
            );
        }
        assert_eq!(source.stats().requests, 4);
        // The probe and three ranges: four requests, one connection.
        assert_eq!(server.connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn range_source_reports_server_and_representation_failures() {
        let data = vec![7_u8; 128 * 1024];
        let server = TestServer::start(data.clone(), ServerMode::IgnoreRanges);
        let error = HttpRangeSource::open(test_fetch(), &server.url(), DEFAULT_CACHE_BYTES)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("does not support byte ranges"));

        let changed = TestServer::start(data, ServerMode::ChangeAfterProbe);
        let (mut source, _) =
            HttpRangeSource::open(test_fetch(), &changed.url(), DEFAULT_CACHE_BYTES).unwrap();
        let mut output = [0_u8; 8];
        let error = source.read_exact(&mut output).unwrap_err().to_string();
        assert!(error.contains("representation may have changed"));
    }

    #[test]
    fn range_source_rejects_an_exhausted_budget_before_fetching() {
        let error = HttpRangeSource::open(test_fetch(), "http://127.0.0.1:1/episode.mp3", 1024)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("requires at least 65536 bytes"));
    }
}
