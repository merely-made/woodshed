#[path="@WOODSHED_ROOT@/crates/woodshed-audio/src/sound.rs"]
pub mod sound;
#[path="@WOODSHED_ROOT@/crates/woodshed-audio/src/sequencer.rs"]
pub mod sequencer;
#[path="@WOODSHED_ROOT@/crates/woodshed-audio/src/samples.rs"]
pub mod samples;

#[cfg(feature="candidates")]
pub fn fundsp_unit(rate:f64)->Box<dyn fundsp::audiounit::AudioUnit> {
    use fundsp::prelude32::*;
    let mut unit=Box::new(sine_hz(110.0)*0.2) as Box<dyn fundsp::audiounit::AudioUnit>;
    unit.set_sample_rate(rate);unit.allocate();unit
}
#[cfg(feature="candidates")]
pub fn soundfont_synth(sf:&std::sync::Arc<rustysynth::SoundFont>,rate:i32)->rustysynth::Synthesizer {
    rustysynth::Synthesizer::new(sf,&rustysynth::SynthesizerSettings::new(rate)).unwrap()
}
