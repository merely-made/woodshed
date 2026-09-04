#![forbid(unsafe_code)]

mod http_range;

use std::{
    env,
    fs::File,
    path::Path,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use http_range::{HttpRangeSource, HttpStats};
use redshank_playback_spike_common::FirewheelHost;
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

#[derive(Debug)]
struct Args {
    source: String,
    seek_seconds: Option<f64>,
    max_seconds: f64,
}

fn parse_args() -> Result<Args> {
    let mut values = env::args().skip(1);
    let source = values
        .next()
        .context("usage: redshank-symphonia-spike <path-or-http-url> [--seek seconds] [--max-seconds seconds]")?;
    let mut seek_seconds = None;
    let mut max_seconds = 1.0;
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--seek" => seek_seconds = Some(value.parse().context("invalid --seek value")?),
            "--max-seconds" => {
                max_seconds = value.parse().context("invalid --max-seconds value")?
            }
            _ => bail!("unknown argument {flag}"),
        }
    }
    if max_seconds <= 0.0 {
        bail!("--max-seconds must be positive");
    }
    Ok(Args {
        source,
        seek_seconds,
        max_seconds,
    })
}

fn main() -> Result<()> {
    let started = Instant::now();
    let args = parse_args()?;
    let mut hint = Hint::new();
    let (source, http_stats): (
        Box<dyn MediaSource>,
        Option<Arc<std::sync::Mutex<HttpStats>>>,
    ) = if args.source.starts_with("http://") {
        let (source, stats) = HttpRangeSource::open(&args.source)?;
        if let Some(extension) = Path::new(&args.source)
            .extension()
            .and_then(|value| value.to_str())
        {
            hint.with_extension(extension);
        }
        (Box::new(source), Some(stats))
    } else {
        let path = Path::new(&args.source);
        if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
            hint.with_extension(extension);
        }
        (
            Box::new(
                File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
            ),
            None,
        )
    };

    let media_stream = MediaSourceStream::new(source, Default::default());
    let probed = symphonia::default::get_probe().format(
        &hint,
        media_stream,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("source has no default audio track")?;
    let track_id = track.id;
    let codec = format!("{:?}", track.codec_params.codec);
    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    if let Some(seconds) = args.seek_seconds {
        format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: Time::from(seconds),
                track_id: Some(track_id),
            },
        )?;
        decoder.reset();
    }

    let mut host = None;
    let mut sample_buffer = None;
    let mut decoded_frames = 0_u64;
    let mut first_audio_ms = None;
    let mut source_sample_rate = 0_u32;
    let mut channels = 0_usize;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(error.into()),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(error.into()),
        };
        let spec = *decoded.spec();
        source_sample_rate = spec.rate;
        channels = spec.channels.count();
        if channels != 1 {
            bail!("Phase 0 comparison fixture must be mono; decoded {channels} channels");
        }
        if host.is_none() {
            host = Some(FirewheelHost::new(source_sample_rate)?);
        }
        if sample_buffer.is_none() {
            sample_buffer = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
        }
        let buffer = sample_buffer
            .as_mut()
            .expect("sample buffer was initialized");
        buffer.copy_interleaved_ref(decoded);
        let target_frames = (args.max_seconds * source_sample_rate as f64).ceil() as u64;
        let remaining = target_frames.saturating_sub(decoded_frames) as usize;
        let samples = &buffer.samples()[..remaining.min(buffer.samples().len())];
        if samples.is_empty() {
            break;
        }
        let playback = host
            .as_ref()
            .expect("Firewheel host was initialized")
            .sink();
        let accepted = playback.push_mono(samples)?;
        if accepted == 0 {
            host.as_mut()
                .expect("Firewheel host was initialized")
                .tick()?;
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        decoded_frames += accepted as u64;
        first_audio_ms.get_or_insert_with(|| started.elapsed().as_millis());
        host.as_mut()
            .expect("Firewheel host was initialized")
            .tick()?;
        if decoded_frames >= target_frames {
            break;
        }
    }

    let mut host = host.context("source produced no decodable audio")?;
    let sink = host.sink();
    host.drain(Duration::from_secs_f64(args.max_seconds + 2.0))?;
    let stats = sink.stats()?;
    println!("backend=symphonia");
    println!("codec={codec}");
    println!("sample_rate={source_sample_rate}");
    println!("channels={channels}");
    println!("decoded_frames={decoded_frames}");
    println!("snapshot_seconds={:.6}", sink.snapshot_seconds()?);
    println!(
        "startup_ms={}",
        first_audio_ms.ok_or_else(|| anyhow!("first audio timestamp missing"))?
    );
    println!("audio_stream_starts=1");
    println!("firewheel_output_sample_rate={}", host.output_sample_rate());
    println!("firewheel_output_channels={}", host.output_channels());
    println!("callbacks={}", stats.callbacks);
    println!("dropped_frames={}", stats.dropped_frames);
    println!("peak_queue_seconds={:.6}", stats.max_occupied_seconds);
    if let Some(stats) = http_stats {
        let stats = stats
            .lock()
            .map_err(|_| anyhow!("HTTP stats lock poisoned"))?;
        println!("http_requests={}", stats.requests);
        println!("http_fetched_bytes={}", stats.fetched_bytes);
        println!("http_peak_cache_bytes={}", stats.peak_cache_bytes);
        println!("http_ranges={:?}", stats.ranges);
    }
    Ok(())
}
