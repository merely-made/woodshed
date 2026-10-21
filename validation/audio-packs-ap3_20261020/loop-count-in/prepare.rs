// SPDX-License-Identifier: MPL-2.0
//! Disposable guitar loop diagnostic: reuse shared patterns, export and render.
//! No device access. Writes only new files in the supplied local directory.
use audio_pattern::{Step, midi::export_midi, render};
use midly::{MetaMessage, Smf, TrackEvent, TrackEventKind};
use std::{error::Error, fs::OpenOptions, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: prepare <local-output-directory>")?;
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    let pack = audio_packs::bundled_drum_kit()?;
    let base = pack.pattern("backbeat")?;
    let mappings: Vec<_> = ["kick", "snare", "hat"]
        .iter()
        .map(|id| Some(pack.definition().voices[*id].midi))
        .collect();
    assert_eq!(
        mappings.iter().map(|v| v.unwrap().note).collect::<Vec<_>>(),
        [36, 38, 42]
    );
    // A–D: quarter-note counts. E: five eighth-note snares, then rest.
    let bars: [(usize, &[usize]); 5] = [
        (0, &[0]),
        (1, &[0, 4]),
        (2, &[0, 4, 8]),
        (0, &[0, 4, 8, 12]),
        (1, &[0, 2, 4, 6, 8]),
    ];
    let mut timeline = vec![(
        0u32,
        TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
    )];
    let mut reference = Vec::new();
    for (index, (track, steps)) in bars.iter().enumerate() {
        let mut pattern = base.clone();
        pattern.bpm = 60.0;
        for lane in &mut pattern.tracks {
            lane.steps.fill(Step::Empty);
            lane.muted = false;
        }
        for step in *steps {
            pattern.tracks[*track].steps[*step] = Step::Active { accent: false };
        }
        let midi = export_midi(&pattern, &mappings)?;
        assert_eq!(
            (midi.ticks_per_quarter, midi.length_ticks, midi.notes),
            (480, 1920, index + 1)
        );
        let bar = Smf::parse(&midi.bytes)?;
        let mut tick = 0u32;
        for event in &bar.tracks[0] {
            tick += event.delta.as_int();
            if let TrackEventKind::Midi { channel, message } = event.kind {
                timeline.push((
                    index as u32 * 1920 + tick,
                    TrackEventKind::Midi { channel, message },
                ));
            }
        }
        assert_eq!(tick, 1920);
        let pcm = render::render_frames(pattern, 48_000.0, 192_000, 1);
        assert!(pcm.iter().all(|v| v.is_finite()));
        assert!(pcm.iter().any(|v| v.abs() > 0.001));
        reference.extend(pcm);
    }
    timeline.sort_by_key(|(tick, _)| *tick);
    let mut previous = 0;
    let mut track = Vec::new();
    for (tick, kind) in timeline {
        track.push(TrackEvent {
            delta: (tick - previous).into(),
            kind,
        });
        previous = tick;
    }
    track.push(TrackEvent {
        delta: (9600 - previous).into(),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    let output = Smf {
        header: midly::Header::new(
            midly::Format::SingleTrack,
            midly::Timing::Metrical(480.into()),
        ),
        tracks: vec![track],
    };
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("12-Ringdown_Bars.mid"))?;
    output.write_std(file)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("reference-60bpm.wav"))?;
    let mut wav = hound::WavWriter::new(
        file,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    assert_eq!(reference.len(), 960_000);
    for sample in reference {
        wav.write_sample((sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)?;
    }
    wav.finalize()?;
    println!(
        "Prepared 5 distinguishable 4/4 bars, SMF0 / PPQ480 / channel10 / 15 balanced notes, no tempo meta; reference is 20 seconds at 60 BPM."
    );
    Ok(())
}
