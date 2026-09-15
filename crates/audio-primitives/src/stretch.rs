//! WSOLA time-stretching: change a stream's duration without moving its pitch.
//!
//! Interleaved `f32` in, interleaved `f32` out, same sample rate and channel
//! count. Each step overlap-adds a ~25 ms window at a fixed synthesis hop; the
//! analysis hop is the synthesis hop divided by the ratio, so the input is read
//! faster or slower than it is written. Before every cross-fade the kernel
//! searches ±8 ms around the analysis anchor for the segment whose head best
//! matches the tail already committed to the output, which is what keeps the
//! seam in phase and the pitch where it was.
//!
//! Ratio 1.0 is a bit-exact passthrough: no windows, no correlation, the input
//! samples are the output samples.

use std::f32::consts::PI;

/// Analysis/synthesis window, in milliseconds. Long enough to hold a low
/// voice's period, short enough to keep transients local.
const WINDOW_MS: f32 = 25.0;
/// Half-width of the waveform-similarity search around the analysis anchor.
const SEARCH_MS: f32 = 8.0;
/// Frames compared per candidate offset. A prefix is enough to pick phase, and
/// capping it keeps the search cost independent of the window length.
const CORRELATION_FRAMES: usize = 256;
/// Coarse search stride; the winner's neighbourhood is then searched by one.
const COARSE_STRIDE: i64 = 4;

/// Slowest supported retiming (output four times as long as the input).
pub const MAX_RATIO: f32 = 4.0;
/// Fastest supported retiming (output a quarter as long as the input).
pub const MIN_RATIO: f32 = 0.25;

/// Streaming WSOLA time-stretcher over interleaved frames.
pub struct Stretcher {
    channels: usize,
    /// Synthesis hop in frames; the window is twice this.
    hop: usize,
    /// Half-width of the similarity search, in frames.
    search: usize,
    /// Output duration over input duration. 1.0 is passthrough.
    ratio: f32,
    /// Rising half-Hann cross-fade, `hop` frames long.
    fade: Vec<f32>,
    /// Interleaved input backlog; its first frame is absolute frame `origin`.
    input: Vec<f32>,
    origin: u64,
    /// Absolute analysis anchor, fractional so the ratio never quantizes.
    anchor: f64,
    /// Second half of the last synthesized window, `hop` frames.
    tail: Vec<f32>,
    /// A window tail stranded by a ratio change, emitted before anything else.
    carry: Vec<f32>,
    consumed: u64,
    produced: u64,
    /// Absolute source frame reached by the last emitted output frame.
    front: u64,
    started: bool,
}

impl Stretcher {
    /// A passthrough stretcher for `channels` interleaved channels.
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let channels = channels.max(1);
        let hop = ((sample_rate as f32 * WINDOW_MS / 2000.0).round() as usize).max(1);
        let search = ((sample_rate as f32 * SEARCH_MS / 1000.0).round() as usize).max(1);
        let fade = (0..hop)
            .map(|frame| 0.5 - 0.5 * (PI * frame as f32 / hop as f32).cos())
            .collect();
        Self {
            channels,
            hop,
            search,
            ratio: 1.0,
            fade,
            input: Vec::with_capacity((2 * hop + 2 * search) * channels * 2),
            origin: 0,
            anchor: 0.0,
            tail: vec![0.0; hop * channels],
            carry: Vec::with_capacity(hop * channels),
            consumed: 0,
            produced: 0,
            front: 0,
            started: false,
        }
    }

    /// Output duration over input duration: 0.8 shortens, 1.25 lengthens.
    /// Clamped to [`MIN_RATIO`, `MAX_RATIO`]. Applies from the next frame.
    pub fn set_ratio(&mut self, ratio: f32) {
        let ratio = if ratio.is_finite() {
            ratio.clamp(MIN_RATIO, MAX_RATIO)
        } else {
            1.0
        };
        if ratio == self.ratio {
            return;
        }
        let was_passthrough = self.ratio == 1.0;
        self.ratio = ratio;
        if ratio == 1.0 {
            // Passthrough copies the backlog through, so the backlog has to
            // start where the emitted output ends: strand the window tail.
            if self.started {
                self.carry.extend_from_slice(&self.tail);
                let resume = self.front + self.hop as u64;
                self.trim_to(resume);
            }
            self.started = false;
        } else if was_passthrough {
            self.anchor = self.origin as f64;
            self.started = false;
        }
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Synthesis hop in frames: the granularity of every counter below.
    pub fn hop_frames(&self) -> usize {
        self.hop
    }

    /// Every input frame handed to [`Stretcher::process`] so far.
    pub fn input_frames_consumed(&self) -> u64 {
        self.consumed
    }

    /// Every output frame written so far.
    pub fn output_frames_produced(&self) -> u64 {
        self.produced
    }

    /// The source frame the last emitted output frame came from: consumed
    /// input minus what is still held in the analysis window.
    pub fn source_frames_emitted(&self) -> u64 {
        self.front
    }

    /// Input frames read but not yet represented in the output.
    pub fn held_frames(&self) -> u64 {
        self.consumed.saturating_sub(self.front)
    }

    /// Retime `input`, appending output frames to `output`. All of `input` is
    /// taken; how much comes back out depends on the ratio and on how much of
    /// the analysis window is already filled.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        debug_assert_eq!(input.len() % self.channels, 0);
        let frames = input.len() / self.channels;
        let input = &input[..frames * self.channels];
        self.consumed += frames as u64;
        self.emit_carry(output);
        if self.ratio == 1.0 {
            output.extend_from_slice(&self.input);
            output.extend_from_slice(input);
            self.produced += (self.input.len() / self.channels) as u64 + frames as u64;
            self.input.clear();
            self.origin = self.consumed;
            self.anchor = self.consumed as f64;
            self.front = self.consumed;
            return;
        }
        self.input.extend_from_slice(input);
        while self.step(output) {}
        let keep = (self.anchor.round() as u64).saturating_sub(self.search as u64);
        self.trim_to(keep.min(self.front));
    }

    /// Emit what is still held and end the stream. The stretcher stays usable;
    /// the next [`Stretcher::process`] starts a fresh analysis window.
    pub fn flush(&mut self, output: &mut Vec<f32>) {
        self.emit_carry(output);
        if self.ratio == 1.0 || !self.started {
            // Nothing is under analysis: a short residue is its own best copy.
            self.produced += (self.input.len() / self.channels) as u64;
            output.extend_from_slice(&self.input);
        } else {
            // Pad past the end so the last windows can still be formed; the
            // final cross-fade then decays into the padding instead of cutting.
            let end = self.origin + (self.input.len() / self.channels) as u64;
            self.input.resize(
                self.input.len() + (2 * self.hop + self.search) * self.channels,
                0.0,
            );
            while (self.anchor.round() as u64) < end && self.step(output) {}
        }
        self.input.clear();
        self.carry.clear();
        self.tail.fill(0.0);
        self.origin = self.consumed;
        self.anchor = self.consumed as f64;
        self.front = self.consumed;
        self.started = false;
    }

    /// Drop all state, counters included.
    pub fn reset(&mut self) {
        self.input.clear();
        self.carry.clear();
        self.tail.fill(0.0);
        self.origin = 0;
        self.anchor = 0.0;
        self.consumed = 0;
        self.produced = 0;
        self.front = 0;
        self.started = false;
    }

    fn window(&self) -> usize {
        2 * self.hop
    }

    fn emit_carry(&mut self, output: &mut Vec<f32>) {
        if self.carry.is_empty() {
            return;
        }
        let frames = (self.carry.len() / self.channels) as u64;
        output.extend_from_slice(&self.carry);
        self.carry.clear();
        self.produced += frames;
        self.front += frames;
    }

    fn trim_to(&mut self, absolute: u64) {
        if absolute <= self.origin {
            return;
        }
        let drop = (((absolute - self.origin) as usize) * self.channels).min(self.input.len());
        self.input.drain(..drop);
        self.origin += (drop / self.channels) as u64;
    }

    /// One overlap-add step. Returns false when the backlog is too short.
    fn step(&mut self, output: &mut Vec<f32>) -> bool {
        let window = self.window();
        let anchor = self.anchor.round() as u64;
        let end = self.origin + (self.input.len() / self.channels) as u64;
        if anchor < self.origin || anchor + (window + self.search) as u64 > end {
            return false;
        }
        let base = if self.started {
            (anchor as i64 + self.best_offset(anchor)) as u64
        } else {
            anchor
        };
        let at = ((base - self.origin) as usize) * self.channels;
        let split = at + self.hop * self.channels;
        if self.started {
            for frame in 0..self.hop {
                let rising = self.fade[frame];
                for channel in 0..self.channels {
                    let index = frame * self.channels + channel;
                    output
                        .push(self.tail[index] * (1.0 - rising) + self.input[at + index] * rising);
                }
            }
        } else {
            output.extend_from_slice(&self.input[at..split]);
            self.started = true;
        }
        self.tail
            .copy_from_slice(&self.input[split..at + window * self.channels]);
        // Monotone: a similarity offset may point behind the last one, and a
        // position that walks backwards is a lie to the listener.
        self.front = self.front.max(base + self.hop as u64);
        self.produced += self.hop as u64;
        self.anchor += self.hop as f64 / self.ratio as f64;
        true
    }

    /// The offset near `anchor` whose segment head best continues the tail.
    fn best_offset(&self, anchor: u64) -> i64 {
        let low = -((anchor - self.origin).min(self.search as u64) as i64);
        let high = self.search as i64;
        let compared = self.hop.min(CORRELATION_FRAMES) * self.channels;
        let mut best = 0;
        let mut best_score = f32::NEG_INFINITY;
        let mut offset = low;
        while offset <= high {
            let score = self.similarity(anchor, offset, compared);
            if score > best_score {
                best_score = score;
                best = offset;
            }
            offset += COARSE_STRIDE;
        }
        for offset in (best - COARSE_STRIDE + 1).max(low)..=(best + COARSE_STRIDE - 1).min(high) {
            let score = self.similarity(anchor, offset, compared);
            if score > best_score {
                best_score = score;
                best = offset;
            }
        }
        best
    }

    /// Cross-correlation of the candidate head against the tail, normalized by
    /// the candidate's energy so a loud passage cannot win on volume alone.
    fn similarity(&self, anchor: u64, offset: i64, compared: usize) -> f32 {
        let at = ((anchor as i64 + offset - self.origin as i64) as usize) * self.channels;
        let mut dot = 0.0;
        let mut energy = 0.0;
        for index in 0..compared {
            let sample = self.input[at + index];
            dot += sample * self.tail[index];
            energy += sample * sample;
        }
        dot / (energy.sqrt() + f32::EPSILON)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn sine(frequency: f32, frames: usize, channels: usize) -> Vec<f32> {
        let mut samples = Vec::with_capacity(frames * channels);
        for frame in 0..frames {
            let value = (2.0 * PI * frequency * frame as f32 / RATE as f32).sin();
            for _ in 0..channels {
                samples.push(value);
            }
        }
        samples
    }

    /// Fundamental from interpolated upward zero crossings of one channel.
    fn fundamental(samples: &[f32], channels: usize, channel: usize) -> f32 {
        let mut first = None;
        let mut last = 0.0;
        let mut crossings = 0_u32;
        let mut previous = samples[channel];
        for frame in 1..samples.len() / channels {
            let current = samples[frame * channels + channel];
            if previous < 0.0 && current >= 0.0 {
                let fraction = -previous / (current - previous);
                let instant = (frame - 1) as f32 + fraction;
                if first.is_none() {
                    first = Some(instant);
                } else {
                    crossings += 1;
                }
                last = instant;
            }
            previous = current;
        }
        let first = first.expect("no zero crossings in the measured signal");
        crossings as f32 * RATE as f32 / (last - first)
    }

    fn run(stretcher: &mut Stretcher, input: &[f32], chunk_frames: usize) -> Vec<f32> {
        let channels = stretcher.channels();
        let mut output = Vec::new();
        for chunk in input.chunks(chunk_frames * channels) {
            stretcher.process(chunk, &mut output);
        }
        stretcher.flush(&mut output);
        output
    }

    #[test]
    fn unity_ratio_is_a_bit_exact_passthrough() {
        let mut stretcher = Stretcher::new(RATE, 2);
        let input: Vec<f32> = (0..4_001)
            .flat_map(|frame| {
                let value = (frame as f32 * 0.37).sin();
                [value, -value]
            })
            .collect();
        let output = run(&mut stretcher, &input, 257);
        assert_eq!(output, input);
        assert_eq!(stretcher.input_frames_consumed(), 4_001);
        assert_eq!(stretcher.output_frames_produced(), 4_001);
        assert_eq!(stretcher.held_frames(), 0);
    }

    #[test]
    fn retimed_sine_keeps_its_fundamental_and_hits_the_ratio() {
        let frames = RATE as usize;
        let input = sine(440.0, frames, 1);
        for ratio in [0.8_f32, 1.25] {
            let mut stretcher = Stretcher::new(RATE, 1);
            stretcher.set_ratio(ratio);
            let output = run(&mut stretcher, &input, 1_024);
            let expected = ratio * frames as f32;
            assert!(
                (output.len() as f32 - expected).abs() / expected < 0.02,
                "ratio {ratio}: {} frames, expected {expected}",
                output.len()
            );
            // Skip the final windows, which decay into the flush padding.
            let measured = fundamental(&output[..output.len() - 4 * stretcher.hop_frames()], 1, 0);
            assert!(
                (measured - 440.0).abs() / 440.0 < 0.01,
                "ratio {ratio}: fundamental {measured} Hz"
            );
        }
    }

    #[test]
    fn counters_track_the_ratio_within_one_hop() {
        for ratio in [0.8_f32, 1.2, 1.5] {
            let mut stretcher = Stretcher::new(RATE, 2);
            stretcher.set_ratio(ratio);
            let input = sine(220.0, RATE as usize, 2);
            let output = run(&mut stretcher, &input, 960);
            assert_eq!(stretcher.input_frames_consumed(), RATE as u64);
            assert_eq!(
                stretcher.output_frames_produced(),
                (output.len() / 2) as u64
            );
            assert_eq!(stretcher.held_frames(), 0);
            let expected = ratio as f64 * RATE as f64;
            let error = (stretcher.output_frames_produced() as f64 - expected).abs();
            assert!(
                error <= stretcher.hop_frames() as f64,
                "ratio {ratio}: produced {} against {expected}",
                stretcher.output_frames_produced()
            );
        }
    }

    #[test]
    fn source_front_trails_consumption_by_less_than_a_window() {
        let mut stretcher = Stretcher::new(RATE, 1);
        stretcher.set_ratio(1.25);
        let input = sine(330.0, 24_000, 1);
        let mut output = Vec::new();
        for chunk in input.chunks(2_048) {
            stretcher.process(chunk, &mut output);
            assert!(stretcher.source_frames_emitted() <= stretcher.input_frames_consumed());
            assert!(
                stretcher.held_frames() <= (2 * stretcher.hop_frames() + 2_048) as u64,
                "held {} frames",
                stretcher.held_frames()
            );
        }
        stretcher.flush(&mut output);
        assert_eq!(
            stretcher.source_frames_emitted(),
            stretcher.input_frames_consumed()
        );
    }

    #[test]
    fn reset_clears_state_and_counters() {
        let mut stretcher = Stretcher::new(RATE, 1);
        stretcher.set_ratio(0.8);
        let mut output = Vec::new();
        stretcher.process(&sine(440.0, 12_000, 1), &mut output);
        assert!(stretcher.output_frames_produced() > 0);
        stretcher.reset();
        assert_eq!(stretcher.input_frames_consumed(), 0);
        assert_eq!(stretcher.output_frames_produced(), 0);
        assert_eq!(stretcher.source_frames_emitted(), 0);
        stretcher.set_ratio(1.0);
        let input = sine(100.0, 64, 1);
        let mut fresh = Vec::new();
        stretcher.process(&input, &mut fresh);
        assert_eq!(fresh, input);
    }

    #[test]
    fn stereo_channels_keep_their_own_pitch() {
        let frames = RATE as usize;
        let mut input = vec![0.0; frames * 2];
        for frame in 0..frames {
            let phase = 2.0 * PI * frame as f32 / RATE as f32;
            input[frame * 2] = (phase * 440.0).sin();
            input[frame * 2 + 1] = (phase * 660.0).sin();
        }
        let mut stretcher = Stretcher::new(RATE, 2);
        stretcher.set_ratio(1.25);
        let output = run(&mut stretcher, &input, 1_200);
        let measured = &output[..output.len() - 8 * stretcher.hop_frames()];
        let left = fundamental(measured, 2, 0);
        let right = fundamental(measured, 2, 1);
        assert!((left - 440.0).abs() / 440.0 < 0.01, "left {left} Hz");
        assert!((right - 660.0).abs() / 660.0 < 0.01, "right {right} Hz");
    }

    #[test]
    fn ratio_changes_mid_stream_stay_continuous() {
        let mut stretcher = Stretcher::new(RATE, 1);
        let input = sine(440.0, 9_600, 1);
        let mut output = Vec::new();
        stretcher.set_ratio(1.25);
        stretcher.process(&input, &mut output);
        stretcher.set_ratio(1.0);
        let tail = sine(440.0, 4_800, 1);
        let before = output.len();
        stretcher.process(&tail, &mut output);
        // The stranded window tail plus every new frame, nothing dropped.
        assert!(output.len() - before >= tail.len());
        stretcher.set_ratio(0.8);
        stretcher.process(&sine(440.0, 9_600, 1), &mut output);
        stretcher.flush(&mut output);
        assert_eq!(stretcher.input_frames_consumed(), 24_000);
        assert_eq!(stretcher.source_frames_emitted(), 24_000);
    }

    #[test]
    fn ratio_is_clamped_to_the_supported_range() {
        let mut stretcher = Stretcher::new(RATE, 1);
        stretcher.set_ratio(99.0);
        assert_eq!(stretcher.ratio(), MAX_RATIO);
        stretcher.set_ratio(0.0);
        assert_eq!(stretcher.ratio(), MIN_RATIO);
        stretcher.set_ratio(f32::NAN);
        assert_eq!(stretcher.ratio(), 1.0);
    }
}
