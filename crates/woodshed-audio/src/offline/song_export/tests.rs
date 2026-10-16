use super::*;
use crate::{Bar, ChordRef, SampleBuffer, SongCursor, TimeSignature};
use std::io::{Cursor, SeekFrom};
use std::sync::Arc;

fn captured_song() -> Song {
    let mut song = Song::new();
    song.click_enabled = false;
    song.bars = vec![Bar {
        bpm: 6000.0,
        time_signature: TimeSignature::new(1, 4),
        ..Default::default()
    }];
    song.bars[0].audio_buffer = Some(SampleBuffer::new(
        vec![
            0.125, -0.375, 0.25, 0.375, 0.5, 0.125, -0.25, -0.5, 0.0, 0.75,
        ],
        1000,
    ));
    song
}
fn export(song: &Song, rate: u32) -> (SongExportReport, Vec<f32>, hound::WavSpec) {
    let mut destination = Cursor::new(Vec::new());
    let report = export_song_wav_to(song, rate, &mut destination).unwrap();
    let mut reader = hound::WavReader::new(Cursor::new(destination.into_inner())).unwrap();
    let spec = reader.spec();
    assert_eq!(reader.duration() as u64, report.frames);
    let samples = reader
        .samples::<f32>()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (report, samples, spec)
}
fn bits(samples: &[f32]) -> Vec<u32> {
    samples.iter().map(|sample| sample.to_bits()).collect()
}

#[test]
fn float_wav_preserves_completed_pcm_default_gain_and_one_cycle_hard_end() {
    for one_shot in [false, true] {
        let mut song = captured_song();
        song.one_shot = one_shot;
        let source = song.bars[0].audio_buffer.as_ref().unwrap().data.clone();
        song.cursor = SongCursor {
            bar_idx: 0,
            sample_in_bar: 7,
        };
        song.playing = true;
        let (report, samples, spec) = export(&song, 1000);
        assert_eq!(
            report,
            SongExportReport {
                frames: 10,
                sample_rate: 1000,
                channels: 1
            }
        );
        assert_eq!(
            spec,
            hound::WavSpec {
                channels: 1,
                sample_rate: 1000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float
            }
        );
        assert_eq!(
            bits(&samples),
            bits(
                &source
                    .iter()
                    .map(|s| (*s * 0.8).clamp(-1.0, 1.0))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(song.cursor.sample_in_bar, 7);
        assert!(song.playing);
        assert_eq!(song.one_shot, one_shot);
        assert!(Arc::ptr_eq(
            &source,
            &song.bars[0].audio_buffer.as_ref().unwrap().data
        ));
    }
}

#[test]
fn wav_matches_live_processor_across_fractional_clocks_measures_and_chunk_boundaries() {
    let mut song = Song::new();
    song.click_enabled = true;
    song.bars = [(137.0, 3, 2), (113.5, 5, 1), (180.25, 7, 2)]
        .into_iter()
        .map(|(bpm, beats, length)| {
            let mut bar = Bar {
                bpm,
                time_signature: TimeSignature::new(beats, 8),
                length,
                ..Default::default()
            };
            let frames = bar.duration_samples(48000.0) as usize;
            bar.audio_buffer = Some(SampleBuffer::new(
                (0..frames)
                    .map(|i| (i % 11) as f32 * 0.0125 - 0.0625)
                    .collect(),
                48000,
            ));
            bar.chord_ref = Some(ChordRef {
                formula_name: "exact".into(),
                root_freq_hz: 217.3,
                pitches_hz: vec![217.3, 281.6, 336.2],
                label: "accepted".into(),
            });
            bar
        })
        .collect();
    let expected_frames = song
        .bars
        .iter()
        .map(|bar| bar.duration_samples(48000.0))
        .sum::<u64>();
    assert!(expected_frames > (CHUNK_FRAMES * 2) as u64);
    let (report, exported, _) = export(&song, 48000);
    assert_eq!(report.frames, expected_frames);
    let mut frozen = song.clone();
    frozen.playing = true;
    frozen.one_shot = true;
    let mut live = OutputRuntime::offline(48000).unwrap();
    assert_eq!(live.native_stream_count(), 0);
    live.set_song(frozen).unwrap();
    live.set_output_gain(0.8).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    let mut expected = Vec::new();
    let mut remaining = expected_frames;
    let mut block = 0;
    while remaining > 0 {
        let frames = remaining.min([127u64, 509, 211][block % 3]) as usize;
        let mut scratch = vec![0.0; frames * 2];
        live.render_offline(&mut scratch).unwrap();
        expected.extend(scratch.chunks_exact(2).map(|frame| frame[0]));
        live.service().unwrap();
        remaining -= frames as u64;
        block += 1;
    }
    assert_eq!(bits(&exported), bits(&expected));
    assert!(!live.snapshot().playing);
    let mut tail = [7.0; 64];
    live.render_offline(&mut tail).unwrap();
    assert!(tail.iter().all(|s| *s == 0.0));
    assert!(exported.iter().any(|sample| sample.abs() > 1e-4));
}

#[test]
fn backing_click_is_honored_without_a_count_in_or_appended_tail() {
    let mut muted = captured_song();
    muted.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.25; 10], 1000));
    let (_, plain, _) = export(&muted, 1000);
    assert!(plain.iter().all(|sample| *sample == 0.2));
    muted.click_enabled = true;
    let (report, with_click, _) = export(&muted, 1000);
    assert_eq!(report.frames, 10);
    assert_eq!(with_click.len(), 10);
    assert_ne!(bits(&with_click), bits(&plain));
    let mut live = OutputRuntime::offline(1000).unwrap();
    muted.playing = true;
    muted.one_shot = true;
    live.set_song(muted).unwrap();
    live.rewind().unwrap();
    live.service().unwrap();
    let mut exact = [0.0; 20];
    live.render_offline(&mut exact).unwrap();
    assert_eq!(
        bits(&with_click),
        bits(&exact.chunks_exact(2).map(|s| s[0]).collect::<Vec<_>>())
    );
}

#[test]
fn frozen_export_does_not_advance_or_change_an_existing_live_runtime() {
    let mut song = captured_song();
    song.playing = true;
    let mut live = OutputRuntime::offline(1000).unwrap();
    live.set_song(song).unwrap();
    live.service().unwrap();
    let mut prefix = [0.0; 6];
    live.render_offline(&mut prefix).unwrap();
    let before = live.snapshot();
    let frozen = live.song();
    export(&frozen, 1000);
    assert_eq!(live.snapshot(), before);
    assert_eq!(live.song().cursor, frozen.cursor);
    assert!(live.song().playing);
}

#[derive(Default)]
struct UntouchedWriter {
    writes: usize,
    seeks: usize,
    data: Cursor<Vec<u8>>,
}
impl Write for UntouchedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.writes += 1;
        self.data.write(data)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Seek for UntouchedWriter {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.seeks += 1;
        self.data.seek(position)
    }
}

#[test]
fn malformed_snapshots_and_preparation_bounds_refuse_before_touching_the_writer() {
    for corruption in 0..17 {
        let mut song = captured_song();
        let mut rate = 1000;
        match corruption {
            0 => rate = 0,
            1 => rate = 999,
            2 => rate = 384001,
            3 => song.bars.clear(),
            4 => song.recording = true,
            5 => song
                .pending
                .push(PendingChange::StartRecording { bar_idx: 0 }),
            6 => song.pending.push(PendingChange::StopRecording),
            7 => song.bars[0].bpm = f32::NAN,
            8 => song.bars[0].bpm = 0.0,
            9 => song.bars[0].time_signature.numerator = 0,
            10 => song.bars[0].length = 0,
            11 => song.bars[0].audio_buffer.as_mut().unwrap().sample_rate_hz = 44100,
            12 => song.bars[0].audio_buffer = Some(SampleBuffer::new(vec![0.1; 9], 1000)),
            13 => {
                Arc::make_mut(&mut song.bars[0].audio_buffer.as_mut().unwrap().data)[0] = f32::NAN
            },
            14 => {
                song.bars[0].bpm = 1.0;
                song.bars[0].length = 255;
                song.bars[0].time_signature.numerator = 16;
            },
            15 => {
                song.bars[0].chord_ref = Some(ChordRef {
                    formula_name: "invalid".into(),
                    root_freq_hz: f32::MAX,
                    pitches_hz: vec![],
                    label: String::new(),
                })
            },
            _ => {
                song.bars[0].chord_ref = Some(ChordRef {
                    formula_name: "invalid".into(),
                    root_freq_hz: 220.0,
                    pitches_hz: vec![220.0; 257],
                    label: String::new(),
                })
            },
        }
        let mut writer = UntouchedWriter::default();
        assert!(
            export_song_wav_to(&song, rate, &mut writer).is_err(),
            "corruption {corruption} passed"
        );
        assert_eq!(
            (writer.writes, writer.seeks),
            (0, 0),
            "corruption {corruption} touched writer"
        );
    }
}

#[test]
fn unrelated_pending_transport_commands_do_not_change_the_complete_export_cycle() {
    let plain = captured_song();
    let mut pending = plain.clone();
    pending.pending = vec![
        PendingChange::SeekTo { bar_idx: 99 },
        PendingChange::SetPlaying(false),
    ];
    assert_eq!(
        bits(&export(&plain, 1000).1),
        bits(&export(&pending, 1000).1)
    );
    assert_eq!(pending.pending.len(), 2);
}

struct FailedWriter;
impl Write for FailedWriter {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("fixture write failure"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Seek for FailedWriter {
    fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
        Ok(0)
    }
}
#[test]
fn buffered_sink_failure_is_reported_at_finalization() {
    assert!(matches!(
        export_song_wav_to(&captured_song(), 1000, FailedWriter),
        Err(SongExportError::Wav(_))
    ));
}
