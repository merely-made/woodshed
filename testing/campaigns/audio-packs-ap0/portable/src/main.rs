//! AP0 disposable embedding probe. No product schema or MIDI map is defined here.
use ap0_portable::{sequencer::*, sound::*};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use serde_json::json;
use std::path::Path;

fn main() {
    let dest = std::env::args().nth(1).unwrap();
    let dest = Path::new(&dest);
    let mut reports = Vec::new();
    for (num, den, div) in [(4, 4, 4), (3, 8, 4), (7, 8, 2), (4, 4, 3)] {
        let mut p = SequencerPattern::metronome_dense(
            120.0,
            TimeSignature::new(num, den),
            Subdivision {
                divisions_per_beat: div,
            },
        );
        p.tracks[0].name = "synthetic-kick".into();
        p.tracks[0].sound = Sound::sample("synthetic-kick");
        let encoded = serde_json::to_vec_pretty(&p).unwrap();
        let decoded: SequencerPattern = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.steps_per_bar(), p.steps_per_bar());
        let ticks_per_step = 480 * 4 / u32::from(den) / u32::from(div);
        assert_eq!(ticks_per_step * u32::from(den) * u32::from(div), 480 * 4);
        let end_tick = ticks_per_step * p.steps_per_bar() as u32;
        let mut events = vec![
            (0, TrackEventKind::Meta(MetaMessage::Tempo(500_000.into()))),
            (
                0,
                TrackEventKind::Meta(MetaMessage::TimeSignature(num, den.ilog2() as u8, 24, 8)),
            ),
        ];
        let mut expected = Vec::new();
        for (index, step) in p.tracks[0].steps.iter().enumerate() {
            if let Step::Active { accent } = step {
                let tick = index as u32 * ticks_per_step;
                expected.push(tick);
                events.push((
                    tick,
                    TrackEventKind::Midi {
                        channel: 9.into(),
                        message: MidiMessage::NoteOn {
                            key: 36.into(),
                            vel: if *accent { 120 } else { 96 }.into(),
                        },
                    },
                ));
                events.push((
                    tick + ticks_per_step / 2,
                    TrackEventKind::Midi {
                        channel: 9.into(),
                        message: MidiMessage::NoteOff {
                            key: 36.into(),
                            vel: 0.into(),
                        },
                    },
                ));
            }
        }
        events.push((end_tick, TrackEventKind::Meta(MetaMessage::EndOfTrack)));
        events.sort_by_key(|(tick, _)| *tick);
        let mut last = 0;
        let track = events
            .into_iter()
            .map(|(tick, kind)| {
                let delta = tick - last;
                last = tick;
                TrackEvent {
                    delta: delta.into(),
                    kind,
                }
            })
            .collect();
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(480.into())),
            tracks: vec![track],
        };
        let mut bytes = Vec::new();
        smf.write_std(&mut bytes).unwrap();
        let parsed = Smf::parse(&bytes).unwrap();
        let mut absolute = 0;
        let actual: Vec<_> = parsed.tracks[0]
            .iter()
            .filter_map(|event| {
                absolute += event.delta.as_int();
                matches!(
                    event.kind,
                    TrackEventKind::Midi {
                        message: MidiMessage::NoteOn { .. },
                        ..
                    }
                )
                .then_some(absolute)
            })
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(absolute, end_tick);
        absolute = 0;
        let note_offs: Vec<_> = parsed.tracks[0]
            .iter()
            .filter_map(|event| {
                absolute += event.delta.as_int();
                matches!(
                    event.kind,
                    TrackEventKind::Midi {
                        message: MidiMessage::NoteOff { .. },
                        ..
                    }
                )
                .then_some(absolute)
            })
            .collect();
        assert_eq!(
            note_offs,
            expected
                .iter()
                .map(|tick| tick + ticks_per_step / 2)
                .collect::<Vec<_>>()
        );
        let stem = format!("pattern-{num}-{den}-{div}");
        std::fs::write(dest.join(format!("{stem}.mid")), &bytes).unwrap();
        std::fs::write(dest.join(format!("{stem}.json")), &encoded).unwrap();
        reports.push(json!({"meter":[num,den],"divisions_per_notated_beat":div,"note_on_ticks":actual,"note_off_ticks":note_offs,"end_tick":end_tick,"midi_bytes":bytes.len(),"step_seconds":p.step_duration().as_secs_f64()}));
    }
    println!("{}", serde_json::to_string_pretty(&reports).unwrap());
}
