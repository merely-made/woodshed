use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    cpal::CpalConfig,
    diff::{Diff, PathBuilder},
    node::NodeID,
    nodes::{
        stream::{
            ResamplingChannelConfig,
            writer::{StreamWriterConfig, StreamWriterNode, StreamWriterState},
        },
        volume::{VolumeNode, VolumeNodeConfig},
    },
};

use crate::backend::Sink;

pub(super) const BUFFER_LOW_WATER_SECONDS: f64 = 0.40;

fn stream_buffer_config() -> ResamplingChannelConfig {
    ResamplingChannelConfig {
        latency_seconds: 0.15,
        capacity_seconds: 1.0,
        // Decoding is a non-realtime producer. Dropping buffered source frames
        // to chase a target occupancy makes playback audibly run ahead.
        overflow_autocorrect_percent_threshold: None,
        // Keep the source clock equal to decoded frames. Ordinary underruns may
        // be silent, but must not insert unaccounted frames into that clock.
        underflow_autocorrect_percent_threshold: None,
        ..Default::default()
    }
}

/// Firewheel's default `num_graph_outputs`: the ports the graph's output node has.
const GRAPH_OUTPUT_CHANNELS: u32 = 2;

pub(super) struct AudioRuntime {
    context: FirewheelContext,
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
        context
            .start_stream(CpalConfig::default())
            .map_err(|error| anyhow!("could not start the default audio output: {error:?}"))?;
        // The graph's output is stereo (Firewheel's default), and it maps onto the
        // first channels of whatever stream the device opened. A six-channel
        // device therefore still offers the graph two ports; connecting one edge
        // per device channel overflowed them (InPortOutOfRange, 2026-09-22).
        let output_channels = context
            .stream_info()
            .context("Firewheel did not report output stream information")?
            .num_stream_out_channels
            .min(GRAPH_OUTPUT_CHANNELS);
        let channels = NonZeroChannelCount::new(output_channels)
            .context("the audio output reported no channels")?;
        let volume_params = VolumeNode::from_percent(100.0);
        let volume = context.add_node(volume_params, Some(VolumeNodeConfig { channels }));
        let edges: Vec<_> = (0..output_channels)
            .map(|channel| (channel, channel))
            .collect();
        context
            .connect(volume, context.graph_out_node_id(), &edges, false)
            .map_err(|error| anyhow!("could not connect the output gain: {error:?}"))?;
        Ok(Self {
            context,
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
        let source_rate =
            NonZeroU32::new(source_rate).context("decoded stream has zero sample rate")?;
        let channels = NonZeroChannelCount::new(source_channels)
            .context("decoded audio has an unsupported channel count")?;
        if source_channels > self.output_channels {
            bail!(
                "decoded audio has {source_channels} channels but the output has only {}",
                self.output_channels
            );
        }
        let writer = self.context.add_node(
            StreamWriterNode,
            Some(StreamWriterConfig {
                channels,
                check_for_silence: true,
            }),
        );
        let edges: Vec<_> = if source_channels == 1 {
            (0..self.output_channels)
                .map(|channel| (0, channel))
                .collect()
        } else {
            (0..source_channels)
                .map(|channel| (channel, channel))
                .collect()
        };
        self.context
            .connect(writer, self.volume, &edges, false)
            .map_err(|error| anyhow!("could not connect audio graph: {error:?}"))?;
        let mut state = self
            .context
            .node_state::<StreamWriterState>(writer)
            .context("Firewheel stream writer missing")?
            .clone();
        let event = state
            .start_stream(
                source_rate,
                self.context
                    .stream_info()
                    .context("Firewheel output stream unavailable")?
                    .sample_rate,
                stream_buffer_config(),
            )
            .map_err(|_| anyhow!("could not start decoded audio stream"))?;
        self.context.queue_event_for(writer, event.into());
        Ok(Sink {
            state: Arc::new(Mutex::new(state)),
            node: writer,
            accepted_frames: 0,
            rate: source_rate.get(),
            channels: source_channels as usize,
        })
    }

    pub(super) fn remove_sink(&mut self, sink: Sink) -> Result<()> {
        let stop = sink.stop();
        let remove = self
            .context
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

    pub(super) fn tick(&mut self) -> Result<()> {
        self.context
            .update()
            .map_err(|error| anyhow!("audio runtime update failed: {error:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_stream_buffer_never_discards_or_inserts_source_time() {
        let config = stream_buffer_config();
        assert_eq!(config.overflow_autocorrect_percent_threshold, None);
        assert_eq!(config.underflow_autocorrect_percent_threshold, None);
        assert!(config.capacity_seconds >= config.latency_seconds * 2.0);
        assert!(BUFFER_LOW_WATER_SECONDS > config.latency_seconds);
        assert!(BUFFER_LOW_WATER_SECONDS < config.capacity_seconds);
    }
}
