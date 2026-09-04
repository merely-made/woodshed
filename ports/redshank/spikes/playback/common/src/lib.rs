#![forbid(unsafe_code)]

use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    cpal::CpalConfig,
    nodes::stream::{
        ResamplingChannelConfig,
        writer::{PushStatus, StreamWriterConfig, StreamWriterNode, StreamWriterState},
    },
};

#[derive(Clone, Copy, Debug, Default)]
pub struct SinkStats {
    pub callbacks: u64,
    pub accepted_frames: u64,
    pub dropped_frames: u64,
    pub corrected_underflow_frames: u64,
    pub max_occupied_seconds: f64,
}

#[derive(Clone)]
pub struct FirewheelSink {
    state: Arc<Mutex<StreamWriterState>>,
    stats: Arc<Mutex<SinkStats>>,
    source_sample_rate: u32,
}

impl FirewheelSink {
    pub fn push_mono(&self, samples: &[f32]) -> Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("stream writer lock poisoned"))?;
        let status = state.push_interleaved(samples);
        let occupied = state.occupied_seconds().unwrap_or(0.0);
        drop(state);

        let (accepted, dropped, corrected) = match status {
            PushStatus::Ok => (samples.len(), 0, 0),
            PushStatus::OutputNotReady => (0, samples.len(), 0),
            PushStatus::OverflowOccurred { num_frames_pushed } => (
                num_frames_pushed,
                samples.len().saturating_sub(num_frames_pushed),
                0,
            ),
            PushStatus::UnderflowCorrected {
                num_zero_frames_pushed,
            } => (samples.len(), 0, num_zero_frames_pushed),
        };

        let mut stats = self
            .stats
            .lock()
            .map_err(|_| anyhow!("stats lock poisoned"))?;
        stats.callbacks += 1;
        stats.accepted_frames += accepted as u64;
        stats.dropped_frames += dropped as u64;
        stats.corrected_underflow_frames += corrected as u64;
        stats.max_occupied_seconds = stats.max_occupied_seconds.max(occupied);
        Ok(accepted)
    }

    pub fn snapshot_seconds(&self) -> Result<f64> {
        let occupied = self
            .state
            .lock()
            .map_err(|_| anyhow!("stream writer lock poisoned"))?
            .occupied_seconds()
            .unwrap_or(0.0);
        let accepted = self
            .stats
            .lock()
            .map_err(|_| anyhow!("stats lock poisoned"))?
            .accepted_frames;
        Ok(presented_seconds(
            accepted,
            self.source_sample_rate,
            occupied,
        ))
    }

    pub fn stats(&self) -> Result<SinkStats> {
        self.stats
            .lock()
            .map(|stats| *stats)
            .map_err(|_| anyhow!("stats lock poisoned"))
    }

    pub fn occupied_seconds(&self) -> Result<f64> {
        self.state
            .lock()
            .map_err(|_| anyhow!("stream writer lock poisoned"))
            .map(|state| state.occupied_seconds().unwrap_or(0.0))
    }
}

pub struct FirewheelHost {
    context: FirewheelContext,
    sink: FirewheelSink,
    output_sample_rate: u32,
    output_channels: u32,
}

impl FirewheelHost {
    pub fn new(source_sample_rate: u32) -> Result<Self> {
        let source_rate =
            NonZeroU32::new(source_sample_rate).context("source sample rate must be non-zero")?;
        let mut context = FirewheelContext::new(FirewheelConfig {
            num_graph_inputs: ChannelCount::ZERO,
            ..Default::default()
        });
        context
            .start_stream(CpalConfig::default())
            .map_err(|error| anyhow!("Firewheel CPAL startup failed: {error:?}"))?;

        let stream_info = context
            .stream_info()
            .context("Firewheel did not report stream information")?;
        let output_rate = stream_info.sample_rate;
        let output_channels = stream_info.num_stream_out_channels;

        let writer_id = context.add_node(
            StreamWriterNode,
            Some(StreamWriterConfig {
                channels: NonZeroChannelCount::MONO,
                check_for_silence: true,
            }),
        );
        let output_id = context.graph_out_node_id();
        let edges = if output_channels >= 2 {
            &[(0, 0), (0, 1)][..]
        } else {
            &[(0, 0)][..]
        };
        context
            .connect(writer_id, output_id, edges, false)
            .map_err(|error| anyhow!("Firewheel graph connection failed: {error:?}"))?;

        let mut writer_state = context
            .node_state::<StreamWriterState>(writer_id)
            .context("Firewheel stream-writer state missing")?
            .clone();
        let event = writer_state
            .start_stream(
                source_rate,
                output_rate,
                ResamplingChannelConfig {
                    latency_seconds: 0.08,
                    capacity_seconds: 6.0,
                    underflow_autocorrect_percent_threshold: None,
                    overflow_autocorrect_percent_threshold: None,
                    ..Default::default()
                },
            )
            .map_err(|_| anyhow!("Firewheel stream writer failed to start"))?;
        context.queue_event_for(writer_id, event.into());

        let sink = FirewheelSink {
            state: Arc::new(Mutex::new(writer_state)),
            stats: Arc::new(Mutex::new(SinkStats::default())),
            source_sample_rate,
        };
        let ready_by = Instant::now() + Duration::from_secs(2);
        while !sink
            .state
            .lock()
            .map_err(|_| anyhow!("stream writer lock poisoned"))?
            .is_ready()
        {
            context
                .update()
                .map_err(|error| anyhow!("Firewheel update failed: {error:?}"))?;
            if Instant::now() >= ready_by {
                return Err(anyhow!(
                    "Firewheel stream writer was not ready within 2 seconds"
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }

        Ok(Self {
            context,
            sink,
            output_sample_rate: output_rate.get(),
            output_channels,
        })
    }

    pub fn sink(&self) -> FirewheelSink {
        self.sink.clone()
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.output_sample_rate
    }

    pub fn output_channels(&self) -> u32 {
        self.output_channels
    }

    pub fn tick(&mut self) -> Result<()> {
        self.context
            .update()
            .map_err(|error| anyhow!("Firewheel update failed: {error:?}"))
    }

    pub fn drain(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        while self.sink.occupied_seconds()? > 0.02 && Instant::now() < deadline {
            self.tick()?;
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

pub fn presented_seconds(accepted_frames: u64, sample_rate: u32, queued_seconds: f64) -> f64 {
    (accepted_frames as f64 / sample_rate as f64 - queued_seconds).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::presented_seconds;

    #[test]
    fn press_time_snapshot_tracks_presented_audio_within_fifty_ms() {
        let sample_rate = 48_000;
        let accepted = (12.345 * sample_rate as f64) as u64;
        let queued = 0.083;
        let expected = 12.262;
        let measured = presented_seconds(accepted, sample_rate, queued);
        assert!((measured - expected).abs() < 0.050);
    }

    #[test]
    fn press_time_snapshot_never_goes_negative() {
        assert_eq!(presented_seconds(240, 48_000, 0.050), 0.0);
    }
}
