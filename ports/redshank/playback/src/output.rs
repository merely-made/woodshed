use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    cpal::{CpalConfig, CpalStream},
    diff::{Diff, PathBuilder},
    node::NodeID,
    nodes::{
        stream::{
            ResamplingChannelConfig, media_channel_config,
            writer::{StreamWriterConfig, StreamWriterNode, StreamWriterState},
        },
        volume::{VolumeNode, VolumeNodeConfig},
    },
};

use crate::backend::Sink;

pub(super) const BUFFER_LOW_WATER_SECONDS: f64 = 0.40;

fn stream_buffer_config() -> ResamplingChannelConfig {
    // The maintained bridge budgets output frames and reserves actual packet
    // headroom. Decoding retains/retries every source frame under backpressure.
    media_channel_config(1.0)
}

/// Firewheel's default `num_graph_outputs`: the ports the graph's output node has.
const GRAPH_OUTPUT_CHANNELS: u32 = 2;

pub(super) struct AudioRuntime {
    // Drop the backend before its graph context.
    stream: CpalStream,
    context: FirewheelContext,
    generation: Arc<()>,
    output_channels: u32,
    /// One host-owned gain every decoded stream passes through.
    volume: NodeID,
    volume_params: VolumeNode,
}

impl AudioRuntime {
    pub(super) fn new() -> Result<Self> {
        let mut context = FirewheelContext::new(FirewheelConfig {
            num_graph_inputs: ChannelCount::ZERO,
            ..Default::default()
        });
        let stream = CpalStream::new(&mut context, CpalConfig::default())
            .map_err(|error| anyhow!("could not start the default audio output: {error:?}"))?;
        // The graph's output is stereo (Firewheel's default), and it maps onto the
        // first channels of whatever stream the device opened. A six-channel
        // device therefore still offers the graph two ports; connecting one edge
        // per device channel overflowed them (InPortOutOfRange, 2026-09-22).
        let output_channels = stream
            .info()
            .num_stream_out_channels
            .min(GRAPH_OUTPUT_CHANNELS);
        let channels = NonZeroChannelCount::new(output_channels)
            .context("the audio output reported no channels")?;
        let volume_params = VolumeNode::from_percent(100.0);
        let volume = context
            .add_node(volume_params, Some(VolumeNodeConfig { channels }))
            .map_err(|error| anyhow!("could not construct output gain: {error:?}"))?;
        let edges: Vec<_> = (0..output_channels)
            .map(|channel| (channel, channel))
            .collect();
        context
            .connect(volume, context.graph_out_node_id(), &edges, false)
            .map_err(|error| anyhow!("could not connect the output gain: {error:?}"))?;
        Ok(Self {
            stream,
            context,
            generation: Arc::new(()),
            output_channels,
            volume,
            volume_params,
        })
    }

    /// Output volume in percent, applied as a smoothed Firewheel gain.
    pub(super) fn set_volume(&mut self, percent: u8) {
        let next = VolumeNode::from_percent(f32::from(percent));
        if next == self.volume_params {
            return;
        }
        let previous = self.volume_params;
        self.volume_params = next;
        let mut queue = self.context.event_queue(self.volume);
        next.diff(&previous, PathBuilder::default(), &mut queue);
    }

    pub(super) fn sink(&mut self, source_rate: u32, source_channels: u32) -> Result<Sink> {
        construct_sink(
            &mut self.context,
            self.volume,
            source_rate,
            source_channels,
            self.stream.info().sample_rate,
            self.output_channels,
            Arc::clone(&self.generation),
        )
    }

    pub(super) fn owns_sink(&self, sink: &Sink) -> bool {
        Arc::ptr_eq(&self.generation, &sink.generation)
    }

    pub(super) fn remove_sink(&mut self, sink: Sink) -> Result<()> {
        retire_sink(&mut self.context, &self.generation, sink)
    }

    pub(super) fn tick(&mut self) -> Result<()> {
        self.stream.log_status();
        if !self.stream.output_stream_ok() {
            bail!("audio output stopped; reopen playback to restart the device");
        }
        self.context
            .update()
            .map_err(|error| anyhow!("audio runtime update failed: {error:?}"))
    }
}

fn construct_sink(
    context: &mut FirewheelContext,
    volume: NodeID,
    source_rate: u32,
    source_channels: u32,
    output_rate: NonZeroU32,
    output_channels: u32,
    generation: Arc<()>,
) -> Result<Sink> {
    let source_rate =
        NonZeroU32::new(source_rate).context("decoded stream has zero sample rate")?;
    let channels = NonZeroChannelCount::new(source_channels)
        .context("decoded audio has an unsupported channel count")?;
    if source_channels > output_channels {
        bail!(
            "decoded audio has {source_channels} channels but the output has only {}",
            output_channels
        );
    }
    let writer = context
        .add_node(
            StreamWriterNode,
            Some(StreamWriterConfig {
                channels,
                check_for_silence: true,
            }),
        )
        .map_err(|error| anyhow!("could not construct decoded stream: {error:?}"))?;
    let edges: Vec<_> = if source_channels == 1 {
        (0..output_channels).map(|channel| (0, channel)).collect()
    } else {
        (0..source_channels)
            .map(|channel| (channel, channel))
            .collect()
    };
    let result = (|| {
        context
            .connect(writer, volume, &edges, false)
            .map_err(|error| anyhow!("could not connect audio graph: {error:?}"))?;
        let mut state = context
            .node_state::<StreamWriterState>(writer)
            .context("Firewheel stream writer missing")?
            .clone();
        let event = state
            .start_stream(source_rate, output_rate, stream_buffer_config())
            .map_err(|_| anyhow!("could not start decoded audio stream"))?;
        context.queue_event_for(writer, event.into());
        Ok(Sink {
            state: Arc::new(Mutex::new(state)),
            node: writer,
            rate: source_rate.get(),
            channels: source_channels as usize,
            generation,
        })
    })();
    if result.is_err() {
        context
            .remove_node(writer)
            .map_err(|error| anyhow!("could not retire rejected audio node: {error:?}"))?;
    }
    result
}

fn retire_sink(context: &mut FirewheelContext, generation: &Arc<()>, sink: Sink) -> Result<()> {
    let stop = sink.stop();
    if !Arc::ptr_eq(generation, &sink.generation) {
        return stop;
    }
    let remove = context
        .remove_node(sink.node)
        .map(|_| ())
        .map_err(|error| anyhow!("could not remove retired audio node: {error:?}"));
    match (stop, remove) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(stop), Ok(())) => Err(stop),
        (Ok(()), Err(remove)) => Err(remove),
        (Err(stop), Err(remove)) => Err(anyhow!(
            "could not stop retired audio: {stop}; additionally {remove}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{RateStage, consume_frames};
    use audioadapter_buffers::direct::InterleavedSlice;
    use firewheel::{
        ActivateInfo, backend::BackendProcessInfo, node::StreamStatus,
        processor::FirewheelProcessor,
    };
    use std::time::Duration;

    struct OfflineSink {
        processor: FirewheelProcessor,
        context: FirewheelContext,
        sink: Sink,
        channels: usize,
        output_rate: u32,
        output_frames: u64,
    }

    impl OfflineSink {
        fn new(source_rate: u32, output_rate: u32, channels: usize) -> Self {
            let mut context = FirewheelContext::new(FirewheelConfig {
                num_graph_inputs: ChannelCount::ZERO,
                num_graph_outputs: ChannelCount::new(channels as u32).unwrap(),
                ..Default::default()
            });
            let writer = context
                .add_node(
                    StreamWriterNode,
                    Some(StreamWriterConfig {
                        channels: NonZeroChannelCount::new(channels as u32).unwrap(),
                        check_for_silence: false,
                    }),
                )
                .unwrap();
            let edges: Vec<_> = (0..channels as u32).map(|ch| (ch, ch)).collect();
            context
                .connect(writer, context.graph_out_node_id(), &edges, false)
                .unwrap();
            let mut state = context
                .node_state::<StreamWriterState>(writer)
                .unwrap()
                .clone();
            let processor = context
                .activate(ActivateInfo {
                    sample_rate: NonZeroU32::new(output_rate).unwrap(),
                    max_block_frames: NonZeroU32::new(8192).unwrap(),
                    num_stream_in_channels: 0,
                    num_stream_out_channels: channels as u32,
                    input_to_output_latency_seconds: 0.0,
                })
                .unwrap();
            let event = state
                .start_stream(
                    NonZeroU32::new(source_rate).unwrap(),
                    NonZeroU32::new(output_rate).unwrap(),
                    stream_buffer_config(),
                )
                .unwrap();
            context.queue_event_for(writer, event.into());
            context.update().unwrap();
            let mut offline = Self {
                processor,
                context,
                sink: Sink {
                    state: Arc::new(Mutex::new(state)),
                    node: writer,
                    rate: source_rate,
                    channels,
                    generation: Arc::new(()),
                },
                channels,
                output_rate,
                output_frames: 0,
            };
            offline.block(1);
            offline
        }

        fn block(&mut self, frames: usize) -> Vec<f32> {
            self.context.update().unwrap();
            let mut samples = vec![0.0; frames * self.channels];
            let input = InterleavedSlice::new(&[], 0, frames).unwrap();
            let mut output =
                InterleavedSlice::new_mut(&mut samples, self.channels, frames).unwrap();
            self.processor.process(
                &input,
                &mut output,
                BackendProcessInfo {
                    frames,
                    process_timestamp: None,
                    duration_since_stream_start: Duration::from_secs_f64(
                        self.output_frames as f64 / f64::from(self.output_rate),
                    ),
                    input_stream_status: StreamStatus::empty(),
                    output_stream_status: StreamStatus::empty(),
                    dropped_frames: 0,
                    process_to_playback_delay: None,
                },
            );
            self.output_frames += frames as u64;
            samples
        }
    }

    #[test]
    fn rejected_stream_admission_retires_its_graph_node() {
        let mut context = FirewheelContext::new(Default::default());
        let volume = context.add_node(VolumeNode::default(), None).unwrap();
        let baseline = context.nodes().count();
        for rate in [1, 999, u32::MAX] {
            assert!(
                construct_sink(
                    &mut context,
                    volume,
                    rate,
                    1,
                    NonZeroU32::new(48000).unwrap(),
                    2,
                    Arc::new(()),
                )
                .is_err()
            );
            assert_eq!(context.nodes().count(), baseline);
        }
        let sink = construct_sink(
            &mut context,
            volume,
            8000,
            1,
            NonZeroU32::new(48000).unwrap(),
            2,
            Arc::new(()),
        )
        .unwrap();
        assert_eq!(context.nodes().count(), baseline + 1);
        sink.stop().unwrap();
        context.remove_node(sink.node).unwrap();
        assert_eq!(context.nodes().count(), baseline);
    }

    #[test]
    fn old_generation_retirement_cannot_remove_a_new_contexts_sink() {
        let mut old_context = FirewheelContext::new(Default::default());
        let old_volume = old_context.add_node(VolumeNode::default(), None).unwrap();
        let old_generation = Arc::new(());
        let old_sink = construct_sink(
            &mut old_context,
            old_volume,
            8000,
            1,
            NonZeroU32::new(48000).unwrap(),
            2,
            old_generation,
        )
        .unwrap();
        let mut context = FirewheelContext::new(Default::default());
        let volume = context.add_node(VolumeNode::default(), None).unwrap();
        let generation = Arc::new(());
        let sink = construct_sink(
            &mut context,
            volume,
            8000,
            1,
            NonZeroU32::new(48000).unwrap(),
            2,
            Arc::clone(&generation),
        )
        .unwrap();
        // Different graphs can reuse the same arena NodeID.
        assert_eq!(old_sink.node, sink.node);
        retire_sink(&mut context, &generation, old_sink).unwrap();
        assert!(context.contains_node(sink.node));
        retire_sink(&mut context, &generation, sink).unwrap();
        assert_eq!(context.nodes().count(), 3);
    }

    #[test]
    fn decoded_stream_buffer_never_discards_or_inserts_source_time() {
        let config = stream_buffer_config();
        assert_eq!(config.overflow_autocorrect_percent_threshold, None);
        assert_eq!(config.underflow_autocorrect_percent_threshold, None);
        assert_eq!(config.latency_seconds, 0.0);
        assert_eq!(config.capacity_seconds, 1.0);
        assert!(BUFFER_LOW_WATER_SECONDS < config.capacity_seconds);
    }

    #[test]
    fn resampled_backlog_and_short_eof_preserve_exact_source_position() {
        for (source_rate, output_rate) in [
            (1000, 48000),
            (8000, 44100),
            (11025, 48000),
            (22050, 48000),
            (48000, 44100),
            (48000, 8000),
            (44100, 48000),
            (48000, 48000),
        ] {
            for channels in [1, 2] {
                for source_frames in [37, source_rate as usize * 3 + 17] {
                    let mut offline = OfflineSink::new(source_rate, output_rate, channels);
                    let source: Vec<f32> = (0..source_frames)
                        .flat_map(|frame| {
                            (0..channels).map(move |ch| {
                                if ch == 0 {
                                    0.25 + (frame % 7) as f32 * 0.01
                                } else {
                                    -0.25
                                }
                            })
                        })
                        .collect();
                    let mut accepted = 0;
                    let mut nonzero = false;
                    let mut observed = 0;
                    for iteration in 0..100000 {
                        let offered =
                            [2713, 4097, 701, 10000][iteration % 4].min(source_frames - accepted);
                        if offered > 0 {
                            accepted += offline
                                .sink
                                .push(&source[accepted * channels..(accepted + offered) * channels])
                                .unwrap();
                        }
                        if accepted == source_frames {
                            offline.sink.finish().unwrap();
                        }
                        let output = offline.block([127, 509, 211][iteration % 3]);
                        nonzero |= output.iter().any(|sample| sample.abs() > 1e-6);
                        let stats = offline.sink.stats().unwrap();
                        assert!(stats.presented_source_frames >= observed);
                        assert!(stats.presented_source_frames <= accepted as u64);
                        observed = stats.presented_source_frames;
                        if stats.drained {
                            break;
                        }
                    }
                    let stats = offline.sink.stats().unwrap();
                    assert!(
                        stats.finished && stats.drained,
                        "{source_rate}->{output_rate}/{channels}/{source_frames}: {stats:?}"
                    );
                    assert_eq!(stats.accepted_source_frames, source_frames as u64);
                    assert_eq!(stats.presented_source_frames, source_frames as u64);
                    assert_eq!(stats.queued_source_frames, 0);
                    assert_eq!(
                        stats.produced_output_frames,
                        (source_frames as u64 * u64::from(output_rate)
                            + u64::from(source_rate) / 2)
                            / u64::from(source_rate)
                    );
                    assert!(
                        nonzero,
                        "all zero output at {source_rate}->{output_rate}/{channels}/{source_frames}"
                    );
                }
            }
        }
    }

    #[test]
    fn retimed_short_file_finishes_the_writer_before_ending_source_time() {
        for percent in [50, 80, 100, 120, 150, 200] {
            let rate = 8000;
            let source_frames = 317;
            let mut stage = RateStage::new(rate, 1, percent);
            let mut pending = Vec::new();
            stage.push(&vec![0.3; source_frames], &mut pending);
            stage.flush(&mut pending);
            let mut offline = OfflineSink::new(rate, 48000, 1);
            for _ in 0..10000 {
                let accepted = offline.sink.push(&pending).unwrap();
                consume_frames(&mut pending, accepted, 1);
                if pending.is_empty() {
                    offline.sink.finish().unwrap();
                }
                offline.block(127);
                if offline.sink.stats().unwrap().drained {
                    break;
                }
            }
            let stats = offline.sink.stats().unwrap();
            assert!(stats.drained, "{percent}%: {stats:?}");
            assert_eq!(
                stage.source_frames_at(stats.presented_source_frames),
                source_frames as u64,
                "{percent}%"
            );
        }
    }
}
