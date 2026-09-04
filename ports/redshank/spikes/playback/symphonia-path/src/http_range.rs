use std::{
    collections::{BTreeMap, VecDeque},
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow};
use symphonia::core::io::MediaSource;

const CHUNK_BYTES: u64 = 64 * 1024;
const CACHE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Default)]
pub struct HttpStats {
    pub requests: u64,
    pub fetched_bytes: u64,
    pub peak_cache_bytes: usize,
    pub ranges: Vec<(u64, u64)>,
}

pub struct HttpRangeSource {
    url: String,
    len: u64,
    position: u64,
    cache: BTreeMap<u64, Vec<u8>>,
    cache_order: VecDeque<u64>,
    cache_bytes: usize,
    stats: Arc<Mutex<HttpStats>>,
}

impl HttpRangeSource {
    pub fn open(url: &str) -> Result<(Self, Arc<Mutex<HttpStats>>)> {
        let mut response = ureq::get(url)
            .header("Range", "bytes=0-0")
            .call()
            .with_context(|| format!("initial range request failed for {url}"))?;
        if response.status().as_u16() != 206 {
            return Err(anyhow!(
                "server returned {}; a seekable range source requires 206",
                response.status()
            ));
        }
        let content_range = response
            .headers()
            .get("Content-Range")
            .context("range response omitted Content-Range")?
            .to_str()
            .context("Content-Range was not text")?;
        let len = content_range
            .rsplit_once('/')
            .context("invalid Content-Range")?
            .1
            .parse::<u64>()
            .context("invalid object length in Content-Range")?;
        let first = response
            .body_mut()
            .read_to_vec()
            .context("failed to read initial range")?;
        let stats = Arc::new(Mutex::new(HttpStats {
            requests: 1,
            fetched_bytes: first.len() as u64,
            peak_cache_bytes: first.len(),
            ranges: vec![(0, 0)],
        }));
        Ok((
            Self {
                url: url.to_owned(),
                len,
                position: 0,
                cache: BTreeMap::new(),
                cache_order: VecDeque::new(),
                cache_bytes: 0,
                stats: stats.clone(),
            },
            stats,
        ))
    }

    fn fetch_chunk(&mut self, start: u64) -> io::Result<()> {
        if self.cache.contains_key(&start) {
            return Ok(());
        }
        let end = (start + CHUNK_BYTES - 1).min(self.len.saturating_sub(1));
        let range = format!("bytes={start}-{end}");
        let mut response = ureq::get(&self.url)
            .header("Range", &range)
            .call()
            .map_err(io::Error::other)?;
        if response.status().as_u16() != 206 {
            return Err(io::Error::other(format!(
                "range request returned {}",
                response.status()
            )));
        }
        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(io::Error::other)?;
        self.cache_bytes += bytes.len();
        self.cache.insert(start, bytes);
        self.cache_order.push_back(start);

        while self.cache_bytes > CACHE_BYTES && self.cache.len() > 1 {
            let Some(oldest) = self.cache_order.pop_front() else {
                break;
            };
            if oldest == start {
                self.cache_order.push_back(oldest);
                break;
            }
            if let Some(removed) = self.cache.remove(&oldest) {
                self.cache_bytes -= removed.len();
            }
        }

        let mut stats = self
            .stats
            .lock()
            .map_err(|_| io::Error::other("HTTP stats lock poisoned"))?;
        stats.requests += 1;
        stats.fetched_bytes += self
            .cache
            .get(&start)
            .map(|chunk| chunk.len() as u64)
            .unwrap_or(0);
        stats.peak_cache_bytes = stats.peak_cache_bytes.max(self.cache_bytes);
        stats.ranges.push((start, end));
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
                .ok_or_else(|| io::Error::other("fetched chunk missing from cache"))?;
            let offset = (self.position - start) as usize;
            let available = chunk.len().saturating_sub(offset);
            if available == 0 {
                break;
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
                "seek outside HTTP object",
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
