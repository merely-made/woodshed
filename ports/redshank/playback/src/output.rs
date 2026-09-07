use std::{
    num::NonZeroU32,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    cpal::CpalConfig,
    nodes::stream::{
        ResamplingChannelConfig,
        writer::{StreamWriterConfig, StreamWriterNode, StreamWriterState},
    },
};

use crate::backend::Sink;

pub(super) struct AudioRuntime {
    context: FirewheelContext,
    output_channels: u32,
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
        let output_channels = context
            .stream_info()
            .context("Firewheel did not report output stream information")?
            .num_stream_out_channels;
        Ok(Self {
            context,
            output_channels,
        })
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
            .connect(writer, self.context.graph_out_node_id(), &edges, false)
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
                ResamplingChannelConfig {
                    latency_seconds: 0.08,
                    capacity_seconds: 0.20,
                    ..Default::default()
                },
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
