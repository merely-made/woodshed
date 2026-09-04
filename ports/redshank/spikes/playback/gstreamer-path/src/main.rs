#![forbid(unsafe_code)]

use std::{
    env, fs,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use redshank_playback_spike_common::{FirewheelHost, FirewheelSink};
use servo_media::player::{
    PlayerEvent, StreamType,
    audio::AudioRenderer,
    context::{GlApi, GlContext, NativeDisplay, PlayerGLContext},
    ipc_channel::ipc,
};
use servo_media::{BackendInit, ClientContextId};
use servo_media_gstreamer::GStreamerBackend;

const FIXTURE_SAMPLE_RATE: u32 = 48_000;

#[derive(Debug)]
struct Args {
    path: String,
    max_seconds: f64,
}

fn parse_args() -> Result<Args> {
    let mut values = env::args().skip(1);
    let path = values
        .next()
        .context("usage: redshank-gstreamer-spike <path> [--max-seconds seconds]")?;
    let mut max_seconds = 1.0;
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--max-seconds" => {
                max_seconds = value.parse().context("invalid --max-seconds value")?
            }
            _ => bail!("unknown argument {flag}"),
        }
    }
    if max_seconds <= 0.0 {
        bail!("--max-seconds must be positive");
    }
    Ok(Args { path, max_seconds })
}

#[derive(Default)]
struct RendererStats {
    callbacks: u64,
    first_callback_ms: Option<u128>,
    last_channel: Option<u32>,
    push_errors: u64,
}

struct FirewheelRenderer {
    sink: FirewheelSink,
    started: Instant,
    stats: RendererStats,
}

impl AudioRenderer for FirewheelRenderer {
    fn render(&mut self, sample: Box<dyn AsRef<[f32]>>, channel: u32) {
        self.stats.callbacks += 1;
        self.stats
            .first_callback_ms
            .get_or_insert_with(|| self.started.elapsed().as_millis());
        self.stats.last_channel = Some(channel);
        if self.sink.push_mono(sample.as_ref().as_ref()).is_err() {
            self.stats.push_errors += 1;
        }
    }
}

struct HeadlessContext;

impl PlayerGLContext for HeadlessContext {
    fn get_gl_context(&self) -> GlContext {
        GlContext::Unknown
    }

    fn get_native_display(&self) -> NativeDisplay {
        NativeDisplay::Headless
    }

    fn get_gl_api(&self) -> GlApi {
        GlApi::None
    }
}

fn main() -> Result<()> {
    let started = Instant::now();
    let args = parse_args()?;
    let bytes = fs::read(&args.path).with_context(|| format!("failed to read {}", args.path))?;
    let mut host = FirewheelHost::new(FIXTURE_SAMPLE_RATE)?;
    let sink = host.sink();
    let renderer = Arc::new(Mutex::new(FirewheelRenderer {
        sink: sink.clone(),
        started,
        stats: RendererStats::default(),
    }));
    let renderer_for_player: Arc<Mutex<dyn AudioRenderer>> = renderer.clone();
    let (event_sender, event_receiver) = ipc::channel::<PlayerEvent>()
        .map_err(|error| anyhow!("player event channel failed: {error}"))?;
    let backend = <GStreamerBackend as BackendInit>::init();
    let context_id = ClientContextId::build(1, 1);
    let player = backend.create_player(
        &context_id,
        StreamType::Seekable,
        event_sender,
        None,
        Some(renderer_for_player),
        Box::new(HeadlessContext),
    );

    {
        let player = player.lock().map_err(|_| anyhow!("player lock poisoned"))?;
        player
            .set_input_size(bytes.len() as u64)
            .map_err(|error| anyhow!("set_input_size failed: {error:?}"))?;
        player
            .push_data(bytes)
            .map_err(|error| anyhow!("push_data failed: {error:?}"))?;
        player
            .end_of_stream()
            .map_err(|error| anyhow!("end_of_stream failed: {error:?}"))?;
        player
            .play()
            .map_err(|error| anyhow!("play failed: {error:?}"))?;
    }

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut position_events = 0_u64;
    let mut last_position = None;
    let mut metadata_events = 0_u64;
    let mut state_events = 0_u64;
    let mut end_of_stream = false;
    while Instant::now() < deadline {
        host.tick()?;
        while let Ok(event) = event_receiver.try_recv() {
            match event {
                PlayerEvent::PositionChanged(seconds) => {
                    position_events += 1;
                    last_position = Some(seconds);
                }
                PlayerEvent::MetadataUpdated(_) => metadata_events += 1,
                PlayerEvent::StateChanged(_) => state_events += 1,
                PlayerEvent::Error(error) => {
                    return Err(anyhow!("GStreamer player error: {error}"));
                }
                PlayerEvent::EndOfStream => end_of_stream = true,
                PlayerEvent::SeekData(_, lock) => lock.unlock(false),
                _ => {}
            }
        }
        if sink.snapshot_seconds()? >= args.max_seconds || end_of_stream {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    player
        .lock()
        .map_err(|_| anyhow!("player lock poisoned"))?
        .pause()
        .map_err(|error| anyhow!("pause failed: {error:?}"))?;
    host.drain(Duration::from_secs(2))?;

    let renderer = renderer
        .lock()
        .map_err(|_| anyhow!("renderer lock poisoned"))?;
    let sink_stats = sink.stats()?;
    if renderer.stats.callbacks == 0 {
        bail!("GStreamer produced no caller-renderer audio within 15 seconds");
    }
    println!("backend=gstreamer");
    println!("fixture_sample_rate={FIXTURE_SAMPLE_RATE}");
    println!("fixture_channels=1");
    println!("snapshot_seconds={:.6}", sink.snapshot_seconds()?);
    println!(
        "startup_ms={}",
        renderer
            .stats
            .first_callback_ms
            .context("first callback timestamp missing")?
    );
    println!("audio_stream_starts=1");
    println!("firewheel_output_sample_rate={}", host.output_sample_rate());
    println!("firewheel_output_channels={}", host.output_channels());
    println!("renderer_callbacks={}", renderer.stats.callbacks);
    println!("renderer_last_channel={:?}", renderer.stats.last_channel);
    println!("renderer_push_errors={}", renderer.stats.push_errors);
    println!("accepted_frames={}", sink_stats.accepted_frames);
    println!("dropped_frames={}", sink_stats.dropped_frames);
    println!("position_events={position_events}");
    println!("last_position={last_position:?}");
    println!("metadata_events={metadata_events}");
    println!("state_events={state_events}");
    println!("end_of_stream={end_of_stream}");
    Ok(())
}
