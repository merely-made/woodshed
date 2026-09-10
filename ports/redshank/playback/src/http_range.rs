use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Seek, SeekFrom},
};

use anyhow::{Context, Result, anyhow, bail};
use redshank_model::RepresentationReceipt;
use symphonia::core::io::MediaSource;
use ureq::ResponseExt;

pub(super) const DEFAULT_CACHE_BYTES: usize = 256 * 1024;
const CHUNK_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct HttpStats {
    pub(super) requests: u64,
    pub(super) fetched_bytes: u64,
    pub(super) peak_cache_bytes: usize,
    pub(super) ranges: Vec<(u64, u64)>,
}

pub(super) struct HttpRangeSource {
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
    pub(super) fn open(url: &str, cache_budget: usize) -> Result<(Self, RepresentationReceipt)> {
        if cache_budget < CHUNK_BYTES as usize {
            bail!(
                "HTTP cache budget is {cache_budget} bytes; range playback requires at least {CHUNK_BYTES} bytes"
            );
        }
        let mut response = ureq::get(url)
            .header("Accept-Encoding", "identity")
            .header("Range", "bytes=0-0")
            .call()
            .with_context(|| format!("could not reach HTTP audio source {url}"))?;
        if response.status().as_u16() != 206 {
            bail!(
                "HTTP audio source does not support byte ranges: expected 206, received {}",
                response.status()
            );
        }
        let (start, end, len) = parse_content_range(header(&response, "Content-Range")?)?;
        if (start, end) != (0, 0) {
            bail!("initial HTTP range returned bytes {start}-{end}, expected 0-0");
        }
        let first = response
            .body_mut()
            .with_config()
            .limit(2)
            .read_to_vec()
            .context("could not read initial HTTP range")?;
        if first.len() != 1 {
            bail!(
                "initial HTTP range returned {} bytes, expected 1",
                first.len()
            );
        }

        let final_url = response.get_uri().to_string();
        let media_type = optional_header(&response, "Content-Type")
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_owned());
        let etag = optional_header(&response, "ETag").map(str::to_owned);
        let last_modified = optional_header(&response, "Last-Modified").map(str::to_owned);
        let validator = etag.clone().or_else(|| last_modified.clone());
        let receipt = RepresentationReceipt {
            requested_url: Some(url.to_owned()),
            final_url: Some(final_url.clone()),
            media_type,
            byte_length: Some(len),
            etag,
            last_modified,
            retrieved_at_ms: Some(now_ms()),
            complete_digest: None,
        };
        Ok((
            Self {
                url: final_url,
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
        let range = format!("bytes={start}-{end}");
        let mut request = ureq::get(&self.url)
            .header("Accept-Encoding", "identity")
            .header("Range", &range);
        if let Some(validator) = &self.validator {
            request = request.header("If-Range", validator);
        }
        let mut response = request.call().map_err(http_error)?;
        if response.status().as_u16() != 206 {
            return Err(io::Error::other(format!(
                "HTTP range {range} was not honored; received {} (the representation may have changed)",
                response.status()
            )));
        }
        let actual =
            parse_content_range(header(&response, "Content-Range").map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
        if actual != (start, end, self.len) {
            return Err(io::Error::other(format!(
                "HTTP range {range} returned bytes {}-{}/{}",
                actual.0, actual.1, actual.2
            )));
        }
        let expected = (end - start + 1) as usize;
        if expected > self.cache_budget {
            return Err(io::Error::other(format!(
                "HTTP range needs {expected} cache bytes but the budget is {}",
                self.cache_budget
            )));
        }
        let bytes = response
            .body_mut()
            .with_config()
            .limit(expected.saturating_add(1) as u64)
            .read_to_vec()
            .map_err(http_error)?;
        if bytes.len() != expected {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("HTTP range {range} returned {} bytes", bytes.len()),
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

fn header<'a>(response: &'a ureq::http::Response<ureq::Body>, name: &str) -> Result<&'a str> {
    optional_header(response, name).ok_or_else(|| anyhow!("HTTP response omitted {name}"))
}

fn optional_header<'a>(
    response: &'a ureq::http::Response<ureq::Body>,
    name: &str,
) -> Option<&'a str> {
    response.headers().get(name)?.to_str().ok()
}

fn parse_content_range(value: &str) -> Result<(u64, u64, u64)> {
    let value = value
        .strip_prefix("bytes ")
        .context("invalid Content-Range unit")?;
    let (range, length) = value
        .split_once('/')
        .context("invalid Content-Range length")?;
    let (start, end) = range
        .split_once('-')
        .context("invalid Content-Range bounds")?;
    let parsed = (
        start.parse().context("invalid Content-Range start")?,
        end.parse().context("invalid Content-Range end")?,
        length
            .parse()
            .context("invalid Content-Range object length")?,
    );
    if parsed.0 > parsed.1 || parsed.1 >= parsed.2 {
        bail!("invalid Content-Range ordering");
    }
    Ok(parsed)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn http_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        net::{SocketAddr, TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    #[derive(Clone, Copy)]
    enum ServerMode {
        Ranges,
        IgnoreRanges,
        ChangeAfterProbe,
    }

    struct TestServer {
        address: SocketAddr,
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
            let thread = thread::spawn(move || {
                while !thread_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            if thread_stop.load(Ordering::Relaxed) {
                                break;
                            }
                            stream.set_nonblocking(false).unwrap();
                            serve(stream, &data, mode);
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

    #[test]
    fn parses_valid_content_range() {
        assert_eq!(
            parse_content_range("bytes 64-127/256").unwrap(),
            (64, 127, 256)
        );
    }

    #[test]
    fn rejects_invalid_content_range() {
        assert!(parse_content_range("bytes 12-9/20").is_err());
        assert!(parse_content_range("items 0-1/2").is_err());
        assert!(parse_content_range("bytes 0-2/2").is_err());
    }

    #[test]
    fn range_source_seeks_within_its_budget_and_records_representation() {
        let data: Vec<_> = (0..3_000_000).map(|index| (index % 251) as u8).collect();
        let server = TestServer::start(data.clone(), ServerMode::Ranges);
        let (mut source, receipt) = HttpRangeSource::open(&server.url(), 128 * 1024).unwrap();

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
    fn range_source_reports_server_and_representation_failures() {
        let data = vec![7_u8; 128 * 1024];
        let server = TestServer::start(data.clone(), ServerMode::IgnoreRanges);
        let error = HttpRangeSource::open(&server.url(), DEFAULT_CACHE_BYTES)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("does not support byte ranges"));

        let changed = TestServer::start(data, ServerMode::ChangeAfterProbe);
        let (mut source, _) = HttpRangeSource::open(&changed.url(), DEFAULT_CACHE_BYTES).unwrap();
        let mut output = [0_u8; 8];
        let error = source.read_exact(&mut output).unwrap_err().to_string();
        assert!(error.contains("representation may have changed"));
    }

    #[test]
    fn range_source_rejects_an_exhausted_budget_before_fetching() {
        let error = HttpRangeSource::open("http://127.0.0.1:1/episode.mp3", 1024)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("requires at least 65536 bytes"));
    }
}
